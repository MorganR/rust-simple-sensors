#![no_std]

/// Universal drivers for reading from DHT11 or DHT22 sensors.
pub mod dhtxx;
/// Universal utilities for reading data from MCP300x series analog-to-digital converters.
///
/// Refer to [this datasheet](https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf) for more
/// information about these devices.
pub mod mcp300x;
