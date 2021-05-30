use super::concurrent;
use embedded_hal::blocking::digital::{InputPin, IoPin, OutputPin, PinState};

#[derive(Debug, PartialEq)]
pub enum Error {}

#[derive(Debug)]
pub struct Pin {
    data_to_read: Option<Vec<u8>>,
    name: &'static str,
    default_data: bool,
}

impl Pin {
    pub fn new(name: &'static str) -> Pin {
        concurrent::set_named_value(name, 0);
        Pin {
            data_to_read: None,
            name: name,
            default_data: false,
        }
    }

    pub fn set_default_data(&mut self, default: bool) {
        self.default_data = default;
        self.data_to_read = None;
    }

    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data_to_read = Some(data);
        concurrent::set_named_value(&self.name, 0);
    }
}

impl InputPin for Pin {
    type Error = Error;

    fn is_high(&self) -> Result<bool, Self::Error> {
        if self.data_to_read.is_none() {
            return Ok(self.default_data);
        }

        let data_index = concurrent::get_and_increment_named_value(&self.name);
        Ok(self.data_to_read.as_ref().unwrap()[data_index] > 0)
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        if self.data_to_read.is_none() {
            return Ok(!self.default_data);
        }

        let data_index = concurrent::get_and_increment_named_value(&self.name);
        Ok(self.data_to_read.as_ref().unwrap()[data_index] == 0)
    }
}

impl OutputPin for Pin {
    type Error = Error;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        return Ok(());
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        return Ok(());
    }
}

impl IoPin<Pin, Pin> for Pin {
    type Error = Error;

    fn into_input_pin(self) -> Result<Pin, Self::Error> {
        Ok(self)
    }

    fn into_output_pin(self, _state: PinState) -> Result<Pin, Self::Error> {
        Ok(self)
    }
}
