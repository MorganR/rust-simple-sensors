use core::time::Duration;
use embedded_hal::digital::{InputPin, IoPin, OutputPin, PinState};

#[derive(Debug, PartialEq)]
pub enum Error<TIoError> {
    /// Wrapped error from the HAL.
    Wrapped(TIoError),
    /// Invalid argument was provided.
    InvalidArgument,
    /// Invalid data was read.
    BadData,
    /// Timeout occurred.
    Timeout,
}

impl<TIoError> From<TIoError> for Error<TIoError> {
    fn from(error: TIoError) -> Error<TIoError> {
        Error::Wrapped(error)
    }
}

// TODO: Test responses in detail.
pub trait Response {
    fn get_humidity(&self) -> f32;
    fn get_temperature(&self) -> f32;
}

trait ResponseInternal {
    fn from_raw_bytes(bytes: &[u8; 4]) -> Self;
    fn is_valid(&self) -> bool;
}

/// Data read from the DHT11.
#[derive(Debug, PartialEq)]
pub struct Dht11Response {
    pub humidity: u8,
    pub humidity_decimal: u8,
    pub temperature: u8,
    pub temperature_decimal: u8,
}

impl Response for Dht11Response {
    fn get_humidity(&self) -> f32 {
        self.humidity as f32 + (self.humidity_decimal as f32 * 0.1)
    }

    fn get_temperature(&self) -> f32 {
        self.temperature as f32 + (self.temperature_decimal as f32 * 0.1)
    }
}

impl ResponseInternal for Dht11Response {
    fn from_raw_bytes(bytes: &[u8; 4]) -> Dht11Response {
        Dht11Response {
            humidity: bytes[0],
            humidity_decimal: bytes[1],
            temperature: bytes[2],
            temperature_decimal: bytes[3],
        }
    }

    fn is_valid(&self) -> bool {
        // DHT11 sensors should only be able to read temperatures from 0-50 degrees Celsius.
        // Validate that + 50% for some wiggle-room in case some sensors can go beyond this.
        (self.humidity < 100 || (self.humidity == 100 && self.humidity_decimal == 0))
            && self.humidity_decimal < 10
            && self.temperature < 75
            && self.temperature_decimal < 10
    }
}

/// Data read from the DHT22.
#[derive(Debug, PartialEq)]
pub struct Dht22Response {
    pub humidity_x10: u16,
    pub temperature_x10: u16,
}

impl Response for Dht22Response {
    fn get_humidity(&self) -> f32 {
        (self.humidity_x10 as f32) * 0.1
    }

    fn get_temperature(&self) -> f32 {
        let result = (self.temperature_x10 & 0x7FFF) as f32 * 0.1;
        if (self.temperature_x10 & 0x8000) != 0 {
            return -result;
        }
        result
    }
}

impl ResponseInternal for Dht22Response {
    fn from_raw_bytes(bytes: &[u8; 4]) -> Dht22Response {
        Dht22Response {
            humidity_x10: ((bytes[0] as u16) << 8) | bytes[1] as u16,
            temperature_x10: ((bytes[2] as u16) << 8) | bytes[3] as u16,
        }
    }

    fn is_valid(&self) -> bool {
        // DHT22 sensors should only be able to read temperatures from -40 - 125 degrees Celsius.
        // Validate that + a some wiggle-room in case some sensors can go beyond this.
        let temp_is_negative = self.temperature_x10 & 0x8000 != 0;
        self.humidity_x10 <= 1000
            && ((temp_is_negative && (self.temperature_x10 & 0x7FFF) < 600)
                || (!temp_is_negative && self.temperature_x10 < 1500))
    }
}

/// The minimum read interval of a DHT11.
///
/// Note that this can vary a bit by device, so check your device's datasheet to be sure. Try
/// doubling this value if you are encountering problems.
pub const MINIMUM_DHT11_READ_INTERVAL: Duration = Duration::from_millis(1000);

/// The minimum read interval of a DHT22.
///
/// Note that this can vary a bit by device, so check your device's datasheet to be sure. Try
/// doubling this value if you are encountering problems.
pub const MINIMUM_DHT22_READ_INTERVAL: Duration = Duration::from_millis(2000);

