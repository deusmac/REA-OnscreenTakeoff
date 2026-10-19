//! Phase 1 data model (plan section 3). Plain data only: no I/O, no math.
//!
//! Lengths are full-precision `f64` inches (imperial) or millimeters (metric); rounding is
//! display only (OST-01-041). Takeoff geometry is in PDF points so a scale change
//! recomputes quantities without moving shapes.

use serde::{Deserialize, Serialize};

pub type Id = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measure {
    Imperial,
    Metric,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub bid_no: String,
    pub job_no: String,
    pub estimator: String,
    pub notes: String,
    pub measure: Measure,
    /// Creation time, ISO 8601 text.
    pub created: String,
    /// Id of the project this one was copied from, if any.
    pub origin_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleStyle {
    Architectural,
    Civil,
    Metric,
    Custom,
}

/// One scale per page (OST-06-003). `real_per_page_unit` is real-world length (inches or
/// millimeters per the project measure) per one PDF point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scale {
    pub real_per_page_unit: f64,
    pub style: ScaleStyle,
    pub label: String,
}

/// A plan page. The PDF is linked by `file_path`, never copied in (OST-03-009).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: Id,
    pub project_id: Id,
    pub index: u32,
    pub sheet_no: String,
    pub sheet_name: String,
    pub file_path: String,
    pub file_page_index: u32,
    /// Page size in PDF points.
    pub width: f64,
    pub height: f64,
    pub scale: Option<Scale>,
    /// Degrees clockwise: 0, 90, 180 or 270.
    pub rotation: u16,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionStyle {
    Linear,
    Area,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CountShape {
    Square,
    Circle,
    Triangle,
    Diamond,
    Ellipse,
    Hexagon,
    Octagon,
    Rectangle,
    DoorRight,
    DoorLeft,
    DoubleDoor,
    DoubleEgress,
    Window,
}

/// Result quantities a condition can report (spec sections 7.2 to 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultKind {
    // Linear
    Length,
    SegmentCount,
    SurfaceOneSide,
    SurfaceBothSides,
    SurfaceTopOrBottom,
    SurfaceTopAndBottom,
    SurfaceOneEnd,
    SurfaceBothEnds,
    SurfaceAllSides,
    // Area
    Area,
    AreaIgnoringBackouts,
    AreaMinusAttachments,
    Perimeter,
    GridLength,
    TileCountAverage,
    TileCountVisible,
    AreaCount,
    // Linear and area
    Volume,
    // Count
    Count,
    TotalHeight,
    CountPerimeter,
    CountTopOrBottom,
    CountTopAndBottom,
    CountOneWidthSide,
    CountBothWidthSides,
    CountOneDepthSide,
    CountBothDepthSides,
    CountAllSides,
    CountAllSidesTopBottom,
    CountVolume,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub id: Id,
    pub project_id: Id,
    /// Condition No., unique within a project.
    pub number: i64,
    pub name: String,
    pub style: ConditionStyle,
    /// Grouping tag; blank shows as Unassigned.
    pub ctype: String,
    pub layer: String,
    pub color: [u8; 3],
    pub pattern: String,
    pub height: f64,
    pub thickness: f64,
    pub width: f64,
    pub depth: f64,
    /// Rise and run.
    pub slope: Option<(f64, f64)>,
    pub count_shape: Option<CountShape>,
    /// Up to three result quantities; Q1 first (OST-05-014).
    pub results: [Option<ResultKind>; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Shape in PDF points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    Polyline(Vec<Point>),
    Polygon {
        outer: Vec<Point>,
        backouts: Vec<Vec<Point>>,
    },
    Point(Point),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Takeoff {
    pub id: Id,
    pub page_id: Id,
    pub condition_id: Id,
    pub bid_area_id: Option<Id>,
    pub negative: bool,
    pub geometry: Geometry,
}

/// Flat in Phase 1; `parent_id` is kept for the Phase 2 tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidArea {
    pub id: Id,
    pub project_id: Id,
    pub name: String,
    pub parent_id: Option<Id>,
    pub order: i64,
}
