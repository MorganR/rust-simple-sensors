use embedded_hal::blocking::spi::Transfer;

/// The maximum SPI clock speed when V<sub>DD</sub> is 5V.
pub const MAX_CLOCK_AT_5V: u32 = 3_600_000;
/// The maximum SPI clock speed when V<sub>DD</sub> is 2.7V.
pub const MAX_CLOCK_AT_2_7V: u32 = 1_350_000;

/// The number of addressable channels on an MCP3004.
pub const NUM_CHANNELS_MCP3004: u8 = 4;
/// The number of addressable channels on an MCP3008.
pub const NUM_CHANNELS_MCP3008: u8 = 8;

#[derive(Debug, PartialEq)]
pub enum Error<TIoError> {
    /// Wrapped error from the SPI driver.
    Wrapped(TIoError),
    /// Invalid argument was provided.
    InvalidArgument,
    /// Bad data was read. Check the connection and try again.
    BadData,
}

impl<TIoError> From<TIoError> for Error<TIoError> {
    fn from(error: TIoError) -> Error<TIoError> {
        Error::Wrapped(error)
    }
}

/// Which channels to read the voltage difference between.
#[derive(Clone, Copy, Debug)]
pub enum DifferentialMode {
    /// Reads V<sub>channel 0</sub> - V<sub>channel 1</sub>.
    ZeroMinusOne,
    /// Reads V<sub>channel 1</sub> - V<sub>channel 0</sub>.
    OneMinusZero,
    /// Reads V<sub>channel 2</sub> - V<sub>channel 3</sub>.
    TwoMinusThree,
    /// Reads V<sub>channel 3</sub> - V<sub>channel 2</sub>.
    ThreeMinusTwo,
    /// Reads V<sub>channel 4</sub> - V<sub>channel 5</sub>. MCP3008 only.
    FourMinusFive,
    /// Reads V<sub>channel 5</sub> - V<sub>channel 4</sub>. MCP3008 only.
    FiveMinusFour,
    /// Reads V<sub>channel 6</sub> - V<sub>channel 7</sub>. MCP3008 only.
    SixMinusSeven,
    /// Reads V<sub>channel 7</sub> - V<sub>channel 6</sub>. MCP3008 only.
    SevenMinusSix,
}

const MAX_DIFFERENTIAL_MODE_MCP3004: DifferentialMode = DifferentialMode::ThreeMinusTwo;

/// The type of read to make from the Mcp300x device.
#[derive(Clone, Copy, Debug)]
pub enum Request {
    /// Read the voltage of the given channel as compared to the voltage between analogue ground
    /// (V<sub>analog ground</sub>) and the reference voltage (V<sub>ref</sub>).
    ///
    /// Output = 1024 * (V<sub>channel</sub> - V<sub>analog ground</sub>) /
    /// (V<sub>ref</sub> - V<sub>analog ground</sub>)
    ///
    /// The given channel must be in the inclusive range \[0,4\] for an MCP3004 or \[0,7\] for an
    /// MCP3008.
    SingleEnded(u8),
    /// Read the voltage of each differential input pair as compared to the voltage between analogue
    /// ground (V<sub>analog ground</sub>) and the reference voltage (V<sub>ref</sub>).
    ///
    /// For example, if reading the CH0 input as IN+ and CH1 input as IN-, the
    /// measured voltage is:
    ///
    /// Output = 1024 * (V<sub>channel a</sub> - V<sub>channel b</sub>) /
    /// (V<sub>ref</sub> - V<sub>analog ground</sub>)
    Differential(DifferentialMode),
}

impl Request {
    /// Converts the request to the appropriate bit sequence for the sensor. See page 19 of
    /// https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf.
    ///
    /// This does *not* include the start-bit that must always be prepended to the request.
    ///
    /// The resulting byte looks like: 0b0000abcd, where:
    ///   a: single-ended (1) or differential read (0)
    ///   bcd: channel select bits, from most-significant to least-significant
    fn to_bits(&self) -> u8 {
        match self {
            Request::SingleEnded(channel) => {
                return 0b1000 | channel;
            }
            Request::Differential(mode) => {
                return *mode as u8;
            }
        }
    }
}

