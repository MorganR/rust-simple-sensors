use simple_sensors::dht11;
use std::time::{Duration, Instant};

mod fake_hal;

#[tokio::test]
async fn test_set_invalid_interval() -> Result<(), dht11::Error<fake_hal::Error>> {
    let mut sensor = dht11::Dht11::new(
        fake_hal::Pin::new("invalid-interval"),
        || Instant::now(),
        |instant| instant.elapsed(),
    )?;

    assert!(!sensor
        .set_minimum_read_interval(Duration::from_nanos(1))
        .is_ok());
    Ok(())
}

#[tokio::test]
async fn test_read_all_zeros() -> Result<(), dht11::Error<fake_hal::Error>> {
    let mut sensor = dht11::Dht11::new(
        fake_hal::Pin::new("all-zeros"),
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
async fn test_read_with_valid_data() -> Result<(), dht11::Error<fake_hal::Error>> {
    let mut fake_pin = fake_hal::Pin::new("invalid-data");
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
async fn test_read_bad_parity() -> Result<(), dht11::Error<fake_hal::Error>> {
    let mut fake_pin = fake_hal::Pin::new("bad-parity");
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
    assert!(match result {
        Err(dht11::Error::BadParity()) => true,
        _ => false,
    });
    Ok(())
}

#[tokio::test]
async fn test_read_with_imperfect_timing() -> Result<(), dht11::Error<fake_hal::Error>> {
    let mut fake_pin = fake_hal::Pin::new("imperfect-timing");
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
async fn test_read_with_timeout() -> Result<(), dht11::Error<fake_hal::Error>> {
    let mut fake_pin = fake_hal::Pin::new("timeout");
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
    assert!(match result {
        Err(dht11::Error::Timeout()) => true,
        _ => false,
    });
    Ok(())
}
