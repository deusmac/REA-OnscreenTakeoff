// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod quantities;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use rea_core::model::{Geometry, Id};
use rea_pdf::{Image, PlanFile};

use quantities::SummaryRow;

/// Longest side, in pixels, of the low-detail whole-page image shown while zooming and panning.
const BASE_PX: f32 = 2048.0;
/// The sharp image of the visible area is rendered once input has been quiet this long.
const SETTLE: Duration = Duration::from_millis(150);
const ZOOM_MIN: f32 = 0.02;
const ZOOM_MAX: f32 = 40.0;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenTakeoff")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    // A PDF or OST package path on the command line opens straight away.
    let start = std::env::args_os().nth(1).map(PathBuf::from);
    eframe::run_native(
        "OpenTakeoff",
        options,
        Box::new(|_cc| {
            let mut app = TakeoffApp::default();
            if let Some(path) = start {
                app.open(&path);
            }
            Ok(Box::new(app))
        }),
    )
}

#[derive(Default)]
struct TakeoffApp {
    plan: Option<Plan>,
    error: Option<String>,
}

/// One page in the page list: where its image comes from and what is drawn on it.
struct Sheet {
    /// Index into `Plan::files`; `None` when the plan file is not available.
    file: Option<usize>,
    /// Page index inside that file.
    page: usize,
    label: String,
    /// Size in PDF points.
    size: Vec2,
    shapes: Vec<(Color32, Geometry)>,
}

struct Plan {
    files: Vec<PlanFile>,
    sheets: Vec<Sheet>,
    summary: Vec<SummaryRow>,
    name: String,
    /// Index into `sheets`.
    page: usize,
    /// Screen points per PDF point.
    zoom: f32,
    /// Where the page's top-left corner sits, relative to the viewer's top-left corner.
    offset: Vec2,
    fit_pending: bool,
    last_input: Instant,
    /// Whole page at low detail; always there once a page is shown.
    base: Option<Shown>,
    /// Visible area at full detail for the current zoom.
    detail: Option<Shown>,
    last_render_ms: u128,
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

type Opened = (Vec<PlanFile>, Vec<Sheet>, Vec<SummaryRow>);

impl TakeoffApp {
    fn open(&mut self, path: &Path) {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into());
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        let result = match ext.as_deref() {
            Some("osp" | "ost") => open_ost(path),
            _ => open_pdf(path),
        };
        match result {
            Ok((files, sheets, summary)) if !sheets.is_empty() => {
                self.plan = Some(Plan {
                    files,
                    sheets,
                    summary,
                    name,
                    page: 0,
                    zoom: 1.0,
                    offset: Vec2::ZERO,
                    fit_pending: true,
                    last_input: Instant::now(),
                    base: None,
                    detail: None,
                    last_render_ms: 0,
                });
                self.error = None;
            }
            Ok(_) => self.error = Some(format!("{} has no pages", path.display())),
            Err(e) => self.error = Some(format!("Could not open {}: {e}", path.display())),
        }
    }

    fn pick_and_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Plans and OST jobs", &["pdf", "osp", "ost"])
            .pick_file()
        {
            self.open(&path);
        }
    }
}

fn open_pdf(path: &Path) -> Result<Opened, String> {
    let file = PlanFile::open(path).map_err(|e| e.to_string())?;
    let sheets = (0..file.page_count())
        .map(|i| {
            let (w, h) = file.page_size(i).unwrap_or((1.0, 1.0));
            Sheet {
                file: Some(0),
                page: i,
                label: format!("Page {}   {:.0} x {:.0} in", i + 1, w / 72.0, h / 72.0),
                size: Vec2::new(w, h),
                shapes: Vec::new(),
            }
        })
        .collect();
    Ok((vec![file], sheets, Vec::new()))
}

