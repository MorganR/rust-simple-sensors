use simple_sensors::dhtxx::{self, Response};
use std::time::{Duration, Instant};

mod fake_hal;
use fake_hal::digital as fake_digital;

macro_rules! test_new_with_invalid_options_fails {
    ($name:ident, $pin_name: expr, $new_dht_fn:expr, $options:expr) => {
        #[tokio::test]
        async fn $name() -> Result<(), dhtxx::Error<fake_digital::Error>> {
            let result = $new_dht_fn(
                fake_digital::Pin::new($pin_name),
                || Instant::now(),
                |instant| instant.elapsed(),
                Some($options),
            );

            assert!(result.is_err());
            assert_eq!(
                result.map(|_| ()).unwrap_err(),
                dhtxx::Error::InvalidArgument::<fake_digital::Error>
            );
            Ok(())
        }
    };
}

test_new_with_invalid_options_fails!(
    dht11_invalid_min_interval,
    "dht11-invalid-interval",
    dhtxx::Dht11::new,
    dhtxx::Options {
        min_read_interval: dhtxx::MIN_DHT11_READ_INTERVAL - Duration::from_millis(1),
        max_attempts: 1,
    }
);

test_new_with_invalid_options_fails!(
    dht22_invalid_min_interval,
    "dht22-invalid-interval",
    dhtxx::Dht22::new,
    dhtxx::Options {
        min_read_interval: dhtxx::MIN_DHT22_READ_INTERVAL - Duration::from_millis(1),
        max_attempts: 1,
    }
);

test_new_with_invalid_options_fails!(
    dht11_invalid_max_attempts,
    "dht11-invalid-attempts",
    dhtxx::Dht11::new,
    dhtxx::Options {
        min_read_interval: dhtxx::MIN_DHT11_READ_INTERVAL,
        max_attempts: 0,
    }
);

test_new_with_invalid_options_fails!(
    dht22_invalid_max_attempts,
    "dht22-invalid-attempts",
    dhtxx::Dht22::new,
    dhtxx::Options {
        min_read_interval: dhtxx::MIN_DHT22_READ_INTERVAL,
        max_attempts: 0,
    }
);

fn create_data_vec(bits: [u8; 40]) -> Vec<u8> {
    // Start with ACK
    let mut data = vec![1, 1, 0, 0, 1, 1];
    for bit in bits.iter() {
        match bit {
            0 => data.extend_from_slice(&[0, 0, 1, 1]),
            1 => data.extend_from_slice(&[0, 0, 1, 1, 1]),
            _ => panic!("Must provide bits as 0s and 1s."),
        }
    }
    // Add END
    data.extend_from_slice(&[0, 0, 1, 1]);
    data
}

#[tokio::test]
async fn read_all_zeros_succeeds() -> Result<(), dhtxx::Error<fake_digital::Error>> {
    let mut pin = fake_digital::Pin::new("all-zeros");
    pin.set_data(create_data_vec([
        0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
        0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
        0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
        0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
        0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
    ]));
    let mut sensor = dhtxx::Dht11::new(pin, || Instant::now(), |instant| instant.elapsed(), None)?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(
        result,
        dhtxx::Dht11Response {
            humidity: 0,
            humidity_decimal: 0,
            temperature: 0,
            temperature_decimal: 0
        }
    );
    Ok(())
}

#[tokio::test]
async fn dht11_read_with_valid_data() -> Result<(), dhtxx::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("dht11-valid-data");
    fake_pin.set_data(create_data_vec([
        0, 0, 0, 1, 0, 0, 0, 1, /*0x11*/
        0, 0, 0, 0, 0, 1, 0, 0, /*0x04*/
        0, 0, 0, 0, 1, 1, 1, 1, /*0x0F*/
        0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
        0, 0, 1, 0, 0, 1, 0, 0, /*0x24*/
    ]));
    let mut sensor = dhtxx::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed(),
        None,
    )?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(
        result,
        dhtxx::Dht11Response {
            humidity: 0x11,
            humidity_decimal: 0x04,
            temperature: 0x0F,
            temperature_decimal: 0
        }
    );
    Ok(())
}

