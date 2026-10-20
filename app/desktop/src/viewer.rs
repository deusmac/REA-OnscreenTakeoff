//! Plan viewer: the page image (a low-detail whole page plus a sharp render of the visible
//! area), zoom and pan, and the takeoffs drawn over it.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use rea_core::model::{Geometry, Id, Page, Point};
use rea_pdf::{Image, PlanFile};

use crate::doc::Doc;

/// Longest side, in pixels, of the low-detail whole-page image shown while zooming and panning.
const BASE_PX: f32 = 2048.0;
/// The sharp image of the visible area is rendered once input has been quiet this long.
const SETTLE: Duration = Duration::from_millis(150);
const ZOOM_MIN: f32 = 0.02;
const ZOOM_MAX: f32 = 40.0;
pub const SELECT_COLOR: Color32 = Color32::from_rgb(255, 200, 0);

pub struct Viewer {
    files: Vec<PlanFile>,
    /// Per project page: index into `files`, `None` when its plan file did not open.
    page_file: Vec<Option<usize>>,
    /// Per project page, in PDF points.
    sizes: Vec<Vec2>,
    /// Plan files that could not be opened.
    pub missing: Vec<String>,
    /// Index into the project's pages.
    pub page: usize,
    /// Screen points per PDF point.
    pub zoom: f32,
    /// Where the page's top-left corner sits, relative to the viewer's top-left corner.
    pub offset: Vec2,
    pub fit_pending: bool,
    pub last_input: Instant,
    base: Option<Shown>,
    detail: Option<Shown>,
    pub last_render_ms: u128,
}

/// A rendered image on the GPU and the part of the page it covers.
struct Shown {
    tex: egui::TextureHandle,
    page: usize,
    /// Pixels per PDF point it was rendered at.
    scale: f32,
    /// Covered area in PDF points.
    area: Rect,
}

