//! The open project: plain model data, every edit the UI makes, undo and redo, and the
//! quantity totals (through `rea_core::ost_totals`, the same path the OST parity uses).

use std::collections::{HashMap, HashSet};

use rea_core::model::{
    BidArea, Condition, ConditionStyle, CountShape, Geometry, Id, Page, Project, ResultKind, Scale,
    Takeoff,
};
use rea_core::ost_totals::{
    OstCondition, OstJob, OstTakeoff, TypAreaCount, ZoneOutline, AREA, COUNT, LINEAR,
};

use crate::quantities::{self, SummaryRow};

const UNDO_LIMIT: usize = 200;

/// Result choices per style: model kind, OST result code, label.
pub const RESULTS: &[(ConditionStyle, ResultKind, u32, &str)] = &[
    (ConditionStyle::Linear, ResultKind::Length, 1, "Length"),
    (
        ConditionStyle::Linear,
        ResultKind::SegmentCount,
        2,
        "Segments",
    ),
    (
        ConditionStyle::Linear,
        ResultKind::SurfaceOneSide,
        3,
        "Wall area one side",
    ),
    (
        ConditionStyle::Linear,
        ResultKind::SurfaceBothSides,
        4,
        "Wall area both sides",
    ),
    (
        ConditionStyle::Linear,
        ResultKind::SurfaceTopOrBottom,
        5,
        "Top or bottom",
    ),
    (
        ConditionStyle::Linear,
        ResultKind::SurfaceTopAndBottom,
        6,
        "Top and bottom",
    ),
    (
        ConditionStyle::Linear,
        ResultKind::SurfaceOneEnd,
        7,
        "One end",
    ),
    (
        ConditionStyle::Linear,
        ResultKind::SurfaceBothEnds,
        8,
        "Both ends",
    ),
    (
        ConditionStyle::Linear,
        ResultKind::SurfaceAllSides,
        9,
        "All sides",
    ),
    (ConditionStyle::Linear, ResultKind::Volume, 20, "Volume"),
    (ConditionStyle::Area, ResultKind::Area, 11, "Area"),
    (
        ConditionStyle::Area,
        ResultKind::AreaMinusAttachments,
        12,
        "Area minus attachments",
    ),
    (ConditionStyle::Area, ResultKind::Perimeter, 15, "Perimeter"),
    (
        ConditionStyle::Area,
        ResultKind::GridLength,
        16,
        "Grid length",
    ),
    (
        ConditionStyle::Area,
        ResultKind::TileCountAverage,
        17,
        "Tile count (average)",
    ),
    (
        ConditionStyle::Area,
        ResultKind::TileCountVisible,
        18,
        "Tile count (visible)",
    ),
    (
        ConditionStyle::Area,
        ResultKind::AreaCount,
        19,
        "Number of areas",
    ),
    (ConditionStyle::Area, ResultKind::Volume, 21, "Volume"),
    (
        ConditionStyle::Area,
        ResultKind::AreaIgnoringBackouts,
        34,
        "Area ignoring backouts",
    ),
    (ConditionStyle::Count, ResultKind::Count, 23, "Count"),
    (
        ConditionStyle::Count,
        ResultKind::TotalHeight,
        24,
        "Total height",
    ),
    (
        ConditionStyle::Count,
        ResultKind::CountTopOrBottom,
        5,
        "Top or bottom",
    ),
    (
        ConditionStyle::Count,
        ResultKind::CountPerimeter,
        13,
        "Perimeter",
    ),
    (
        ConditionStyle::Count,
        ResultKind::CountOneWidthSide,
        25,
        "One width side",
    ),
    (
        ConditionStyle::Count,
        ResultKind::CountBothWidthSides,
        26,
        "Both width sides",
    ),
    (ConditionStyle::Count, ResultKind::CountVolume, 20, "Volume"),
];

fn result_code(style: ConditionStyle, k: ResultKind) -> Option<u32> {
    RESULTS
        .iter()
        .find(|r| r.0 == style && r.1 == k)
        .map(|r| r.2)
}

/// The model result for an OST result code, when we have one.
pub fn result_from_code(style: ConditionStyle, code: u32) -> Option<ResultKind> {
    RESULTS
        .iter()
        .find(|r| r.0 == style && r.2 == code)
        .map(|r| r.1)
}

pub fn result_label(style: ConditionStyle, k: ResultKind) -> &'static str {
    RESULTS
        .iter()
        .find(|r| r.0 == style && r.1 == k)
        .map_or("?", |r| r.3)
}

