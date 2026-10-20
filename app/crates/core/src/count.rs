//! Count-condition results (OST-05-035).
//!
//! C = count, H, W, D = height, width, depth in inches. Depth equals width for every area
//! shape except the rectangle (OST-05-033). Results are full precision (OST-01-041).
//!
//! Circle volume is a true cylinder, C * H * pi * W^2 / 4: OST's stored totals show it
//! (docs/study/formats/ost-quantities.md), so U-06 is closed for volume.
//! unknown: the other circle results and triangle, hexagon, octagon geometry keep the
//! bounding-box formulas of the spec (U-06) until data shows otherwise. Which results each
//! shape offers (OST-05-036) is a UI concern and is not enforced here.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountShape {
    Square,
    Circle,
    Triangle,
    Diamond,
    Ellipse,
    Hexagon,
    Octagon,
    Rectangle,
    /// OST shape id 12. Identity unknown (U-06); only its perimeter is evidenced: 4 * W / sqrt(3).
    Shape12,
}

/// Dimensions of one counted object, in inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CountDims {
    pub height: f64,
    pub width: f64,
    pub depth: f64,
    pub round: bool,
    /// Perimeter of one object; 2 * (W + D) unless the shape says otherwise.
    pub perimeter_each: f64,
}

impl CountDims {
    /// Builds dimensions; `depth` is honored only for the rectangle, else D = W (OST-05-033).
    pub fn new(shape: CountShape, height: f64, width: f64, depth: f64) -> Self {
        let depth = if shape == CountShape::Rectangle {
            depth
        } else {
            width
        };
        Self {
            height,
            width,
            depth,
            round: shape == CountShape::Circle,
            perimeter_each: if shape == CountShape::Shape12 {
                4.0 * width / 3f64.sqrt()
            } else {
                2.0 * (width + depth)
            },
        }
    }
}

/// All count results for a number of objects of the given dimensions (OST-05-035).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CountResults {
    pub count: f64,
    pub total_height: f64,
    pub perimeter: f64,
    pub top_or_bottom: f64,
    pub top_and_bottom: f64,
    pub width_side: f64,
    pub both_width_sides: f64,
    pub depth_side: f64,
    pub both_depth_sides: f64,
    pub all_sides: f64,
    pub all_sides_plus_top_bottom: f64,
    pub volume: f64,
    /// Opening shapes: top plus both sides, C * (2H + W) (OST-05-037).
    pub opening_perimeter_three_sides: f64,
    /// Opening shapes: all four sides, C * 2 * (H + W) (OST-05-037).
    pub opening_perimeter_four_sides: f64,
}

