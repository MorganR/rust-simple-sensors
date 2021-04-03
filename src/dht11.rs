use embedded_hal::digital::{InputPin, OutputPin, IoPin, PinState};
// TODO: Remove the std library for true portability.
use std::fmt;
use std::time::{Instant, Duration};
use tokio::time::{sleep, sleep_until};

#[derive(Debug)]
pub enum Error<TIoError> {
    /// Wrapped error from the HAL.
    Wrapped(TIoError),
    /// Invalid argument was provided.
    InvalidArgument(String),
    /// Invalid data was read.
    Data(String),
}

impl<TIoError> std::error::Error for Error<TIoError> 
    where TIoError : fmt::Debug + std::error::Error + 'static {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            &Error::Wrapped(ref err) => Some(err),
            _ => None,
        }
    }
}

impl<TIoError> std::fmt::Display for Error<TIoError>
    where TIoError : fmt::Debug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            &Error::Wrapped(ref err) => write!(f, "IO error: {:?}", err),
            &Error::InvalidArgument(ref msg) => write!(f, "Invalid argument: {}", msg),
            &Error::Data(ref msg) => write!(f, "Bad data read: {}", msg),
        }
    }
}

#[derive(Debug)]
pub struct DhtResponse {
    pub humidity: f32,
    pub temperature: f32,
}

impl DhtResponse {
    fn from_raw_bytes(bytes: &[u8; 4]) -> DhtResponse {
        DhtResponse {
            humidity: format!("{}.{}", bytes[0], bytes[1]).parse().unwrap(),
            temperature: format!("{}.{}", bytes[2] as i8, bytes[3]).parse().unwrap(),
        }
    }
}

impl std::fmt::Display for DhtResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RH: {:.2}%, T: {:.2}\u{00B0}C", self.humidity, self.temperature)
    }
}

pub struct Dht11<TInputPin, TOutputPin> {
    input_pin: Option<TInputPin>,
    output_pin: Option<TOutputPin>,
    last_read_time: Instant,
}

const MINIMUM_READ_INTERVAL: Duration = Duration::from_secs(1);

