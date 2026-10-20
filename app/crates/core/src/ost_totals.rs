//! OST stored totals (`BidTakeoffTotals`) recomputed from one bid's conditions and takeoffs.
//! One implementation for the parity harness (`tests/ost_parity.rs`) and the app Summary.
//!
//! Inputs are OST's raw numbers: positions in real inches as stored, result codes
//! (`Quantity1..3`) and unit codes (`UOM1..3`). Outputs are in OST base units: inches,
//! square inches, cubic inches and counts. Rules and their evidence:
//! docs/study/formats/ost-quantities.md.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::area::{
    area_results, grid_length, grid_tile_count, rounded_perimeter, rounded_ring, slope_factor,
    tile_count_average, AreaShape, Grid,
};
use crate::count::{count_results, CountDims, CountShape};
use crate::linear::{
    linear_results, ost_segment, points_inside, LinearInput, Opening, Point, Slope,
};
use crate::model::Id;
use crate::scale::{Scale, ScaleStyle};

/// OST condition `Type` codes.
pub const LINEAR: i64 = 0;
pub const AREA: i64 = 1;
pub const COUNT: i64 = 2;
pub const ATTACHED_COUNT: i64 = 3;
/// One point per takeoff; OST's total is the takeoff count (JC's databases, 2026-10-20).
pub const COUNT_POINT: i64 = 4;

/// A condition with the OST fields that feed totals. Lengths in inches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OstCondition {
    pub uid: Id,
    /// `Type`: see the constants above.
    pub kind: i64,
    /// `Shape` of a count: 1 circle, 3 rectangle, 12 the shape with a 4 W / sqrt(3)
    /// perimeter, anything else square.
    pub shape: i64,
    pub width: f64,
    pub height: f64,
    pub thickness: f64,
    pub depth: f64,
    pub rise: f64,
    pub run: f64,
    /// `Quantity1..3`: which result each slot shows (code table in the format notes).
    pub results: [u32; 3],
    /// `UOM1..3`: display unit of each slot; -1 marks an unused slot.
    pub uoms: [i64; 3],
    /// `RoundQuantity` on: area edges round up to `round_up` inches.
    pub round_quantity: bool,
    pub round_up: f64,
    /// `Backout` on an attached count: its openings cut the wall it sits on.
    pub backout: bool,
    /// `DropRun` on a linear: each takeoff's `count` is a drop length added to the run.
    pub drop_run: bool,
    /// `GridSize1`, `GridSize2`.
    pub grid_size: (f64, f64),
    pub gap: f64,
}

/// A takeoff with its raw OST fields. `None` stands for OST's empty or 0 UID.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OstTakeoff {
    pub uid: Id,
    pub page: Id,
    pub condition: Id,
    pub zone: Option<Id>,
    pub area: Option<Id>,
    pub typ_area: Option<Id>,
    pub parent: Option<Id>,
    pub typ_group: Option<Id>,
    /// `Position` numbers in real inches, laid out per condition type: linear
    /// `x1;y1;x2;y2;s` or `x1;y1;x2;y2;px;py;s`, area a ring, count `x;y`, attached count
    /// `d;side;x;y`.
    pub position: Vec<f64>,
    /// `Count`: the drop length in inches when the condition has Drop/Run on.
    pub count: f64,
    pub negative: bool,
    pub grid_offset: (f64, f64),
    pub grid_rotation: f64,
}

/// A zone region drawn on a page (stored by OST as a takeoff with a zone and no condition).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ZoneOutline {
    pub zone: Id,
    pub page: Id,
    /// Real inches.
    pub ring: Vec<Point>,
}

/// How many copies of a typical area a bid area holds (`BidTypAreaCounts`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TypAreaCount {
    pub area: Id,
    pub typ_area: Id,
    pub count: f64,
}

