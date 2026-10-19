// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use rea_pdf::{Image, PlanFile};

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
    // A PDF path on the command line opens straight away.
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

struct Plan {
    file: PlanFile,
    name: String,
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

impl TakeoffApp {
    fn open(&mut self, path: &Path) {
        match PlanFile::open(path) {
            Ok(file) if file.page_count() > 0 => {
                self.plan = Some(Plan {
                    file,
                    name: path
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into()),
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
            .add_filter("PDF plans", &["pdf"])
            .pick_file()
        {
            self.open(&path);
        }
    }
}

impl Plan {
    fn page_size(&self) -> Vec2 {
        let (w, h) = self.file.page_size(self.page).unwrap_or((1.0, 1.0));
        Vec2::new(w, h)
    }

    fn go_to(&mut self, page: usize) {
        if page < self.file.page_count() && page != self.page {
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
        let size = self.page_size();
        if self.base.as_ref().is_none_or(|b| b.page != self.page) {
            let started = Instant::now();
            let scale = BASE_PX / size.x.max(size.y);
            let img = self.file.render_page(self.page, scale)?;
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
        let img = self.file.render_region(
            self.page,
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
        let to_screen = |r: Rect| {
            Rect::from_min_max(
                view.min + self.offset + r.min.to_vec2() * self.zoom,
                view.min + self.offset + r.max.to_vec2() * self.zoom,
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
                    if ui.button("Open plan PDF...  Ctrl+O").clicked() {
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
                        "{}   Page {} of {}   Zoom {:.0}%   Render {} ms",
                        p.name,
                        p.page + 1,
                        p.file.page_count(),
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
                    ui.label("Open a plan PDF to list its pages.");
                    return;
                };
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in 0..plan.file.page_count() {
                        let (w, h) = plan.file.page_size(i).unwrap_or_default();
                        let text = format!("Page {}   {:.0} x {:.0} in", i + 1, w / 72.0, h / 72.0);
                        if ui.selectable_label(i == plan.page, text).clicked() {
                            plan.go_to(i);
                        }
                    }
                });
            });

        let mut failed = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_gray(90)))
            .show(ui, |ui| {
                let Some(plan) = &mut self.plan else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.35);
                        ui.heading("Open a plan PDF to begin");
                        if ui.button("Open plan PDF...").clicked() {
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
