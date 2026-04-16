//! Decimal-aware token amount type.
//!
//! [`Amount`] wraps a raw `u128` integer value (the token's smallest unit) and
//! carries a reference to the token it belongs to, enabling correct decimal
//! formatting and arithmetic without floating-point rounding errors.
//!
//! # Examples
//!
//! ```rust
//! use starkzap_rs::{Amount, tokens::mainnet};
//!
//! let usdc = mainnet::usdc();
//! let amount = Amount::parse("10.5", &usdc)?;
//! assert_eq!(amount.raw(), 10_500_000); // 6 decimals
//! println!("{}", amount.to_formatted()); // "10.5 USDC"
//! # Ok::<(), starkzap_rs::StarkzapError>(())
//! ```

use std::fmt;
use starknet::core::types::Felt;

use crate::{error::{Result, StarkzapError}, tokens::Token};

/// A token amount held as a raw integer in the token's smallest unit.
///
/// Internally this is a `u128`, which safely covers the full u256 `low` limb
/// used by Starknet's ERC-20 `Uint256` type. Values exceeding `u128::MAX` are
/// rejected at parse time with [`StarkzapError::AmountOverflow`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount {
    /// Raw integer in the token's smallest unit (e.g., wei for 18-decimal tokens).
    raw: u128,
    /// Token decimals — stored so formatting is self-contained.
    decimals: u8,
    /// Human-readable symbol for display.
    symbol: String,
}

impl Amount {
    /// Parse a human-readable decimal string into an [`Amount`].
    ///
    /// # Arguments
    ///
    /// * `value` — decimal string like `"10"`, `"0.5"`, `"1.000001"`
    /// * `token` — the [`Token`] this amount belongs to
    ///
    /// # Errors
    ///
    /// Returns [`StarkzapError::AmountParse`] if `value` is not a valid decimal.
    /// Returns [`StarkzapError::AmountOverflow`] if the scaled value exceeds `u128::MAX`.
    pub fn parse(value: &str, token: &Token) -> Result<Self> {
        let raw = parse_decimal(value, token.decimals)?;
        Ok(Self {
            raw,
            decimals: token.decimals,
            symbol: token.symbol.clone(),
        })
    }

    /// Construct an [`Amount`] directly from a raw integer value.
    pub fn from_raw(raw: u128, token: &Token) -> Self {
        Self {
            raw,
            decimals: token.decimals,
            symbol: token.symbol.clone(),
        }
    }

    /// The raw integer value in the token's smallest unit.
    pub fn raw(&self) -> u128 {
        self.raw
    }

    /// Format as a human-readable string with symbol, e.g. `"10.5 USDC"`.
    pub fn to_formatted(&self) -> String {
        format!("{} {}", self.to_decimal_string(), self.symbol)
    }

    /// Format as a decimal string without the symbol, e.g. `"10.5"`.
    pub fn to_decimal_string(&self) -> String {
        format_decimal(self.raw, self.decimals)
    }

    /// Convert to the two-felt Starknet `Uint256` representation `[low, high]`.
    ///
    /// Since [`Amount`] is bounded to `u128`, `high` is always `0`.
    pub fn to_u256_felts(&self) -> [Felt; 2] {
        [Felt::from(self.raw), Felt::ZERO]
    }

    /// Checked addition. Returns `None` on overflow.
    pub fn checked_add(&self, other: &Amount) -> Option<Amount> {
        let raw = self.raw.checked_add(other.raw)?;
        Some(Amount {
            raw,
            decimals: self.decimals,
            symbol: self.symbol.clone(),
        })
    }

    /// Returns `true` if the amount is zero.
    pub fn is_zero(&self) -> bool {
        self.raw == 0
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_formatted())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a decimal string into a scaled `u128` integer.
fn parse_decimal(value: &str, decimals: u8) -> Result<u128> {
    let value = value.trim();

    if value.is_empty() {
        return Err(StarkzapError::AmountParse { input: value.to_string() });
    }

    let (integer_part, fractional_part) = match value.split_once('.') {
        Some((int, frac)) => (int, frac),
        None => (value, ""),
    };

    // Validate characters
    if !integer_part.chars().all(|c| c.is_ascii_digit()) ||
       !fractional_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(StarkzapError::AmountParse { input: value.to_string() });
    }

