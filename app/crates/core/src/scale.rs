//! Page scale: real-world inches per PDF point, presets and calculate-from-known-length.
//!
//! A PDF point is 1/72 inch of paper. Scale is stored as real inches per point at full
//! precision (OST-01-041). Native units are inches; metric scales are plain ratios, so they
//! work in any length unit.

use std::fmt;

/// PDF points per inch of paper.
pub const POINTS_PER_INCH: f64 = 72.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleStyle {
    Architectural,
    Civil,
    Metric,
    Custom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scale {
    inches_per_point: f64,
    pub style: ScaleStyle,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScaleError {
    /// A drawn distance, real length or ratio was zero, negative or not finite.
    NotPositive,
}

impl fmt::Display for ScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value must be greater than zero")
    }
}

impl std::error::Error for ScaleError {}

fn positive(v: f64) -> Result<f64, ScaleError> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(ScaleError::NotPositive)
    }
}

// unknown: OST-06-012 says "predefined list" without naming entries; these are the common
// sheet scales and can be extended without changing the math.
/// Architectural presets as (paper inches, real feet, label).
pub const ARCHITECTURAL_PRESETS: &[(f64, f64, &str)] = &[
    (3.0, 1.0, "3\" = 1'-0\""),
    (1.5, 1.0, "1 1/2\" = 1'-0\""),
    (1.0, 1.0, "1\" = 1'-0\""),
    (0.75, 1.0, "3/4\" = 1'-0\""),
    (0.5, 1.0, "1/2\" = 1'-0\""),
    (0.375, 1.0, "3/8\" = 1'-0\""),
    (0.25, 1.0, "1/4\" = 1'-0\""),
    (0.1875, 1.0, "3/16\" = 1'-0\""),
    (0.125, 1.0, "1/8\" = 1'-0\""),
    (0.09375, 1.0, "3/32\" = 1'-0\""),
    (0.0625, 1.0, "1/16\" = 1'-0\""),
];

/// Civil presets: feet per paper inch.
pub const CIVIL_FEET_PER_INCH: &[f64] = &[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 100.0];

/// Metric presets: ratio 1:N.
pub const METRIC_RATIOS: &[f64] = &[
    1.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 500.0, 1000.0,
];

impl Scale {
    /// Real inches represented by one PDF point.
    pub fn inches_per_point(&self) -> f64 {
        self.inches_per_point
    }

    /// Architectural: `paper_inches` on the sheet equal `real_feet` in the building,
    /// for example 0.25 and 1.0 for 1/4" = 1'-0".
    pub fn architectural(
        paper_inches: f64,
        real_feet: f64,
        label: &str,
    ) -> Result<Scale, ScaleError> {
        let (p, r) = (positive(paper_inches)?, positive(real_feet)?);
        Ok(Scale {
            inches_per_point: r * 12.0 / p / POINTS_PER_INCH,
            style: ScaleStyle::Architectural,
            label: label.to_string(),
        })
    }

    /// Civil: 1 inch on the sheet equals `feet_per_inch` feet, for example 20 for 1" = 20'.
    pub fn civil(feet_per_inch: f64) -> Result<Scale, ScaleError> {
        let f = positive(feet_per_inch)?;
        Ok(Scale {
            inches_per_point: f * 12.0 / POINTS_PER_INCH,
            style: ScaleStyle::Civil,
            label: format!("1\" = {f}'"),
        })
    }

    /// Metric ratio 1:`ratio`, for example 50 for 1:50.
    pub fn metric(ratio: f64) -> Result<Scale, ScaleError> {
        let r = positive(ratio)?;
        Ok(Scale {
            inches_per_point: r / POINTS_PER_INCH,
            style: ScaleStyle::Metric,
            label: format!("1:{r}"),
        })
    }

