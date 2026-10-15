//! Measurement entry and display.
//!
//! Imperial dimension boxes use packed feet-inch entry: the last two digits are inches and
//! the digits before them are feet (OST-01-038). Inches above 11 carry into feet, so `824`
//! means 8 ft 24 in = 10 ft 0 in. A decimal part is a fraction of an inch (OST-01-040).
//! Metric boxes take millimeters (OST-01-039).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Invalid(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "enter a value"),
            ParseError::Invalid(s) => write!(f, "not a valid measurement: {s}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses packed feet-inch entry and returns total inches.
///
/// Apostrophes, quotes and fractions like 1/2 are rejected for now (open item U-01).
pub fn parse_packed_imperial(input: &str) -> Result<f64, ParseError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(ParseError::Empty);
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s, ""),
    };
    let all_digits = |p: &str| p.chars().all(|c| c.is_ascii_digit());
    if (whole.is_empty() && frac.is_empty()) || !all_digits(whole) || !all_digits(frac) {
        return Err(ParseError::Invalid(s.to_string()));
    }
    let split = whole.len().saturating_sub(2);
    let number = |p: &str| {
        if p.is_empty() {
            0.0
        } else {
            p.parse::<f64>().unwrap_or(0.0)
        }
    };
    let feet = number(&whole[..split]);
    let mut inches = number(&whole[split..]);
    if !frac.is_empty() {
        inches += format!("0.{frac}").parse::<f64>().unwrap_or(0.0);
    }
    Ok(feet * 12.0 + inches)
}

/// Parses a metric entry in millimeters.
pub fn parse_millimeters(input: &str) -> Result<f64, ParseError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(ParseError::Empty);
    }
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| ParseError::Invalid(s.to_string()))
}

/// Formats inches as feet and inches, for example `10' 6"`. Inches round to `decimals`.
pub fn format_feet_inches(total_inches: f64, decimals: usize) -> String {
    let scale = 10f64.powi(decimals as i32);
    let rounded = (total_inches * scale).round() / scale;
    let feet = (rounded / 12.0).floor();
    let inches = rounded - feet * 12.0;
    format!("{feet:.0}' {inches:.decimals$}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_entry_matches_ledger_examples() {
        // OST-01-038 and QB-01-038 examples.
        assert_eq!(parse_packed_imperial("8"), Ok(8.0));
        assert_eq!(parse_packed_imperial("10"), Ok(10.0));
        assert_eq!(parse_packed_imperial("800"), Ok(96.0));
        assert_eq!(parse_packed_imperial("824"), Ok(120.0));
        assert_eq!(parse_packed_imperial("1000"), Ok(120.0));
        assert_eq!(parse_packed_imperial("1010"), Ok(130.0));
        assert_eq!(parse_packed_imperial("6808"), Ok(824.0));
        assert_eq!(parse_packed_imperial("1308"), Ok(164.0));
    }

    #[test]
    fn packed_entry_scale_examples() {
        // OST-06-013: 2500 = 25'0", 10006 = 100'6".
        assert_eq!(parse_packed_imperial("2500"), Ok(300.0));
        assert_eq!(parse_packed_imperial("10006"), Ok(1206.0));
    }

    #[test]
    fn packed_entry_fraction_of_inch() {
        assert_eq!(parse_packed_imperial("806.5"), Ok(102.5));
        assert_eq!(parse_packed_imperial(".25"), Ok(0.25));
    }

    #[test]
    fn packed_entry_rejects_unsupported_input() {
        assert_eq!(parse_packed_imperial(""), Err(ParseError::Empty));
        assert!(parse_packed_imperial("8'6\"").is_err());
        assert!(parse_packed_imperial("1/2").is_err());
        assert!(parse_packed_imperial(".").is_err());
    }

    #[test]
    fn millimeters() {
        assert_eq!(parse_millimeters("1200"), Ok(1200.0));
        assert!(parse_millimeters("-5").is_err());
    }

    #[test]
    fn feet_inches_display() {
        assert_eq!(format_feet_inches(126.0, 0), "10' 6\"");
        assert_eq!(format_feet_inches(102.5, 1), "8' 6.5\"");
    }
}