impl<TInputPin, TOutputPin, TError> Dht11<TInputPin, TOutputPin>
where TInputPin : InputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError>,
      TOutputPin : OutputPin<Error = TError> + IoPin<TInputPin, TOutputPin, Error = TError> {

    /// Constructs a DHT sensor that reads from the given pin.
    pub fn new<TIoPin>(pin: TIoPin) -> Result<Dht11<TInputPin, TOutputPin>, Error<TError>> 
    where TIoPin : IoPin<TInputPin, TOutputPin, Error = TError> {
        Ok(Dht11 {
            input_pin: None,
            output_pin: Some(pin.try_switch_to_output_pin(PinState::High).map_err(|err| Error::Wrapped(err))?),
            last_read_time: Instant::now(),
        })
    }

    /// Reads data from the DHT sensor using the minimum read interval. 
    /// 
    /// This will asynchronously sleep if it is called within the minimum read 
    /// interval of the DHT sensor (1 second). Reads tend to be more reliable
    /// with a slightly longer delay, eg. 2 seconds, so consider calling
    /// [`read_with_minimum_interval`](method@crate::dht11::Dht11::read_with_minimum_interval)
    /// if error rates are high.
    /// 
    /// Due to the tight timing necessary to distinguish bits in the DHT's
    /// response, this performs blocking I/O reads while receiving data. This
    /// takes about 4ms (full range: 3200-4800us, depending on the data).
    #[allow(dead_code)]
    pub async fn read(&mut self) -> Result<DhtResponse, Error<TError>> {
        self.read_with_minimum_interval(MINIMUM_READ_INTERVAL).await
    }

    /// Reads data from the DHT sensor, enforcing that the given minimum time
    /// interval has passed between reads.
    /// 
    /// This will asynchronously sleep if it is called within the provided
    /// minimum read interval. This interval must be greater than or equal to
    /// the minimum DHT sensor read interval (1 second). Reads tend to be more
    /// reliable with a slightly longer delay, eg. 2 seconds, so this method
    /// gives users greater flexibility if error rates are high.
    /// 
    /// Due to the tight timing necessary to distinguish bits in the DHT's
    /// response, this performs blocking I/O reads while receiving data. This
    /// takes about 4ms (full range: 3200-4800us, depending on the data).
    pub async fn read_with_minimum_interval(&mut self, minimum_read_interval: Duration) -> Result<DhtResponse, Error<TError>> {
        if minimum_read_interval < MINIMUM_READ_INTERVAL {
            return Err(
                Error::InvalidArgument(
                format!("Minimum read interval must be >= {:.1}s.", MINIMUM_READ_INTERVAL.as_secs())));
        }

        // Double check that the output is driven high so the DHT is reading data.
        if self.output_pin.is_none() {
            self.swap_to_output_mode()?;
        }

        let first_read_time = self.last_read_time + minimum_read_interval;
        if Instant::now() < first_read_time {
            sleep_until(first_read_time.into()).await;
        }

        self.request_data().await?;
        let bytes = self.receive_data()?;
        Ok(DhtResponse::from_raw_bytes(&bytes))
    }

    async fn request_data(&mut self) -> Result<(), Error<TError>> {
        self.output_pin.as_mut().unwrap().try_set_low().map_err(|err| Error::Wrapped(err))?;
        sleep(Duration::from_millis(18)).await;
        Ok(())
    }

    fn receive_data(&mut self) -> Result<[u8; 4], Error<TError>> {
        let mut bit_durations = [Duration::new(0, 0); 40];
        self.input_pin = Some(self.output_pin.take().unwrap().try_switch_to_input_pin().map_err(|err| Error::Wrapped(err))?);
        let input_pin: &TInputPin = &mut self.input_pin.as_ref().unwrap();

        // Block for the ACK, and use this to estimate a timeout.
        let mut counter = match read_ack(input_pin) {
            Err(err) => {
                self.swap_to_output_mode()?;
                return Err(err);
            },
            Ok(count) => count
        };
        let counter_timeout = counter << 6;

        let mut end_of_bit = Instant::now();
        let mut start_of_bit: Instant;
        counter = 0;
        for i in 0..40 {
            counter = match read_bit_with_timeout(input_pin, counter_timeout, counter) {
                Err(err) => {
                    self.swap_to_output_mode()?;
                    return Err(err);
                }
                Ok(count) => count
            };

            start_of_bit = end_of_bit;
            end_of_bit = Instant::now();
            bit_durations[i] = end_of_bit - start_of_bit;
        }

        self.swap_to_output_mode()?;

        let high_humidity = parse_byte(&bit_durations[0..8]);
        // This DHT11 sensor always sends 0 for this byte. Use hard-coded zero
        // to reduce timing errors.
        let low_humidity = 0;
        let high_temp = parse_byte(&bit_durations[16..24]);
        // This DHT11 sensor always sends a value from 0-9 for this byte. Mask
        // the value to reduce timing errors.
        let low_temp = parse_byte(&bit_durations[24..32]) & 0b1111u8;
        let parity = parse_byte(&bit_durations[32..40]);

        let sum: u16 = high_humidity as u16 + low_humidity as u16 + high_temp as u16 + low_temp as u16;
        // The last 8 bits should match the parity byte.
        let expected_parity = sum.to_be_bytes()[1];

        if parity != expected_parity {
            return Err(Error::Data("Parity mismatch; try again.".to_owned()));
        }

        Ok([high_humidity, low_humidity, high_temp, low_temp])
    }

    fn swap_to_output_mode(&mut self) -> Result<(), Error<TError>> {
        self.output_pin = Some(
            self.input_pin.take().unwrap()
                .try_switch_to_output_pin(PinState::High)
                .map_err(|err| Error::Wrapped(err))?);
        self.last_read_time = Instant::now();
        Ok(())
    }
}

#[inline]
fn read_bit_with_timeout<TInput, TError>(
    input_pin: &TInput, timeout: u32, start_count: u32) -> Result<u32, Error<TError>>
where TInput: InputPin<Error = TError> {
    const TIMEOUT_MESSAGE: &str = "Timeout while reading data; try again.";
    let mut counter = start_count;
    while input_pin.try_is_low().map_err(|err| Error::Wrapped(err))? {
        counter += 1;
        if counter > timeout {
            return Err(Error::Data(TIMEOUT_MESSAGE.to_owned()));
        }
    };
    while input_pin.try_is_high().map_err(|err| Error::Wrapped(err))? {
        counter += 1;
        if counter > timeout {
            return Err(Error::Data(TIMEOUT_MESSAGE.to_owned()));
        }
    };
    Ok(counter)
}

#[inline]
fn read_ack<TInput, TError>(input_pin: &TInput) -> Result<u32, Error<TError>>
where TInput: InputPin<Error = TError> {
    let mut counter: u32 = 0;
    while input_pin.try_is_high().map_err(|err| Error::Wrapped(err))? { counter += 1; };
    while input_pin.try_is_low().map_err(|err| Error::Wrapped(err))? { counter += 1; };
    while input_pin.try_is_high().map_err(|err| Error::Wrapped(err))? { counter += 1; };
    Ok(counter)
}

fn parse_byte(bit_durations: &[Duration]) -> u8 {
    const THRESHOLD: Duration = Duration::from_micros(100u64);
    let mut byte = 0u8;
    for i in 0..8 {
        if bit_durations[i] > THRESHOLD {
            byte |= 1 << (7 - i);
        }
    }
    return byte;
}