// #![no_std]

/// Universal drivers for reading from DHT11 or DHT22 sensors.
///
/// Communication with DHT sensors is performed using a single pin that can switch between input
/// and output mode. This pin should use a pull-up resistor so the idle state is high. A 4.7kOhm
/// resistor is often a good choice, but refer to your device's datasheet to be sure.
pub mod dhtxx;

/// Universal utilities for communicating with a DS18B20 temperature sensor.
///
/// This digital temperature sensor communicates over a single pin. Multiple sensors can be
/// connected to the same line and communicated with individually. The line should use a 4.7kOhm
/// pull-up resistor so the idle state is high. If the line is pulled low for more than 480
/// microseconds, then all DS18B20 sensors on the line will be reset.
///
/// Refer to [this datasheet](https://datasheets.maximintegrated.com/en/ds/DS18B20.pdf) for more
/// information about this sensor.
pub mod ds18b20;

/// Universal utilities for reading data from MCP300x series analog-to-digital converters.
///
/// Refer to [this datasheet](https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf) for more
/// information about these devices.
pub mod mcp300x;
