use embedded_hal::nb::spi::FullDuplex;

mod fake_hal;
use fake_hal::spi::*;

#[test]
#[should_panic]
fn new_with_more_reads_than_writes_panics() {
    SPI::new(vec![FakeRead::Success(1)], vec![]);
}

#[test]
#[should_panic]
fn too_many_writes_panics() {
    let mut spi = SPI::new(vec![], vec![]);
    let _ = spi.send(0);
}

#[test]
fn read_synchronous() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![FakeRead::Success(4)], vec![FakeWrite::Success()]);

    spi.send(0)?;
    let result = spi.read()?;

    assert_eq!(result, 4);
    Ok(())
}

#[test]
fn read_multiple() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(
        vec![FakeRead::Success(1), FakeRead::Success(2)],
        vec![FakeWrite::Success(), FakeWrite::Success()],
    );

    spi.send(0)?;
    assert_eq!(spi.read()?, 1);

    spi.send(0)?;
    assert_eq!(spi.read()?, 2);
    Ok(())
}

#[test]
fn read_asynchronous() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![FakeRead::AsyncSuccess(4)], vec![FakeWrite::Success()]);

    spi.send(0)?;
    let result = spi.read();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::WouldBlock);

    let result = spi.read()?;
    assert_eq!(result, 4);
    Ok(())
}

#[test]
fn read_error() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![FakeRead::Error()], vec![FakeWrite::Success()]);

    spi.send(0)?;
    let result = spi.read();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::Other(SpiError()));
    Ok(())
}

#[test]
fn read_error_async() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![FakeRead::AsyncError()], vec![FakeWrite::Success()]);

    spi.send(0)?;
    let result = spi.read();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::WouldBlock);

    let result = spi.read();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::Other(SpiError()));
    Ok(())
}

#[test]
fn read_no_write_fails() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![FakeRead::Success(4)], vec![FakeWrite::Success()]);

    let result = spi.read();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::Other(SpiError()));
    Ok(())
}

#[test]
fn read_more_than_write_fails() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(
        vec![FakeRead::Success(0), FakeRead::Success(1)],
        vec![FakeWrite::Success(), FakeWrite::Success()],
    );

    spi.send(0)?;
    spi.read()?;
    let result = spi.read();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::Other(SpiError()));
    Ok(())
}

#[test]
fn write_synchronous() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![], vec![FakeWrite::Success()]);

    spi.send(4)?;
    assert_eq!(spi.get_written_data(), [4]);
    Ok(())
}

#[test]
fn write_asynchronous() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![], vec![FakeWrite::AsyncSuccess()]);

    let async_result = spi.send(4);
    assert!(async_result.is_err());
    assert_eq!(async_result.unwrap_err(), nb::Error::WouldBlock);
    assert_eq!(spi.get_written_data(), []);

    spi.send(4)?;
    assert_eq!(spi.get_written_data(), [4]);
    Ok(())
}

#[test]
fn write_multiple() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![], vec![FakeWrite::Error(), FakeWrite::Success()]);

    let result = spi.send(0);
    assert!(result.is_err());

    let result = spi.send(0);
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn write_error() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![], vec![FakeWrite::Error()]);

    let result = spi.send(4);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::Other(SpiError()));
    Ok(())
}

#[test]
fn write_async_error() -> nb::Result<(), SpiError> {
    let mut spi = SPI::new(vec![], vec![FakeWrite::AsyncError()]);

    let result = spi.send(4);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::WouldBlock);

    let result = spi.send(4);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), nb::Error::Other(SpiError()));
    Ok(())
}
