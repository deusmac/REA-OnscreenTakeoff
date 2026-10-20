//! Linear-condition results (OST-05-015) with slope (OST-05-019).
//!
//! Inputs and outputs are inches, square inches and cubic inches at full precision
//! (OST-01-041); display rounding belongs to the caller. OST applies no waste (OST-05-018).

use crate::scale::Scale;

/// A point (x, y).
pub type Point = (f64, f64);

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

/// An opening (door, window) attached to a linear run, in inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opening {
    pub width: f64,
    pub height: f64,
}

pub struct LinearInput<'a> {
    /// Drawn polyline vertices in page points. N points make N-1 segments.
    pub points: &'a [(f64, f64)],
    /// Arc height (sagitta) per segment in page points; 0 or missing means straight.
    pub sagittas: &'a [f64],
    pub scale: &'a Scale,
    /// Height in inches.
    pub height: f64,
    /// Thickness in inches.
    pub thickness: f64,
    pub slope: Option<Slope>,
    /// Drop/run lengths added to the run, in real inches (OST-05-016).
    pub drops: f64,
    /// Attached openings whose condition backs out of the wall.
    pub openings: &'a [Opening],
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

/// Length of a circular arc from its chord and sagitta (arc height). Zero sagitta is the
/// chord. A sagitta above half the chord is the major arc. Sign only says which side.
// Evidence: OST stores a curved segment as its chord plus a signed arc height; this arc
// length reproduces OST's stored length totals (docs/study/formats/ost-quantities.md).
pub fn arc_length(chord: f64, sagitta: f64) -> f64 {
    let h = sagitta.abs();
    if h == 0.0 || chord == 0.0 {
        return chord;
    }
    let r = (chord * chord / 4.0 + h * h) / (2.0 * h);
    let half = (chord / 2.0 / r).min(1.0).asin();
    let theta = if h > chord / 2.0 {
        2.0 * std::f64::consts::PI - 2.0 * half
    } else {
        2.0 * half
    };
    r * theta
}

/// One OST linear takeoff position (the numbers of `BidTakeoffs.Position`) as start, end
/// and signed sagitta. Two layouts exist: `x1;y1;x2;y2;s` and `x1;y1;x2;y2;px;py;s`. In the
/// seven-number form the fifth and sixth numbers are a point (a rotation pivot on rotated
/// takeoffs, a point near the arc on curved ones), never the sagitta.
// Evidence: 2635 rotated straight segments in JC's databases carry a pivot far off the
// segment in numbers five and six; reading it as a sagitta turned 8 segments of 16 in
// (OST total 128 in) into 35290 in (docs/study/formats/ost-quantities.md).
pub fn ost_segment(v: &[f64]) -> Option<(Point, Point, f64)> {
    if v.len() < 4 {
        return None;
    }
    let sag = match v.len() {
        n if n >= 7 => v[6],
        5 | 6 => v[4],
        _ => 0.0,
    };
    Some(((v[0], v[1]), (v[2], v[3]), sag))
}

/// True when every point lies inside a closed ring (even-odd rule). OST credits a takeoff
/// with no zone of its own to the zone whose outline holds it; any condition type.
// Evidence: 24 zone rows in JC's databases (linear and area) equal the takeoffs inside the
// zone outline on that page, and the page row without a zone then stores 0
// (docs/study/formats/ost-quantities.md).
// shortcut: a takeoff that crosses the outline is treated as outside; no such case in the
// data, revisit if OST is seen splitting a takeoff at a zone edge.
pub fn points_inside(points: &[Point], ring: &[Point]) -> bool {
    let inside = |&(x, y): &Point| {
        let mut c = false;
        for (i, &(x1, y1)) in ring.iter().enumerate() {
            let (x2, y2) = ring[(i + 1) % ring.len()];
            if (y1 > y) != (y2 > y) && x < (x2 - x1) * (y - y1) / (y2 - y1) + x1 {
                c = !c;
            }
        }
        c
    };
    !points.is_empty() && points.iter().all(inside)
}

/// Total drawn length in page points with optional per-segment arc heights.
fn drawn_points(points: &[(f64, f64)], sagittas: &[f64]) -> f64 {
    points
        .windows(2)
        .enumerate()
        .map(|(i, w)| {
            let chord = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
            arc_length(chord, sagittas.get(i).copied().unwrap_or(0.0))
        })
        .sum()
}

