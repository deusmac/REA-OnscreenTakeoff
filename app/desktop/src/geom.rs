//! Plane geometry on takeoff shapes in page points: hit testing, vertex handles and the
//! running length and area shown while drawing.

use rea_core::model::{Geometry, Id, Point};

fn dist(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// Distance from `p` to the segment `a`-`b`.
pub fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return dist(p, a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    dist(
        p,
        Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        },
    )
}

/// Even-odd test of `p` against a closed ring.
pub fn inside_ring(p: Point, ring: &[Point]) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// The vertices a user can drag.
pub fn vertices(g: &Geometry) -> &[Point] {
    match g {
        Geometry::Polyline(v) => v,
        Geometry::Polygon { outer, .. } => outer,
        Geometry::Point(p) => std::slice::from_ref(p),
    }
}

/// How close `p` is to the shape, or `None` when it is farther than `tol`. A click inside
/// an area counts as `tol`, so an edge or a point nearby wins over the area's inside.
pub fn hit(g: &Geometry, p: Point, tol: f64) -> Option<f64> {
    let edges = |v: &[Point], closed: bool| {
        let mut d = v
            .windows(2)
            .map(|w| dist_to_segment(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min);
        if closed && v.len() > 2 {
            d = d.min(dist_to_segment(p, v[v.len() - 1], v[0]));
        }
        if v.len() == 1 {
            d = dist(p, v[0]);
        }
        d
    };
    let d = match g {
        Geometry::Point(q) => dist(p, *q),
        Geometry::Polyline(v) => edges(v, false),
        Geometry::Polygon { outer, .. } => {
            let d = edges(outer, true);
            if d > tol && inside_ring(p, outer) {
                tol
            } else {
                d
            }
        }
    };
    (d <= tol).then_some(d)
}

/// The takeoff under `p`: the closest hit; on a tie the one drawn last (on top).
pub fn pick<'a>(
    shapes: impl Iterator<Item = (Id, &'a Geometry)>,
    p: Point,
    tol: f64,
) -> Option<Id> {
    let mut best: Option<(f64, Id)> = None;
    for (id, g) in shapes {
        if let Some(d) = hit(g, p, tol) {
            if best.is_none_or(|(bd, _)| d <= bd) {
                best = Some((d, id));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Index of the vertex within `tol` of `p`, closest first.
pub fn vertex_at(g: &Geometry, p: Point, tol: f64) -> Option<usize> {
    vertices(g)
        .iter()
        .enumerate()
        .map(|(i, v)| (i, dist(p, *v)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Moves vertex `i` to `p`. Out-of-range indexes are ignored.
pub fn move_vertex(g: &mut Geometry, i: usize, p: Point) {
    let v = match g {
        Geometry::Polyline(v) => v.get_mut(i),
        Geometry::Polygon { outer, .. } => outer.get_mut(i),
        Geometry::Point(q) => (i == 0).then_some(q),
    };
    if let Some(v) = v {
        *v = p;
    }
}

/// Length of an open polyline.
pub fn polyline_len(v: &[Point]) -> f64 {
    v.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Area of a closed ring (shoelace), always positive.
pub fn ring_area(v: &[Point]) -> f64 {
    let n = v.len();
    (0..n)
        .map(|i| {
            let (a, b) = (v[i], v[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn square() -> Geometry {
        Geometry::Polygon {
            outer: vec![
                pt(0.0, 0.0),
                pt(100.0, 0.0),
                pt(100.0, 100.0),
                pt(0.0, 100.0),
            ],
            backouts: vec![],
        }
    }

    #[test]
    fn hit_segment_within_tolerance() {
        let g = Geometry::Polyline(vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 50.0)]);
        assert!(hit(&g, pt(50.0, 3.0), 4.0).is_some());
        assert!(hit(&g, pt(103.0, 25.0), 4.0).is_some());
        assert!(hit(&g, pt(50.0, 10.0), 4.0).is_none());
    }

    #[test]
    fn hit_inside_polygon_and_count_point() {
        assert_eq!(hit(&square(), pt(50.0, 50.0), 4.0), Some(4.0));
        assert!(hit(&square(), pt(150.0, 50.0), 4.0).is_none());
        let c = Geometry::Point(pt(10.0, 10.0));
        assert!(hit(&c, pt(12.0, 11.0), 4.0).is_some());
        assert!(hit(&c, pt(20.0, 10.0), 4.0).is_none());
    }

    #[test]
    fn pick_prefers_count_inside_area() {
        let area = square();
        let count = Geometry::Point(pt(50.0, 50.0));
        let wall = Geometry::Polyline(vec![pt(0.0, 200.0), pt(100.0, 200.0)]);
        let shapes = [(1, &area), (2, &count), (3, &wall)];
        assert_eq!(pick(shapes.into_iter(), pt(51.0, 50.0), 4.0), Some(2));
        assert_eq!(pick(shapes.into_iter(), pt(30.0, 30.0), 4.0), Some(1));
        assert_eq!(pick(shapes.into_iter(), pt(30.0, 201.0), 4.0), Some(3));
        assert_eq!(pick(shapes.into_iter(), pt(300.0, 300.0), 4.0), None);
    }

    #[test]
    fn vertex_move() {
        let mut g = square();
        let i = vertex_at(&g, pt(98.0, 101.0), 4.0).unwrap();
        assert_eq!(i, 2);
        move_vertex(&mut g, i, pt(120.0, 120.0));
        assert_eq!(vertices(&g)[2], pt(120.0, 120.0));
        assert_eq!(vertices(&g).len(), 4);
        assert!(vertex_at(&g, pt(50.0, 50.0), 4.0).is_none());
        // Area follows the moved corner: 100x100 plus two 20x100 triangles.
        assert!((ring_area(vertices(&g)) - 12000.0).abs() < 1e-9);
        let mut c = Geometry::Point(pt(1.0, 1.0));
        move_vertex(&mut c, 0, pt(5.0, 6.0));
        assert_eq!(c, Geometry::Point(pt(5.0, 6.0)));
    }

    #[test]
    fn running_measures() {
        assert_eq!(
            polyline_len(&[pt(0.0, 0.0), pt(3.0, 4.0), pt(3.0, 10.0)]),
            11.0
        );
        assert_eq!(ring_area(vertices(&square())), 10000.0);
    }
}
