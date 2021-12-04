use simple_sensors::mcp300x;

mod fake_hal;
use fake_hal::spi as fake_spi;

macro_rules! test_synchronous_read_success {
    ($name:ident, $read_fn:expr, $request:expr) => {
        #[test]
        fn $name() -> Result<(), mcp300x::Error<fake_spi::SpiError>> {
            let mut spi = fake_spi::SPI::new(
                vec![
                    fake_spi::FakeRead::Success(0),
                    fake_spi::FakeRead::Success(1),
                    fake_spi::FakeRead::Success(0xF1),
                ],
                vec![
                    fake_spi::FakeWrite::Success(),
                    fake_spi::FakeWrite::Success(),
                    fake_spi::FakeWrite::Success(),
                ],
            );

            let result = $read_fn($request, &mut spi)?;

            assert_eq!(result, 0x1F1);
            Ok(())
        }
    };
}

test_synchronous_read_success!(
    read_synchronous_3008_se0,
    mcp300x::read_mcp3008,
    mcp300x::Request::SingleEnded(0)
);
test_synchronous_read_success!(
    read_synchronous_3008_se7,
    mcp300x::read_mcp3008,
    mcp300x::Request::SingleEnded(7)
);
test_synchronous_read_success!(
    read_synchronous_3004_se0,
    mcp300x::read_mcp3004,
    mcp300x::Request::SingleEnded(0)
);
test_synchronous_read_success!(
    read_synchronous_3004_se3,
    mcp300x::read_mcp3004,
    mcp300x::Request::SingleEnded(3)
);
test_synchronous_read_success!(
    read_synchronous_3008_d01,
    mcp300x::read_mcp3008,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::ZeroMinusOne)
);
test_synchronous_read_success!(
    read_synchronous_3008_d76,
    mcp300x::read_mcp3008,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::SevenMinusSix)
);
test_synchronous_read_success!(
    read_synchronous_3004_d01,
    mcp300x::read_mcp3004,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::ZeroMinusOne)
);
test_synchronous_read_success!(
    read_synchronous_3004_d32,
    mcp300x::read_mcp3004,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::ThreeMinusTwo)
);

#[test]
fn read_ignores_noise() -> Result<(), mcp300x::Error<fake_spi::SpiError>> {
    let mut spi = fake_spi::SPI::new(
        vec![
            fake_spi::FakeRead::Success(0xFF),
            fake_spi::FakeRead::Success(0xFB),
            fake_spi::FakeRead::Success(0x44),
        ],
        vec![
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
        ],
    );

    let result = mcp300x::read_mcp3008(mcp300x::Request::SingleEnded(0), &mut spi)?;

    assert_eq!(result, 0x344);
    Ok(())
}

#[test]
fn read_missing_preceding_null_fails() -> Result<(), mcp300x::Error<fake_spi::SpiError>> {
    let mut spi = fake_spi::SPI::new(
        vec![
            fake_spi::FakeRead::Success(0xFF),
            fake_spi::FakeRead::Success(0xFF),
            fake_spi::FakeRead::Success(0x0),
        ],
        vec![
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
            fake_spi::FakeWrite::Success(),
        ],
    );

    let result = mcp300x::read_mcp3008(mcp300x::Request::SingleEnded(0), &mut spi);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        mcp300x::Error::BadData::<fake_spi::SpiError>
    );
    Ok(())
}

#[test]
fn sends_expected_request() -> Result<(), mcp300x::Error<fake_spi::SpiError>> {
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

    mcp300x::read_mcp3008(mcp300x::Request::SingleEnded(1), &mut spi)?;

    assert_eq!(spi.get_written_data(), [0x1, 0x90, 0x0]);
    Ok(())
}

#[test]
fn asynchronous_writes_succeed() -> Result<(), mcp300x::Error<fake_spi::SpiError>> {
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

    let result = mcp300x::read_mcp3008(
        mcp300x::Request::Differential(mcp300x::DifferentialMode::OneMinusZero),
        &mut spi,
    )?;

    assert_eq!(result, 0xF0);
    assert_eq!(spi.get_written_data(), [0x1, 0x10, 0x0]);
    Ok(())
}

macro_rules! test_invalid_request {
    ($name:ident, $read_fn:expr, $request:expr) => {
        #[test]
        fn $name() -> Result<(), mcp300x::Error<fake_spi::SpiError>> {
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

            let result = $read_fn($request, &mut spi);
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                mcp300x::Error::InvalidArgument::<fake_spi::SpiError>
            );
            Ok(())
        }
    };
}

test_invalid_request!(
    invalid_channel_mcp3008,
    mcp300x::read_mcp3008,
    mcp300x::Request::SingleEnded(8)
);
test_invalid_request!(
    invalid_channel_mcp3004,
    mcp300x::read_mcp3004,
    mcp300x::Request::SingleEnded(4)
);
test_invalid_request!(
    invalid_differential_mode_mcp3004_4m5,
    mcp300x::read_mcp3004,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::FourMinusFive)
);
test_invalid_request!(
    invalid_differential_mode_mcp3004_5m4,
    mcp300x::read_mcp3004,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::FiveMinusFour)
);
test_invalid_request!(
    invalid_differential_mode_mcp3004_6m7,
    mcp300x::read_mcp3004,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::SixMinusSeven)
);
test_invalid_request!(
    invalid_differential_mode_mcp3004_7m6,
    mcp300x::read_mcp3004,
    mcp300x::Request::Differential(mcp300x::DifferentialMode::SevenMinusSix)
);
