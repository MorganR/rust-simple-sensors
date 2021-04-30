mod concurrent;
use embedded_hal::digital::{InputPin, IoPin, OutputPin, PinState};

#[derive(Debug)]
pub enum Error {}

pub struct Pin {
    data_to_read: Option<Vec<u8>>,
    name: &'static str,
}

impl Pin {
    pub fn new(name: &'static str) -> Pin {
        concurrent::set_named_value(name, 0);
        Pin {
            data_to_read: None,
            name: name,
        }
    }

    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data_to_read = Some(data);
        concurrent::set_named_value(&self.name, 0);
    }
}

impl InputPin for Pin {
    type Error = Error;

    fn try_is_high(&self) -> Result<bool, Self::Error> {
        if self.data_to_read.is_none() {
            return Ok(false);
        }

        let data_index = concurrent::get_and_increment_named_value(&self.name);
        Ok(self.data_to_read.as_ref().unwrap()[data_index] > 0)
    }

    fn try_is_low(&self) -> Result<bool, Self::Error> {
        if self.data_to_read.is_none() {
            return Ok(false);
        }

        let data_index = concurrent::get_and_increment_named_value(&self.name);
        Ok(self.data_to_read.as_ref().unwrap()[data_index] == 0)
    }
}

impl OutputPin for Pin {
    type Error = Error;

    fn try_set_low(&mut self) -> Result<(), Self::Error> {
        return Ok(());
    }

    fn try_set_high(&mut self) -> Result<(), Self::Error> {
        return Ok(());
    }
}

impl IoPin<Pin, Pin> for Pin {
    type Error = Error;

    fn try_into_input_pin(self) -> Result<Pin, Self::Error> {
        Ok(self)
    }

    fn try_into_output_pin(self, _state: PinState) -> Result<Pin, Self::Error> {
        Ok(self)
    }
}