/// Options to modify the behavior of the DHT driver.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// The minimum time interval that must pass between reads. Cannot be below this sensor's
    /// absolute minimum read interval (i.e. [`MINIMUM_DHT11_READ_INTERVAL`] or
    /// [`MINIMUM_DHT22_READ_INTERVAL`])
    pub minimum_read_interval: Duration,
    /// The maximum number of read attempts for any call to `Dht11::read` or `Dht22::read`.
    ///
    /// Keep in mind the `minimum_read_interval` when setting this option. For example, if the
    /// `minimum_read_interval` is set to 2 seconds, and this is set to 3 attempts, each read
    /// could take over 6 seconds.
    pub maximum_attempts: u8,
}

pub const DEFAULT_DHT11_OPTIONS: Options = Options {
    minimum_read_interval: MINIMUM_DHT11_READ_INTERVAL,
    maximum_attempts: 1,
};

pub const DEFAULT_DHT22_OPTIONS: Options = Options {
    minimum_read_interval: MINIMUM_DHT22_READ_INTERVAL,
    maximum_attempts: 1,
};

macro_rules! dhtxx_impl {
    ($name:ident,
     default_options: $default_options:expr,
     minimum_read_interval: $minimum_read_interval:expr,
     ping_duration: $ping_duration:expr,
     response_type: $response_type:ty
    ) => {
        #[derive(Debug)]
        pub struct $name<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>
        where
            TimeFn: Fn() -> TTime,
            ElapsedFn: Fn(TTime) -> Duration,
            TTime: Copy,
        {
            base: DhtBase<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>,
            options: Options,
        }

        impl<TInputPin, TOutputPin, TError, TimeFn, ElapsedFn, TTime>
            $name<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>
        where
            TInputPin: InputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError>,
            TOutputPin: OutputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError>,
            TimeFn: Fn() -> TTime,
            ElapsedFn: Fn(TTime) -> Duration,
            TTime: Copy,
        {
            /// Constructs a DHT sensor that reads from the given pin.
            ///
            /// Reads can sometimes be more reliable with a longer delay, eg. 2 seconds, so consider setting
            /// the `options` value with a longer minimum read interval if error rates are high. If options
            /// is `None`, then the default options is used (see [`DEFAULT_DHT11_OPTIONS`] or
            /// [`DEFAULT_DHT22_OPTIONS]`).
            ///
            /// The provided `time_fn` closure should provide some representation of a given instant that
            /// can be used with `elapsed_since_fn` to determine how much time has passed since then. It
            /// does not need to reflect real dates and times, but only needs to be capable of providing
            /// reasonably accurate durations (i.e. with millisecond precision or better).
            pub fn new(
                pin: TOutputPin,
                time_fn: TimeFn,
                elapsed_since_fn: ElapsedFn,
                options: Option<Options>,
            ) -> Result<$name<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>, Error<TError>> {
                Ok($name {
                    base: DhtBase::new(pin, time_fn, elapsed_since_fn)?,
                    options: if options.is_none() {
                        $default_options
                    } else {
                        let options = options.unwrap();
                        if options.minimum_read_interval < $minimum_read_interval
                            || options.maximum_attempts < 1
                        {
                            return Err(Error::InvalidArgument);
                        }
                        options
                    },
                })
            }

            /// Reads data from the DHT sensor using the minimum read interval.
            ///
            /// This will asynchronously sleep using the provided `delay_fn` if `read` is called within the
            /// minimum read interval of this DHT sensor. The provided function needs to be capable of
            /// millisecond precision or better.
            ///
            /// Due to the tight timing necessary to distinguish bits in the DHT's response, this performs
            /// blocking I/O reads while receiving data. This blocking portion takes about 4ms (full range:
            /// 3200-4800us, depending on the data).
            pub async fn read<DelayFn, EmptyFuture>(
                &mut self,
                delay_fn: DelayFn,
            ) -> Result<$response_type, Error<TError>>
            where
                DelayFn: Fn(Duration) -> EmptyFuture,
                EmptyFuture: core::future::Future<Output = ()>,
            {
                self.base
                    .read::<DelayFn, EmptyFuture, $response_type>(
                        $ping_duration,
                        self.options.minimum_read_interval,
                        delay_fn,
                    )
                    .await
            }
        }
    };
}

const DHT11_PING_DURATION: Duration = Duration::from_millis(18);
const DHT22_PING_DURATION: Duration = Duration::from_millis(1);

dhtxx_impl!(
    Dht11,
    default_options: DEFAULT_DHT11_OPTIONS,
    minimum_read_interval: MINIMUM_DHT11_READ_INTERVAL,
    ping_duration: DHT11_PING_DURATION,
    response_type: Dht11Response
);