#[tokio::test]
async fn dht22_read_with_valid_data() -> Result<(), dhtxx::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("dht22-valid-data");
    fake_pin.set_data(create_data_vec([
        0, 0, 0, 0, 0, 0, 1, 0, /*0x02*/
        1, 0, 0, 0, 0, 0, 0, 0, /*0x80*/
        0, 0, 0, 0, 0, 0, 0, 1, /*0x01*/
        0, 0, 0, 0, 0, 1, 0, 0, /*0x04*/
        1, 0, 0, 0, 0, 1, 1, 1, /*0x87*/
    ]));
    let mut sensor = dhtxx::Dht22::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed(),
        None,
    )?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(
        result,
        dhtxx::Dht22Response {
            humidity_x10: 0x0280,
            temperature_x10: 0x0104,
        }
    );
    Ok(())
}

macro_rules! test_read_bad_data_fails {
    ($name:ident, $pin_name: expr, $new_dht_fn:expr, $data:expr) => {
        #[tokio::test]
        async fn $name() -> Result<(), dhtxx::Error<fake_digital::Error>> {
            let mut pin = fake_digital::Pin::new($pin_name);
            pin.set_data($data);
            let mut sensor = $new_dht_fn(pin, Instant::now, |instant| instant.elapsed(), None)?;

            let result = sensor
                .read(|duration| tokio::time::sleep(duration.into()))
                .await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                dhtxx::Error::BadData::<fake_digital::Error>,
            );
            Ok(())
        }
    };
}

test_read_bad_data_fails!(
    dht11_read_with_invalid_temperature_fails,
    "dht11-invalid-temp",
    dhtxx::Dht11::new,
    create_data_vec([
        0, 0, 0, 1, 0, 0, 0, 1, /* Byte 0 = 0x11 */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 1 = 0x00 */
        1, 0, 1, 1, 1, 0, 1, 1, /* Byte 2 = 0xBB */
        0, 0, 0, 0, 0, 0, 0, 1, /* Byte 3 = 0x01 */
        0, 1, 0, 0, 1, 1, 0, 1, /* Parity = 0x0D */
    ])
);

test_read_bad_data_fails!(
    dht11_read_with_invalid_humidity_fails,
    "dht11-invalid-humidity",
    dhtxx::Dht11::new,
    create_data_vec([
        0, 1, 1, 0, 0, 1, 0, 1, /* Byte 0 = 0x65 (101) */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 1 = 0x00 */
        0, 0, 0, 0, 1, 0, 0, 1, /* Byte 2 = 0x09 */
        0, 0, 0, 0, 0, 0, 0, 1, /* Byte 3 = 0x01 */
        0, 1, 1, 0, 1, 1, 1, 1, /* Parity = 0x6F */
    ])
);

test_read_bad_data_fails!(
    dht11_read_bad_parity_fails,
    "dht11-bad-parity",
    dhtxx::Dht11::new,
    create_data_vec([
        0, 0, 0, 1, 0, 0, 0, 1, /* Byte 0 = 0x11 */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 1 = 0x00 */
        0, 0, 0, 0, 1, 1, 1, 1, /* Byte 2 = 0x0F */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 3 = 0x00 */
        0, 0, 0, 1, 0, 0, 0, 1, /* Parity = 0x11 */
    ])
);

test_read_bad_data_fails!(
    dht22_read_with_invalid_temperature_fails,
    "dht22-invalid-temp",
    dhtxx::Dht11::new,
    create_data_vec([
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 0 = 0x00 */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 1 = 0x00 */
        0, 0, 0, 0, 0, 1, 0, 1, /* Byte 2 = 0x05 */
        1, 1, 1, 0, 0, 1, 1, 1, /* Byte 3 = 0xE7 */
        1, 1, 1, 0, 1, 1, 0, 0, /* Parity = 0xEC */
    ])
);

