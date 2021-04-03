use embedded_hal::digital::{InputPin, IoPin, OutputPin, PinState};
use sensors::dht11;
use std::time::Duration;

#[derive(Debug)]
enum FakeError {}

struct FakePin {}

impl InputPin for FakePin {
    type Error = FakeError;

    fn try_is_high(&self) -> Result<bool, Self::Error> {
        return Ok(true);
    }

    fn try_is_low(&self) -> Result<bool, Self::Error> {
        return Ok(false);
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
async fn test_read_with_bad_interval() -> Result<(), dht11::Error<FakeError>> {
    let mut sensor = dht11::Dht11::new(FakePin{})?;

    assert!(!sensor.read_with_minimum_interval(Duration::from_nanos(1)).await.is_ok());
    Ok(())
}