dhtxx_impl!(
    Dht22,
    default_options: DEFAULT_DHT22_OPTIONS,
    minimum_read_interval: MINIMUM_DHT22_READ_INTERVAL,
    ping_duration: DHT22_PING_DURATION,
    response_type: Dht22Response
);

#[derive(Debug)]
struct DhtBase<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>
where
    TimeFn: Fn() -> TTime,
    ElapsedFn: Fn(TTime) -> Duration,
    TTime: Copy,
{
    input_pin: Option<TInputPin>,
    output_pin: Option<TOutputPin>,
    last_read_time: TTime,
    time_fn: TimeFn,
    elapsed_since_fn: ElapsedFn,
}

impl<TInputPin, TOutputPin, TError, TimeFn, ElapsedFn, TTime>
    DhtBase<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>
where
    TInputPin: InputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError>,
    TOutputPin: OutputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError>,
    TimeFn: Fn() -> TTime,
    ElapsedFn: Fn(TTime) -> Duration,
    TTime: Copy,
{
    /// Constructs a DHT sensor that reads from the given pin.
    ///
    /// The provided `time_fn` closure should provide some representation of a
    /// given instant that can be used with `elapsed_since_fn` to determine how
    /// much time has passed since then. It does not need to reflect real
    /// dates and times, but only needs to be capable of providing reasonably
    /// accurate durations (i.e. with millisecond precision or better).
    fn new(
        pin: TOutputPin,
        time_fn: TimeFn,
        elapsed_since_fn: ElapsedFn,
    ) -> Result<DhtBase<TInputPin, TOutputPin, TimeFn, ElapsedFn, TTime>, Error<TError>> {
        Ok(DhtBase {
            input_pin: None,
            output_pin: Some(pin),
            last_read_time: time_fn(),
            time_fn: time_fn,
            elapsed_since_fn: elapsed_since_fn,
        })
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
    async fn read<DelayFn, EmptyFuture, TResponse>(
        &mut self,
        ping_duration: Duration,
        minimum_read_interval: Duration,
        delay_fn: DelayFn,
    ) -> Result<TResponse, Error<TError>>
    where
        DelayFn: Fn(Duration) -> EmptyFuture,
        EmptyFuture: core::future::Future<Output = ()>,
        TResponse: Response + ResponseInternal,
    {
        // Double check that the output is driven high so the DHT is ready to send data.
        if self.output_pin.is_none() {
            self.swap_to_output_mode()?;
        }

        let elapsed_since_last_read = (self.elapsed_since_fn)(self.last_read_time);
        if elapsed_since_last_read < minimum_read_interval {
            let to_wait = minimum_read_interval - elapsed_since_last_read;
            delay_fn(to_wait).await;
        }

        self.request_data(ping_duration, delay_fn).await?;
        let bytes = self.receive_data()?;
        let result = TResponse::from_raw_bytes(&bytes);
        if !result.is_valid() {
            return Err(Error::BadData);
        }
        Ok(result)
    }

    async fn request_data<DelayFn, EmptyFuture>(
        &mut self,
        ping_duration: Duration,
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
            .map_err(Error::Wrapped)?;
        delay_fn(ping_duration).await;
        Ok(())
    }

    fn receive_data(&mut self) -> Result<[u8; 4], Error<TError>> {
        let mut bit_ticks = [0u32; 40];
        self.input_pin = Some(
            self.output_pin
                .take()
                .unwrap()
                .try_into_input_pin()
                .map_err(Error::Wrapped)?,
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
        let low_humidity = parse_byte(&bit_ticks[8..16], threshold);
        let high_temp = parse_byte(&bit_ticks[16..24], threshold);
        let low_temp = parse_byte(&bit_ticks[24..32], threshold);
        let parity = parse_byte(&bit_ticks[32..40], threshold);

        let sum: u16 =
            high_humidity as u16 + low_humidity as u16 + high_temp as u16 + low_temp as u16;
        // The last 8 bits should match the parity byte.
        let expected_parity = sum.to_be_bytes()[1];

        if parity != expected_parity {
            return Err(Error::BadData);
        }

        Ok([high_humidity, low_humidity, high_temp, low_temp])
    }

    fn swap_to_output_mode(&mut self) -> Result<(), Error<TError>> {
        self.output_pin = Some(
            self.input_pin
                .take()
                .unwrap()
                .try_into_output_pin(PinState::High)
                .map_err(Error::Wrapped)?,
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
