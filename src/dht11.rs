use core::time::Duration;
use embedded_hal::digital::{InputPin, IoPin, OutputPin, PinState};

#[derive(Debug, PartialEq)]
pub enum Error<TIoError> {
    /// Wrapped error from the HAL.
    Wrapped(TIoError),
    /// Invalid argument was provided.
    InvalidArgument,
    /// Invalid data was read.
    BadParity,
    /// Timeout occurred.
    Timeout,
}

impl<TIoError> From<TIoError> for Error<TIoError> {
    fn from(error: TIoError) -> Error<TIoError> {
        Error::Wrapped(error)
    }
}

#[derive(Debug, PartialEq)]
pub struct DhtResponse {
    pub humidity: u8,
    pub humidity_decimal: u8,
    pub temperature: i8,
    pub temperature_decimal: u8,
}

impl DhtResponse {
    fn from_raw_bytes(bytes: &[u8; 4]) -> DhtResponse {
        DhtResponse {
            humidity: bytes[0],
            humidity_decimal: bytes[1],
            temperature: bytes[2] as i8,
            temperature_decimal: bytes[3],
        }
    }
}

pub struct Dht11<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>
where
    TimeFn: Fn() -> TTime,
    ElapsedFn: Fn(TTime) -> Duration,
    TTime: Copy,
{
    input_pin: Option<TInputPin>,
    output_pin: Option<TOutputPin>,
    minimum_read_interval: Duration,
    last_read_time: TTime,
    time_fn: TimeFn,
    elapsed_since_fn: ElapsedFn,
}

pub const MINIMUM_READ_INTERVAL: Duration = Duration::from_millis(1000);

impl<TInputPin, TOutputPin, TError, TimeFn, ElapsedFn, TTime>
    Dht11<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>
