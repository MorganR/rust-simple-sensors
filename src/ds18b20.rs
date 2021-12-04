use core::time::Duration;
use embedded_hal::delay::blocking::DelayUs;
use embedded_hal::digital::PinState;
use embedded_hal::digital::blocking::{InputPin, IoPin, OutputPin};

/// The maximum resolution of the sensor when in 12-bit mode.
pub const MAX_RESOLUTION_F32: f32 = 0.0625;

const RESET_TIME_US: u16 = 480;
// Sensors send a 60-240us starting 15-60us after the reset.
const FIRST_PRESENCE_PULSE_DELAY_US: u8 = 30;
const SECOND_PRESENCE_PULSE_DELAY_US: u8 = 30;
const POST_PRESENCE_PULSE_DELAY_US: u16 =
    RESET_TIME_US - FIRST_PRESENCE_PULSE_DELAY_US as u16 - SECOND_PRESENCE_PULSE_DELAY_US as u16;

const READ_WRITE_RECOVERY_TIME_US: u8 = 1;
const MIN_READ_WRITE_DURATION_US: u8 = 60;
const WRITE_1_DURATION_US: u8 = 1;
const WRITE_1_POST_BIT_DELAY_US: u8 = MIN_READ_WRITE_DURATION_US - WRITE_1_DURATION_US;
const WRITE_0_DURATION_US: u8 = 60;
const READ_REQUEST_DURATION_US: u8 = 1;
const READ_SAMPLE_DELAY_US: u8 = 15 - READ_REQUEST_DURATION_US;
const READ_POST_SAMPLE_DELAY_US: u8 = MIN_READ_WRITE_DURATION_US - READ_SAMPLE_DELAY_US;

const REPOWER_WINDOW: Duration = Duration::from_micros(10);

const EEPROM_COPY_TIME: Duration = Duration::from_millis(10);

const CONVERSION_TIME_9BIT: Duration = Duration::from_micros(93_750);
const CONVERSION_TIME_10BIT: Duration = Duration::from_micros(187_500);
const CONVERSION_TIME_11BIT: Duration = Duration::from_millis(375);
const CONVERSION_TIME_12BIT: Duration = Duration::from_millis(750);

#[derive(Debug)]
pub enum Error<TDelayError, TIoError, TInError, TOutError> {
    WrappedDelay(TDelayError),
    WrappedIo(TIoError),
    WrappedInput(TInError),
    WrappedOutput(TOutError),
    NoSensorsFound,
    BadData,
}

#[derive(Copy, Clone, Debug)]
pub enum ResolutionMode {
    /// Nine-bit resolution reads the temperature in 0.5 degree increments.
    NineBit = 0b00,
    /// Ten-bit resolution reads the temperature in 0.25 degree increments.
    TenBit = 0b01,
    /// Eleven-bit resolution reads the temperature in 0.125 degree increments.
    ElevenBit = 0b10,
    /// Twelve-bit resolution reads the temperature in 0.0625 degree increments.
    TwelveBit = 0b11,
}

impl ResolutionMode {
    fn get_conversion_time(self) -> Duration {
        match self {
            ResolutionMode::NineBit => CONVERSION_TIME_9BIT,
            ResolutionMode::TenBit => CONVERSION_TIME_10BIT,
            ResolutionMode::ElevenBit => CONVERSION_TIME_11BIT,
            ResolutionMode::TwelveBit => CONVERSION_TIME_12BIT,
        }
    }

    fn get_configuration_byte(self) -> u8 {
        return ((self as u8) << 5) | 0b1111;
    }
}

// Necessary access sequence:
// 1. Initialization.
// 2. ROM command (followed by required data, if any).
// 3. DS18B20 function command (followed by data, if any).
//    This step is skipped if step 2 was either a Search ROM or an Alarm Search function.

/// The ID of a given device.
///
/// Used to speak with that device directly when there are mutliple devices on the line.
pub struct DeviceId([u8; 8]);

impl DeviceId {
    fn crc(&self) -> u8 {
        self.0[7]
    }

    fn serial(&self) -> u64 {
        let mut copy = [0u8; 8];
        for i in 1..7 {
            copy[i - 1] = self.0[i];
        }
        u64::from_le_bytes(copy)
    }

    fn family_code(&self) -> u8 {
        self.0[0]
    }

    fn calculated_crc(&self) -> u8 {
        let mut copy = self.0;
        copy[7] = 0;

        let x = u64::from_le_bytes(copy);

        compute_crc(x)
    }
}

