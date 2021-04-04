use embedded_hal::digital::{InputPin, IoPin, OutputPin, PinState};
use common_sensors::dht11;
use std::time::{Instant, Duration};

#[derive(Debug)]
enum FakeError {}

static mut data_index: usize = 0;

struct FakePin {
    data_to_read: Option<Vec<u8>>,
}

impl FakePin {
    fn new() -> FakePin {
        unsafe {
            data_index = 0;
        }
        FakePin{ data_to_read: None }
    }

    fn set_data(&mut self, data: Vec<u8>) {
        self.data_to_read = Some(data);
        unsafe {
            data_index = 0;
        }
    }

    fn clear_data(&mut self) {
        self.data_to_read = None;
        unsafe {
            data_index = 0;
        }
    }
}

impl InputPin for FakePin {
    type Error = FakeError;

    fn try_is_high(&self) -> Result<bool, Self::Error> {
        if self.data_to_read.is_none() {
            return Ok(false);
        }

        unsafe {
            data_index += 1;
            Ok(self.data_to_read.as_ref().unwrap()[data_index - 1] > 0)
        }
    }

    fn try_is_low(&self) -> Result<bool, Self::Error> {
        if self.data_to_read.is_none() {
            return Ok(false);
        }

        unsafe {
            data_index += 1;
            Ok(self.data_to_read.as_ref().unwrap()[data_index - 1] == 0)
        }
    }
}

impl OutputPin for FakePin {
    type Error = FakeError;

    fn try_set_low(&mut self) -> Result<(), Self::Error> {
        return Ok(());
    }

    fn try_set_high(&mut self) -> Result<(), Self::Error> {
        return Ok(());
    }
}

impl IoPin<FakePin, FakePin> for FakePin {
    type Error = FakeError;

    fn try_switch_to_input_pin(self) -> Result<FakePin, Self::Error> {
        Ok(self)
    }

    fn try_switch_to_output_pin(self, _state: PinState) -> Result<FakePin, Self::Error> {
        Ok(self)
    }
}

#[tokio::test]
async fn test_set_invalid_interval() -> Result<(), dht11::Error<FakeError>> {
    let mut sensor = dht11::Dht11::new(
        FakePin::new(),
        || Instant::now(),
        |instant| instant.elapsed())?;

    assert!(!sensor.set_minimum_read_interval(Duration::from_nanos(1)).is_ok());
    Ok(())
}

#[tokio::test]
async fn test_read_all_zeros() -> Result<(), dht11::Error<FakeError>> {
    let mut sensor = dht11::Dht11::new(
        FakePin::new(),
        || Instant::now(),
        |instant| instant.elapsed())?;

    let result = sensor.read(|duration| tokio::time::sleep(duration.into())).await?;
    assert_eq!(result, dht11::DhtResponse{ humidity: 0, humidity_decimal: 0, temperature: 0, temperature_decimal: 0});
    Ok(())
}

#[tokio::test]
async fn test_read_with_valid_data() -> Result<(), dht11::Error<FakeError>> {
    let mut fake_pin = FakePin::new();
    fake_pin.set_data(vec![
        /* ACK */
        1,1,0,0,1,1,
        /* Byte 0 = 0x11 */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,
        /* Byte 1 = 0x00 */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,
        /* Byte 2 = 0x0F */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1, 0,0,1,1,1, 0,0,1,1,1, 0,0,1,1,1,
        /* Byte 3 = 0x00 */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,
        /* Parity = 0x20 */
        0,0,1,1, 0,0,1,1, 0,0,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,
        /* End */
        0,0,1,1,]);
    let mut sensor = dht11::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed())?;

    let result = sensor.read(|duration| tokio::time::sleep(duration.into())).await?;
    assert_eq!(result, dht11::DhtResponse{ humidity: 0x11, humidity_decimal: 0, temperature: 0x0F, temperature_decimal: 0});
    Ok(())
}

#[tokio::test]
async fn test_read_bad_parity() -> Result<(), dht11::Error<FakeError>> {
    let mut fake_pin = FakePin::new();
    fake_pin.set_data(vec![
        /* ACK */
        1,1,0,0,1,1,
        /* Byte 0 = 0x11 */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,
        /* Byte 1 = 0x00 */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,
        /* Byte 2 = 0x0F */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1, 0,0,1,1,1, 0,0,1,1,1, 0,0,1,1,1,
        /* Byte 3 = 0x00 */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,
        /* Parity = 0x21 */
        0,0,1,1, 0,0,1,1, 0,0,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,
        /* End */
        0,0,1,1,]);
    let mut sensor = dht11::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed())?;

    let result = sensor.read(|duration| tokio::time::sleep(duration.into())).await;
    assert!(match result {
        Err(dht11::Error::BadParity()) => true,
        _ => false
    });
    Ok(())
}

#[tokio::test]
async fn test_read_with_imperfect_timing() -> Result<(), dht11::Error<FakeError>> {
    let mut fake_pin = FakePin::new();
    fake_pin.set_data(vec![
        /* ACK */
        1,1,0,0,1,1,
        /* Byte 0 = 0x11 */
        0,0,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,1,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,1,1,1,
        /* Byte 1 = 0x00 */
        0,0,1,1, 0,0,1,1, 0,0,1,1,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,
        /* Byte 2 = 0x0F */
        0,0,1,1, 0,0,1, 0,0,1,1,1, 0,0,1,1, 0,0,1,1,1,1,1,1,1, 0,0,1,1,1,1,1,1,1,1, 0,0,1,1,1,1,1,1,1,1, 0,0,1,1,1,1,1,1,1,1,1,1,
        /* Byte 3 = 0x00 */
        0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,1, 0,0,1,1,1,
        /* Parity = 0x20 */
        0,0,1,1, 0,0,1,1, 0,0,1,1,1,1,1,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,
        /* End */
        0,0,1,1,]);
    let mut sensor = dht11::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed())?;

    let result = sensor.read(|duration| tokio::time::sleep(duration.into())).await?;
    assert_eq!(result, dht11::DhtResponse{ humidity: 0x11, humidity_decimal: 0, temperature: 0x0F, temperature_decimal: 0});
    Ok(())
}

#[tokio::test]
async fn test_read_with_timeout() -> Result<(), dht11::Error<FakeError>> {
    let mut fake_pin = FakePin::new();
    fake_pin.set_data(vec![
        /* ACK */
        1,1,0,0,1,1,
        /* Byte 0 = 0x11 */
        0,0,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,1,1,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1, 0,0,1,1,1,1,1,1,
        /* Byte 1 = Invalid */
        0,0,1,1, 0,0,1,1, 0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1 ]);
    let mut sensor = dht11::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed())?;

    let result = sensor.read(|duration| tokio::time::sleep(duration.into())).await;
    assert!(match result {
        Err(dht11::Error::Timeout()) => true,
        _ => false,
    });
    Ok(())
}