where
    TInputPin: InputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError>,
    TOutputPin: OutputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError>,
    TimeFn: Fn() -> TTime,
    ElapsedFn: Fn(TTime) -> Duration,
    TTime: Copy,
{
    /// Constructs a DHT sensor that reads from the given pin and uses the
    /// default minimum read interval (1 second).
    ///
    /// Reads can sometimes be more reliable with a longer delay, eg. 2 seconds,
    /// so consider calling
    /// [`set_minimum_read_interval`](method@crate::dht11::Dht11::set_minimum_read_interval)
    /// with a longer interval if error rates are high.
    ///
    /// The provided `time_fn` closure should provide some representation of a
    /// given instant that can be used with `elapsed_sinc_fn` to determine how
    /// much time has passed since then. It does not need to reflect real
    /// dates and times, but only needs to be capable of providing reasonably
    /// accurate durations (i.e. with millisecond precision or better).
    pub fn new(
        pin: TOutputPin,
        time_fn: TimeFn,
        elapsed_since_fn: ElapsedFn,
    ) -> Result<Dht11<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>, Error<TError>> {
        Ok(Dht11 {
            input_pin: None,
            output_pin: Some(pin),
            minimum_read_interval: MINIMUM_READ_INTERVAL,
            last_read_time: time_fn(),
            time_fn: time_fn,
            elapsed_since_fn: elapsed_since_fn,
        })
    }

    /// Modifies the minimum read interval enforced by this sensor. This must
    /// be greater than the DHT's default minimum read interval of 1 second.
    pub fn set_minimum_read_interval(
        &mut self,
        read_interval: Duration,
    ) -> Result<(), Error<TError>> {
        if read_interval < MINIMUM_READ_INTERVAL {
            return Err(Error::InvalidArgument);
        }

        self.minimum_read_interval = read_interval;
        Ok(())
    }

    /// Reads data from the DHT sensor using the minimum read interval.
    ///
    /// This will asynchronously sleep using the provided `delay_fn` if `read`
    /// is called within the minimum read interval of this DHT sensor. The
    /// provided function needs to be capable of millisecond precision or
    /// better.
    ///
    /// Due to the tight timing necessary to distinguish bits in the DHT's
    /// response, this performs blocking I/O reads while receiving data. This
    /// takes about 4ms (full range: 3200-4800us, depending on the data).
    pub async fn read<DelayFn, EmptyFuture>(
        &mut self,
        delay_fn: DelayFn,
    ) -> Result<DhtResponse, Error<TError>>
    where
        DelayFn: Fn(Duration) -> EmptyFuture,
        EmptyFuture: core::future::Future<Output = ()>,
    {
        // Double check that the output is driven high so the DHT is ready to send data.
        if self.output_pin.is_none() {
            self.swap_to_output_mode()?;
        }

        let elapsed_since_last_read = (self.elapsed_since_fn)(self.last_read_time);
        if elapsed_since_last_read < self.minimum_read_interval {
            let to_wait = self.minimum_read_interval - elapsed_since_last_read;
            delay_fn(to_wait).await;
        }

        self.request_data(delay_fn).await?;
        let bytes = self.receive_data()?;
        Ok(DhtResponse::from_raw_bytes(&bytes))
    }

    async fn request_data<DelayFn, EmptyFuture>(
        &mut self,
        delay_fn: DelayFn,
    ) -> Result<(), Error<TError>>
    where
        DelayFn: Fn(Duration) -> EmptyFuture,
        EmptyFuture: core::future::Future<Output = ()>,
    {
        self.output_pin
            .as_mut()
            .unwrap()
            .try_set_low()
            .map_err(|err| Error::Wrapped(err))?;
        delay_fn(Duration::from_millis(18)).await;
        Ok(())
    }

    fn receive_data(&mut self) -> Result<[u8; 4], Error<TError>> {
        let mut bit_ticks = [0u32; 40];
        self.input_pin = Some(
            self.output_pin
                .take()
                .unwrap()
                .try_into_input_pin()
                .map_err(|err| Error::Wrapped(err))?,
        );
        let input_pin: &TInputPin = &mut self.input_pin.as_ref().unwrap();

        // Block for the ACK, and use this to estimate a timeout.
        let ack_counter = match read_ack(input_pin) {
            Err(err) => {
                self.swap_to_output_mode()?;
                return Err(err);
            }
            Ok(count) => count,
        };
        let bit_timeout = ack_counter << 2;

        for i in 0..40 {
            bit_ticks[i] = match read_bit_with_timeout(input_pin, bit_timeout) {
                Err(err) => {
                    self.swap_to_output_mode()?;
                    return Err(err);
                }
                Ok(count) => count,
            };
        }

        self.swap_to_output_mode()?;

        let threshold = determine_tick_threshold(&bit_ticks);
        let high_humidity = parse_byte(&bit_ticks[0..8], threshold);
        // This DHT11 sensor always sends 0 for this byte. Use hard-coded zero
        // to reduce timing errors.
        let low_humidity = 0;
        let high_temp = parse_byte(&bit_ticks[16..24], threshold);
        // This DHT11 sensor always sends a value from 0-9 for this byte. Mask
        // the value to reduce timing errors.
        let low_temp = parse_byte(&bit_ticks[24..32], threshold) & 0b1111u8;
        let parity = parse_byte(&bit_ticks[32..40], threshold);

        let sum: u16 =
            high_humidity as u16 + low_humidity as u16 + high_temp as u16 + low_temp as u16;
        // The last 8 bits should match the parity byte.
        let expected_parity = sum.to_be_bytes()[1];

        if parity != expected_parity {
            return Err(Error::BadParity);
        }

        Ok([high_humidity, low_humidity, high_temp, low_temp])
    }

    fn swap_to_output_mode(&mut self) -> Result<(), Error<TError>> {
        self.output_pin = Some(
            self.input_pin
                .take()
                .unwrap()
                .try_into_output_pin(PinState::High)
                .map_err(|err| Error::Wrapped(err))?,
        );
        self.last_read_time = (self.time_fn)();
        Ok(())
    }
}