test_read_bad_data_fails!(
    dht22_read_with_invalid_humidity_fails,
    "dht22-invalid-humidity",
    dhtxx::Dht11::new,
    create_data_vec([
        0, 0, 0, 0, 0, 0, 1, 1, /* Byte 0 = 0x03 */
        1, 1, 1, 0, 1, 0, 1, 0, /* Byte 1 = 0xEA */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 2 = 0x00 */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 3 = 0x00 */
        1, 1, 1, 0, 1, 1, 0, 1, /* Parity = 0xED */
    ])
);

test_read_bad_data_fails!(
    dht22_read_bad_parity_fails,
    "dht22-bad-parity",
    dhtxx::Dht11::new,
    create_data_vec([
        0, 0, 0, 0, 0, 0, 0, 1, /* Byte 0 = 0x01 */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 1 = 0x00 */
        0, 0, 0, 0, 0, 0, 0, 1, /* Byte 2 = 0x01 */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 3 = 0x00 */
        0, 0, 1, 0, 0, 0, 1, 1, /* Parity = 0x23 */
    ])
);

#[tokio::test]
async fn read_with_negative_temperature_dht22_succeeds(
) -> Result<(), dhtxx::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("negative-temperature-dht22");
    fake_pin.set_data(create_data_vec([
        0, 0, 0, 0, 0, 0, 0, 1, /* Byte 0 = 0x01 */
        0, 0, 0, 0, 0, 0, 0, 0, /* Byte 1 = 0x00 */
        1, 0, 0, 0, 0, 0, 0, 1, /* Byte 2 = 0x81 */
        0, 0, 0, 0, 0, 0, 0, 1, /* Byte 3 = 0x01 */
        1, 0, 0, 0, 0, 0, 1, 1, /* Parity = 0x93 */
    ]));
    let mut sensor = dhtxx::Dht22::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed(),
        None,
    )?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(result.get_temperature(), -25.7f32);
    Ok(())
}

#[tokio::test]
async fn read_with_imperfect_timing_succeeds() -> Result<(), dhtxx::Error<fake_digital::Error>> {
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
    let mut sensor = dhtxx::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed(),
        None,
    )?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await?;
    assert_eq!(
        result,
        dhtxx::Dht11Response {
            humidity: 0x11,
            humidity_decimal: 0,
            temperature: 0x0F,
            temperature_decimal: 0
        }
    );
    Ok(())
}

#[tokio::test]
async fn read_with_bit_timeout_fails() -> Result<(), dhtxx::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("timeout");
    fake_pin.set_data(vec![
        /* ACK */
        1, 1, 0, 0, 1, 1, /* Byte 0 = 0x11 */
        0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, /* Byte 1 = Invalid */
        0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    ]);
    let mut sensor = dhtxx::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed(),
        None,
    )?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        dhtxx::Error::BadData::<fake_digital::Error>
    );
    Ok(())
}

#[tokio::test]
async fn read_with_no_response_fails() -> Result<(), dhtxx::Error<fake_digital::Error>> {
    let mut fake_pin = fake_digital::Pin::new("no_response");
    fake_pin.set_default_data(true);
    let mut sensor = dhtxx::Dht11::new(
        fake_pin,
        || Instant::now(),
        |instant| instant.elapsed(),
        None,
    )?;

    let result = sensor
        .read(|duration| tokio::time::sleep(duration.into()))
        .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        dhtxx::Error::NoResponse::<fake_digital::Error>
    );
    Ok(())
}