/// Results a new condition starts with.
pub fn default_results(style: ConditionStyle) -> [Option<ResultKind>; 3] {
    match style {
        ConditionStyle::Linear => [
            Some(ResultKind::Length),
            Some(ResultKind::SurfaceOneSide),
            None,
        ],
        ConditionStyle::Area => [Some(ResultKind::Area), Some(ResultKind::Perimeter), None],
        ConditionStyle::Count => [Some(ResultKind::Count), None, None],
    }
}

const PALETTE: [[u8; 3]; 8] = [
    [220, 50, 47],
    [38, 139, 210],
    [133, 153, 0],
    [211, 54, 130],
    [203, 75, 22],
    [42, 161, 152],
    [108, 113, 196],
    [181, 137, 0],
];

/// OST fields of an imported job. While a takeoff's geometry and a condition's results are
/// what was imported, totals use OST's stored positions and codes, so an opened OST job
/// shows OST's own numbers; after an edit they come from our geometry.
#[derive(Debug, Clone, Default)]
pub struct OstSource {
    pub conditions: HashMap<Id, ([Option<ResultKind>; 3], OstCondition)>,
    pub takeoffs: HashMap<Id, (Geometry, OstTakeoff)>,
    pub zone_outlines: Vec<ZoneOutline>,
    pub negative_zones: HashSet<Id>,
    pub typ_area_counts: Vec<TypAreaCount>,
}

#[derive(Clone)]
struct Snapshot {
    pages: Vec<Page>,
    conditions: Vec<Condition>,
    takeoffs: Vec<Takeoff>,
}

pub struct Doc {
    pub project: Project,
    pub pages: Vec<Page>,
    pub conditions: Vec<Condition>,
    pub takeoffs: Vec<Takeoff>,
    pub bid_areas: Vec<BidArea>,
    pub ost: OstSource,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Bumped on every change; totals are recomputed when it moves.
    pub rev: u64,
    pub saved_rev: u64,
}

impl Doc {
    pub fn new(project: Project, pages: Vec<Page>) -> Doc {
        Doc {
            project,
            pages,
            conditions: Vec::new(),
            takeoffs: Vec::new(),
            bid_areas: Vec::new(),
            ost: OstSource::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            rev: 0,
            saved_rev: 0,
        }
    }

    pub fn modified(&self) -> bool {
        self.rev != self.saved_rev
    }

    fn next_id(&self) -> Id {
        let ids = self.pages.iter().map(|p| p.id);
        let ids = ids.chain(self.conditions.iter().map(|c| c.id));
        let ids = ids.chain(self.takeoffs.iter().map(|t| t.id));
        let ids = ids.chain(self.bid_areas.iter().map(|a| a.id));
        ids.chain([self.project.id]).max().unwrap_or(0) + 1
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            pages: self.pages.clone(),
            conditions: self.conditions.clone(),
            takeoffs: self.takeoffs.clone(),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.pages = s.pages;
        self.conditions = s.conditions;
        self.takeoffs = s.takeoffs;
        self.rev += 1;
    }