fn compute_crc(x: u64) -> u8 {
    let x4 = x.wrapping_mul(x).wrapping_mul(x).wrapping_mul(x);
    let x5 = x4.wrapping_mul(x);
    let x8 = x5.wrapping_mul(x).wrapping_mul(x).wrapping_mul(x);

    (x8.wrapping_add(x5).wrapping_add(x4).wrapping_add(1) & 0xFF) as u8
}

/// Represents a temperature reading from the sensor.
#[derive(Debug)]
pub struct Temperature {
    decimal: i8,
    integer: i16,
}

impl Temperature {
    pub fn from_bytes(low_sig: u8, high_sig: u8) -> Self {
        if high_sig > i8::MAX as u8 {
            // -0x100 = 0xFF00
            let signed_high_sig = -0x100 | high_sig as i16;
            return Temperature {
                // -0x10 = 0xF0
                decimal: -0x10 | (low_sig & 0xF) as i8,
                // Add one since "-1" is actually -0 (i.e. just the decimal is negative).
                integer: (signed_high_sig << 4 | (low_sig >> 4) as i16) + 1,
            };
        } else {
            return Temperature {
                decimal: (low_sig & 0xF) as i8,
                integer: (high_sig as i16) << 4 | (low_sig >> 4) as i16,
            };
        }
    }

    /// The integer part of the temperature measurement.
    pub fn integer_part(&self) -> i16 {
        self.integer
    }

    /// The decimal part of the temperature measurement.
    ///
    /// Can be multiplied by [`MAX_RESOLUTION_F32`] or [`MAX_RESOLUTION_F64`] to convert it to a
    /// floating point value. Note that the actual decimal resolution will be limited depending on
    /// the [`ResolutionMode`] that was used during the read.
    pub fn decimal_part(&self) -> i8 {
        self.decimal
    }

    /// The nearest integer of the temperature measurement.
    ///
    /// Calculated without performing floating-point operations.
    pub fn nearest_integer(&self) -> i16 {
        self.integer + (self.decimal.signum() * (self.decimal.abs() >> 3)) as i16
    }
}

impl From<Temperature> for f32 {
    fn from(temp: Temperature) -> Self {
        temp.integer_part() as f32 + temp.decimal_part() as f32 * MAX_RESOLUTION_F32
    }
}

impl From<Temperature> for f64 {
    fn from(temp: Temperature) -> Self {
        temp.integer_part() as f64 + temp.decimal_part() as f64 * MAX_RESOLUTION_F32 as f64
    }
}

pub struct ReadResult<TData, TIoPin, TInPin, TOutPin, TIoError, TInError, TOutError>
where
    TIoPin: IoPin<TInPin, TOutPin, Error = TIoError>,
    TInPin: InputPin<Error = TInError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TOutPin: OutputPin<Error = TOutError> + IoPin<TInPin, TOutPin, Error = TIoError>,
{
    pub data: TData,
    pub pin: TOutPin,
    phantom_io_pin: core::marker::PhantomData<TIoPin>,
    phantom_in_pin: core::marker::PhantomData<TInPin>,
}

pub fn read_temperature<
    TIoPin,
    TInPin,
    TOutPin,
    TIoError,
    TInError,
    TOutError,
    TDelay,
    TDelayError,
>(
    pin: TIoPin,
    delay: &mut TDelay,
) -> Result<
    ReadResult<Temperature, TOutPin, TInPin, TOutPin, TIoError, TInError, TOutError>,
    Error<TDelayError, TIoError, TInError, TOutError>,
