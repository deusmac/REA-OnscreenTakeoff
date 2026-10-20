//! Mouse and keyboard tools on the plan: select and edit, draw linear, area and count
//! takeoffs, and pick two points to calibrate the scale. Left mouse only; panning stays on
//! the right and middle buttons and the wheel.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use rea_core::model::{ConditionStyle, Geometry, Id, Point};
use rea_core::units::format_feet_inches;

use crate::doc::Doc;
use crate::geom;
use crate::viewer::Viewer;

/// Screen pixels within which a click hits a line, a point or a vertex.
const HIT_PX: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Draw(Id),
    Calibrate,
}

/// Keys pressed this frame while no text box has focus.
#[derive(Debug, Default, Clone, Copy)]
pub struct Keys {
    pub enter: bool,
    pub escape: bool,
    pub backspace: bool,
    pub delete: bool,
}

pub struct Tools {
    pub tool: Tool,
    /// Points of the shape being drawn, or the calibration points.
    pub points: Vec<Point>,
    pub selected: Option<Id>,
    /// Takeoff and vertex being dragged.
    drag: Option<(Id, usize)>,
    /// Shown in the status bar until the next action.
    pub message: Option<String>,
    /// Two calibration points picked; the app asks for the real length.
    pub calibrated: Option<(Point, Point)>,
    /// Running length or area of the shape in progress, for the status bar.
    pub running_text: Option<String>,
}

impl Default for Tools {
    fn default() -> Self {
        Tools {
            tool: Tool::Select,
            points: Vec::new(),
            selected: None,
            drag: None,
            message: None,
            calibrated: None,
            running_text: None,
        }
    }
}

impl Tools {
    pub fn set(&mut self, tool: Tool) {
        self.tool = tool;
        self.points.clear();
        self.drag = None;
        if tool != Tool::Select {
            self.selected = None;
        }
    }

    /// Drops work in progress, for example on a page change.
    pub fn reset(&mut self) {
        self.points.clear();
        self.drag = None;
        self.selected = None;
    }

    /// The style being drawn, if the draw tool is armed with a live condition.
    fn drawing(&self, doc: &Doc) -> Option<(Id, ConditionStyle)> {
        match self.tool {
            Tool::Draw(id) => doc.condition(id).map(|c| (id, c.style)),
            _ => None,
        }
    }

    /// Ends the shape in progress and stores it.
    fn finish(&mut self, doc: &mut Doc, page: usize) {
        let Some((cid, style)) = self.drawing(doc) else {
            return;
        };
        let pts = std::mem::take(&mut self.points);
        let geometry = match style {
            ConditionStyle::Linear if pts.len() >= 2 => Geometry::Polyline(pts),
            ConditionStyle::Area if pts.len() >= 3 => Geometry::Polygon {
                outer: pts,
                backouts: Vec::new(),
            },
            ConditionStyle::Area if !pts.is_empty() => {
                self.message = Some("An area needs at least 3 points.".into());
                return;
            }
            _ => return,
        };
        if let Err(e) = doc.add_takeoff(page, cid, geometry) {
            self.message = Some(e);
        }
    }