pub fn linear_results(input: &LinearInput) -> Result<LinearResults, LinearError> {
    let factor = match input.slope {
        Some(s) => s.factor().ok_or(LinearError::InvalidSlope)?,
        None => 1.0,
    };
    // unknown: whether slope also scales drops; drops are vertical, so they are added after.
    let l = input
        .scale
        .to_real_inches(drawn_points(input.points, input.sagittas))
        * factor
        + input.drops;
    let (h, t) = (input.height, input.thickness);
    // Openings (OST evidence, docs/study/formats/ost-quantities.md): a zero-height opening
    // removes nothing; each other opening removes W * min(Hopening, H) per wall side, and an
    // opening at least as tall as the wall also removes its width from the length.
    let (mut cut_area, mut cut_len) = (0.0, 0.0);
    for o in input.openings.iter().filter(|o| o.height != 0.0) {
        cut_area += o.width * o.height.min(h);
        if o.height >= h {
            cut_len += o.width;
        }
    }
    // unknown: U-03 how drops affect area and volume; here they are part of L everywhere.
    // unknown: whether openings reduce results other than length and side areas (no data).
    // End areas are per segment: OST stored code 8 (both ends) as 2 * H * T * 2 for a run of
    // two segments (sample database). One end (code 7) has no data; assumed H * T per segment.
    // Callers pass one segment per call, so H * T here is already per segment.
    Ok(LinearResults {
        length: l - cut_len,
        segment_count: input.points.len().saturating_sub(1),
        area_one_side: h * l - cut_area,
        area_both_sides: (h * l - cut_area) * 2.0,
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
    use crate::scale::ScaleStyle;

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
            sagittas: &[],
            scale: &s,
            height: h,
            thickness: t,
            slope,
            drops: 0.0,
            openings: &[],
        })
        .unwrap()
    }

    fn unit() -> Scale {
        Scale::from_known_length((0.0, 0.0), (1.0, 0.0), 1.0, ScaleStyle::Custom).unwrap()
    }

    // OST answer key: half circle, chord 20, sagitta 10 is pi * 10; minor and major arcs.
    #[test]
    fn arc_length_from_sagitta() {
        assert!((arc_length(20.0, 10.0) - std::f64::consts::PI * 10.0).abs() < 1e-9);
        assert!((arc_length(20.0, -10.0) - std::f64::consts::PI * 10.0).abs() < 1e-9);
        assert_eq!(arc_length(20.0, 0.0), 20.0);
        assert!(arc_length(20.0, 15.0) > std::f64::consts::PI * 10.0);
    }

    // OST answer key (JC's databases): a rotated straight segment stores a pivot in numbers
    // five and six and its sagitta last; a five-number position keeps the sagitta fifth.
    #[test]
    fn ost_segment_layouts() {
        let rotated = [943.486, 774.299, 943.486, 790.299, 1875.531, 1564.593, 0.0];
        let (a, b, s) = ost_segment(&rotated).unwrap();
        assert_eq!((a, b, s), ((943.486, 774.299), (943.486, 790.299), 0.0));
        let curved = [
            2365.864, 1656.908, 2331.301, 1618.057, 2357.033, 1633.356, -9.056,
        ];
        assert_eq!(ost_segment(&curved).unwrap().2, -9.056);
        assert_eq!(ost_segment(&[0.0, 0.0, 20.0, 0.0, 10.0]).unwrap().2, 10.0);
        assert_eq!(ost_segment(&[0.0, 0.0, 20.0, 0.0]).unwrap().2, 0.0);
        assert!(ost_segment(&[1.0, 2.0]).is_none());
    }

    // OST answer key (JC's databases): zone membership needs every point inside the outline.
    #[test]
    fn points_inside_zone_outline() {
        let ring = [(0.0, 0.0), (100.0, 0.0), (100.0, 50.0), (0.0, 50.0)];
        assert!(points_inside(&[(10.0, 10.0), (90.0, 40.0)], &ring));
        assert!(!points_inside(&[(10.0, 10.0), (150.0, 40.0)], &ring));
        assert!(!points_inside(&[(-5.0, 10.0), (-1.0, 40.0)], &ring));
        assert!(!points_inside(&[], &ring));
    }

    // OST answer key: drops add to length (OST-05-017 resolved: L = segments + drops).
    #[test]
    fn drops_add_to_length() {
        let s = unit();
        let r = linear_results(&LinearInput {
            points: &[(0.0, 0.0), (100.0, 0.0)],
            sagittas: &[],
            scale: &s,
            height: 0.0,
            thickness: 0.0,
            slope: None,
            drops: 60.0,
            openings: &[],
        })
        .unwrap();
        assert_eq!(r.length, 160.0);
    }

    // OST answer key: a 99 in wall, 156 in high, with a 36 x 84 door: one side loses 3024;
    // a full-height opening also cuts its width from the length; zero height cuts nothing.
    #[test]
    fn openings_back_out_of_wall() {
        let s = unit();
        let input = |openings: &[Opening]| {
            linear_results(&LinearInput {
                points: &[(0.0, 0.0), (99.0, 0.0)],
                sagittas: &[],
                scale: &s,
                height: 156.0,
                thickness: 4.0,
                slope: None,
                drops: 0.0,
                openings,
            })
            .unwrap()
        };
        let door = input(&[Opening {
            width: 36.0,
            height: 84.0,
        }]);
        assert_eq!(door.area_one_side, 99.0 * 156.0 - 3024.0);
        assert_eq!(door.area_both_sides, 2.0 * (99.0 * 156.0 - 3024.0));
        assert_eq!(door.length, 99.0);
        let full = input(&[Opening {
            width: 36.0,
            height: 200.0,
        }]);
        assert_eq!(full.length, 63.0);
        assert_eq!(full.area_one_side, 63.0 * 156.0);
        let flat = input(&[Opening {
            width: 36.0,
            height: 0.0,
        }]);
        assert_eq!(flat.length, 99.0);
        assert_eq!(flat.area_one_side, 99.0 * 156.0);
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
            sagittas: &[],
            scale: &s,
            height: 1.0,
            thickness: 1.0,
            slope: Some(Slope {
                rise: 4.0,
                run: 0.0,
            }),
            drops: 0.0,
            openings: &[],
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