>
where
    TIoPin: IoPin<TInPin, TOutPin, Error = TIoError>,
    TInPin: InputPin<Error = TInError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TOutPin: OutputPin<Error = TOutError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TDelay: DelayUs<Error = TDelayError>,
{
    let pin = reset(pin, delay)?;
    let mut pin = write_byte(RomCommand::Skip as u8, pin, delay)?;
    pin = write_byte(FunctionCommand::ConvertTemperature as u8, pin, delay)?;
    delay
        .delay_us(CONVERSION_TIME_12BIT.as_micros() as u32)
        .map_err(Error::WrappedDelay)?;
    let pin = reset(pin, delay)?;
    let mut pin = write_byte(RomCommand::Skip as u8, pin, delay)?;
    pin = write_byte(FunctionCommand::ReadScratchpad as u8, pin, delay)?;
    let mut data = [0u8; 9];
    for i in 0..9 {
        let byte_and_pin = read_byte(pin, delay)?;
        pin = byte_and_pin.pin;
        data[i] = byte_and_pin.data;
    }

    let mut x = 0u64;
    let mut shift = 0u8;
    // let mut x_7 = 0u64;
    for i in 0..8 {
        x += (data[i] as u64) << shift;
        // if i == 6 {
        //     x_7 = x;
        // }
        shift += 8;
    }
    // TODO: Confirm correct way to compute CRC.
    // let crc_7 = compute_crc(x_7);
    let crc = compute_crc(x);
    if crc != data[8] {
        return Err(Error::BadData);
    }

    let temperature = Temperature::from_bytes(data[0], data[1]);
    Ok(ReadResult {
        data: temperature,
        pin: pin
            .into_output_pin(PinState::High)
            .map_err(Error::WrappedIo)?,
        phantom_io_pin: core::marker::PhantomData,
        phantom_in_pin: core::marker::PhantomData,
    })
}

/// Administrative commands for operating the 1-bit data line.
///
/// These are used to retrieve information about devices on the line, or to request those devices
/// to perform more specific operations (see [`FunctionCommand`]).
pub enum RomCommand {
    /// Searches for all devices on the line.
    ///
    /// Cannot be followed by a function command.
    Search = 0xF0,
    /// Reads the peripheral device's 64-bit ROM code.
    ///
    /// Only possible when there is a single device on the line.
    Read = 0x33,
    /// Selects a specific peripheral device by it's 64-bit ROM code.
    ///
    /// Only the selected device will respond to the subsequent function command.
    Match = 0x55,
    /// Addresses all devices simultaneously.
    ///
    /// If there is only one device on the line, this can be used instead of `Match` for all
    /// function commands. However, if there are multiple devices on the line, then this only works
    /// for some commands, such as `ConvertTemperature`.
    Skip = 0xCC,
    /// Identical to `Search`, except only devices whose alarm flag is set will respond.
    ///
    /// Cannot be followed by a function command.
    AlarmSearch = 0xEC,
}

/// Requests the sensor perform some operation.
///
/// These commands can only be sent after a `Read`, `Match`, or `Skip` [`RomCommand`].
pub enum FunctionCommand {
    /// Stores the current temperature in the 2-byte temperature register in the scratchpad memory.
    ///
    /// If in parasitic power mode, the line must be pulled-up within 10us of sending this command
    /// and held high while the conversion happens. If in external power supply mode, the
    /// controller can send read slots to determine when the conversion is complete.  The sensor
    /// will respond with 0s while the conversion is happening, and respond with a 1 once it is
    /// complete.
    ///
    /// After this command, the sensor returns to its low-power state.
    ConvertTemperature = 0x44,
    /// Write three bytes of data to the sensor's scratchpad.
    ///
    /// Bytes are written starting at byte 2 of the scratchpad (T<sub>H</sub>). Bytes:
    ///
    /// 1. Byte 2: (T<sub>H</sub>)
    /// 2. Byte 3: (T<sub>L</sub>)
    /// 3. Byte 4: Configuration (i.e. [`ResolutionMode`])
    WriteScratchpad = 0x4E,
    /// Reads the contents of the sensor's scratchpad.
    ///
    /// A reset may be issued mid-read to cancel the rest of the read.
    ///
    /// Bytes:
    ///
    /// 1. Byte 0: Temperature least-significant byte.
    /// 2. Byte 1: Temperature most-significant byte.
    /// 3. Byte 2: High temperature threshold for the alarm (T<sub>H</sub>).
    /// 4. Byte 3: Low temperature threshold for the alarm (T<sub>L</sub>).
    /// 5. Byte 4: Configuration (i.e. [`ResolutionMode`]).
    /// 6. Byte 5: Reserved (0xFF)
    /// 7. Byte 6: Reserved
    /// 8. Byte 7: Reserved (0x10)
    /// 9. Byte 8: The CRC byte.
    ReadScratchpad = 0xBE,
    /// Copies the T<sub>H</sub>, T<sub>L</sub>, and configuration bytes from the scratchpad to
    /// EEPROM.
    ///
    /// If in parasitic power mode, the line must be pulled high within 10us after this command, and
    /// held for 10ms to power the device during the operation. If in external power supply mode,
    /// the controller can send read slots to determine when the conversion is complete.  The sensor
    /// will respond with 0s while the conversion is happening, and respond with a 1 once it is
    /// complete.
    CopyScratchpad = 0x48,
    /// Copies the T<sub>H</sub>, T<sub>L</sub>, and configuration bytes from EEPROM to the
    /// scratchpad.
    ///
    /// If in parasitic power mode, the line must be pulled high within 10us after this command, and
    /// held for 10ms to power the device during the operation. If in external power supply mode,
    /// the controller can send read slots to determine when the conversion is complete.  The sensor
    /// will respond with 0s while the conversion is happening, and respond with a 1 once it is
    /// complete.
    RecallEeprom = 0xB8,
    /// Issued to determine if any devices are using parasitic power.
    ///
    /// If any devices are relying on parasitic power, they will hold the line low during the read
    /// slot.
    ReadPowerSupply = 0xB4,
}

