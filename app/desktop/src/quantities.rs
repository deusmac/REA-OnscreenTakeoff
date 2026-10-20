//! Condition totals for the Summary panel, computed with our core math from an imported bid.
//!
//! One row per condition: linear gives length (LF) and wall area (SF) when the condition has
//! a height; area gives area (SF) and perimeter (LF); count gives each (EA). Negative
//! takeoffs subtract, and an area takeoff whose parent is another area takeoff is a backout
//! of that parent (docs/study/formats/ost-quantities.md).
// shortcut: no zones, typical groups, Round Quantity, drops or wall openings yet; the parity
// harness (crates/core/tests/ost_parity.rs) has those rules, move them here when the UI needs them.

use std::collections::HashMap;

use rea_core::area::{area_results, slope_factor, AreaShape};
use rea_core::linear::{arc_length, polyline_points};
use rea_core::model::{Condition, ConditionStyle, Geometry, Id, Page, Takeoff};

pub struct SummaryRow {
    pub number: i64,
    pub name: String,
    pub color: [u8; 3],
    pub q1: f64,
    pub u1: &'static str,
    pub q2: Option<(f64, &'static str)>,
}

/// `parent` maps a takeoff to its parent takeoff; `sagitta` holds the arc height (in page
/// points) of curved two-point segments.
pub fn summary(
    pages: &[Page],
    conditions: &[Condition],
    takeoffs: &[Takeoff],
    parent: &HashMap<Id, Id>,
    sagitta: &HashMap<Id, f64>,
) -> Vec<SummaryRow> {
    let rpp: HashMap<Id, f64> = pages
        .iter()
        .filter_map(|p| p.scale.as_ref().map(|s| (p.id, s.real_per_page_unit)))
        .collect();
    let by_id: HashMap<Id, &Takeoff> = takeoffs.iter().map(|t| (t.id, t)).collect();
    let style: HashMap<Id, ConditionStyle> = conditions.iter().map(|c| (c.id, c.style)).collect();
    // Backout rings per parent area takeoff.
    let mut backouts: HashMap<Id, Vec<Vec<(f64, f64)>>> = HashMap::new();
    for t in takeoffs {
        if let (Some(p), Geometry::Polygon { outer, .. }) = (parent.get(&t.id), &t.geometry) {
            if by_id
                .get(p)
                .is_some_and(|pt| style.get(&pt.condition_id) == Some(&ConditionStyle::Area))
            {
                backouts
                    .entry(*p)
                    .or_default()
                    .push(outer.iter().map(|q| (q.x, q.y)).collect());
            }
        }
    }

    let mut rows: Vec<SummaryRow> = conditions
        .iter()
        .map(|c| {
            let mut q1 = 0.0;
            let mut q2 = 0.0;
            for t in takeoffs.iter().filter(|t| t.condition_id == c.id) {
                let sign = if t.negative { -1.0 } else { 1.0 };
                let k = rpp.get(&t.page_id).copied();
                match (&t.geometry, c.style, k) {
                    (Geometry::Polyline(v), ConditionStyle::Linear, Some(k)) => {
                        let pts: Vec<(f64, f64)> = v.iter().map(|q| (q.x, q.y)).collect();
                        let pts_len = polyline_points(&pts);
                        let len = match (pts.len(), sagitta.get(&t.id)) {
                            (2, Some(s)) => arc_length(pts_len, *s),
                            _ => pts_len,
                        };
                        q1 += sign * len * k;
                    }
                    (Geometry::Polygon { outer, .. }, ConditionStyle::Area, Some(k))
                        if !parent.get(&t.id).is_some_and(|p| backouts.contains_key(p)) =>
                    {
                        let ring: Vec<(f64, f64)> = outer.iter().map(|q| (q.x, q.y)).collect();
                        let holes = backouts.get(&t.id).cloned().unwrap_or_default();
                        let hole_refs: Vec<&[(f64, f64)]> =
                            holes.iter().map(|h| h.as_slice()).collect();
                        let shape = AreaShape {
                            outer: &ring,
                            backouts: &hole_refs,
                        };
                        let slope = c.slope.map_or(1.0, |(rise, run)| slope_factor(rise, run));
                        let r = area_results(&shape, k, 0.0, slope);
                        q1 += sign * r.area;
                        q2 += sign * r.perimeter;
                    }
                    (Geometry::Point(_), ConditionStyle::Count, _) => q1 += sign,
                    _ => {}
                }
            }
            let (q1, u1, q2) = match c.style {
                ConditionStyle::Linear => (
                    q1 / 12.0,
                    "LF",
                    (c.height > 0.0).then(|| (q1 * c.height / 144.0, "SF")),
                ),
                ConditionStyle::Area => (q1 / 144.0, "SF", Some((q2 / 12.0, "LF"))),
                ConditionStyle::Count => (q1, "EA", None),
            };
            SummaryRow {
                number: c.number,
                name: c.name.clone(),
                color: c.color,
                q1,
                u1,
                q2,
            }
        })
        .collect();
    rows.sort_by_key(|r| r.number);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_core::model::{Point, Scale, ScaleStyle};

    fn page() -> Page {
        Page {
            id: 1,
            project_id: 1,
            index: 0,
            sheet_no: String::new(),
            sheet_name: String::new(),
            file_path: String::new(),
            file_page_index: 0,
            width: 2592.0,
            height: 1728.0,
            // 1/8 in = 1 ft: 96 real inches per sheet inch, 96/72 per point.
            scale: Some(Scale {
                real_per_page_unit: 96.0 / 72.0,
                style: ScaleStyle::Architectural,
                label: String::new(),
            }),
            rotation: 0,
            complete: false,
        }
    }

    fn cond(id: Id, style: ConditionStyle, height: f64) -> Condition {
        Condition {
            id,
            project_id: 1,
            number: id,
            name: format!("C{id}"),
            style,
            ctype: String::new(),
            layer: String::new(),
            color: [0, 0, 0],
            pattern: String::new(),
            height,
            thickness: 0.0,
            width: 0.0,
            depth: 0.0,
            slope: None,
            count_shape: None,
            results: [None, None, None],
        }
    }

    fn tk(id: Id, condition_id: Id, negative: bool, geometry: Geometry) -> Takeoff {
        Takeoff {
            id,
            page_id: 1,
            condition_id,
            bid_area_id: None,
            negative,
            geometry,
        }
    }

    fn sq(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![
            Point { x, y },
            Point { x: x + s, y },
            Point { x: x + s, y: y + s },
            Point { x, y: y + s },
        ]
    }

    #[test]
    fn totals_per_condition() {
        let conds = [
            cond(1, ConditionStyle::Linear, 96.0),
            cond(2, ConditionStyle::Area, 0.0),
            cond(3, ConditionStyle::Count, 0.0),
        ];
        let line = |a: f64, b: f64| {
            Geometry::Polyline(vec![Point { x: a, y: 0.0 }, Point { x: b, y: 0.0 }])
        };
        let poly = |v| Geometry::Polygon {
            outer: v,
            backouts: vec![],
        };
        let pt = Geometry::Point(Point { x: 1.0, y: 1.0 });
        let takeoffs = [
            tk(10, 1, false, line(0.0, 72.0)),
            tk(11, 1, false, line(72.0, 144.0)),
            tk(20, 2, false, poly(sq(0.0, 0.0, 72.0))),
            tk(21, 2, false, poly(sq(10.0, 10.0, 36.0))), // backout of 20
            tk(30, 3, false, pt.clone()),
            tk(31, 3, false, pt.clone()),
            tk(32, 3, true, pt),
        ];
        let parent = HashMap::from([(21, 20)]);
        let rows = summary(&[page()], &conds, &takeoffs, &parent, &HashMap::new());
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        // 144 pt * 96/72 = 192 in = 16 LF; wall 16 LF * 8 ft = 128 SF.
        assert!(near(rows[0].q1, 16.0) && near(rows[0].q2.unwrap().0, 128.0));
        // 96 x 96 in = 64 SF minus 48 x 48 in = 16 SF backout.
        assert!(near(rows[1].q1, 48.0));
        // Outer 32 ft plus backout 16 ft of perimeter.
        assert!(near(rows[1].q2.unwrap().0, 48.0));
        assert!(near(rows[2].q1, 1.0));
    }
}