/// Opens an OST package: plan PDFs are written to a temp folder (Pdfium reads from files),
/// pages follow OST's page order, and each page carries its takeoffs.
fn open_ost(path: &Path) -> Result<Opened, String> {
    let imp = rea_ost::import_path(path).map_err(|e| e.to_string())?;
    let stem = path
        .file_stem()
        .map_or_else(|| "job".into(), |s| s.to_string_lossy().into_owned());
    let dir = std::env::temp_dir().join("opentakeoff").join(stem);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    let mut by_archive: HashMap<String, usize> = HashMap::new();
    for (i, pf) in imp.plan_files.iter().enumerate() {
        if !pf.archive_path.to_ascii_lowercase().ends_with(".pdf") {
            continue; // shortcut: image plan pages (TIFF) show blank until we render images
        }
        let out = dir.join(format!("plan{i}.pdf"));
        std::fs::write(&out, &pf.bytes).map_err(|e| e.to_string())?;
        if let Ok(f) = PlanFile::open(&out) {
            by_archive.insert(pf.archive_path.to_ascii_lowercase(), files.len());
            files.push(f);
        }
    }

    let b = &imp.bid;
    let colors: HashMap<Id, Color32> = b
        .conditions
        .iter()
        .map(|c| (c.id, Color32::from_rgb(c.color[0], c.color[1], c.color[2])))
        .collect();
    let mut pages: Vec<_> = b.pages.iter().collect();
    pages.sort_by_key(|p| p.index);
    let sheets = pages
        .iter()
        .map(|p| {
            let file = by_archive
                .get(&rea_ost::plan_archive_path(&p.file_path).to_ascii_lowercase())
                .copied();
            let page = p.file_page_index as usize;
            let size = file
                .and_then(|f| files[f].page_size(page))
                .map_or(Vec2::new(p.width as f32, p.height as f32), |(w, h)| {
                    Vec2::new(w, h)
                });
            let title = format!("{} {}", p.sheet_no, p.sheet_name);
            let shapes = b
                .takeoffs
                .iter()
                .filter(|t| t.page_id == p.id)
                .map(|t| {
                    let c = colors.get(&t.condition_id).copied().unwrap_or(Color32::RED);
                    (c, t.geometry.clone())
                })
                .collect();
            Sheet {
                file,
                page,
                label: if title.trim().is_empty() {
                    format!("Page {}", p.index + 1)
                } else {
                    title.trim().to_string()
                },
                size,
                shapes,
            }
        })
        .collect();

    let parent: HashMap<Id, Id> = b
        .takeoff_raw
        .iter()
        .filter_map(|(id, raw)| {
            let p: Id = raw.extra.get("ParentUID")?.trim().parse().ok()?;
            (p != 0).then_some((*id, p))
        })
        .collect();
    let sagitta: HashMap<Id, f64> = b
        .takeoff_raw
        .iter()
        .filter_map(|(id, raw)| raw.arc.as_ref().map(|a| (*id, a.sagitta)))
        .collect();
    let summary = quantities::summary(&b.pages, &b.conditions, &b.takeoffs, &parent, &sagitta);
    Ok((files, sheets, summary))
}

impl Plan {
    fn sheet(&self) -> &Sheet {
        &self.sheets[self.page]
    }

    fn page_size(&self) -> Vec2 {
        self.sheet().size
    }

    fn go_to(&mut self, page: usize) {
        if page < self.sheets.len() && page != self.page {
            self.page = page;
            self.fit_pending = true;
        }
    }

    fn fit(&mut self, view: Rect) {
        let size = self.page_size();
        self.zoom = (view.width() / size.x).min(view.height() / size.y) * 0.98;
        self.offset = (view.size() - size * self.zoom) / 2.0;
    }

    /// Zooms by `factor`, keeping the page point under `anchor` (a screen position) still.
    fn zoom_at(&mut self, view: Rect, anchor: Pos2, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let on_page = (anchor - view.min - self.offset) / self.zoom;
        self.offset = anchor - view.min - on_page * new_zoom;
        self.zoom = new_zoom;
    }

    fn upload(&mut self, ctx: &egui::Context, name: &str, img: Image) -> egui::TextureHandle {
        let color = egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba);
        ctx.load_texture(name, color, egui::TextureOptions::LINEAR)
    }

    /// Renders whatever is missing: the base image after a page change, and the sharp image of
    /// the visible area once zoom and pan have settled.
    fn refresh(&mut self, ctx: &egui::Context, view: Rect, ppp: f32) -> Result<(), rea_pdf::Error> {
        let Some(fi) = self.sheet().file else {
            return Ok(()); // no plan file: the page shows blank with its takeoffs
        };
        let pi = self.sheet().page;
        let size = self.page_size();
        if self.base.as_ref().is_none_or(|b| b.page != self.page) {
            let started = Instant::now();
            let scale = BASE_PX / size.x.max(size.y);
            let img = self.files[fi].render_page(pi, scale)?;
            let tex = self.upload(ctx, "base", img);
            self.base = Some(Shown {
                tex,
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
            pi,
            scale,
            px.min.x as i32,
            px.min.y as i32,
            px.width().max(1.0) as i32,
            px.height().max(1.0) as i32,
        )?;
        let tex = self.upload(ctx, "detail", img);
        self.detail = Some(Shown {
            tex,
            page: self.page,
            scale,
            area: Rect::from_min_max(px.min / scale, px.max / scale),
        });
        self.last_render_ms = started.elapsed().as_millis();
        Ok(())
    }

    fn paint(&self, painter: &egui::Painter, view: Rect) {
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

        // Takeoffs over the plan, in the condition's color.
        let pos =
            |p: &rea_core::model::Point| origin + Vec2::new(p.x as f32, p.y as f32) * self.zoom;
        for (color, g) in &self.sheet().shapes {
            let stroke = Stroke::new(2.0, *color);
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
                    painter.circle_filled(pos(p), 4.0, *color);
                }
            }
        }
    }
}