    /// Records the state before an edit. Every edit calls it first.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.rev += 1;
    }

    /// Marks a change inside an edit already checkpointed (a vertex drag in progress).
    pub fn touch(&mut self) {
        self.rev += 1;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(s);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(s);
        true
    }

    /// Rewrites plan file paths everywhere, history included (plans moved on Save As).
    pub fn relink(&mut self, moved: &HashMap<String, String>) {
        let fix = |pages: &mut Vec<Page>| {
            for p in pages {
                if let Some(n) = moved.get(&p.file_path) {
                    p.file_path = n.clone();
                }
            }
        };
        fix(&mut self.pages);
        self.undo.iter_mut().for_each(|s| fix(&mut s.pages));
        self.redo.iter_mut().for_each(|s| fix(&mut s.pages));
    }

    pub fn condition(&self, id: Id) -> Option<&Condition> {
        self.conditions.iter().find(|c| c.id == id)
    }

    pub fn takeoff_mut(&mut self, id: Id) -> Option<&mut Takeoff> {
        self.takeoffs.iter_mut().find(|t| t.id == id)
    }

    /// Takeoff is only possible on a page with a scale.
    pub fn can_draw(&self, page: usize) -> Result<(), String> {
        match self.pages.get(page) {
            None => Err("No page is open.".into()),
            Some(p) if p.scale.is_none() => Err(
                "This page has no scale. Pick a scale or use Calibrate on the toolbar first."
                    .into(),
            ),
            Some(_) => Ok(()),
        }
    }

    pub fn set_scale(&mut self, pages: &[usize], scale: Scale) {
        self.checkpoint();
        for &i in pages {
            if let Some(p) = self.pages.get_mut(i) {
                p.scale = Some(scale.clone());
            }
        }
    }

    pub fn add_takeoff(
        &mut self,
        page: usize,
        condition_id: Id,
        geometry: Geometry,
    ) -> Result<Id, String> {
        self.can_draw(page)?;
        let id = self.next_id();
        let page_id = self.pages[page].id;
        self.checkpoint();
        self.takeoffs.push(Takeoff {
            id,
            page_id,
            condition_id,
            bid_area_id: None,
            negative: false,
            geometry,
        });
        Ok(id)
    }

    pub fn delete_takeoff(&mut self, id: Id) {
        if self.takeoffs.iter().any(|t| t.id == id) {
            self.checkpoint();
            self.takeoffs.retain(|t| t.id != id);
        }
    }

    pub fn new_condition(&self, style: ConditionStyle) -> Condition {
        let number = self.conditions.iter().map(|c| c.number).max().unwrap_or(0) + 1;
        let style_name = match style {
            ConditionStyle::Linear => "Linear",
            ConditionStyle::Area => "Area",
            ConditionStyle::Count => "Count",
        };
        Condition {
            id: self.next_id(),
            project_id: self.project.id,
            number,
            name: format!("{style_name} {number}"),
            style,
            ctype: String::new(),
            layer: String::new(),
            color: PALETTE[number as usize % PALETTE.len()],
            pattern: String::new(),
            height: if style == ConditionStyle::Area {
                0.0
            } else {
                96.0
            },
            thickness: 0.0,
            width: if style == ConditionStyle::Count {
                12.0
            } else {
                0.0
            },
            depth: 0.0,
            slope: None,
            count_shape: (style == ConditionStyle::Count).then_some(CountShape::Square),
            results: default_results(style),
        }
    }

    /// Adds a new condition or replaces the one with the same id.
    pub fn put_condition(&mut self, c: Condition) {
        self.checkpoint();
        match self.conditions.iter_mut().find(|x| x.id == c.id) {
            Some(x) => *x = c,
            None => self.conditions.push(c),
        }
    }

    /// A copy with a new number, id and color.
    pub fn duplicate_condition(&self, id: Id) -> Option<Condition> {
        let src = self.condition(id)?;
        let mut c = self.new_condition(src.style);
        let (nid, number, color) = (c.id, c.number, c.color);
        c.clone_from(src);
        c.id = nid;
        c.number = number;
        c.color = color;
        c.name = format!("{} copy", src.name);
        Some(c)
    }

    pub fn takeoff_count(&self, condition: Id) -> usize {
        self.takeoffs
            .iter()
            .filter(|t| t.condition_id == condition)
            .count()
    }

    /// Deletes the condition and all its takeoffs on every page.
    pub fn delete_condition(&mut self, id: Id) {
        self.checkpoint();
        self.conditions.retain(|c| c.id != id);
        self.takeoffs.retain(|t| t.condition_id != id);
    }

    /// Checks a condition before it is stored: name, unique number, finite sizes.
    pub fn validate_condition(&self, c: &Condition) -> Result<(), String> {
        if c.name.trim().is_empty() {
            return Err("Enter a name.".into());
        }
        if self
            .conditions
            .iter()
            .any(|x| x.id != c.id && x.number == c.number)
        {
            return Err(format!("Condition number {} is already used.", c.number));
        }
        if let Some((_, run)) = c.slope {
            if run == 0.0 {
                return Err("Slope run must not be zero.".into());
            }
        }
        if c.results[0].is_none() {
            return Err("Pick the first result.".into());
        }
        Ok(())
    }

    /// The OST condition fields that drive totals.
    fn ost_condition(&self, c: &Condition) -> OstCondition {
        let (rise, run) = c.slope.unwrap_or((0.0, 0.0));
        let kind = match c.style {
            ConditionStyle::Linear => LINEAR,
            ConditionStyle::Area => AREA,
            ConditionStyle::Count => COUNT,
        };
        let mut o = match self.ost.conditions.get(&c.id) {
            Some((results, o)) if *results == c.results && style_of(o.kind) == c.style => o.clone(),
            _ => OstCondition {
                kind,
                shape: match c.count_shape {
                    Some(CountShape::Circle) => 1,
                    Some(CountShape::Rectangle) => 3,
                    _ => 0,
                },
                results: c
                    .results
                    .map(|r| r.and_then(|k| result_code(c.style, k)).unwrap_or(0)),
                // 0 is "the usual unit" for the result's dimension; -1 marks an unused slot.
                uoms: c.results.map(|r| if r.is_some() { 0 } else { -1 }),
                ..Default::default()
            },
        };
        o.uid = c.id;
        o.width = c.width;
        o.height = c.height;
        o.thickness = c.thickness;
        o.depth = c.depth;
        o.rise = rise;
        o.run = run;
        o
    }

    /// Everything `ost_totals` needs, positions in real inches.
    pub fn job(&self) -> OstJob {
        let scales: HashMap<Id, f64> = self
            .pages
            .iter()
            .filter_map(|p| Some((p.id, p.scale.as_ref()?.real_per_page_unit)))
            .collect();
        let mut takeoffs = Vec::with_capacity(self.takeoffs.len());
        for t in &self.takeoffs {
            let tpl = self.ost.takeoffs.get(&t.id);
            if let Some((_, o)) = tpl.filter(|(g, _)| *g == t.geometry) {
                takeoffs.push(OstTakeoff {
                    condition: t.condition_id,
                    negative: t.negative,
                    ..o.clone()
                });
                continue;
            }
            let Some(&s) = scales.get(&t.page_id) else {
                continue; // unscaled page: nothing to measure
            };
            let base = match tpl {
                Some((_, o)) => OstTakeoff {
                    condition: t.condition_id,
                    negative: t.negative,
                    ..o.clone()
                },
                None => OstTakeoff {
                    uid: t.id,
                    page: t.page_id,
                    condition: t.condition_id,
                    area: t.bid_area_id,
                    negative: t.negative,
                    ..Default::default()
                },
            };
            match &t.geometry {
                // OST keeps one straight segment per linear takeoff: x1;y1;x2;y2;arc.
                Geometry::Polyline(v) => {
                    for w in v.windows(2) {
                        takeoffs.push(OstTakeoff {
                            position: vec![w[0].x * s, w[0].y * s, w[1].x * s, w[1].y * s, 0.0],
                            ..base.clone()
                        });
                    }
                }
                // shortcut: backout rings drawn in our geometry are not sent; we do not
                // draw backouts yet (OST backouts arrive as their own takeoffs).
                Geometry::Polygon { outer, .. } => takeoffs.push(OstTakeoff {
                    position: outer.iter().flat_map(|p| [p.x * s, p.y * s]).collect(),
                    ..base
                }),
                Geometry::Point(p) => {
                    // An attached count keeps its distance and side in front of the point.
                    let mut position = if base.position.len() == 4 {
                        base.position[..2].to_vec()
                    } else {
                        Vec::new()
                    };
                    position.extend([p.x * s, p.y * s]);
                    takeoffs.push(OstTakeoff { position, ..base });
                }
            }
        }
        OstJob {
            conditions: self
                .conditions
                .iter()
                .map(|c| self.ost_condition(c))
                .collect(),
            takeoffs,
            zone_outlines: self.ost.zone_outlines.clone(),
            negative_zones: self.ost.negative_zones.clone(),
            typ_area_counts: self.ost.typ_area_counts.clone(),
        }
    }

    pub fn summary(&self) -> Vec<SummaryRow> {
        quantities::summary(&self.conditions, &self.job())
    }
}