#[inline]
fn read_bit_with_timeout<TInput, TError>(
    input_pin: &TInput,
    timeout: u32,
) -> Result<u32, Error<TError>>
where
    TInput: InputPin<Error = TError>,
{
    let mut counter = 0u32;
    while input_pin.try_is_low().map_err(|err| Error::Wrapped(err))? {
        counter += 1;
        if counter > timeout {
            return Err(Error::Timeout);
        }
    }
    while input_pin.try_is_high().map_err(|err| Error::Wrapped(err))? {
        counter += 1;
        if counter > timeout {
            return Err(Error::Timeout);
        }
    }
    Ok(counter)
}

#[inline]
fn read_ack<TInput, TError>(input_pin: &TInput) -> Result<u32, Error<TError>>
where
    TInput: InputPin<Error = TError>,
{
    let mut counter: u32 = 0;
    while input_pin.try_is_high().map_err(|err| Error::Wrapped(err))? {
        counter += 1;
    }
    while input_pin.try_is_low().map_err(|err| Error::Wrapped(err))? {
        counter += 1;
    }
    while input_pin.try_is_high().map_err(|err| Error::Wrapped(err))? {
        counter += 1;
    }
    Ok(counter)
}

// (index, count)
#[derive(Clone, Copy)]
struct Peak(i8, u8);

fn determine_tick_threshold(bit_ticks: &[u32]) -> u32 {
    let mut min = u32::MAX;
    let mut max = 0;

    for ticks in bit_ticks.iter() {
        if *ticks < min {
            min = *ticks;
        }
        if *ticks > max {
            max = *ticks;
        }
    }

    // Make a histogram.
    let range = max - min + 1;
    let mut buckets = [0u8; 10];
    for ticks in bit_ticks.iter() {
        let mut bucket = ((*ticks - min) * buckets.len() as u32 / range) as usize;
        if bucket >= buckets.len() {
            bucket = buckets.len() - 1;
        }
        buckets[bucket] += 1;
    }

    // Find peaks in the histogram.
    let mut peaks = [Peak(-1, 0); 10];
    let mut previous_count = 0;
    let mut num_peaks = 0;
    for i in 0..buckets.len() {
        let count = buckets[i];
        if count > previous_count {
            if i == buckets.len() - 1 || count > buckets[i + 1] {
                peaks[num_peaks] = Peak(i as i8, count);
                num_peaks += 1;
            }
        }
        previous_count = count;
    }

    // Take the midpoint if two peaks were not found.
    if num_peaks < 2 {
        return min + range / 2;
    }

    // Determine the two highest peaks. These should correspond to the expected
    // number of ticks for the zero (short) and one (long) bits.
    let mut highest_peak = &peaks[peaks.len() - 1];
    let mut second_highest_peak = &peaks[peaks.len() - 1];
    for peak in peaks.iter() {
        let peak_count = peak.1;
        if peak_count > highest_peak.1 {
            second_highest_peak = highest_peak;
            highest_peak = peak;
        } else if peak_count > second_highest_peak.1 {
            second_highest_peak = peak;
        }
    }

    // Convert a histogram index into an approximate number of ticks.
    let index_to_ticks = |index| {
        if index == (buckets.len() - 1) as i8 {
            return max;
        }
        let base = range * index as u32 / 10 + min;
        let next_base = range * (index as u32 + 1) / 10 + min;
        return (base + next_base) / 2;
    };

    let high_ticks: u32;
    let low_ticks: u32;
    if highest_peak.0 > second_highest_peak.0 {
        high_ticks = index_to_ticks(highest_peak.0);
        low_ticks = index_to_ticks(second_highest_peak.0);
    } else {
        low_ticks = index_to_ticks(highest_peak.0);
        high_ticks = index_to_ticks(second_highest_peak.0);
    }
    // Use the mean of the two peaks as the threshold.
    return (high_ticks + low_ticks) / 2;
}

fn parse_byte(bit_ticks: &[u32], threshold: u32) -> u8 {
    let mut byte = 0u8;
    for i in 0..8 {
        if bit_ticks[i] > threshold {
            byte |= 1 << (7 - i);
        }
    }
    return byte;
}