/// Everything one bid needs for its totals.
#[derive(Debug, Clone, Default)]
pub struct OstJob {
    pub conditions: Vec<OstCondition>,
    pub takeoffs: Vec<OstTakeoff>,
    pub zone_outlines: Vec<ZoneOutline>,
    /// Zones marked `IsNegativeValues`: their rows store the quantity with the sign flipped.
    pub negative_zones: HashSet<Id>,
    pub typ_area_counts: Vec<TypAreaCount>,
}

/// The key of an OST total row: page, zone, bid area, typical area, condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TotalKey {
    pub page: Id,
    pub zone: Option<Id>,
    pub area: Option<Id>,
    pub typ_area: Option<Id>,
    pub condition: Id,
}

/// One total row. A slot is `None` when it is unused (UOM -1) or its result code is not
/// modeled.
#[derive(Debug, Clone, PartialEq)]
pub struct TotalRow {
    pub key: TotalKey,
    pub quantities: [Option<f64>; 3],
}

impl OstTakeoff {
    fn sign(&self) -> f64 {
        if self.negative {
            -1.0
        } else {
            1.0
        }
    }

    fn pairs(&self) -> Vec<Point> {
        pairs(&self.position)
    }

    /// The points that place the takeoff on the page, for the zone test.
    fn placed_points(&self, kind: i64) -> Vec<Point> {
        let v = &self.position;
        match kind {
            LINEAR => ost_segment(v).map_or(vec![], |(a, b, _)| vec![a, b]),
            ATTACHED_COUNT => pairs(v.get(2..4).unwrap_or(&[])),
            _ => pairs(v),
        }
    }
}

fn pairs(v: &[f64]) -> Vec<Point> {
    v.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

struct Ctx<'a> {
    conds: HashMap<Id, &'a OstCondition>,
    kids: HashMap<Id, Vec<&'a OstTakeoff>>,
}

impl Ctx<'_> {
    fn kids_of(&self, t: &OstTakeoff) -> impl Iterator<Item = (&OstTakeoff, &OstCondition)> {
        self.kids
            .get(&t.uid)
            .into_iter()
            .flatten()
            .filter_map(|k| Some((*k, *self.conds.get(&k.condition)?)))
    }
}

