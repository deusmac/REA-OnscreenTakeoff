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
        // U-05 closed by OST answer key: the stored perimeter is L + BL (115 of 115 rows).
        perimeter: l + bl,
        perimeter_without_backouts: l,
        // unknown: slope also applies to the backout volume (plain reading).
        volume: (a - ba) * depth,
        volume_ignore_backouts: a * depth,
    }
}

/// Tile Count (Average): area over one tile's area, fractional (OST-05-023). Zero when
/// the tile size is not set. Same units for both, for example square inches.
// Evidence: OST answer key, 6761.82 sq in with a 24 x 48 in tile stores 5.8696.
pub fn tile_count_average(area: f64, tile_w: f64, tile_h: f64) -> f64 {
    let tile = tile_w * tile_h;
    if tile > 0.0 {
        area / tile
    } else {
        0.0
    }
}

/// Area Counts: number of area objects (OST-05-023).
pub fn area_count(objects: usize) -> usize {
    objects
}

/// Rounds a length up to the next whole multiple of `step` (the condition's Round Quantity
/// setting). A length already on a multiple, within float noise, stays put.
pub fn round_up_length(len: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return len;
    }
    (len / step - 1e-6).ceil().max(0.0) * step
}

/// Ring rebuilt for Round Quantity: starting at the first vertex, each drawn edge keeps its
/// direction but its length is rounded up to `step`. The closing edge is not walked; the
/// ring closes straight from the last walked point back to the start.
// Evidence: OST answer keys (OST_Projects tk 454, 1184, 1392; 63-BOB tk 529): rounded area
// matches this walk exactly, including L-shapes whose walk does not close.
pub fn rounded_ring(ring: &[Point], step: f64) -> Vec<Point> {
    let mut out = Vec::with_capacity(ring.len());
    let Some(&first) = ring.first() else {
        return out;
    };
    let (mut x, mut y) = first;
    out.push(first);
    for w in ring.windows(2) {
        let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
        let len = dx.hypot(dy);
        if len == 0.0 {
            continue;
        }
        let k = round_up_length(len, step) / len;
        x += dx * k;
        y += dy * k;
        out.push((x, y));
    }
    out
}

/// Perimeter under Round Quantity: every edge of the closed ring, closing edge included,
/// rounded up to `step` and summed.
// Evidence: OST answer keys, perimeter (code 15) matches on every rounded row checked.
pub fn rounded_perimeter(ring: &[Point], step: f64) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (x1, y1) = ring[i];
            let (x2, y2) = ring[(i + 1) % n];
            round_up_length((x2 - x1).hypot(y2 - y1), step)
        })
        .sum()
}

/// Area centroid of a ring (shoelace form). Falls back to the vertex mean for a degenerate
/// ring.
fn ring_centroid(ring: &[Point]) -> Point {
    let n = ring.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x1, y1) = ring[i];
        let (x2, y2) = ring[(i + 1) % n];
        let c = x1 * y2 - x2 * y1;
        a += c;
        cx += (x1 + x2) * c;
        cy += (y1 + y2) * c;
    }
    if a.abs() < 1e-12 {
        let m = n.max(1) as f64;
        return (
            ring.iter().map(|p| p.0).sum::<f64>() / m,
            ring.iter().map(|p| p.1).sum::<f64>() / m,
        );
    }
    (cx / (3.0 * a), cy / (3.0 * a))
}

/// Tile grid of a gridded area condition, as stored per takeoff.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    /// Tile size along the grid's own x axis (condition GridSize1).
    pub size_x: f64,
    /// Tile size along the grid's own y axis (condition GridSize2).
    pub size_y: f64,
    /// Gap between tiles (condition Gap); the line pitch is size plus gap.
    pub gap: f64,
    /// Grid origin offset from the outer ring's centroid (takeoff GridOffsetX/Y).
    pub offset: Point,
    /// Grid rotation in radians about its origin (takeoff GridRotation).
    pub rotation: f64,
}

impl Grid {
    /// Rings moved into the grid frame: origin at the outer centroid plus offset, axes
    /// turned by the rotation.
    // Evidence: OST answer keys, 218 of 221 grid lengths and 65 of 73 tile counts match
    // with this origin; the first vertex, bounding box corners and centre all fail.
    fn frame(&self, shape: &AreaShape) -> Vec<Vec<Point>> {
        let c = ring_centroid(shape.outer);
        let (ox, oy) = (c.0 + self.offset.0, c.1 + self.offset.1);
        let (sin, cos) = self.rotation.sin_cos();
        let to = |&(x, y): &Point| {
            let (dx, dy) = (x - ox, y - oy);
            (dx * cos + dy * sin, -dx * sin + dy * cos)
        };
        std::iter::once(shape.outer)
            .chain(shape.backouts.iter().copied())
            .map(|r| r.iter().map(to).collect())
            .collect()
    }

