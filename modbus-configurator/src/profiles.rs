//! Supported configurable RS485 Modbus-RTU sensor models and their register maps.
//!
//! All supported sensor families store their slave address and baud rate in holding registers,
//! but they differ in *which* registers those are, how the baud rate is encoded, and which write
//! function code they accept — captured by [`SensorProfile`]. See `../docs/sensor-profiles.md` for
//! the full register maps and wire examples.

/// A configurable RS485 sensor model.
///
/// Each variant is paired (via [`SensorType::profile`]) with the vendor-specific holding-register
/// addresses the sensor uses to persist its slave address and baud rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorType {
    /// SN-300AL-GH-N01 — and the broader "N01" RS485 family that shares the
    /// `0x07D0`/`0x07D1` address/baud-rate register map (slave addresses `1..=254`).
    Sn300AlGhN01,
    /// Generic RS485 light-intensity sensor (address register `0x0100`, baud-rate register
    /// `0x0101`, slave addresses `1..=251`). Requires a power cycle after programming.
    LightIntensityRs485,
    /// Soil multi-parameter sensor (temperature / moisture / EC / NPK / pH) — the 7-in-1/8-in-1
    /// RS485 family documented in "Soil Multi-parameter Sensor Manual V2.2". Config registers at
    /// `0x0080`/`0x0081`, written with function code `0x10`; default address 2, default baud 9600.
    SoilMultiParameter,
}

impl SensorType {
    /// The wire profile (register addresses + limits) for this sensor model.
    pub const fn profile(self) -> SensorProfile {
        match self {
            SensorType::Sn300AlGhN01 => SensorProfile {
                name: "SN-300AL-GH-N01",
                address_register: 0x07D0,
                baud_register: 0x07D1,
                max_slave_address: 254,
                requires_power_cycle: false,
                write_function16: false,
                baud_encoding: BaudEncoding::CodeIndex,
            },
            SensorType::LightIntensityRs485 => SensorProfile {
                name: "LightIntensity-RS485",
                address_register: 0x0100,
                baud_register: 0x0101,
                max_slave_address: 251,
                requires_power_cycle: true,
                write_function16: false,
                baud_encoding: BaudEncoding::CodeIndex,
            },
            SensorType::SoilMultiParameter => SensorProfile {
                name: "Soil-Multi-Parameter",
                address_register: 0x0080,
                baud_register: 0x0081,
                max_slave_address: 247,
                requires_power_cycle: false,
                write_function16: true,
                baud_encoding: BaudEncoding::BaudDiv10HexDigits,
            },
        }
    }
}

/// Wire profile describing where a sensor stores its address and baud rate.
#[derive(Clone, Copy, Debug)]
pub struct SensorProfile {
    /// Human-readable model name for logs.
    pub name: &'static str,
    /// Holding register holding the Modbus slave address.
    pub address_register: u16,
    /// Holding register holding the baud-rate code (encoding per [`Self::baud_encoding`]).
    pub baud_register: u16,
    /// Maximum legal slave address for this model (minimum is always 1).
    pub max_slave_address: u8,
    /// Whether the sensor must be power-cycled for the new settings to take effect.
    pub requires_power_cycle: bool,
    /// Whether configuration writes must use function code `0x10` (write multiple registers,
    /// count = 1) instead of `0x06` (write single register).
    pub write_function16: bool,
    /// How the baud rate is encoded in the baud-rate register.
    pub baud_encoding: BaudEncoding,
}

/// How a sensor's baud-rate register encodes the baud rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaudEncoding {
    /// Small integer index: `0 = 2400`, `1 = 4800`, `2 = 9600` (SN-300 & light-intensity families).
    CodeIndex,
    /// Baud rate ÷ 10 with the decimal digits packed as hex nibbles — the soil multi-parameter
    /// family's encoding, per the manual's example where 19200 baud is written as data
    /// `0x19 0x20` (i.e. "1920"). `1200 → 0x0120`, `9600 → 0x0960`, `38400 → 0x3840`.
    BaudDiv10HexDigits,
}

/// Baud rates supported by the SN-300 and light-intensity families (code-index encoding).
pub const CODE_INDEX_BAUD_RATES: &[u32] = &[2400, 4800, 9600];

/// Baud rates supported by the soil multi-parameter family (manual: 1200..38400, max 38400).
pub const SOIL_BAUD_RATES: &[u32] = &[1200, 2400, 4800, 9600, 19200, 38400];

/// Encode a baud rate (bits/s) into the register value for the given encoding.
///
/// Returns `None` for rates the encoding does not support.
pub const fn encode_baud_rate(baud_rate: u32, encoding: BaudEncoding) -> Option<u16> {
    match encoding {
        BaudEncoding::CodeIndex => match baud_rate {
            2400 => Some(0),
            4800 => Some(1),
            9600 => Some(2),
            _ => None,
        },
        BaudEncoding::BaudDiv10HexDigits => {
            let v = baud_rate / 10;
            // Pack the decimal digits of (baud / 10) into hex nibbles: 1920 -> 0x1920.
            // Only 3- or 4-digit values are representable (1200..=99990 baud ÷ 10).
            if v < 100 || v > 9999 {
                None
            } else {
                let packed = (((v / 1000) % 10) << 12)
                    | (((v / 100) % 10) << 8)
                    | (((v / 10) % 10) << 4)
                    | (v % 10);
                Some(packed as u16)
            }
        }
    }
}

/// Decode a register baud-rate value back to bits/s for the given encoding.
pub const fn decode_baud_rate(code: u16, encoding: BaudEncoding) -> Option<u32> {
    match encoding {
        BaudEncoding::CodeIndex => match code {
            0 => Some(2400),
            1 => Some(4800),
            2 => Some(9600),
            _ => None,
        },
        BaudEncoding::BaudDiv10HexDigits => {
            // Unpack hex nibbles as decimal digits and multiply back by 10.
            let digits = ((code >> 12) as u32) * 1000
                + (((code >> 8) & 0xF) as u32) * 100
                + (((code >> 4) & 0xF) as u32) * 10
                + ((code & 0xF) as u32);
            Some(digits * 10)
        }
    }
}