/// Every result OST can store for the condition, keyed by result code, summed over `ts`.
/// A code missing from the map is not modeled.
fn results(ctx: &Ctx, c: &OstCondition, ts: &[&OstTakeoff]) -> HashMap<u32, f64> {
    let mut out: HashMap<u32, f64> = HashMap::new();
    let mut add = |code: u32, v: f64| *out.entry(code).or_insert(0.0) += v;
    // Positions are already real inches, so the drawing scale is 1 inch per unit.
    let unit = Scale::from_known_length((0.0, 0.0), (1.0, 0.0), 1.0, ScaleStyle::Custom)
        .expect("unit scale");
    match c.kind {
        LINEAR => {
            // A linear takeoff with a parent is not part of its condition's total.
            for t in ts.iter().filter(|t| t.parent.is_none()) {
                let openings: Vec<Opening> = ctx
                    .kids_of(t)
                    .filter(|(_, kc)| matches!(kc.kind, COUNT | ATTACHED_COUNT) && kc.backout)
                    .map(|(_, kc)| Opening {
                        width: kc.width,
                        height: kc.height,
                    })
                    .collect();
                let Some((a, b, sag)) = ost_segment(&t.position) else {
                    continue;
                };
                let Ok(r) = linear_results(&LinearInput {
                    points: &[a, b],
                    sagittas: &[sag],
                    scale: &unit,
                    height: c.height,
                    thickness: c.thickness,
                    slope: (c.run != 0.0).then_some(Slope {
                        rise: c.rise,
                        run: c.run,
                    }),
                    drops: if c.drop_run { t.count } else { 0.0 },
                    openings: &openings,
                }) else {
                    continue;
                };
                let k = t.sign();
                for (code, v) in [
                    (1, r.length),
                    (2, r.segment_count as f64),
                    (3, r.area_one_side),
                    (4, r.area_both_sides),
                    (5, r.area_top_or_bottom),
                    (6, r.area_top_and_bottom),
                    (7, r.area_one_end),
                    (8, r.area_both_ends),
                    (9, r.area_all_sides),
                    (20, r.volume),
                ] {
                    add(code, k * v);
                }
            }
        }
        AREA => {
            let f = slope_factor(c.rise, c.run);
            let step = if c.round_quantity { c.round_up } else { 0.0 };
            for t in ts.iter().filter(|t| t.parent.is_none()) {
                let outer = t.pairs();
                // A backout counts with its parent, whatever condition it sits in.
                let rings: Vec<Vec<Point>> = ctx
                    .kids_of(t)
                    .filter(|(_, kc)| kc.kind == AREA)
                    .map(|(k, _)| k.pairs())
                    .collect();
                let refs: Vec<&[Point]> = rings.iter().map(Vec::as_slice).collect();
                let shape = AreaShape {
                    outer: &outer,
                    backouts: &refs,
                };
                // Round Quantity: edges round up to the step (area and perimeter only).
                let rounded = (step > 0.0).then(|| rounded_ring(&outer, step));
                let mut r = area_results(
                    &AreaShape {
                        outer: rounded.as_deref().unwrap_or(&outer),
                        backouts: &refs,
                    },
                    1.0,
                    c.thickness,
                    f,
                );
                if step > 0.0 {
                    let bl = r.perimeter - r.perimeter_without_backouts;
                    r.perimeter_without_backouts = rounded_perimeter(&outer, step);
                    r.perimeter = r.perimeter_without_backouts + bl;
                }
                // Grid length and tile count use the drawn shape, not the rounded one.
                let grid = Grid {
                    size_x: c.grid_size.0,
                    size_y: c.grid_size.1,
                    gap: c.gap,
                    offset: t.grid_offset,
                    rotation: t.grid_rotation,
                };
                let k = t.sign();
                for (code, v) in [
                    (11, r.area),
                    (12, r.area), // minus attachments; attachments not modeled
                    (14, r.perimeter_without_backouts),
                    (15, r.perimeter),
                    (16, grid_length(&shape, &grid)),
                    (17, tile_count_average(r.area, c.grid_size.0, c.grid_size.1)),
                    (18, grid_tile_count(&shape, &grid) as f64),
                    (19, 1.0),
                    (21, r.volume),
                    (33, r.perimeter), // plus attachments; attachments not modeled
                    (34, r.area_ignore_backouts),
                ] {
                    add(code, k * v);
                }
            }
        }
        COUNT | ATTACHED_COUNT | COUNT_POINT => {
            let n: f64 = ts.iter().map(|t| t.sign()).sum();
            let shape = match c.shape {
                1 => CountShape::Circle,
                3 => CountShape::Rectangle,
                12 => CountShape::Shape12,
                _ => CountShape::Square,
            };
            let r = count_results(n, CountDims::new(shape, c.height, c.width, c.depth));
            for (code, v) in [
                (23, r.count),
                (24, r.total_height),
                (5, r.top_or_bottom),
                (13, r.perimeter),
                (25, r.width_side),
                (26, r.both_width_sides),
                (20, r.volume),
                (31, r.opening_perimeter_four_sides),
                (32, r.opening_perimeter_three_sides),
            ] {
                add(code, v);
            }
        }
        _ => {}
    }
    out
}

