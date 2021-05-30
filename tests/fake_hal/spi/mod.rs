use embedded_hal::nb::spi::FullDuplex;
use nb;

#[derive(Debug, PartialEq)]
pub struct SpiError();

pub enum FakeRead {
    Success(u8),
    Error(),
    AsyncSuccess(u8),
    AsyncError(),
}

pub enum FakeWrite {
    Success(),
    Error(),
    AsyncSuccess(),
    AsyncError(),
}

enum LastOp {
    None,
    FakeRead,
    FakeWrite,
}

pub struct SPI {
    reads: Vec<FakeRead>,
    writes: Vec<FakeWrite>,
    current_read: Option<FakeRead>,
    current_write: Option<FakeWrite>,
    last_complete_op: LastOp,
    written_data: Vec<u8>,
}

impl SPI {
    pub fn new(reads: Vec<FakeRead>, writes: Vec<FakeWrite>) -> SPI {
        if reads.len() > writes.len() {
            panic!("There must be at least as many writes as reads.");
        }
        SPI {
            written_data: Vec::with_capacity(writes.len()),
            reads: reads,
            writes: writes,
            current_read: None,
            current_write: None,
            last_complete_op: LastOp::None,
        }
    }

    pub fn get_written_data(&self) -> &[u8] {
        self.written_data.as_slice()
    }
}

impl FullDuplex<u8> for SPI {
    type Error = SpiError;
    fn read(&mut self) -> nb::Result<u8, SpiError> {
        match self.last_complete_op {
            LastOp::FakeWrite => {}
            _ => return Err(nb::Error::Other(SpiError())),
        }
        if self.current_read.is_none() {
            self.current_read = Some(self.reads.remove(0));
            let read = self.current_read.as_ref().unwrap();
            match *read {
                FakeRead::AsyncError() => return Err(nb::Error::WouldBlock),
                FakeRead::AsyncSuccess(_) => return Err(nb::Error::WouldBlock),
                _ => {}
            }
        }
        let read = self.current_read.take().unwrap();
        self.last_complete_op = LastOp::FakeRead;
        match read {
            FakeRead::Success(data) => return Ok(data),
            FakeRead::AsyncSuccess(data) => return Ok(data),
            _ => {}
        }
        Err(nb::Error::Other(SpiError()))
    }

    fn send(&mut self, word: u8) -> nb::Result<(), SpiError> {
        if self.current_write.is_none() {
            self.current_write = Some(self.writes.remove(0));
            let write = self.current_write.as_ref().unwrap();
            match *write {
                FakeWrite::AsyncError() => return Err(nb::Error::WouldBlock),
                FakeWrite::AsyncSuccess() => return Err(nb::Error::WouldBlock),
                _ => {}
            }
        }
        let write = self.current_write.take().unwrap();
        self.last_complete_op = LastOp::FakeWrite;
        match write {
            FakeWrite::Success() => {
                self.written_data.push(word);
                return Ok(());
            }
            FakeWrite::AsyncSuccess() => {
                self.written_data.push(word);
                return Ok(());
            }
            _ => {}
        }
        Err(nb::Error::Other(SpiError()))
    }
}