macro_rules! mcp_300x_impl {
    ($name:ident, $is_arg_invalid:expr) => {
        /// Reads the requested data from the device.
        ///
        /// It's important that the given SPI interface is configured correctly to work with the
        /// device. This means:
        ///
        /// * Use 8 bits per word.
        /// * Depending on V<sub>DD</sub>, the maximum possible clock speed is:
        ///   * *2.7V*: 1.35MHz ([`MAX_CLOCK_AT_2_7V`])
        ///   * *5V*: 3.6MHz ([`MAX_CLOCK_AT_5V`])
        /// * Data is sent most-significant-bit first
        /// * SPI mode: 0 (i.e. idle low, capture on first transition)
        /// * Chip select is performed automatically by the SPI driver, or manually around this
        ///   function call.
        ///
        /// This returns [`Error::InvalidArgument`] if the request is not possible for this device
        /// type. See [`Request`] for more details.
        ///
        /// A valid response is in the range \[0, 1023\]. What this means depends on the request:
        ///
        /// * `SingleEnded`: 1024 * (V<sub>channel</sub> - V<sub>analog ground</sub>) /
        ///   (V<sub>ref</sub> - V<sub>analog ground</sub>)
        /// * `Differential`: 1024 * (V<sub>channel a</sub> - V<sub>channel b</sub>) /
        ///   (V<sub>ref</sub> - V<sub>analog ground</sub>)
        ///
        /// Refer to [this datasheet](https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf) for more
        /// information about these devices.
        pub fn $name<TSpi, TIoError>(
            request: Request,
            spi: &mut TSpi,
        ) -> Result<u16, Error<TIoError>>
        where
            TSpi: Transfer<u8, Error = TIoError>,
        {
            if $is_arg_invalid(request) {
                return Err(Error::InvalidArgument);
            }
            read(request, spi)
        }
    };
}

mcp_300x_impl!(read_mcp3004, |request| match request {
    Request::SingleEnded(channel) => channel >= NUM_CHANNELS_MCP3004,
    Request::Differential(mode) => (mode as u8) > (MAX_DIFFERENTIAL_MODE_MCP3004 as u8),
});
mcp_300x_impl!(read_mcp3008, |request| match request {
    Request::SingleEnded(channel) => channel >= NUM_CHANNELS_MCP3008,
    _ => false,
});

fn read<TSpi, TIoError>(request: Request, spi: &mut TSpi) -> Result<u16, Error<TIoError>>
where
    TSpi: Transfer<u8, Error = TIoError>,
{
    // Send the request aligned such that it is easy to read data using 8-bit words. See page 21 of
    // https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf.
    //
    // The request sequence looks like:
    //   1 - start bit
    //   1/0 - single-ended/differential read
    //   X X X - channel select bits
    let mut tx_buf: [u8; 3] = [0x1, request.to_bits() << 4, 0x0];
    let rx = spi.transfer(&mut tx_buf)?;

    if (rx[1] & 0b100) != 0 {
        // MCP300x sensors should send a null-bit right before the data. If this is missing, then
        // this read can't be trusted.
        return Err(Error::BadData);
    }

    Ok((((rx[1] & 0b11) as u16) << 8) + rx[2] as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Request::{Differential, SingleEnded};

    macro_rules! test_request_to_bits {
        ($name:ident, $req:expr, $expected:expr) => {
            #[test]
            fn $name() {
                assert_eq!($req.to_bits(), $expected);
            }
        };
    }

    test_request_to_bits!(single_edge_0, SingleEnded(0), 0b1000);
    test_request_to_bits!(single_edge_1, SingleEnded(1), 0b1001);
    test_request_to_bits!(single_edge_2, SingleEnded(2), 0b1010);
    test_request_to_bits!(single_edge_3, SingleEnded(3), 0b1011);
    test_request_to_bits!(single_edge_4, SingleEnded(4), 0b1100);
    test_request_to_bits!(single_edge_5, SingleEnded(5), 0b1101);
    test_request_to_bits!(single_edge_6, SingleEnded(6), 0b1110);
    test_request_to_bits!(single_edge_7, SingleEnded(7), 0b1111);
    test_request_to_bits!(
        differential_0_minus_1,
        Differential(DifferentialMode::ZeroMinusOne),
        0b0000
    );
    test_request_to_bits!(
        differential_1_minus_0,
        Differential(DifferentialMode::OneMinusZero),
        0b0001
    );
    test_request_to_bits!(
        differential_2_minus_3,
        Differential(DifferentialMode::TwoMinusThree),
        0b0010
    );
    test_request_to_bits!(
        differential_3_minus_2,
        Differential(DifferentialMode::ThreeMinusTwo),
        0b0011
    );
    test_request_to_bits!(
        differential_4_minus_5,
        Differential(DifferentialMode::FourMinusFive),
        0b0100
    );
    test_request_to_bits!(
        differential_5_minus_4,
        Differential(DifferentialMode::FiveMinusFour),
        0b0101
    );
    test_request_to_bits!(
        differential_6_minus_7,
        Differential(DifferentialMode::SixMinusSeven),
        0b0110
    );
    test_request_to_bits!(
        differential_7_minus_6,
        Differential(DifferentialMode::SevenMinusSix),
        0b0111
    );
}