/// The stored total rows of one bid, sorted by key.
///
/// - Takeoffs in a typical group are left out (OST keeps them in typical-group totals).
/// - A takeoff without its own zone gets the zone whose outline holds all its points; a
///   zone row is signed by the zone's `IsNegativeValues`, and the no-zone row of the same
///   page, area, typical area and condition stores 0 when nothing is left outside.
/// - A row with a bid area and a typical area is the typical area's own row (no bid area)
///   times that bid area's copy count.
/// - A `parent` that is not a takeoff of this bid (a stale link from copying a bid) is
///   ignored.
pub fn bid_totals(job: &OstJob) -> Vec<TotalRow> {
    let conds: HashMap<Id, &OstCondition> = job.conditions.iter().map(|c| (c.uid, c)).collect();
    let uids: HashSet<Id> = job.takeoffs.iter().map(|t| t.uid).collect();
    let mut kids: HashMap<Id, Vec<&OstTakeoff>> = HashMap::new();
    for t in &job.takeoffs {
        if let Some(p) = t.parent.filter(|p| uids.contains(p)) {
            kids.entry(p).or_default().push(t);
        }
    }
    let ctx = Ctx { conds, kids };

    let mut groups: BTreeMap<TotalKey, Vec<&OstTakeoff>> = BTreeMap::new();
    for t in job.takeoffs.iter().filter(|t| t.typ_group.is_none()) {
        let Some(c) = ctx.conds.get(&t.condition) else {
            continue;
        };
        let zone = t.zone.or_else(|| {
            let pts = t.placed_points(c.kind);
            job.zone_outlines
                .iter()
                .find(|z| z.page == t.page && points_inside(&pts, &z.ring))
                .map(|z| z.zone)
        });
        let key = TotalKey {
            page: t.page,
            zone,
            area: t.area,
            typ_area: t.typ_area,
            condition: t.condition,
        };
        groups.entry(key).or_default().push(t);
    }

    let mut rows: BTreeMap<TotalKey, [Option<f64>; 3]> = BTreeMap::new();
    for (key, ts) in &groups {
        let c = ctx.conds[&key.condition];
        let vals = results(&ctx, c, ts);
        let sign = match key.zone {
            Some(z) if job.negative_zones.contains(&z) => -1.0,
            _ => 1.0,
        };
        rows.insert(*key, slots(c, |code| vals.get(&code).map(|v| v * sign)));
        if key.zone.is_some() {
            let outside = TotalKey { zone: None, ..*key };
            if !groups.contains_key(&outside) {
                rows.insert(outside, slots(c, |_| Some(0.0)));
            }
        }
    }

    // Typical-area rows per bid area. A takeoff carrying both ids directly has no row of its
    // own: OST's row for that pair is always the multiplied one.
    rows.retain(|k, _| k.area.is_none() || k.typ_area.is_none());
    let typical: Vec<(TotalKey, [Option<f64>; 3])> = rows
        .iter()
        .filter(|(k, _)| k.area.is_none() && k.typ_area.is_some())
        .map(|(k, v)| (*k, *v))
        .collect();
    for (k, v) in typical {
        for tc in job
            .typ_area_counts
            .iter()
            .filter(|tc| Some(tc.typ_area) == k.typ_area)
        {
            let key = TotalKey {
                area: Some(tc.area),
                ..k
            };
            rows.insert(key, v.map(|q| q.map(|q| q * tc.count)));
        }
    }

    rows.into_iter()
        .map(|(key, quantities)| TotalRow { key, quantities })
        .collect()
}

/// The three slots of a condition: `None` when the slot is unused or `value` has no result.
fn slots(c: &OstCondition, value: impl Fn(u32) -> Option<f64>) -> [Option<f64>; 3] {
    std::array::from_fn(|i| {
        if c.uoms[i] == -1 {
            None
        } else {
            value(c.results[i])
        }
    })
}