impl eframe::App for TakeoffApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.pick_and_open();
        }

        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.add_enabled(false, egui::Button::new("New project (M2)"));
                    if ui.button("Open plan PDF or OST job...  Ctrl+O").clicked() {
                        ui.close();
                        self.pick_and_open();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                match (&self.error, &self.plan) {
                    (Some(e), _) => ui.colored_label(Color32::RED, e),
                    (None, Some(p)) => ui.label(format!(
                        "{}   Page {} of {}   {}   Zoom {:.0}%   Render {} ms",
                        p.name,
                        p.page + 1,
                        p.sheets.len(),
                        p.sheet().label,
                        p.zoom * 100.0,
                        p.last_render_ms
                    )),
                    (None, None) => ui.label("No plan open"),
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("OpenTakeoff {}", rea_core::VERSION));
                });
            });
        });

        egui::Panel::left("pages")
            .default_size(220.0)
            .show(ui, |ui| {
                ui.heading("Pages");
                ui.separator();
                let Some(plan) = &mut self.plan else {
                    ui.label("Open a plan PDF or OST job to list its pages.");
                    return;
                };
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in 0..plan.sheets.len() {
                        let s = &plan.sheets[i];
                        let text = if s.shapes.is_empty() {
                            s.label.clone()
                        } else {
                            format!("{}   ({})", s.label, s.shapes.len())
                        };
                        if ui.selectable_label(i == plan.page, text).clicked() {
                            plan.go_to(i);
                        }
                    }
                });
            });

        if let Some(plan) = self.plan.as_ref().filter(|p| !p.summary.is_empty()) {
            egui::Panel::right("summary")
                .default_size(380.0)
                .show(ui, |ui| {
                    ui.heading("Summary");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("summary_grid")
                            .striped(true)
                            .num_columns(4)
                            .show(ui, |ui| {
                                ui.strong("No.");
                                ui.strong("Condition");
                                ui.strong("Qty 1");
                                ui.strong("Qty 2");
                                ui.end_row();
                                for r in &plan.summary {
                                    ui.horizontal(|ui| {
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::splat(10.0),
                                            egui::Sense::hover(),
                                        );
                                        let [cr, cg, cb] = r.color;
                                        ui.painter().rect_filled(
                                            rect,
                                            2.0,
                                            Color32::from_rgb(cr, cg, cb),
                                        );
                                        ui.label(r.number.to_string());
                                    });
                                    ui.label(&r.name);
                                    ui.label(format!("{:.2} {}", r.q1, r.u1));
                                    ui.label(
                                        r.q2.map_or(String::new(), |(q, u)| format!("{q:.2} {u}")),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
                });
        }

        let mut failed = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_gray(90)))
            .show(ui, |ui| {
                let Some(plan) = &mut self.plan else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.35);
                        ui.heading("Open a plan PDF or OST job to begin");
                        if ui.button("Open...").clicked() {
                            failed = Some(None);
                        }
                    });
                    return;
                };
                let (view, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());
                let ppp = ctx.pixels_per_point();

                // Keys work anywhere in the window unless a text box has focus.
                if !ctx.egui_wants_keyboard_input() {
                    let (pg_up, pg_down, typed) = ctx.input(|i| {
                        let typed: String = i
                            .events
                            .iter()
                            .filter_map(|e| {
                                if let egui::Event::Text(t) = e {
                                    Some(t.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        (
                            i.key_pressed(egui::Key::PageUp),
                            i.key_pressed(egui::Key::PageDown),
                            typed,
                        )
                    });
                    if pg_up {
                        plan.go_to(plan.page.saturating_sub(1));
                    }
                    if pg_down {
                        plan.go_to(plan.page + 1);
                    }
                    for c in typed.chars() {
                        match c {
                            '*' => plan.fit_pending = true,
                            '+' | '=' => plan.zoom_at(view, view.center(), 1.25),
                            '-' => plan.zoom_at(view, view.center(), 0.8),
                            '/' => plan.zoom_at(view, view.center(), 1.0 / plan.zoom),
                            _ => continue,
                        }
                        plan.last_input = Instant::now();
                    }
                }
                if plan.fit_pending {
                    plan.fit(view);
                    plan.fit_pending = false;
                }

                // Pan: right or middle drag, wheel, Shift+wheel. Zoom: Ctrl+wheel at the cursor.
                if response.dragged_by(egui::PointerButton::Secondary)
                    || response.dragged_by(egui::PointerButton::Middle)
                {
                    plan.offset += response.drag_delta();
                    plan.last_input = Instant::now();
                }
                if response.hovered() {
                    let (zoom, scroll, cursor) = ctx
                        .input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
                    if zoom != 1.0 {
                        plan.zoom_at(view, cursor.unwrap_or(view.center()), zoom);
                        plan.last_input = Instant::now();
                    }
                    if scroll != Vec2::ZERO {
                        plan.offset += scroll;
                        plan.last_input = Instant::now();
                    }
                }

                if let Err(e) = plan.refresh(&ctx, view, ppp) {
                    failed = Some(Some(e.to_string()));
                }
                plan.paint(&ui.painter_at(view), view);
            });
        match failed {
            Some(Some(e)) => self.error = Some(e),
            Some(None) => self.pick_and_open(),
            None => {}
        }
    }
}