/// Shift the pending CRC byte by a single bit.
///
/// If performed for the first 56 bits that are read, starting with a CRC of 0, this should result
/// in the expected CRC byte.
///
/// Continuing to operate this for the CRC byte should then result in 0.
fn shift_crc_bit(bit: u8, crc: u8) -> u8 {
    let xored_bit = (crc & 0x1) ^ bit;
    let to_xor = xored_bit << 2 + xored_bit << 3;
    ((crc >> 1) ^ to_xor) + bit << 7
}

/// Resets the line to prepare for the next rom command.
fn reset<TIoPin, TInPin, TOutPin, TIoError, TInError, TOutError, TDelay, TDelayError>(
    pin: TIoPin,
    delay: &mut TDelay,
) -> Result<TInPin, Error<TDelayError, TIoError, TInError, TOutError>>
where
    TIoPin: IoPin<TInPin, TOutPin, Error = TIoError>,
    TInPin: InputPin<Error = TInError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TOutPin: OutputPin<Error = TOutError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TDelay: DelayUs<Error = TDelayError>,
{
    // let mut results: std::vec::Vec<bool> = std::vec::Vec::with_capacity(1000);
    // Hold pin low for at least 480us.
    let mut pin = pin
        .into_output_pin(PinState::Low)
        .map_err(Error::WrappedIo)?;
    delay
        .delay_us(RESET_TIME_US as u32)
        .map_err(Error::WrappedDelay)?;
    pin.set_high().map_err(Error::WrappedOutput)?;

    // Check that we receive a presence pulse.
    let pin = pin.into_input_pin().map_err(Error::WrappedIo)?;
    // let start = std::time::Instant::now();
    // let end = Duration::from_micros(RESET_TIME_US as u64);
    // while start.elapsed() < end {
    //     results.push(pin.is_high().map_err(Error::WrappedInput)?);
    // }
    // println!("Reset pulse states:");
    // for signal in results.iter() {
    //     println!("{}", signal);
    // }
    delay
        .delay_us(FIRST_PRESENCE_PULSE_DELAY_US as u32)
        .map_err(Error::WrappedDelay)?;
    let mut is_present = pin.is_low().map_err(Error::WrappedInput)?;
    delay
        .delay_us(FIRST_PRESENCE_PULSE_DELAY_US as u32)
        .map_err(Error::WrappedDelay)?;
    is_present |= pin.is_low().map_err(Error::WrappedInput)?;
    if !is_present {
        return Err(Error::NoSensorsFound);
    }

    // Wait the remaining time.
    delay
        .delay_us(POST_PRESENCE_PULSE_DELAY_US as u32)
        .map_err(Error::WrappedDelay)?;
    Ok(pin)
}

fn write_byte<TIoPin, TInPin, TOutPin, TIoError, TInError, TOutError, TDelay, TDelayError>(
    byte: u8,
    pin: TIoPin,
    delay: &mut TDelay,
) -> Result<TOutPin, Error<TDelayError, TIoError, TInError, TOutError>>
where
    TIoPin: IoPin<TInPin, TOutPin, Error = TIoError>,
    TInPin: InputPin<Error = TInError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TOutPin: OutputPin<Error = TOutError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TDelay: DelayUs<Error = TDelayError>,
{
    let mut byte = byte;
    let mut out_pin: TOutPin = pin
        .into_output_pin(PinState::High)
        .map_err(Error::WrappedIo)?;
    for _ in 0..8 {
        out_pin = write_bit(byte & 1, out_pin, delay)?;
        byte >>= 1;
    }
    Ok(out_pin)
}