/// `count` is the net count (negative takeoffs subtract, OST-06-110), so it may be below 0.
pub fn count_results(count: f64, d: CountDims) -> CountResults {
    let c = count;
    let (h, w, dp) = (d.height, d.width, d.depth);
    let footprint = if d.round {
        std::f64::consts::PI * w * w / 4.0
    } else {
        w * dp
    };
    CountResults {
        count: c,
        total_height: c * h,
        perimeter: c * d.perimeter_each,
        top_or_bottom: c * w * dp,
        top_and_bottom: c * 2.0 * w * dp,
        width_side: c * h * w,
        both_width_sides: c * 2.0 * h * w,
        depth_side: c * h * dp,
        both_depth_sides: c * 2.0 * h * dp,
        all_sides: c * 2.0 * (h * w + h * dp),
        all_sides_plus_top_bottom: c * 2.0 * (h * w + h * dp + w * dp),
        volume: c * h * footprint,
        opening_perimeter_three_sides: c * (2.0 * h + w),
        opening_perimeter_four_sides: c * 2.0 * (h + w),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 4 columns, 96 in tall, 12 x 12 in (square), hand-checked.
    fn cols() -> CountResults {
        count_results(4.0, CountDims::new(CountShape::Square, 96.0, 12.0, 99.0))
    }

    #[test]
    fn count_height_perimeter() {
        // OST-05-035: Count = C; Total Height = C*H; Perimeter = C*2*(W+D).
        let r = cols();
        assert_eq!(r.count, 4.0);
        assert_eq!(r.total_height, 384.0);
        assert_eq!(r.perimeter, 4.0 * 2.0 * 24.0);
    }

    #[test]
    fn top_bottom_surfaces() {
        // OST-05-035: top or bottom = C*W*D; top and bottom = 2x that.
        let r = cols();
        assert_eq!(r.top_or_bottom, 576.0);
        assert_eq!(r.top_and_bottom, 1152.0);
    }

    #[test]
    fn side_surfaces() {
        // OST-05-035: single/both width and depth sides, all sides, all plus top and bottom.
        let r = cols();
        assert_eq!(r.width_side, 4.0 * 96.0 * 12.0);
        assert_eq!(r.both_width_sides, 2.0 * r.width_side);
        assert_eq!(r.depth_side, r.width_side);
        assert_eq!(r.both_depth_sides, r.both_width_sides);
        assert_eq!(r.all_sides, 4.0 * 2.0 * (96.0 * 12.0 * 2.0));
        assert_eq!(r.all_sides_plus_top_bottom, r.all_sides + r.top_and_bottom);
    }

    #[test]
    fn volume_non_rectangle_uses_w_squared() {
        // OST-05-035: Volume = C*H*W^2 when D = W (OST-05-033).
        assert_eq!(cols().volume, 4.0 * 96.0 * 144.0);
    }

    #[test]
    fn rectangle_uses_own_depth() {
        // OST-05-035 / OST-05-033: rectangle volume = C*H*W*D.
        let r = count_results(2.0, CountDims::new(CountShape::Rectangle, 10.0, 3.0, 5.0));
        assert_eq!(r.volume, 2.0 * 10.0 * 3.0 * 5.0);
        assert_eq!(r.perimeter, 2.0 * 2.0 * 8.0);
    }

    #[test]
    fn zero_height_is_flat() {
        // OST-05-033: objects are 3D unless Height = 0; flat gives no volume or sides.
        let r = count_results(3.0, CountDims::new(CountShape::Octagon, 0.0, 10.0, 0.0));
        assert_eq!(r.volume, 0.0);
        assert_eq!(r.all_sides, 0.0);
        assert_eq!(r.top_or_bottom, 300.0);
    }

    #[test]
    fn circle_volume_is_cylinder() {
        // OST answer key: 77 round columns, 108 in tall, 12 in across give 940517.44 cu in.
        let r = count_results(77.0, CountDims::new(CountShape::Circle, 108.0, 12.0, 0.0));
        assert!((r.volume - 940_517.442).abs() < 1e-3);
    }

    #[test]
    fn shape_12_perimeter() {
        // OST answer key (JC Commercial 2026, conds 1848 and 2101): 20 objects, W 36, H 216,
        // D 0 store perimeter 1662.769 in, which is 20 * 4 * 36 / sqrt(3).
        let r = count_results(20.0, CountDims::new(CountShape::Shape12, 216.0, 36.0, 0.0));
        assert!((r.perimeter - 1662.769).abs() < 1e-3);
    }

    #[test]
    fn width_side_codes() {
        // OST answer key codes 25 and 26: a 36 x 84 object stores 3024 and 6048 (W * H and
        // 2 * W * H); a 60 x 84 x 12 object stores 10080 for code 26, so depth is unused.
        let r = count_results(1.0, CountDims::new(CountShape::Rectangle, 84.0, 60.0, 12.0));
        assert_eq!(r.width_side, 5040.0);
        assert_eq!(r.both_width_sides, 10080.0);
    }

    #[test]
    fn opening_perimeters() {
        // OST-05-037 with OST answer key: one 36 x 84 door frame is 204 in (top and both
        // sides); 15 windows 60 x 36 all round are 2880 in.
        let door = count_results(1.0, CountDims::new(CountShape::Square, 84.0, 36.0, 0.0));
        assert_eq!(door.opening_perimeter_three_sides, 204.0);
        let win = count_results(15.0, CountDims::new(CountShape::Square, 36.0, 60.0, 0.0));
        assert_eq!(win.opening_perimeter_four_sides, 2880.0);
    }
}