    fn pitch(&self) -> (f64, f64) {
        (self.size_x + self.gap, self.size_y + self.gap)
    }
}

/// Length of the ring chords on the line `axis == c` (0 = vertical line x = c), even-odd.
fn chord_length(rings: &[Vec<Point>], axis: usize, c: f64) -> f64 {
    let o = 1 - axis;
    let get = |p: &Point, a: usize| if a == 0 { p.0 } else { p.1 };
    let mut hits: Vec<f64> = Vec::new();
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (a, b) = (&r[i], &r[(i + 1) % n]);
            let (aa, ba) = (get(a, axis), get(b, axis));
            if (aa > c) != (ba > c) {
                let t = (c - aa) / (ba - aa);
                hits.push(get(a, o) + t * (get(b, o) - get(a, o)));
            }
        }
    }
    hits.sort_by(f64::total_cmp);
    hits.chunks_exact(2).map(|p| p[1] - p[0]).sum()
}

fn span(rings: &[Vec<Point>], axis: usize) -> (f64, f64) {
    rings
        .iter()
        .flatten()
        .fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            let v = if axis == 0 { p.0 } else { p.1 };
            (lo.min(v), hi.max(v))
        })
}

/// Grid Length (area code 16): total length of the grid lines inside the area (outer
/// minus backouts). Lines run at whole multiples of the pitch from the grid origin.
// Evidence: OST answer keys, 218 of 221 rows; with Gap 1 on 16 in tiles the pitch is 17
// (OST_Sample_Projects_2026 total 1003: 367495.378 exact).
pub fn grid_length(shape: &AreaShape, grid: &Grid) -> f64 {
    let (px, py) = grid.pitch();
    if px <= 0.0 || py <= 0.0 || shape.outer.len() < 3 {
        return 0.0;
    }
    let rings = grid.frame(shape);
    let mut total = 0.0;
    for (axis, pitch) in [(0, px), (1, py)] {
        let (lo, hi) = span(&rings, axis);
        let mut k = (lo / pitch).floor() + 1.0;
        while k * pitch < hi {
            total += chord_length(&rings, axis, k * pitch);
            k += 1.0;
        }
    }
    total
}

/// Keeps the part of a ring on one side of the line `axis == v` (Sutherland-Hodgman).
fn clip(ring: &[Point], axis: usize, v: f64, keep_above: bool) -> Vec<Point> {
    let get = |p: &Point| if axis == 0 { p.0 } else { p.1 };
    let n = ring.len();
    let mut out = Vec::with_capacity(n + 2);
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let ina = (get(&a) >= v) == keep_above;
        let inb = (get(&b) >= v) == keep_above;
        if ina {
            out.push(a);
        }
        if ina != inb {
            let t = (v - get(&a)) / (get(&b) - get(&a));
            out.push((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)));
        }
    }
    out
}

fn strip(ring: &[Point], axis: usize, lo: f64, hi: f64) -> Vec<Point> {
    clip(&clip(ring, axis, lo, true), axis, hi, false)
}

