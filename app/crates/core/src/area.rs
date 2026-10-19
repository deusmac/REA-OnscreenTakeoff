//! Area-condition results (OST-05-023).
//!
//! Inputs are rings of page points (PDF points) and a `real_per_point` factor (real inches
//! per PDF point). Areas come out in square inches, lengths in inches, volumes in cubic
//! inches; callers convert. Values are full precision (OST-01-041).
//!
//! Slope applies to area and volume only, never to perimeter (OST-05-022). The factor is
//! sqrt(1 + (rise/run)^2) (OST-05-031).
//!
//! Attachment results (AA, AL, AV) are not modeled yet (open item U-07).

pub type Point = (f64, f64);

/// Absolute area of a ring in square points (shoelace). The ring is implicitly closed.
fn ring_area_pts(ring: &[Point]) -> f64 {
    let n = ring.len();
    let twice: f64 = (0..n)
        .map(|i| {
            let (x1, y1) = ring[i];
            let (x2, y2) = ring[(i + 1) % n];
            x1 * y2 - x2 * y1
        })
        .sum();
    twice.abs() / 2.0
}

/// Closed perimeter of a ring in points.
fn ring_perimeter_pts(ring: &[Point]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (x1, y1) = ring[i];
            let (x2, y2) = ring[(i + 1) % n];
            (x2 - x1).hypot(y2 - y1)
        })
        .sum()
}

/// Slope factor for a rise:run pair (OST-05-031). Zero run is treated as flat.
// unknown: elevation takeoff uses run:rise (OST-05-022); the caller must swap the pair.
pub fn slope_factor(rise: f64, run: f64) -> f64 {
    if run == 0.0 {
        return 1.0;
    }
    (1.0 + (rise / run).powi(2)).sqrt()
}

/// One area object: an outer ring and its backout rings.
pub struct AreaShape<'a> {
    pub outer: &'a [Point],
    pub backouts: &'a [&'a [Point]],
}

/// Area results for one object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaResults {
    /// A - BA (OST-05-023).
    pub area: f64,
    /// A, ignoring backouts.
    pub area_ignore_backouts: f64,
    /// L + BL.
    pub perimeter: f64,
    /// L only.
    pub perimeter_without_backouts: f64,
    /// A * T - BV.
    pub volume: f64,
    /// A * T.
    pub volume_ignore_backouts: f64,
}

/// Computes all area results. `depth` is the condition thickness in inches (0 gives zero
/// volume); `slope` is the factor from [`slope_factor`], use 1.0 for flat.
pub fn area_results(shape: &AreaShape, real_per_point: f64, depth: f64, slope: f64) -> AreaResults {
    let sq = real_per_point * real_per_point * slope;
    let a = ring_area_pts(shape.outer) * sq;
    let ba: f64 = shape.backouts.iter().map(|r| ring_area_pts(r)).sum::<f64>() * sq;
    let l = ring_perimeter_pts(shape.outer) * real_per_point;
    let bl: f64 = shape
        .backouts
        .iter()
        .map(|r| ring_perimeter_pts(r))
        .sum::<f64>()
        * real_per_point;
    AreaResults {
        area: a - ba,
        area_ignore_backouts: a,
        // unknown: sign convention of BL (U-05); plain reading of the description is L + BL.
        perimeter: l + bl,
        perimeter_without_backouts: l,
        // unknown: slope also applies to the backout volume (plain reading).
        volume: (a - ba) * depth,
        volume_ignore_backouts: a * depth,
    }
}

/// Area Counts: number of area objects (OST-05-023).
pub fn area_count(objects: usize) -> usize {
    objects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> Vec<Point> {
        vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]
    }

    // 10 ft x 20 ft room (120 x 240 in) with a 2 ft x 3 ft backout, 1 point = 1 inch.
    fn room() -> AreaResults {
        let outer = rect(120.0, 240.0);
        let bo = rect(24.0, 36.0);
        area_results(
            &AreaShape {
                outer: &outer,
                backouts: &[&bo],
            },
            1.0,
            4.0,
            1.0,
        )
    }

    #[test]
    fn area_minus_and_ignoring_backouts() {
        // OST-05-023: Area = A - BA; Area (Ignore Backout Areas) = A.
        let r = room();
        assert_eq!(r.area_ignore_backouts / 144.0, 200.0);
        assert_eq!(r.area / 144.0, 194.0);
    }

    #[test]
    fn perimeter_variants() {
        // OST-05-023: perimeter includes backout perimeters; the variant omits them.
        let r = room();
        assert_eq!(r.perimeter_without_backouts, 720.0);
        assert_eq!(r.perimeter, 720.0 + 120.0);
    }

    #[test]
    fn volume_with_backout_volume() {
        // OST-05-023: V = A*T - BV, 4 in thick: 194 SF * 4 in.
        let r = room();
        assert_eq!(r.volume, 194.0 * 144.0 * 4.0);
        assert_eq!(r.volume_ignore_backouts, 200.0 * 144.0 * 4.0);
    }

    #[test]
    fn page_scale_applies_to_lengths_and_areas() {
        // OST-01-041 / OST-05-023: 0.5 in per point halves lengths, quarters areas.
        let outer = rect(240.0, 480.0);
        let shape = AreaShape {
            outer: &outer,
            backouts: &[],
        };
        let r = area_results(&shape, 0.5, 0.0, 1.0);
        assert_eq!(r.area, 120.0 * 240.0);
        assert_eq!(r.perimeter, 720.0);
        assert_eq!(r.volume, 0.0);
    }

    #[test]
    fn slope_affects_area_and_volume_only() {
        // OST-05-022 / OST-05-031: 12 in x 12 in at 4:12 gives 144 * 1.0541 sq in.
        let f = slope_factor(4.0, 12.0);
        assert!((f - 1.054_092_553).abs() < 1e-8);
        let outer = rect(12.0, 12.0);
        let shape = AreaShape {
            outer: &outer,
            backouts: &[],
        };
        let r = area_results(&shape, 1.0, 2.0, f);
        assert!((r.area - 144.0 * f).abs() < 1e-9);
        assert!((r.volume - 288.0 * f).abs() < 1e-9);
        assert_eq!(r.perimeter, 48.0);
    }

    #[test]
    fn ring_orientation_does_not_matter() {
        // OST-05-023: clockwise and counter-clockwise rings give the same area.
        let mut cw = rect(10.0, 10.0);
        cw.reverse();
        assert_eq!(ring_area_pts(&cw), 100.0);
    }

    #[test]
    fn area_count_is_object_count() {
        // OST-05-023: Area Counts = number of area objects.
        assert_eq!(area_count(3), 3);
    }
}