fn read_byte<TIoPin, TInPin, TOutPin, TIoError, TInError, TOutError, TDelay, TDelayError>(
    pin: TIoPin,
    delay: &mut TDelay,
) -> Result<
    ReadResult<u8, TIoPin, TInPin, TOutPin, TIoError, TInError, TOutError>,
    Error<TDelayError, TIoError, TInError, TOutError>,
>
where
    TIoPin: IoPin<TInPin, TOutPin, Error = TIoError>,
    TInPin: InputPin<Error = TInError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TOutPin: OutputPin<Error = TOutError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TDelay: DelayUs<Error = TDelayError>,
{
    let mut byte = 0u8;
    let mut out_pin = pin
        .into_output_pin(PinState::High)
        .map_err(Error::WrappedIo)?;
    for bit in 0..8 {
        let result = read_bit(out_pin, delay)?;
        out_pin = result.pin;
        byte += (result.data as u8) << bit;
    }
    Ok(ReadResult {
        data: byte,
        pin: out_pin,
        phantom_io_pin: core::marker::PhantomData,
        phantom_in_pin: core::marker::PhantomData,
    })
}

/// Writes a single bit to the line.
fn write_bit<TIoPin, TInPin, TOutPin, TIoError, TInError, TOutError, TDelay, TDelayError>(
    bit: u8,
    pin: TIoPin,
    delay: &mut TDelay,
) -> Result<TOutPin, Error<TDelayError, TIoError, TInError, TOutError>>
where
    TIoPin: IoPin<TInPin, TOutPin, Error = TIoError>,
    TInPin: InputPin<Error = TInError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TOutPin: OutputPin<Error = TOutError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TDelay: DelayUs<Error = TDelayError>,
{
    // Ensure we wait for recovery period between reads/writes.
    let mut pin: TOutPin = pin
        .into_output_pin(PinState::High)
        .map_err(Error::WrappedIo)?;
    delay
        .delay_us(READ_WRITE_RECOVERY_TIME_US as u32)
        .map_err(Error::WrappedDelay)?;

    // Output bit.
    pin.set_low().map_err(Error::WrappedOutput)?;
    let op_delay_us: u8 = match bit {
        0 => WRITE_0_DURATION_US,
        _ => WRITE_1_DURATION_US,
    };
    delay
        .delay_us(op_delay_us as u32)
        .map_err(Error::WrappedDelay)?;

    // Return high and wait.
    pin.set_high().map_err(Error::WrappedOutput)?;
    if bit != 0 {
        delay
            .delay_us(WRITE_1_POST_BIT_DELAY_US as u32)
            .map_err(Error::WrappedDelay)?;
    }
    Ok(pin)
}

/// Reads a single bit from the line.
fn read_bit<TIoPin, TInPin, TOutPin, TIoError, TInError, TOutError, TDelay, TDelayError>(
    pin: TIoPin,
    delay: &mut TDelay,
) -> Result<
    ReadResult<bool, TOutPin, TInPin, TOutPin, TIoError, TInError, TOutError>,
    Error<TDelayError, TIoError, TInError, TOutError>,