/// Tile Count (area code 18): number of grid cells that hold any part of the area (outer
/// minus backouts). A cut tile counts as a whole tile.
// Evidence: OST answer keys, 65 of 73 rows (JC_Commercial total 4313: 192 tiles where
// the average count is 150.5). Round Quantity does not change it.
pub fn grid_tile_count(shape: &AreaShape, grid: &Grid) -> usize {
    let (px, py) = grid.pitch();
    if px <= 0.0 || py <= 0.0 || shape.outer.len() < 3 {
        return 0;
    }
    let rings = grid.frame(shape);
    let (x0, x1) = span(&rings, 0);
    let (y0, y1) = span(&rings, 1);
    let min_area = 1e-9 * px * py;
    let mut n = 0;
    for i in (x0 / px).floor() as i64..(x1 / px).ceil() as i64 {
        let (lo, hi) = (i as f64 * px, (i + 1) as f64 * px);
        let cols: Vec<Vec<Point>> = rings.iter().map(|r| strip(r, 0, lo, hi)).collect();
        if cols[0].len() < 3 {
            continue;
        }
        for j in (y0 / py).floor() as i64..(y1 / py).ceil() as i64 {
            let (lo, hi) = (j as f64 * py, (j + 1) as f64 * py);
            let covered: f64 = cols
                .iter()
                .enumerate()
                .filter(|(_, r)| r.len() >= 3)
                .map(|(k, r)| {
                    let a = ring_area_pts(&strip(r, 1, lo, hi));
                    if k == 0 {
                        a
                    } else {
                        -a
                    }
                })
                .sum();
            if covered > min_area {
                n += 1;
            }
        }
    }
    n
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
    fn tile_count_average_is_fractional() {
        // OST-05-023 with OST answer key: 6761.82 sq in of 24 x 48 tiles is 5.8696.
        assert!((tile_count_average(6761.82, 24.0, 48.0) - 5.869_635).abs() < 1e-6);
        assert_eq!(tile_count_average(100.0, 0.0, 10.0), 0.0);
    }

    #[test]
    fn round_quantity_rectangle_rounds_each_side() {
        // OST answer key (63-BOB tk 529): 169.142 x 102.664 in, Round Quantity 24 in,
        // stores area 23040 (192 x 120) and perimeter 624.
        let outer = rect(169.142, 102.664);
        let r = rounded_ring(&outer, 24.0);
        assert!((ring_area_pts(&r) - 23040.0).abs() < 1e-6);
        assert!((rounded_perimeter(&outer, 24.0) - 624.0).abs() < 1e-9);
        assert_eq!(round_up_length(48.0, 24.0), 48.0);
    }

    #[test]
    fn round_quantity_walk_does_not_close() {
        // OST answer key (OST_Projects tk 454): L-shape with edges 64, 93, 149, 193, 213,
        // 100 stores area 43488 and perimeter 888 with Round Quantity 24 in.
        let outer = vec![
            (0.0, 0.0),
            (64.0, 0.0),
            (64.0, -93.0),
            (213.0, -93.0),
            (213.0, 100.0),
            (0.0, 100.0),
        ];
        assert!((ring_area_pts(&rounded_ring(&outer, 24.0)) - 43488.0).abs() < 1e-6);
        assert!((rounded_perimeter(&outer, 24.0) - 888.0).abs() < 1e-9);
    }

    fn grid(size: f64, gap: f64) -> Grid {
        Grid {
            size_x: size,
            size_y: size,
            gap,
            offset: (0.0, 0.0),
            rotation: 0.0,
        }
    }

    #[test]
    fn grid_is_anchored_at_the_centroid() {
        // OST answer keys: the grid origin is the area centroid plus the stored offset.
        // A 96 x 48 room with 24 in tiles has lines at x = 24, 48, 72 and y = 24: 3*48 + 96.
        let outer = rect(96.0, 48.0);
        let shape = AreaShape {
            outer: &outer,
            backouts: &[],
        };
        assert!((grid_length(&shape, &grid(24.0, 0.0)) - 240.0).abs() < 1e-9);
        assert_eq!(grid_tile_count(&shape, &grid(24.0, 0.0)), 8);
        // Offset by 12 in: lines at 12, 36, 60, 84 and 12, 36; cut tiles count whole.
        let g = Grid {
            offset: (12.0, 12.0),
            ..grid(24.0, 0.0)
        };
        assert!((grid_length(&shape, &g) - (4.0 * 48.0 + 2.0 * 96.0)).abs() < 1e-9);
        assert_eq!(grid_tile_count(&shape, &g), 15);
    }

    #[test]
    fn grid_gap_widens_the_pitch() {
        // OST answer key (OST_Sample_Projects_2026 total 1003): Gap 1 on 16 in tiles puts
        // lines 17 in apart. A 68 x 34 room centred on a line: x = -17, 0, 17; y = 0.
        let outer = vec![(-34.0, -17.0), (34.0, -17.0), (34.0, 17.0), (-34.0, 17.0)];
        let shape = AreaShape {
            outer: &outer,
            backouts: &[],
        };
        assert!((grid_length(&shape, &grid(16.0, 1.0)) - (3.0 * 34.0 + 68.0)).abs() < 1e-9);
    }

    #[test]
    fn grid_skips_backouts_and_follows_rotation() {
        // OST-05-023: backouts remove grid lines and tiles they cover fully.
        let outer = rect(96.0, 96.0);
        let hole = vec![(18.0, 18.0), (78.0, 18.0), (78.0, 78.0), (18.0, 78.0)];
        let shape = AreaShape {
            outer: &outer,
            backouts: &[&hole],
        };
        // Lines at 24, 48, 72 each way, each loses 60 in inside the hole; the 4 tiles
        // fully inside the hole drop out.
        assert!((grid_length(&shape, &grid(24.0, 0.0)) - (6.0 * 96.0 - 6.0 * 60.0)).abs() < 1e-9);
        assert_eq!(grid_tile_count(&shape, &grid(24.0, 0.0)), 12);
        // A quarter turn swaps the axes of a 24 x 48 tile on a 96 x 48 room.
        let wide = rect(96.0, 48.0);
        let plain = AreaShape {
            outer: &wide,
            backouts: &[],
        };
        let mut g = Grid {
            size_x: 24.0,
            size_y: 48.0,
            gap: 0.0,
            offset: (0.0, 0.0),
            rotation: 0.0,
        };
        assert_eq!(grid_tile_count(&plain, &g), 8);
        g.rotation = std::f64::consts::FRAC_PI_2;
        assert_eq!(grid_tile_count(&plain, &g), 4);
    }

    #[test]
    fn area_count_is_object_count() {
        // OST-05-023: Area Counts = number of area objects.
        assert_eq!(area_count(3), 3);
    }
}