    pub fn handle(
        &mut self,
        doc: &mut Doc,
        viewer: &Viewer,
        view: Rect,
        resp: &egui::Response,
        keys: Keys,
    ) {
        let page = viewer.page;
        let tol = HIT_PX / viewer.zoom as f64;
        let at = |p: Pos2| viewer.to_page(view, p);
        let click = resp
            .clicked_by(egui::PointerButton::Primary)
            .then(|| resp.interact_pointer_pos())
            .flatten()
            .map(at);
        let double = resp.double_clicked_by(egui::PointerButton::Primary);
        if self.tool != Tool::Select && self.drawing(doc).is_none() && self.tool != Tool::Calibrate
        {
            self.set(Tool::Select); // the armed condition was deleted
        }

        match self.tool {
            Tool::Draw(cid) => {
                let style = self.drawing(doc).map(|d| d.1);
                if double {
                    self.finish(doc, page);
                } else if let Some(p) = click {
                    self.message = None;
                    if let Err(e) = doc.can_draw(page) {
                        self.message = Some(e);
                    } else if style == Some(ConditionStyle::Count) {
                        if let Err(e) = doc.add_takeoff(page, cid, Geometry::Point(p)) {
                            self.message = Some(e);
                        }
                    } else if self
                        .points
                        .last()
                        .is_none_or(|q| (q.x - p.x).hypot(q.y - p.y) > tol)
                    {
                        self.points.push(p);
                    }
                }
                if keys.enter {
                    self.finish(doc, page);
                }
                if keys.backspace {
                    self.points.pop();
                }
                if keys.escape {
                    if self.points.is_empty() {
                        self.set(Tool::Select);
                    } else {
                        self.points.clear();
                    }
                }
            }
            Tool::Calibrate => {
                if let Some(p) = click {
                    self.points.push(p);
                    if let [a, b] = self.points[..] {
                        self.calibrated = Some((a, b));
                        self.set(Tool::Select);
                    }
                }
                if keys.escape {
                    self.set(Tool::Select);
                }
            }
            Tool::Select => {
                let page_id = doc.pages.get(page).map(|p| p.id);
                if resp.drag_started_by(egui::PointerButton::Primary) {
                    let origin = resp.ctx.input(|i| i.pointer.press_origin()).map(at);
                    let hit = origin.and_then(|o| {
                        let t = doc.takeoffs.iter().find(|t| Some(t.id) == self.selected)?;
                        Some((t.id, geom::vertex_at(&t.geometry, o, tol)?))
                    });
                    if let Some(h) = hit {
                        doc.checkpoint();
                        self.drag = Some(h);
                    }
                }
                if let Some((id, i)) = self.drag {
                    if let Some(p) = resp.interact_pointer_pos().map(at) {
                        if let Some(t) = doc.takeoff_mut(id) {
                            geom::move_vertex(&mut t.geometry, i, p);
                            doc.touch();
                        }
                    }
                    if resp.drag_stopped() {
                        self.drag = None;
                    }
                }
                if let Some(p) = click {
                    self.selected = geom::pick(
                        doc.takeoffs
                            .iter()
                            .filter(|t| Some(t.page_id) == page_id)
                            .map(|t| (t.id, &t.geometry)),
                        p,
                        tol,
                    );
                }
                if keys.delete {
                    if let Some(id) = self.selected.take() {
                        doc.delete_takeoff(id);
                    }
                }
                if keys.escape {
                    self.selected = None;
                }
            }
        }
    }

    /// The shape in progress with a rubber band to the cursor.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        viewer: &Viewer,
        view: Rect,
        doc: &Doc,
        hover: Option<Pos2>,
    ) {
        if self.points.is_empty() {
            return;
        }
        let color = match self.tool {
            Tool::Draw(id) => doc.condition(id).map_or(Color32::RED, |c| {
                Color32::from_rgb(c.color[0], c.color[1], c.color[2])
            }),
            _ => Color32::from_rgb(255, 0, 255),
        };
        let mut line: Vec<Pos2> = self
            .points
            .iter()
            .map(|p| viewer.to_screen(view, *p))
            .collect();
        line.extend(hover);
        let closing =
            matches!(self.drawing(doc), Some((_, ConditionStyle::Area))) && line.len() > 2;
        if closing {
            painter.line_segment(
                [line[line.len() - 1], line[0]],
                Stroke::new(1.0, color.gamma_multiply(0.6)),
            );
        }
        for p in &line[..self.points.len()] {
            painter.circle_filled(*p, 3.0, color);
        }
        painter.line(line, Stroke::new(2.0, color));
    }

    /// Running length, area or calibration distance for the status bar.
    pub fn running(&self, doc: &Doc, page: usize, hover: Option<Point>) -> Option<String> {
        if self.points.is_empty() {
            return None;
        }
        let mut pts = self.points.clone();
        pts.extend(hover);
        let scale = doc
            .pages
            .get(page)?
            .scale
            .as_ref()
            .map(|s| s.real_per_page_unit);
        match (self.tool, scale) {
            (Tool::Calibrate, _) => Some(format!(
                "Calibrate: click the second point ({:.1} pt so far)",
                geom::polyline_len(&pts)
            )),
            (Tool::Draw(_), Some(s)) => match self.drawing(doc)?.1 {
                ConditionStyle::Linear => Some(format!(
                    "Length {}",
                    format_feet_inches(geom::polyline_len(&pts) * s, 0)
                )),
                ConditionStyle::Area => Some(format!(
                    "Area {:.2} SF   Perimeter {}",
                    geom::ring_area(&pts) * s * s / 144.0,
                    format_feet_inches(
                        (geom::polyline_len(&pts)
                            + pts
                                .first()
                                .zip(pts.last())
                                .map_or(0.0, |(a, b)| { (a.x - b.x).hypot(a.y - b.y) }))
                            * s,
                        0
                    )
                )),
                ConditionStyle::Count => None,
            },
            _ => None,
        }
    }
}
