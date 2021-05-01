use embedded_hal::blocking::spi::Transfer;

/// The maximum SPI clock speed when V_DD is 5V.
pub const MAX_CLK_AT_5V: u32 = 3_600_000;
/// The maximum SPI clock speed when V_DD is 2.7V.
pub const MAX_CLK_AT_2_7V: u32 = 1_350_000;

const NUM_CHANNELS: u8 = 8;

#[derive(Debug, PartialEq)]
pub enum Error<TIoError> {
    /// Wrapped error from the SPI.
    Wrapped(TIoError),
    /// Invalid argument was provided.
    InvalidArgument,
}

impl<TIoError> From<TIoError> for Error<TIoError> {
    fn from(error: TIoError) -> Error<TIoError> {
        Error::Wrapped(error)
    }
}

/// Which channels to read the voltage difference between.
#[derive(Clone, Copy, Debug)]
pub enum DifferentialMode {
    /// Reads V_channel_0 - V_channel_1.
    ZeroMinusOne,
    /// Reads V_channel_1 - V_channel_0.
    OneMinusZero,
    /// Reads V_channel_2 - V_channel_3.
    TwoMinusThree,
    /// Reads V_channel_3 - V_channel_2.
    ThreeMinusTwo,
    /// Reads V_channel_4 - V_channel_5.
    FourMinusFive,
    /// Reads V_channel_5 - V_channel_4.
    FiveMinusFour,
    /// Reads V_channel_6 - V_channel_7.
    SixMinusSeven,
    /// Reads V_channel_7 - V_channel_6.
    SevenMinusSix,
}

/// The type of read to make from the Mcp3008.
#[derive(Debug)]
pub enum Mcp3008Request {
    /// Read the voltage of the given channel as compared to the voltage between
    /// analogue ground (AGND) and the reference voltage (V_REF).
    ///
    /// I.e. `Output = 1024 * CHX / (V_REF - AGND);`
    ///
    /// The given channel must be in the inclusive range [0,7].
    SingleEnded(u8),
    /// Read the voltage of each differential input pair as compared to the
    /// voltage between analogue ground (AGND) and the reference voltage
    /// (V_REF).
    ///
    /// For example, if reading the CH0 input as IN+ and CH1 input as IN-, the
    /// measured voltage is:
    ///
    /// `Output = 1024 * (IN+ - IN-) / (V_REF - AGND);`
    Differential(DifferentialMode),
}

impl Mcp3008Request {
    /// Converts the request to the appropriate bit sequence for the sensor.
    /// See page 19 of https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf.
    ///
    /// This does *not* include the start-bit that must always be prepended to
    /// the request.
    ///
    /// The resulting byte looks like: 0b0000abcd, where:
    ///   a: single-ended (1) or differential read (0)
    ///   bcd: channel select bits, from most-significant to least-significant
    fn to_bits(&self) -> u8 {
        match self {
            Mcp3008Request::SingleEnded(channel) => {
                return 0b1000 | channel;
            }
            Mcp3008Request::Differential(mode) => {
                return *mode as u8;
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Mcp3008Response(pub u16);

// TODO: Generalize to MCP300X support.
/// A driver for reading values from an MCP3008 analog-to-digital converter.
pub struct Mcp3008<'spi, TSpi, TIoError>
where
    TSpi: Transfer<u8, Error = TIoError>,
{
    spi: &'spi mut TSpi,
}

impl<'spi, TSpi, TIoError> Mcp3008<'spi, TSpi, TIoError>
where
    TSpi: Transfer<u8, Error = TIoError>,
{
    /// Constructs an MCP3008 driver.
    ///
    /// It's important that the given SPI interface is configured correctly to
    /// work with the MCP3008 driver. This means:
    ///  
    /// * Use 8 bits per word.
    /// * Depending on V_DD, the maximum possible clock speed is:
    ///   * *2.7V*: 1.35MHz (MAX_CLOCK_AT_2_7V)
    ///   * *5V*: 3.6MHz (MAX_CLOCK_AT_5V)
    /// * Data is sent most-significant-bit first
    /// * SPI mode: 0 (i.e. idle low, capture on first transition)
    ///
    /// Refer to
    /// [the datasheet](https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf)
    /// for more information.
    pub fn new(spi: &mut TSpi) -> Mcp3008<TSpi, TIoError> {
        Mcp3008 { spi: spi }
    }

    /// Reads the requested data from the MCP3008.
    pub fn read(&mut self, request: Mcp3008Request) -> Result<Mcp3008Response, Error<TIoError>> {
        if let Mcp3008Request::SingleEnded(channel) = request {
            if channel >= NUM_CHANNELS {
                return Err(Error::InvalidArgument);
            }
        }

        // Send the request aligned such that it is easy to read data using
        // 8-bit words. See page 21 of
        // https://cdn-shop.adafruit.com/datasheets/MCP3008.pdf.
        //
        // The request sequence looks like:
        //   1 - start bit
        //   1/0 - single-ended/differential read
        //   X X X - channel select bits
        let mut tx_buf: [u8; 3] = [0x1, request.to_bits() << 4, 0x0];
        let rx = self.spi.try_transfer(&mut tx_buf)?;

        Ok(Mcp3008Response((((rx[1] & 3) as u16) << 8) + rx[2] as u16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Mcp3008Request::{Differential, SingleEnded};

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