fn style_of(kind: i64) -> ConditionStyle {
    match kind {
        LINEAR => ConditionStyle::Linear,
        AREA => ConditionStyle::Area,
        _ => ConditionStyle::Count,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use rea_core::model::{Measure, Point, ScaleStyle};

    pub fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// A two-page project; page 0 at 1/4" = 1'-0" (48 real inches per paper inch), page 1
    /// unscaled.
    pub fn sample() -> Doc {
        let page = |id: Id, index: u32, scale: Option<Scale>| Page {
            id,
            project_id: 1,
            index,
            sheet_no: format!("A{index}"),
            sheet_name: "Plan".into(),
            file_path: "C:\\plans\\set.pdf".into(),
            file_page_index: index,
            width: 2592.0,
            height: 1728.0,
            scale,
            rotation: 0,
            complete: false,
        };
        let quarter = Scale {
            real_per_page_unit: 48.0 / 72.0,
            style: ScaleStyle::Architectural,
            label: "1/4\" = 1'-0\"".into(),
        };
        Doc::new(
            Project {
                id: 1,
                name: "Sample".into(),
                bid_no: String::new(),
                job_no: String::new(),
                estimator: String::new(),
                notes: String::new(),
                measure: Measure::Imperial,
                created: "2026-10-20".into(),
                origin_id: None,
            },
            vec![page(2, 0, Some(quarter)), page(3, 1, None)],
        )
    }

    /// One condition and one takeoff of each style on page 0.
    pub fn filled() -> Doc {
        let mut d = sample();
        for style in [
            ConditionStyle::Linear,
            ConditionStyle::Area,
            ConditionStyle::Count,
        ] {
            let c = d.new_condition(style);
            d.put_condition(c);
        }
        let ids: Vec<Id> = d.conditions.iter().map(|c| c.id).collect();
        // 72 pt at 48/72 in per pt = 48 in = 4 ft per paper inch.
        d.add_takeoff(
            0,
            ids[0],
            Geometry::Polyline(vec![pt(0.0, 0.0), pt(72.0, 0.0), pt(72.0, 36.0)]),
        )
        .unwrap();
        d.add_takeoff(
            0,
            ids[1],
            Geometry::Polygon {
                outer: vec![pt(0.0, 0.0), pt(72.0, 0.0), pt(72.0, 72.0), pt(0.0, 72.0)],
                backouts: vec![],
            },
        )
        .unwrap();
        d.add_takeoff(0, ids[2], Geometry::Point(pt(10.0, 10.0)))
            .unwrap();
        d.add_takeoff(0, ids[2], Geometry::Point(pt(20.0, 10.0)))
            .unwrap();
        d
    }

    fn q(rows: &[SummaryRow], i: usize, slot: usize) -> f64 {
        rows[i].qty[slot].unwrap().0
    }

    #[test]
    fn native_totals_per_style() {
        let rows = filled().summary();
        // Linear: 4 ft + 2 ft = 6 LF; wall one side at 8 ft high = 48 SF.
        assert!((q(&rows, 0, 0) - 6.0).abs() < 1e-9);
        assert!((q(&rows, 0, 1) - 48.0).abs() < 1e-9);
        assert!(rows[0].qty[2].is_none());
        // Area: 4 ft square = 16 SF, perimeter 16 LF.
        assert!((q(&rows, 1, 0) - 16.0).abs() < 1e-9);
        assert!((q(&rows, 1, 1) - 16.0).abs() < 1e-9);
        // Count: 2 EA.
        assert!((q(&rows, 2, 0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn unscaled_page_blocks_takeoff() {
        let mut d = filled();
        let c = d.conditions[0].id;
        let n = d.takeoffs.len();
        let err = d
            .add_takeoff(1, c, Geometry::Point(pt(1.0, 1.0)))
            .unwrap_err();
        assert!(err.contains("no scale"));
        assert_eq!(d.takeoffs.len(), n);
        assert!(d.can_draw(0).is_ok());
    }

    #[test]
    fn undo_redo_takeoffs_and_conditions() {
        let mut d = filled();
        let before = d.takeoffs.clone();
        let wall = d.conditions[0].id;
        d.delete_condition(wall);
        assert_eq!(d.conditions.len(), 2);
        assert_eq!(d.takeoffs.len(), before.len() - 1);
        assert!(d.undo());
        assert_eq!(d.conditions.len(), 3);
        assert_eq!(d.takeoffs, before);
        assert!(d.redo());
        assert_eq!(d.conditions.len(), 2);
        assert!(d.undo());
        // A new edit clears redo.
        let first = d.takeoffs[0].id;
        d.delete_takeoff(first);
        assert!(!d.can_redo());
        assert!(d.undo());
        assert_eq!(d.takeoffs, before);
        // Undo all the way back to an empty project.
        while d.undo() {}
        assert!(d.conditions.is_empty() && d.takeoffs.is_empty());
        assert!(d.modified());
    }

    #[test]
    fn vertex_drag_updates_totals_and_undoes() {
        let mut d = filled();
        let area = d.takeoffs[1].id;
        d.checkpoint();
        let t = d.takeoff_mut(area).unwrap();
        crate::geom::move_vertex(&mut t.geometry, 2, pt(144.0, 72.0));
        d.touch();
        // Trapezoid 0..72 top, 0..144 at y=72: (72 + 144) / 2 * 72 pt^2, at 48/72 in per pt.
        let sf = (72.0 + 144.0) / 2.0 * 72.0 * (48.0f64 / 72.0).powi(2) / 144.0;
        assert!((q(&d.summary(), 1, 0) - sf).abs() < 1e-9);
        d.undo();
        assert!((q(&d.summary(), 1, 0) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn duplicate_gets_new_number_and_id() {
        let mut d = filled();
        let c = d.duplicate_condition(d.conditions[0].id).unwrap();
        assert_eq!(c.number, 4);
        assert!(d.validate_condition(&c).is_ok());
        d.put_condition(c.clone());
        let mut clash = c;
        clash.number = 1;
        assert!(d.validate_condition(&clash).is_err());
    }
}
