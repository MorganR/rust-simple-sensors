use embedded_hal::blocking::spi::transfer::Default as DefaultTransfer;
use simple_sensors::mcp3008;

mod fake_hal;
use fake_hal::spi as fake_spi;

impl DefaultTransfer<u8> for fake_spi::SPI {}

#[test]
fn read_synchronous() -> Result<(), mcp3008::Error<fake_spi::SpiError>> {
    let mut spi = fake_spi::SPI::new(
        vec![
            fake_spi::FakeRead::Success(0),
            fake_spi::FakeRead::Success(1),
            fake_spi::FakeRead::Success(0xFF),
        ],
        vec![
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
        ],
    );
    let mut sensor = mcp3008::Mcp3008::new(&mut spi);

    let result = sensor.read(mcp3008::Mcp3008Request::SingleEnded(0))?;

    assert_eq!(result, mcp3008::Mcp3008Response(0x1FF));
    Ok(())
}

#[test]
fn expected_request_sent() -> Result<(), mcp3008::Error<fake_spi::SpiError>> {
    let mut spi = fake_spi::SPI::new(
        vec![
            fake_spi::FakeRead::Success(0),
            fake_spi::FakeRead::Success(0),
            fake_spi::FakeRead::Success(0),
        ],
        vec![
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
        ],
    );
    let mut sensor = mcp3008::Mcp3008::new(&mut spi);

    sensor.read(mcp3008::Mcp3008Request::SingleEnded(1))?;

    assert_eq!(spi.get_written_data(), [0x1, 0x90, 0x0]);
    Ok(())
}

#[test]
fn asynchronous_writes_succeed() -> Result<(), mcp3008::Error<fake_spi::SpiError>> {
    let mut spi = fake_spi::SPI::new(
        vec![
            fake_spi::FakeRead::Success(0),
            fake_spi::FakeRead::Success(0),
            fake_spi::FakeRead::Success(0xF0),
        ],
        vec![
            fake_spi::FakeWrite::AsyncSuccess(),
            fake_spi::FakeWrite::AsyncSuccess(),
            fake_spi::FakeWrite::AsyncSuccess(),
        ],
    );
    let mut sensor = mcp3008::Mcp3008::new(&mut spi);

    let result = sensor.read(mcp3008::Mcp3008Request::Differential(
        mcp3008::DifferentialMode::OneMinusZero,
    ))?;

    assert_eq!(result, mcp3008::Mcp3008Response(0xF0));
    assert_eq!(spi.get_written_data(), [0x1, 0x10, 0x0]);
    Ok(())
}

#[test]
fn read_invalid_channel_fails() -> Result<(), mcp3008::Error<fake_spi::SpiError>> {
    let mut spi = fake_spi::SPI::new(
        vec![
            fake_spi::FakeRead::Success(0),
            fake_spi::FakeRead::Success(0),
            fake_spi::FakeRead::Success(0xF0),
        ],
        vec![
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
        ],
    );
    let mut sensor = mcp3008::Mcp3008::new(&mut spi);

    let result = sensor.read(mcp3008::Mcp3008Request::SingleEnded(8));
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        mcp3008::Error::InvalidArgument::<fake_spi::SpiError>
    );
    Ok(())
}