impl Viewer {
    /// Opens each plan file once; a page whose file is missing shows blank with its takeoffs.
    pub fn new(pages: &[Page]) -> Viewer {
        let mut files = Vec::new();
        let mut by_path: HashMap<&str, Option<usize>> = HashMap::new();
        let mut missing = Vec::new();
        let page_file = pages
            .iter()
            .map(|p| {
                *by_path.entry(&p.file_path).or_insert_with(|| {
                    let opened = if crate::project::local_plan_path(&p.file_path) {
                        PlanFile::open(Path::new(&p.file_path)).ok()
                    } else {
                        None
                    };
                    match opened.ok_or(()) {
                        Ok(f) => {
                            files.push(f);
                            Some(files.len() - 1)
                        }
                        Err(_) => {
                            missing.push(p.file_path.clone());
                            None
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        let sizes = pages
            .iter()
            .zip(&page_file)
            .map(|(p, f)| {
                f.and_then(|f| files[f].page_size(p.file_page_index as usize))
                    .map_or(Vec2::new(p.width as f32, p.height as f32), |(w, h)| {
                        Vec2::new(w, h)
                    })
            })
            .collect();
        Viewer {
            files,
            page_file,
            sizes,
            missing,
            page: 0,
            zoom: 1.0,
            offset: Vec2::ZERO,
            fit_pending: true,
            last_input: Instant::now(),
            base: None,
            detail: None,
            last_render_ms: 0,
        }
    }

    pub fn page_size(&self) -> Vec2 {
        self.sizes
            .get(self.page)
            .copied()
            .unwrap_or(Vec2::splat(1.0))
    }

    pub fn go_to(&mut self, page: usize) {
        if page < self.sizes.len() && page != self.page {
            self.page = page;
            self.fit_pending = true;
        }
    }

    pub fn fit(&mut self, view: Rect) {
        let size = self.page_size();
        self.zoom = (view.width() / size.x).min(view.height() / size.y) * 0.98;
        self.offset = (view.size() - size * self.zoom) / 2.0;
    }

    /// Zooms by `factor`, keeping the page point under `anchor` (a screen position) still.
    pub fn zoom_at(&mut self, view: Rect, anchor: Pos2, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let on_page = (anchor - view.min - self.offset) / self.zoom;
        self.offset = anchor - view.min - on_page * new_zoom;
        self.zoom = new_zoom;
    }

    pub fn to_screen(&self, view: Rect, p: Point) -> Pos2 {
        view.min + self.offset + Vec2::new(p.x as f32, p.y as f32) * self.zoom
    }

    pub fn to_page(&self, view: Rect, s: Pos2) -> Point {
        let v = (s - view.min - self.offset) / self.zoom;
        Point {
            x: v.x as f64,
            y: v.y as f64,
        }
    }

    fn upload(ctx: &egui::Context, name: &str, img: Image) -> egui::TextureHandle {
        let color = egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba);
        ctx.load_texture(name, color, egui::TextureOptions::LINEAR)
    }

    /// Renders whatever is missing: the base image after a page change, and the sharp image of
    /// the visible area once zoom and pan have settled.
    pub fn refresh(
        &mut self,
        ctx: &egui::Context,
        view: Rect,
        ppp: f32,
        file_page: usize,
    ) -> Result<(), rea_pdf::Error> {
        let Some(fi) = self.page_file.get(self.page).copied().flatten() else {
            return Ok(()); // no plan file: the page shows blank with its takeoffs
        };
        let size = self.page_size();
        if self.base.as_ref().is_none_or(|b| b.page != self.page) {
            let started = Instant::now();
            let scale = BASE_PX / size.x.max(size.y);
            let img = self.files[fi].render_page(file_page, scale)?;
            self.base = Some(Shown {
                tex: Self::upload(ctx, "base", img),
                page: self.page,
                scale,
                area: Rect::from_min_size(Pos2::ZERO, size),
            });
            self.detail = None;
            self.last_render_ms = started.elapsed().as_millis();
        }

        let scale = self.zoom * ppp;
        let base_scale = self.base.as_ref().map_or(0.0, |b| b.scale);
        let visible = Rect::from_min_size(
            Pos2::ZERO - self.offset / self.zoom,
            view.size() / self.zoom,
        )
        .intersect(Rect::from_min_size(Pos2::ZERO, size));
        let up_to_date = self.detail.as_ref().is_some_and(|d| {
            d.page == self.page
                && (d.scale - scale).abs() < scale * 0.01
                && d.area.contains_rect(visible)
        });
        if scale <= base_scale * 1.1 || up_to_date || !visible.is_positive() {
            return Ok(());
        }
        if self.last_input.elapsed() < SETTLE {
            ctx.request_repaint_after(SETTLE);
            return Ok(());
        }
        let started = Instant::now();
        let px = Rect::from_min_max((visible.min * scale).floor(), (visible.max * scale).ceil());
        let img = self.files[fi].render_region(
            file_page,
            scale,
            px.min.x as i32,
            px.min.y as i32,
            px.width().max(1.0) as i32,
            px.height().max(1.0) as i32,
        )?;
        self.detail = Some(Shown {
            tex: Self::upload(ctx, "detail", img),
            page: self.page,
            scale,
            area: Rect::from_min_max(px.min / scale, px.max / scale),
        });
        self.last_render_ms = started.elapsed().as_millis();
        Ok(())
    }

    pub fn paint_plan(&self, painter: &egui::Painter, view: Rect) {
        let origin = view.min + self.offset;
        let to_screen = |r: Rect| {
            Rect::from_min_max(
                origin + r.min.to_vec2() * self.zoom,
                origin + r.max.to_vec2() * self.zoom,
            )
        };
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let page = to_screen(Rect::from_min_size(Pos2::ZERO, self.page_size()));
        painter.rect_filled(
            page.translate(Vec2::splat(3.0)),
            0.0,
            Color32::from_black_alpha(60),
        );
        painter.rect_filled(page, 0.0, Color32::WHITE);
        if let Some(base) = self.base.as_ref().filter(|b| b.page == self.page) {
            painter.image(base.tex.id(), to_screen(base.area), uv, Color32::WHITE);
        }
        // A sharp image from an older zoom would sit at the wrong size; skip it.
        let scale = self.zoom * painter.ctx().pixels_per_point();
        if let Some(detail) = self
            .detail
            .as_ref()
            .filter(|d| d.page == self.page && (d.scale - scale).abs() < scale * 0.01)
        {
            painter.image(detail.tex.id(), to_screen(detail.area), uv, Color32::WHITE);
        }
    }

    /// The page's takeoffs in their condition colors; the selected one highlighted with its
    /// vertex handles.
    pub fn paint_takeoffs(
        &self,
        painter: &egui::Painter,
        view: Rect,
        doc: &Doc,
        selected: Option<Id>,
    ) {
        let Some(page_id) = doc.pages.get(self.page).map(|p| p.id) else {
            return;
        };
        let colors: HashMap<Id, Color32> = doc
            .conditions
            .iter()
            .map(|c| (c.id, Color32::from_rgb(c.color[0], c.color[1], c.color[2])))
            .collect();
        let pos = |p: &Point| self.to_screen(view, *p);
        let mut chosen = None;
        for t in doc.takeoffs.iter().filter(|t| t.page_id == page_id) {
            let color = colors.get(&t.condition_id).copied().unwrap_or(Color32::RED);
            if Some(t.id) == selected {
                chosen = Some(t);
            }
            shape(painter, &t.geometry, pos, Stroke::new(2.0, color));
        }
        if let Some(t) = chosen {
            shape(painter, &t.geometry, pos, Stroke::new(4.0, SELECT_COLOR));
            for v in crate::geom::vertices(&t.geometry) {
                let r = Rect::from_center_size(pos(v), Vec2::splat(8.0));
                painter.rect_filled(r, 0.0, Color32::WHITE);
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(1.5, Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

fn shape(painter: &egui::Painter, g: &Geometry, pos: impl Fn(&Point) -> Pos2, stroke: Stroke) {
    match g {
        Geometry::Polyline(v) => {
            painter.line(v.iter().map(pos).collect(), stroke);
        }
        Geometry::Polygon { outer, .. } => {
            let mut ring: Vec<Pos2> = outer.iter().map(pos).collect();
            if let Some(first) = ring.first().copied() {
                ring.push(first);
            }
            painter.line(ring, stroke);
        }
        Geometry::Point(p) => {
            painter.circle_filled(pos(p), stroke.width + 3.0, stroke.color);
        }
    }
}