/// Whole-job totals per condition: the sum of its rows. A typical area's own row counts only
/// when no bid area holds copies of it; otherwise its multiplied rows count.
// unknown: how OST's Summary adds typical areas to the job total; this avoids counting a
// typical area both once and per copy.
pub fn condition_totals(job: &OstJob, rows: &[TotalRow]) -> BTreeMap<Id, [Option<f64>; 3]> {
    let copied: HashSet<Id> = job.typ_area_counts.iter().map(|t| t.typ_area).collect();
    let mut out: BTreeMap<Id, [Option<f64>; 3]> = BTreeMap::new();
    for r in rows {
        if r.key.area.is_none() && r.key.typ_area.is_some_and(|t| copied.contains(&t)) {
            continue;
        }
        let sum = out.entry(r.key.condition).or_insert([None; 3]);
        for (s, q) in sum.iter_mut().zip(r.quantities) {
            if let Some(q) = q {
                *s = Some(s.unwrap_or(0.0) + q);
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dim {
    Length,
    Area,
    Volume,
    Count,
}

fn code_dim(code: u32) -> Dim {
    match code {
        1 | 13..=16 | 24 | 31..=33 => Dim::Length,
        3..=12 | 25 | 26 | 34 => Dim::Area,
        20 | 21 => Dim::Volume,
        _ => Dim::Count,
    }
}

/// Display unit for a result code under a `UOM` code, and the divisor from OST's base units.
/// A unit code that does not fit the result's dimension (or is unknown, such as code 1)
/// falls back to the usual unit for that dimension.
pub fn display_unit(code: u32, uom: i64) -> (&'static str, f64) {
    let dim = code_dim(code);
    match (uom, dim) {
        (0, Dim::Count) => ("EA", 1.0),
        (2, Dim::Length) => ("LF", 12.0),
        (5, Dim::Area) => ("SF", 144.0),
        (6, Dim::Area) => ("SY", 1296.0),
        (9, Dim::Volume) => ("CY", 46656.0),
        (_, Dim::Length) => ("LF", 12.0),
        (_, Dim::Area) => ("SF", 144.0),
        (_, Dim::Volume) => ("CY", 46656.0),
        (_, Dim::Count) => ("EA", 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(uid: Id, kind: i64, results: [u32; 3], uoms: [i64; 3]) -> OstCondition {
        OstCondition {
            uid,
            kind,
            results,
            uoms,
            ..Default::default()
        }
    }

    fn tk(uid: Id, condition: Id, position: &[f64]) -> OstTakeoff {
        OstTakeoff {
            uid,
            page: 1,
            condition,
            position: position.to_vec(),
            ..Default::default()
        }
    }

    fn near(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-6)
    }

    // Format notes, linear codes 1 and 3, wall openings and negative takeoffs.
    #[test]
    fn linear_with_opening_and_negative() {
        let mut wall = cond(1, LINEAR, [1, 3, 2], [2, 5, 0]);
        wall.height = 120.0;
        let mut door = cond(2, ATTACHED_COUNT, [23, 0, 0], [0, -1, -1]);
        (door.width, door.height, door.backout) = (36.0, 84.0, true);
        let mut d = tk(11, 2, &[10.0, 0.0, 10.0, 0.0]);
        d.parent = Some(10);
        let mut neg = tk(12, 1, &[0.0, 0.0, 24.0, 0.0, 0.0]);
        neg.negative = true;
        let job = OstJob {
            conditions: vec![wall, door],
            takeoffs: vec![tk(10, 1, &[0.0, 0.0, 240.0, 0.0, 0.0, 0.0, 0.0]), d, neg],
            ..Default::default()
        };
        let rows = bid_totals(&job);
        assert_eq!(rows.len(), 2);
        let w = &rows[0].quantities;
        // 240 - 24 = 216 in; side area 216 * 120 minus the 36 x 84 door.
        assert!(near(w[0], 216.0));
        assert!(near(w[1], 216.0 * 120.0 - 36.0 * 84.0));
        assert!(near(w[2], 0.0)); // one segment minus one negative segment
        assert!(near(rows[1].quantities[0], 1.0));
        assert_eq!(rows[1].quantities[1], None); // unused slot
    }

    // Format notes: an area backout counts with its parent; typical-group takeoffs are out.
    #[test]
    fn area_backout_and_typical_group() {
        let job = OstJob {
            conditions: vec![cond(1, AREA, [11, 15, 19], [5, 2, 0])],
            takeoffs: vec![
                tk(10, 1, &[0.0, 0.0, 120.0, 0.0, 120.0, 120.0, 0.0, 120.0]),
                OstTakeoff {
                    parent: Some(10),
                    ..tk(11, 1, &[10.0, 10.0, 22.0, 10.0, 22.0, 22.0, 10.0, 22.0])
                },
                OstTakeoff {
                    typ_group: Some(5),
                    ..tk(12, 1, &[0.0, 0.0, 12.0, 0.0, 12.0, 12.0])
                },
            ],
            ..Default::default()
        };
        let rows = bid_totals(&job);
        assert_eq!(rows.len(), 1);
        let q = rows[0].quantities;
        assert!(near(q[0], 14400.0 - 144.0));
        assert!(near(q[1], 480.0 + 48.0));
        assert!(near(q[2], 1.0));
    }

    // Format notes, zone rule: the zone row holds the takeoffs inside the outline, signed by
    // IsNegativeValues, and the no-zone sibling stores 0.
    #[test]
    fn zone_outline_sign_and_zero_sibling() {
        let job = OstJob {
            conditions: vec![cond(1, COUNT, [23, 0, 0], [0, -1, -1])],
            takeoffs: vec![tk(10, 1, &[5.0, 5.0]), tk(11, 1, &[6.0, 6.0])],
            zone_outlines: vec![ZoneOutline {
                zone: 7,
                page: 1,
                ring: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            }],
            negative_zones: HashSet::from([7]),
            ..Default::default()
        };
        let rows = bid_totals(&job);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].key.zone, None);
        assert!(near(rows[0].quantities[0], 0.0));
        assert_eq!(rows[1].key.zone, Some(7));
        assert!(near(rows[1].quantities[0], -2.0));
    }

    // Format notes, typical areas: area row = typical row times the area's copy count.
    #[test]
    fn typical_area_multiplier_and_job_total() {
        let job = OstJob {
            conditions: vec![cond(1, COUNT, [23, 0, 0], [0, -1, -1])],
            takeoffs: vec![OstTakeoff {
                typ_area: Some(3),
                ..tk(10, 1, &[1.0, 1.0])
            }],
            typ_area_counts: vec![
                TypAreaCount {
                    area: 20,
                    typ_area: 3,
                    count: 4.0,
                },
                TypAreaCount {
                    area: 21,
                    typ_area: 3,
                    count: 2.0,
                },
            ],
            ..Default::default()
        };
        let rows = bid_totals(&job);
        assert_eq!(rows.len(), 3);
        assert!(near(rows[1].quantities[0], 4.0) && rows[1].key.area == Some(20));
        assert!(near(rows[2].quantities[0], 2.0));
        assert!(near(condition_totals(&job, &rows)[&1][0], 6.0));
    }

    // Format notes, codes 25 and 26: one and both width sides, depth never enters.
    #[test]
    fn count_width_sides_and_shape_12() {
        let mut c = cond(1, COUNT, [25, 26, 13], [5, 5, 2]);
        (c.width, c.height, c.depth, c.shape) = (36.0, 84.0, 99.0, 12);
        let job = OstJob {
            conditions: vec![c],
            takeoffs: vec![tk(10, 1, &[0.0, 0.0])],
            ..Default::default()
        };
        let q = bid_totals(&job)[0].quantities;
        assert!(near(q[0], 3024.0) && near(q[1], 6048.0));
        assert!(near(q[2], 4.0 * 36.0 / 3f64.sqrt()));
    }

    #[test]
    fn units() {
        assert_eq!(display_unit(1, 2), ("LF", 12.0));
        assert_eq!(display_unit(11, 6), ("SY", 1296.0));
        assert_eq!(display_unit(24, 1), ("LF", 12.0));
        assert_eq!(display_unit(21, 9), ("CY", 46656.0));
        assert_eq!(display_unit(23, 0), ("EA", 1.0));
    }
}