macro_rules! test_retry_success {
    ($name:ident, $pin_name: expr, $new_dht_fn:expr) => {
        #[tokio::test]
        async fn $name() -> Result<(), dhtxx::Error<fake_digital::Error>> {
            let mut pin = fake_digital::Pin::new($pin_name);
            // Bad parity
            let mut data = create_data_vec([
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 1, /*0x01*/
            ]);
            // Valid data
            data.append(&mut create_data_vec([
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
            ]));
            pin.set_data(data);
            let mut sensor = $new_dht_fn(
                pin,
                || Instant::now(),
                |instant| instant.elapsed(),
                Some(dhtxx::Options {
                    min_read_interval: std::cmp::max(
                        dhtxx::MIN_DHT11_READ_INTERVAL,
                        dhtxx::MIN_DHT22_READ_INTERVAL,
                    ),
                    max_attempts: 2,
                }),
            )?;

            let result = sensor.read(tokio::time::sleep).await?;

            assert_eq!(result.get_humidity(), 0.0);
            assert_eq!(result.get_temperature(), 0.0);
            Ok(())
        }
    };
}

test_retry_success!(dh11_retry_success, "dht11-retry-success", dhtxx::Dht11::new);
test_retry_success!(dh22_retry_success, "dht22-retry-success", dhtxx::Dht22::new);

macro_rules! test_retry_bad_data {
    ($name:ident, $pin_name: expr, $new_dht_fn:expr) => {
        #[tokio::test]
        async fn $name() -> Result<(), dhtxx::Error<fake_digital::Error>> {
            let mut pin = fake_digital::Pin::new($pin_name);
            // Bad parity
            let mut data = create_data_vec([
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 1, /*0x01*/
            ]);
            // Bad parity
            data.append(&mut create_data_vec([
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 0, /*0x00*/
                0, 0, 0, 0, 0, 0, 0, 1, /*0x00*/
            ]));
            pin.set_data(data);
            let mut sensor = $new_dht_fn(
                pin,
                || Instant::now(),
                |instant| instant.elapsed(),
                Some(dhtxx::Options {
                    min_read_interval: std::cmp::max(
                        dhtxx::MIN_DHT11_READ_INTERVAL,
                        dhtxx::MIN_DHT22_READ_INTERVAL,
                    ),
                    max_attempts: 2,
                }),
            )?;

            let result = sensor.read(tokio::time::sleep).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                dhtxx::Error::BadData::<fake_digital::Error>
            );
            Ok(())
        }
    };
}

test_retry_bad_data!(
    dh11_retry_bad_data,
    "dht11-retry-bad-data",
    dhtxx::Dht11::new
);
test_retry_bad_data!(
    dh22_retry_bad_data,
    "dht22-retry-bad-data",
    dhtxx::Dht22::new
);

macro_rules! test_retry_fail_on_no_response {
    ($name:ident, $pin_name: expr, $new_dht_fn:expr) => {
        #[tokio::test]
        async fn $name() -> Result<(), dhtxx::Error<fake_digital::Error>> {
            let mut pin = fake_digital::Pin::new($pin_name);
            pin.set_default_data(true);
            let mut sensor = $new_dht_fn(
                pin,
                || Instant::now(),
                |instant| instant.elapsed(),
                Some(dhtxx::Options {
                    min_read_interval: std::cmp::max(
                        dhtxx::MIN_DHT11_READ_INTERVAL,
                        dhtxx::MIN_DHT22_READ_INTERVAL,
                    ),
                    max_attempts: 2,
                }),
            )?;

            let result = sensor.read(tokio::time::sleep).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                dhtxx::Error::NoResponse::<fake_digital::Error>
            );
            Ok(())
        }
    };
}

test_retry_fail_on_no_response!(
    dht11_retry_fail_on_no_response,
    "dht11-retry-fail-on-no-response",
    dhtxx::Dht11::new
);
test_retry_fail_on_no_response!(
    dht22_retry_fail_on_no_response,
    "dht22-retry-fail-on-no-response",
    dhtxx::Dht22::new
);
