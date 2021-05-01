use simple_sensors::dht11;
use std::time::{Duration, Instant};

mod fake_hal;
use fake_hal::digital as fake_digital;

#[tokio::test]
async fn set_invalid_interval_fails() -> Result<(), dht11::Error<fake_digital::Error>> {
    let mut sensor = dht11::Dht11::new(
        fake_digital::Pin::new("invalid-interval"),
        || Instant::now(),
        |instant| instant.elapsed(),
    )?;

    let result = sensor.set_minimum_read_interval(Duration::from_nanos(1));
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        dht11::Error::InvalidArgument::<fake_digital::Error>
    );
    Ok(())
}

#[tokio::test]
async fn read_all_zeros_succeeds() -> Result<(), dht11::Error<fake_digital::Error>> {
    let mut sensor = dht11::Dht11::new(
        fake_digital::Pin::new("all-zeros"),
        || Instant::now(),
        |instant| instant.elapsed(),
    )?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(
        result,
        dht11::DhtResponse {
            humidity: 0,
            humidity_decimal: 0,
            temperature: 0,
            temperature_decimal: 0
        }
    );
    Ok(())
}

#[tokio::test]
async fn read_with_valid_data() -> Result<(), dht11::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("invalid-data");
    fake_pin.set_data(vec![
        /* ACK */
        1, 1, 0, 0, 1, 1, /* Byte 0 = 0x11 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0,
        0, 1, 1, 1, /* Byte 1 = 0x00 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0,
        1, 1, /* Byte 2 = 0x0F */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1,
        1, 0, 0, 1, 1, 1, /* Byte 3 = 0x00 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0,
        1, 1, /* Parity = 0x20 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0,
        0, 1, 1, /* End */
        0, 0, 1, 1,
    ]);
    let mut sensor = dht11::Dht11::new(fake_pin, || Instant::now(), |instant| instant.elapsed())?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(
        result,
        dht11::DhtResponse {
            humidity: 0x11,
            humidity_decimal: 0,
            temperature: 0x0F,
            temperature_decimal: 0
        }
    );
    Ok(())
}

#[tokio::test]
async fn read_bad_parity_fails() -> Result<(), dht11::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("bad-parity");
    fake_pin.set_data(vec![
        /* ACK */
        1, 1, 0, 0, 1, 1, /* Byte 0 = 0x11 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0,
        0, 1, 1, 1, /* Byte 1 = 0x00 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0,
        1, 1, /* Byte 2 = 0x0F */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1,
        1, 0, 0, 1, 1, 1, /* Byte 3 = 0x00 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0,
        1, 1, /* Parity = 0x21 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0,
        0, 1, 1, 1, /* End */
        0, 0, 1, 1,
    ]);
    let mut sensor = dht11::Dht11::new(fake_pin, || Instant::now(), |instant| instant.elapsed())?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        dht11::Error::BadParity::<fake_digital::Error>
    );
    Ok(())
}

#[tokio::test]
async fn read_with_imperfect_timing_succeeds() -> Result<(), dht11::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("imperfect-timing");
    fake_pin.set_data(vec![
        /* ACK */
        1, 1, 0, 0, 1, 1, /* Byte 0 = 0x11 */
        0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, /* Byte 1 = 0x00 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1,
        1, 0, 0, 1, 1, /* Byte 2 = 0x0F */
        0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1,
        1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        /* Byte 3 = 0x00 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1,
        0, 0, 1, 1, 1, /* Parity = 0x20 */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0, 1, 1, 0, 0, 1, 1, /* End */
        0, 0, 1, 1,
    ]);
    let mut sensor = dht11::Dht11::new(fake_pin, || Instant::now(), |instant| instant.elapsed())?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(
        result,
        dht11::DhtResponse {
            humidity: 0x11,
            humidity_decimal: 0,
            temperature: 0x0F,
            temperature_decimal: 0
        }
    );
    Ok(())
}

#[tokio::test]
async fn read_with_timeout_fails() -> Result<(), dht11::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("timeout");
    fake_pin.set_data(vec![
        /* ACK */
        1, 1, 0, 0, 1, 1, /* Byte 0 = 0x11 */
        0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, /* Byte 1 = Invalid */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    ]);
    let mut sensor = dht11::Dht11::new(fake_pin, || Instant::now(), |instant| instant.elapsed())?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        dht11::Error::Timeout::<fake_digital::Error>
    );
    Ok(())
}