    let dec = decimals as usize;

    // Pad or truncate fractional part to exactly `decimals` digits
    let frac_padded = if fractional_part.len() > dec {
        // Truncate (no rounding — be explicit)
        fractional_part[..dec].to_string()
    } else {
        format!("{:0<width$}", fractional_part, width = dec)
    };

    let int_val: u128 = if integer_part.is_empty() {
        0
    } else {
        integer_part.parse::<u128>().map_err(|_| StarkzapError::AmountParse {
            input: value.to_string(),
        })?
    };

    let scale = 10u128
        .checked_pow(dec as u32)
        .ok_or(StarkzapError::AmountOverflow)?;

    let frac_val: u128 = if frac_padded.is_empty() {
        0
    } else {
        frac_padded.parse::<u128>().map_err(|_| StarkzapError::AmountParse {
            input: value.to_string(),
        })?
    };

    int_val
        .checked_mul(scale)
        .and_then(|v| v.checked_add(frac_val))
        .ok_or(StarkzapError::AmountOverflow)
}

/// Format a raw integer as a decimal string with the given precision.
fn format_decimal(raw: u128, decimals: u8) -> String {
    if decimals == 0 {
        return raw.to_string();
    }

    let scale = 10u128.pow(decimals as u32);
    let integer = raw / scale;
    let fraction = raw % scale;

    if fraction == 0 {
        integer.to_string()
    } else {
        // Pad fraction to full width then trim trailing zeros
        let frac_str = format!("{:0>width$}", fraction, width = decimals as usize);
        let frac_trimmed = frac_str.trim_end_matches('0');
        format!("{}.{}", integer, frac_trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Token;

    fn usdc() -> Token {
        Token {
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            address: Felt::ZERO,
        }
    }

    fn strk() -> Token {
        Token {
            symbol: "STRK".to_string(),
            name: "Starknet Token".to_string(),
            decimals: 18,
            address: Felt::ZERO,
        }
    }

    #[test]
    fn parse_whole_number() {
        let a = Amount::parse("10", &usdc()).unwrap();
        assert_eq!(a.raw(), 10_000_000);
    }

    #[test]
    fn parse_decimal() {
        let a = Amount::parse("10.5", &usdc()).unwrap();
        assert_eq!(a.raw(), 10_500_000);
    }

    #[test]
    fn parse_max_precision() {
        let a = Amount::parse("0.000001", &usdc()).unwrap();
        assert_eq!(a.raw(), 1);
    }

    #[test]
    fn parse_strk_18_decimals() {
        let a = Amount::parse("1.5", &strk()).unwrap();
        assert_eq!(a.raw(), 1_500_000_000_000_000_000u128);
    }

    #[test]
    fn format_round_trip() {
        let a = Amount::parse("10.5", &usdc()).unwrap();
        assert_eq!(a.to_decimal_string(), "10.5");
        assert_eq!(a.to_formatted(), "10.5 USDC");
    }

    #[test]
    fn format_no_trailing_zeros() {
        let a = Amount::parse("1.10", &usdc()).unwrap();
        assert_eq!(a.to_decimal_string(), "1.1");
    }

    #[test]
    fn to_u256_felts() {
        let a = Amount::parse("1", &usdc()).unwrap();
        let [low, high] = a.to_u256_felts();
        assert_eq!(low, Felt::from(1_000_000u128));
        assert_eq!(high, Felt::ZERO);
    }

    #[test]
    fn invalid_input_errors() {
        assert!(Amount::parse("abc", &usdc()).is_err());
        assert!(Amount::parse("", &usdc()).is_err());
        assert!(Amount::parse("1.2.3", &usdc()).is_err());
    }
}