>
where
    TIoPin: IoPin<TInPin, TOutPin, Error = TIoError>,
    TInPin: InputPin<Error = TInError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TOutPin: OutputPin<Error = TOutError> + IoPin<TInPin, TOutPin, Error = TIoError>,
    TDelay: DelayUs<Error = TDelayError>,
{
    // Ensure we wait for recovery period between reads/writes.
    let mut pin: TOutPin = pin
        .into_output_pin(PinState::High)
        .map_err(Error::WrappedIo)?;
    delay
        .delay_us(READ_WRITE_RECOVERY_TIME_US as u32)
        .map_err(Error::WrappedDelay)?;

    // Request bit.
    pin.set_low().map_err(Error::WrappedOutput)?;
    delay
        .delay_us(READ_REQUEST_DURATION_US as u32)
        .map_err(Error::WrappedDelay)?;
    pin.set_high().map_err(Error::WrappedOutput)?;

    // Read bit after sample delay.
    let pin: TInPin = pin.into_input_pin().map_err(Error::WrappedIo)?;
    delay
        .delay_us(READ_SAMPLE_DELAY_US as u32)
        .map_err(Error::WrappedDelay)?;
    let result = pin.is_high().map_err(Error::WrappedInput)?;

    // Wait for minimum read interval.
    let pin: TOutPin = pin
        .into_output_pin(PinState::High)
        .map_err(Error::WrappedIo)?;
    delay
        .delay_us(READ_POST_SAMPLE_DELAY_US as u32)
        .map_err(Error::WrappedDelay)?;

    Ok(ReadResult {
        pin: pin,
        data: result,
        phantom_io_pin: core::marker::PhantomData,
        phantom_in_pin: core::marker::PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_integer_part() {
        let temperature = Temperature::from_bytes(0xF9, 0x12);

        assert_eq!(temperature.integer_part(), 0x12F);
    }

    #[test]
    fn temp_negative_integer_part() {
        let temperature = Temperature::from_bytes(0x6E, 0xFE);

        assert_eq!(temperature.integer_part(), -25);
    }

    #[test]
    fn temp_negative_zero_integer_part() {
        let temperature = Temperature::from_bytes(0xF0, 0xFF);

        assert_eq!(temperature.integer_part(), 0);
    }

    #[test]
    fn temp_decimal_part() {
        let temperature = Temperature::from_bytes(0xF9, 0x12);

        assert_eq!(temperature.decimal_part(), 0x9);
    }

    #[test]
    fn temp_negative_decimal_part() {
        let temperature = Temperature::from_bytes(0x6E, 0xFE);

        assert_eq!(temperature.decimal_part(), -2);
    }

    #[test]
    fn temp_negative_zero_decimal_part() {
        let temperature = Temperature::from_bytes(0xF1, 0xFF);

        assert_eq!(temperature.decimal_part(), -15);
    }

    #[test]
    fn temp_nearest_integer_rounds_up() {
        let temperature = Temperature::from_bytes(0xF8, 0x12);

        assert_eq!(temperature.nearest_integer(), 0x130);
    }

    #[test]
    fn temp_nearest_integer_rounds_down() {
        let temperature = Temperature::from_bytes(0xF7, 0x12);

        assert_eq!(temperature.nearest_integer(), 0x12F);
    }

    #[test]
    fn temp_negative_nearest_integer_rounds_towards_zero() {
        let temperature = Temperature::from_bytes(0x6E, 0xFE);

        assert_eq!(temperature.nearest_integer(), -25);
    }

    #[test]
    fn temp_negative_nearest_integer_rounds_away_from_zero() {
        let temperature = Temperature::from_bytes(0x68, 0xFE);

        assert_eq!(temperature.nearest_integer(), -26);
    }

    macro_rules! assert_close {
        ($left:expr, $right:expr, $delta:expr) => {
            assert!(
                ($left - $right).abs() <= $delta,
                "left = {}, right = {}, not within delta = {}",
                $left,
                $right,
                $delta
            );
        };
    }

    macro_rules! test_temp_to_float {
        ($name:ident, $type_conv:expr, $low_sig:expr, $high_sig:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let temperature = Temperature::from_bytes($low_sig, $high_sig);

                let temp_float = $type_conv(temperature);
                assert_close!(temp_float, $expected, 0.000001);
            }
        };
    }

    test_temp_to_float!(temp_to_f32_no_decimal, f32::from, 0xF0, 0x12, 303.0);
    test_temp_to_float!(temp_to_f64_no_decimal, f64::from, 0xF0, 0x12, 303.0);
    test_temp_to_float!(temp_to_f32_full_precision, f32::from, 0xF5, 0x01, 31.3125);
    test_temp_to_float!(temp_to_f64_full_precision, f64::from, 0xF5, 0x01, 31.3125);
    test_temp_to_float!(temp_to_f32_low_precision, f32::from, 0xF8, 0x01, 31.5);
    test_temp_to_float!(temp_to_f64_low_precision, f64::from, 0xF8, 0x01, 31.5);
    test_temp_to_float!(temp_to_f32_negative, f32::from, 0x6E, 0xFE, -25.125);
    test_temp_to_float!(temp_to_f64_negative, f64::from, 0x6E, 0xFE, -25.125);
    test_temp_to_float!(temp_to_f32_negative_zero_a, f32::from, 0xF8, 0xFF, -0.5);
    test_temp_to_float!(temp_to_f64_negative_zero_a, f64::from, 0xF8, 0xFF, -0.5);
    test_temp_to_float!(temp_to_f32_negative_zero_b, f32::from, 0xF7, 0xFF, -0.5625);
    test_temp_to_float!(temp_to_f64_negative_zero_b, f64::from, 0xF7, 0xFF, -0.5625);
}