    /// Calculate from a known length (OST-06-015 to OST-06-018): two measured page points
    /// and the real length typed by the user, in inches. The exact typed value is used;
    /// no rounding.
    pub fn from_known_length(
        a: (f64, f64),
        b: (f64, f64),
        real_length_inches: f64,
        style: ScaleStyle,
    ) -> Result<Scale, ScaleError> {
        let real = positive(real_length_inches)?;
        let drawn = positive((b.0 - a.0).hypot(b.1 - a.1))?;
        Ok(Scale {
            inches_per_point: real / drawn,
            style,
            label: "Custom".to_string(),
        })
    }

    /// Converts a page distance in points to real inches.
    pub fn to_real_inches(&self, page_points: f64) -> f64 {
        page_points * self.inches_per_point
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // OST-06-002: scale is drawn distance to real distance, e.g. 3/8" on page = 1' real.
    #[test]
    fn three_eighths_inch_is_one_foot() {
        let s = Scale::architectural(0.375, 1.0, "3/8\" = 1'-0\"").unwrap();
        assert!(close(s.to_real_inches(0.375 * 72.0), 12.0));
    }

    // OST-19-011: default 1/8" = 1'-0": one paper inch is 96 real inches.
    #[test]
    fn eighth_inch_default() {
        let s = Scale::architectural(0.125, 1.0, "1/8\" = 1'-0\"").unwrap();
        assert!(close(s.to_real_inches(72.0), 96.0));
    }

    // OST-06-012: civil style, 1" = 20' means one paper inch is 240 real inches.
    #[test]
    fn civil_one_inch_twenty_feet() {
        let s = Scale::civil(20.0).unwrap();
        assert_eq!(s.style, ScaleStyle::Civil);
        assert!(close(s.to_real_inches(72.0), 240.0));
    }

    // OST-06-012: metric style; ratio 1:50 makes 72 pt (1 paper inch) equal 50 inches.
    #[test]
    fn metric_ratio() {
        let s = Scale::metric(50.0).unwrap();
        assert!(close(s.to_real_inches(72.0), 50.0));
    }

    // OST-06-015 to OST-06-017: scale = typed real length / drawn length. A 3-4-5 drag of
    // 360 points (216, 288) representing 30'-0" is 1 real inch per point.
    #[test]
    fn calculate_from_known_length() {
        let s = Scale::from_known_length((0.0, 0.0), (216.0, 288.0), 360.0, ScaleStyle::Custom)
            .unwrap();
        assert!(close(s.inches_per_point(), 1.0));
    }

    // OST-06-016: the label rounds up but the math uses the exact typed value.
    #[test]
    fn exact_typed_value_is_used() {
        let s =
            Scale::from_known_length((0.0, 0.0), (100.0, 0.0), 120.5, ScaleStyle::Custom).unwrap();
        assert!(close(s.to_real_inches(100.0), 120.5));
    }

    // OST-01-041: full precision, no rounding.
    #[test]
    fn full_precision_kept() {
        let s = Scale::from_known_length((0.0, 0.0), (7.0, 0.0), 10.0, ScaleStyle::Custom).unwrap();
        assert_eq!(s.inches_per_point(), 10.0 / 7.0);
    }

    // unknown: error behavior is not in the ledger; zero or negative input is rejected.
    #[test]
    fn rejects_zero_and_negative() {
        assert!(Scale::civil(0.0).is_err());
        assert!(Scale::metric(-1.0).is_err());
        assert!(
            Scale::from_known_length((1.0, 1.0), (1.0, 1.0), 10.0, ScaleStyle::Custom).is_err()
        );
        assert!(
            Scale::from_known_length((0.0, 0.0), (1.0, 0.0), f64::NAN, ScaleStyle::Custom).is_err()
        );
    }

    #[test]
    fn presets_all_build() {
        for (p, r, l) in ARCHITECTURAL_PRESETS {
            assert!(Scale::architectural(*p, *r, l).is_ok());
        }
        for f in CIVIL_FEET_PER_INCH {
            assert!(Scale::civil(*f).is_ok());
        }
        for m in METRIC_RATIOS {
            assert!(Scale::metric(*m).is_ok());
        }
    }
}
