//! Linear-condition results (OST-05-015) with slope (OST-05-019).
//!
//! Inputs and outputs are inches, square inches and cubic inches at full precision
//! (OST-01-041); display rounding belongs to the caller. OST applies no waste (OST-05-018).

use crate::scale::Scale;

/// Rise over run, for example 4 and 12 for a 4:12 slope.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slope {
    pub rise: f64,
    pub run: f64,
}

impl Slope {
    /// Multiplier from plan length to true length: sqrt(1 + (rise/run)^2) (OST-05-019).
    /// None when run is zero or the values are not finite.
    pub fn factor(&self) -> Option<f64> {
        if !self.rise.is_finite() || !self.run.is_finite() || self.run == 0.0 {
            return None;
        }
        Some((1.0 + (self.rise / self.run).powi(2)).sqrt())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinearError {
    InvalidSlope,
}

pub struct LinearInput<'a> {
    /// Drawn polyline vertices in page points. N points make N-1 segments.
    pub points: &'a [(f64, f64)],
    pub scale: &'a Scale,
    /// Height in inches.
    pub height: f64,
    /// Thickness in inches.
    pub thickness: f64,
    pub slope: Option<Slope>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearResults {
    pub length: f64,
    pub segment_count: usize,
    pub area_one_side: f64,
    pub area_both_sides: f64,
    pub area_top_or_bottom: f64,
    pub area_top_and_bottom: f64,
    pub area_one_end: f64,
    pub area_both_ends: f64,
    pub area_all_sides: f64,
    pub volume: f64,
}

/// Total drawn length in page points.
pub fn polyline_points(points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum()
}

pub fn linear_results(input: &LinearInput) -> Result<LinearResults, LinearError> {
    let factor = match input.slope {
        Some(s) => s.factor().ok_or(LinearError::InvalidSlope)?,
        None => 1.0,
    };
    let l = input.scale.to_real_inches(polyline_points(input.points)) * factor;
    let (h, t) = (input.height, input.thickness);
    // unknown: U-03 how drop/run affects area and volume; not modeled. Here sloped length
    // feeds the area and volume formulas (the plain reading of L in OST-05-015).
    // unknown: U-04 end areas are a single H*T, not multiplied by segment count.
    Ok(LinearResults {
        length: l,
        segment_count: input.points.len().saturating_sub(1),
        area_one_side: h * l,
        area_both_sides: h * l * 2.0,
        area_top_or_bottom: t * l,
        area_top_and_bottom: t * l * 2.0,
        area_one_end: h * t,
        area_both_ends: h * t * 2.0,
        area_all_sides: t * l * 2.0 + h * l * 2.0,
        volume: h * t * l,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // 1/8" = 1'-0" scale: 1 pt = 96/72 in, so 108 pt = 144 in = 12 ft.
    fn scale() -> Scale {
        Scale::architectural(0.125, 1.0, "1/8\" = 1'-0\"").unwrap()
    }

    fn run(points: &[(f64, f64)], h: f64, t: f64, slope: Option<Slope>) -> LinearResults {
        let s = scale();
        linear_results(&LinearInput {
            points,
            scale: &s,
            height: h,
            thickness: t,
            slope,
        })
        .unwrap()
    }

    // OST-05-015: Length = L.
    #[test]
    fn length_is_drawn_length() {
        let r = run(&[(0.0, 0.0), (108.0, 0.0)], 0.0, 0.0, None);
        assert!(close(r.length, 144.0));
    }

    // OST-05-015: a square has 4 segments (closed polyline repeats the first point).
    #[test]
    fn square_has_four_segments() {
        let p = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ];
        assert_eq!(run(&p, 0.0, 0.0, None).segment_count, 4);
    }

    // unknown: fewer than two points is zero segments and zero length.
    #[test]
    fn single_point_is_empty() {
        let r = run(&[(5.0, 5.0)], 0.0, 0.0, None);
        assert_eq!(r.segment_count, 0);
        assert_eq!(r.length, 0.0);
    }

    // OST-05-015: surface areas and volume, L = 144 in, H = 96 in, T = 6 in.
    #[test]
    fn areas_and_volume() {
        let r = run(&[(0.0, 0.0), (108.0, 0.0)], 96.0, 6.0, None);
        assert!(close(r.area_one_side, 96.0 * 144.0)); // H*L
        assert!(close(r.area_both_sides, 2.0 * 96.0 * 144.0)); // H*L*2
        assert!(close(r.area_top_or_bottom, 6.0 * 144.0)); // T*L
        assert!(close(r.area_top_and_bottom, 2.0 * 6.0 * 144.0)); // T*L*2
        assert!(close(r.area_one_end, 96.0 * 6.0)); // H*T
        assert!(close(r.area_both_ends, 2.0 * 96.0 * 6.0)); // H*T*2
        assert!(close(
            r.area_all_sides,
            2.0 * 6.0 * 144.0 + 2.0 * 96.0 * 144.0
        )); // duct
        assert!(close(r.volume, 96.0 * 6.0 * 144.0)); // H*T*L
    }

    // OST-05-019: 12 ft drawn at 4:12 slope is 12.649 LF, factor 1.05409.
    #[test]
    fn slope_worked_example() {
        let slope = Slope {
            rise: 4.0,
            run: 12.0,
        };
        assert!((slope.factor().unwrap() - 1.05409).abs() < 1e-5);
        let r = run(&[(0.0, 0.0), (108.0, 0.0)], 0.0, 0.0, Some(slope));
        assert!((r.length / 12.0 - 12.649).abs() < 5e-4);
    }

    // OST-05-019: zero run has no defined slope factor.
    #[test]
    fn zero_run_rejected() {
        let s = scale();
        let input = LinearInput {
            points: &[(0.0, 0.0), (1.0, 0.0)],
            scale: &s,
            height: 1.0,
            thickness: 1.0,
            slope: Some(Slope {
                rise: 4.0,
                run: 0.0,
            }),
        };
        assert_eq!(linear_results(&input), Err(LinearError::InvalidSlope));
    }

    // OST-01-041: full precision, no rounding in the math.
    #[test]
    fn no_rounding() {
        let r = run(&[(0.0, 0.0), (1.0, 0.0)], 0.0, 0.0, None);
        assert_eq!(r.length, 96.0 / 72.0);
    }
}
