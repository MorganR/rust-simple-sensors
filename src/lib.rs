#![no_std]

/// A universal driver for reading from a DHT11 sensor.
pub mod dht11;
/// Provides universal utilities for reading data from MCP300x series analog-to-digital converters.
///
/// Refer to [this datasheet](https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf) for more
/// information about these devices.
pub mod mcp300x;
