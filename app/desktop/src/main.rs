// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod conditions_ui;
mod doc;
mod geom;
mod project;
mod quantities;
mod tools;
mod viewer;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui::{self, Color32, Vec2};
use rea_core::model::{Id, Point, Scale, ScaleStyle};
use rea_core::scale::{self as cs, ARCHITECTURAL_PRESETS, CIVIL_FEET_PER_INCH, METRIC_RATIOS};
use rea_core::units::{format_feet_inches, parse_packed_imperial};

use conditions_ui::{qty_text, ConditionsUi};
use doc::Doc;
use quantities::SummaryRow;
use tools::{Keys, Tool, Tools};
use viewer::Viewer;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenTakeoff")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    // A project, OST job or plan PDF path on the command line opens straight away.
    let start = std::env::args_os().nth(1).map(PathBuf::from);
    eframe::run_native(
        "OpenTakeoff",
        options,
        Box::new(|_cc| {
            let mut app = App::default();
            if let Some(path) = start {
                app.load(&path);
            }
            Ok(Box::new(app))
        }),
    )
}

/// An action that waits on the unsaved-changes prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    New,
    Open,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Pages,
    Conditions,
}

struct Calibration {
    a: Point,
    b: Point,
    length: String,
    all_pages: bool,
}

/// The open project and everything the UI keeps about it.
struct Open {
    doc: Doc,
    /// The `.reaproj` file; `None` until first saved.
    path: Option<PathBuf>,
    /// Temp folder of an OST job's plans, until a save moves them next to the project.
    unpacked: Option<PathBuf>,
    viewer: Viewer,
    tools: Tools,
    conds: ConditionsUi,
    summary: Vec<SummaryRow>,
    summary_rev: Option<u64>,
    tab: Tab,
    scale_all: bool,
    calibration: Option<Calibration>,
}

#[derive(Default)]
struct App {
    open: Option<Open>,
    error: Option<String>,
    info: Option<String>,
    pending: Option<Pending>,
    allow_close: bool,
}

fn model_scale(s: cs::Scale) -> Scale {
    Scale {
        real_per_page_unit: s.inches_per_point(),
        style: match s.style {
            cs::ScaleStyle::Architectural => ScaleStyle::Architectural,
            cs::ScaleStyle::Civil => ScaleStyle::Civil,
            cs::ScaleStyle::Metric => ScaleStyle::Metric,
            cs::ScaleStyle::Custom => ScaleStyle::Custom,
        },
        label: s.label,
    }
}

/// Every preset scale, in menu order.
fn presets() -> Vec<Scale> {
    let arch = ARCHITECTURAL_PRESETS
        .iter()
        .filter_map(|(p, r, l)| cs::Scale::architectural(*p, *r, l).ok());
    let civil = CIVIL_FEET_PER_INCH
        .iter()
        .filter_map(|f| cs::Scale::civil(*f).ok());
    let metric = METRIC_RATIOS
        .iter()
        .filter_map(|m| cs::Scale::metric(*m).ok());
    arch.chain(civil).chain(metric).map(model_scale).collect()
}

fn page_label(doc: &Doc, i: usize) -> String {
    let p = &doc.pages[i];
    let title = format!("{} {}", p.sheet_no, p.sheet_name);
    if title.trim().is_empty() {
        format!("Page {}", i + 1)
    } else {
        title.trim().to_string()
    }
}

impl App {
    fn load(&mut self, path: &Path) {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        let result = match ext.as_deref() {
            Some(project::EXT) => project::open_reaproj(path).map(|d| (d, Some(path.into()), None)),
            Some("osp" | "ost") => project::open_ost(path).map(|(d, dir)| (d, None, Some(dir))),
            _ => project::new_from_pdf(path).map(|d| (d, None, None)),
        };
        match result {
            Ok((doc, path, unpacked)) if !doc.pages.is_empty() => {
                let viewer = Viewer::new(&doc.pages);
                self.error = (!viewer.missing.is_empty())
                    .then(|| format!("Plan file not found: {}", viewer.missing.join(", ")));
                self.info = None;
                self.open = Some(Open {
                    doc,
                    path,
                    unpacked,
                    viewer,
                    tools: Tools::default(),
                    conds: ConditionsUi::default(),
                    summary: Vec::new(),
                    summary_rev: None,
                    tab: Tab::Conditions,
                    scale_all: false,
                    calibration: None,
                });
            }
            Ok(_) => self.error = Some(format!("{} has no pages", path.display())),
            Err(e) => self.error = Some(format!("Could not open {}: {e}", path.display())),
        }
    }

    /// Runs `action` now, or after the unsaved-changes prompt.
    fn request(&mut self, ctx: &egui::Context, action: Pending) {
        if self.open.as_ref().is_some_and(|o| o.doc.modified()) {
            self.pending = Some(action);
        } else {
            self.run(ctx, action);
        }
    }

    fn run(&mut self, ctx: &egui::Context, action: Pending) {
        match action {
            Pending::New => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("New project from plan PDF")
                    .add_filter("Plan PDF", &["pdf"])
                    .pick_file()
                {
                    self.load(&p);
                }
            }
            Pending::Open => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter(
                        "Projects, OST jobs and plans",
                        &[project::EXT, "osp", "ost", "pdf"],
                    )
                    .pick_file()
                {
                    self.load(&p);
                }
            }
            Pending::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Saves to the project file, asking for one the first time. True when saved.
    fn save(&mut self, save_as: bool) -> bool {
        let Some(o) = &mut self.open else {
            return false;
        };
        let path = match (&o.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let Some(p) = rfd::FileDialog::new()
                    .set_title("Save project")
                    .add_filter("OpenTakeoff project", &[project::EXT])
                    .set_file_name(format!("{}.{}", o.doc.project.name, project::EXT))
                    .save_file()
                else {
                    return false;
                };
                p.with_extension(project::EXT)
            }
        };
        match project::save(&mut o.doc, &path, o.unpacked.as_deref()) {
            Ok(()) => {
                self.info = Some(format!("Saved {}", path.display()));
                self.error = None;
                o.path = Some(path);
                o.unpacked = None;
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }

    fn unsaved_prompt(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending else {
            return;
        };
        let name = self
            .open
            .as_ref()
            .map_or(String::new(), |o| o.doc.project.name.clone());
        let mut choice = None;
        egui::Modal::new(egui::Id::new("unsaved")).show(ctx, |ui| {
            ui.heading("Unsaved changes");
            ui.label(format!("Save changes to {name} first?"));
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(0);
                }
                if ui.button("Don't save").clicked() {
                    choice = Some(1);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.pending = None;
                if self.save(false) {
                    self.run(ctx, action);
                }
            }
            Some(1) => {
                self.pending = None;
                self.run(ctx, action);
            }
            Some(_) => self.pending = None,
            None => {}
        }
    }
}

impl Open {
    fn page_scale_label(&self) -> String {
        self.doc.pages[self.viewer.page]
            .scale
            .as_ref()
            .map_or("No scale".into(), |s| s.label.clone())
    }

    fn scale_pages(&self) -> Vec<usize> {
        if self.scale_all {
            (0..self.doc.pages.len()).collect()
        } else {
            vec![self.viewer.page]
        }
    }

    fn go_to(&mut self, page: usize) {
        self.viewer.go_to(page);
        self.tools.reset();
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.tools.tool == Tool::Select, "Select (V)")
                .clicked()
            {
                self.tools.set(Tool::Select);
            }
            ui.separator();
            ui.label("Page scale");
            let mut picked = None;
            egui::ComboBox::from_id_salt("scale")
                .selected_text(self.page_scale_label())
                .height(500.0)
                .show_ui(ui, |ui| {
                    for s in presets() {
                        if ui.selectable_label(false, &s.label).clicked() {
                            picked = Some(s);
                        }
                    }
                });
            if let Some(s) = picked {
                let pages = self.scale_pages();
                self.doc.set_scale(&pages, s);
                self.tools.message = None;
            }
            ui.checkbox(&mut self.scale_all, "All pages");
            if ui
                .selectable_label(self.tools.tool == Tool::Calibrate, "Calibrate")
                .on_hover_text("Click two points a known distance apart, then type the length")
                .clicked()
            {
                self.tools.set(Tool::Calibrate);
            }
            ui.separator();
            if ui
                .add_enabled(self.doc.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                self.doc.undo();
            }
            if ui
                .add_enabled(self.doc.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                self.doc.redo();
            }
        });
    }

    fn calibration_dialog(&mut self, ctx: &egui::Context) {
        if let Some((a, b)) = self.tools.calibrated.take() {
            self.calibration = Some(Calibration {
                a,
                b,
                length: String::new(),
                all_pages: self.scale_all,
            });
        }
        let Some(cal) = &mut self.calibration else {
            return;
        };
        let mut done = None;
        egui::Modal::new(egui::Id::new("calibrate")).show(ctx, |ui| {
            ui.heading("Calibrate scale");
            ui.label("Real length between the two points, in feet and inches (2500 is 25' 0\"):");
            let r = ui.text_edit_singleline(&mut cal.length);
            r.request_focus();
            let parsed = parse_packed_imperial(&cal.length);
            ui.weak(match &parsed {
                Ok(v) => format_feet_inches(*v, 2),
                Err(e) => e.to_string(),
            });
            ui.checkbox(&mut cal.all_pages, "Apply to all pages");
            ui.horizontal(|ui| {
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Apply").clicked() || enter) && parsed.is_ok() {
                    done = Some(parsed.clone().ok());
                }
                if ui.button("Cancel").clicked() {
                    done = Some(None);
                }
            });
        });
        match done {
            Some(Some(inches)) => {
                let (a, b, all) = (cal.a, cal.b, cal.all_pages);
                self.calibration = None;
                match cs::Scale::from_known_length(
                    (a.x, a.y),
                    (b.x, b.y),
                    inches,
                    cs::ScaleStyle::Custom,
                ) {
                    Ok(s) => {
                        let mut s = model_scale(s);
                        s.label = format!(
                            "1\" = {} (calibrated)",
                            format_feet_inches(s.real_per_page_unit * 72.0, 2)
                        );
                        let pages = if all {
                            (0..self.doc.pages.len()).collect()
                        } else {
                            vec![self.viewer.page]
                        };
                        self.doc.set_scale(&pages, s);
                        self.tools.message = None;
                    }
                    Err(e) => self.tools.message = Some(format!("Calibration failed: {e}")),
                }
            }
            Some(None) => self.calibration = None,
            None => {}
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Closing the window with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.open.as_ref().is_some_and(|o| o.doc.modified())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }

        let dialog =
            self.pending.is_some() || self.open.as_ref().is_some_and(|o| o.calibration.is_some());
        if !dialog {
            let (new, open, save, undo, redo) = ctx.input_mut(|i| {
                use egui::{Key, Modifiers as M};
                (
                    i.consume_key(M::COMMAND, Key::N),
                    i.consume_key(M::COMMAND, Key::O),
                    i.consume_key(M::COMMAND, Key::S),
                    i.consume_key(M::COMMAND, Key::Z),
                    i.consume_key(M::COMMAND, Key::Y)
                        || i.consume_key(M::COMMAND | M::SHIFT, Key::Z),
                )
            });
            if new {
                self.request(&ctx, Pending::New);
            }
            if open {
                self.request(&ctx, Pending::Open);
            }
            if save {
                self.save(false);
            }
            if let Some(o) = &mut self.open {
                if undo {
                    o.doc.undo();
                }
                if redo {
                    o.doc.redo();
                }
            }
        }

        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New project from PDF...  Ctrl+N").clicked() {
                        ui.close();
                        self.request(&ctx, Pending::New);
                    }
                    if ui.button("Open...  Ctrl+O").clicked() {
                        ui.close();
                        self.request(&ctx, Pending::Open);
                    }
                    let has = self.open.is_some();
                    if ui
                        .add_enabled(has, egui::Button::new("Save  Ctrl+S"))
                        .clicked()
                    {
                        ui.close();
                        self.save(false);
                    }
                    if ui
                        .add_enabled(has, egui::Button::new("Save As..."))
                        .clicked()
                    {
                        ui.close();
                        self.save(true);
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ui.close();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                if let Some(o) = &mut self.open {
                    ui.menu_button("Edit", |ui| {
                        if ui
                            .add_enabled(o.doc.can_undo(), egui::Button::new("Undo  Ctrl+Z"))
                            .clicked()
                        {
                            o.doc.undo();
                        }
                        if ui
                            .add_enabled(o.doc.can_redo(), egui::Button::new("Redo  Ctrl+Y"))
                            .clicked()
                        {
                            o.doc.redo();
                        }
                    });
                }
            });
        });

        if let Some(o) = &mut self.open {
            egui::Panel::top("toolbar").show(ui, |ui| o.toolbar(ui));
            if o.summary_rev != Some(o.doc.rev) {
                o.summary = o.doc.summary();
                o.summary_rev = Some(o.doc.rev);
            }
        }

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(o) = &self.open {
                    ui.label(format!(
                        "{}{}   Page {} of {}   {}   Scale: {}   Zoom {:.0}%",
                        o.doc.project.name,
                        if o.doc.modified() { " *" } else { "" },
                        o.viewer.page + 1,
                        o.doc.pages.len(),
                        page_label(&o.doc, o.viewer.page),
                        o.page_scale_label(),
                        o.viewer.zoom * 100.0,
                    ));
                    let tool = match o.tools.tool {
                        Tool::Select => "Select".to_string(),
                        Tool::Calibrate => "Calibrate".to_string(),
                        Tool::Draw(id) => o
                            .doc
                            .condition(id)
                            .map_or(String::new(), |c| format!("Drawing {}", c.name)),
                    };
                    ui.separator();
                    ui.label(tool);
                    if let Some(r) = &o.tools.running_text {
                        ui.separator();
                        ui.strong(r);
                    }
                    if let Some(m) = &o.tools.message {
                        ui.separator();
                        ui.colored_label(Color32::from_rgb(220, 60, 40), m);
                    }
                } else {
                    ui.label("No project open");
                }
                if let Some(e) = &self.error {
                    ui.separator();
                    ui.colored_label(Color32::RED, e);
                } else if let Some(i) = &self.info {
                    ui.separator();
                    ui.label(i);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("OpenTakeoff {}", rea_core::VERSION));
                });
            });
        });

        if let Some(o) = &mut self.open {
            egui::Panel::left("side")
                .default_size(260.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut o.tab, Tab::Pages, "Pages");
                        ui.selectable_value(&mut o.tab, Tab::Conditions, "Conditions");
                    });
                    ui.separator();
                    match o.tab {
                        Tab::Pages => {
                            let mut counts: HashMap<Id, usize> = HashMap::new();
                            for t in &o.doc.takeoffs {
                                *counts.entry(t.page_id).or_default() += 1;
                            }
                            let mut go = None;
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for i in 0..o.doc.pages.len() {
                                    let mut text = page_label(&o.doc, i);
                                    if let Some(n) = counts.get(&o.doc.pages[i].id) {
                                        text = format!("{text}   ({n})");
                                    }
                                    if o.doc.pages[i].scale.is_none() {
                                        text = format!("{text}   no scale");
                                    }
                                    if ui.selectable_label(i == o.viewer.page, text).clicked() {
                                        go = Some(i);
                                    }
                                }
                            });
                            if let Some(i) = go {
                                o.go_to(i);
                            }
                        }
                        Tab::Conditions => {
                            let page = o.viewer.page;
                            o.conds
                                .panel(ui, &mut o.doc, &o.summary, &mut o.tools, page);
                        }
                    }
                });

            egui::Panel::right("summary")
                .default_size(420.0)
                .show(ui, |ui| {
                    ui.heading("Summary");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("summary_grid")
                            .striped(true)
                            .num_columns(5)
                            .show(ui, |ui| {
                                ui.strong("No.");
                                ui.strong("Condition");
                                ui.strong("Qty 1");
                                ui.strong("Qty 2");
                                ui.strong("Qty 3");
                                ui.end_row();
                                for r in &o.summary {
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
                                    for q in r.qty {
                                        ui.label(qty_text(q));
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                });
        }

        let mut failed = None;
        let mut start_open = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_gray(90)))
            .show(ui, |ui| {
                let Some(o) = &mut self.open else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.35);
                        ui.heading("Open a project, an OST job or a plan PDF to begin");
                        if ui.button("Open...").clicked() {
                            start_open = true;
                        }
                    });
                    return;
                };
                let (view, resp) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let ppp = ctx.pixels_per_point();

                // Keys work anywhere in the window unless a text box or dialog has focus.
                let mut keys = Keys::default();
                if !ctx.egui_wants_keyboard_input() && !dialog {
                    let (pg_up, pg_down, typed, k, v_key) = ctx.input(|i| {
                        let typed: String = i
                            .events
                            .iter()
                            .filter_map(|e| match e {
                                egui::Event::Text(t) => Some(t.as_str()),
                                _ => None,
                            })
                            .collect();
                        let none = i.modifiers.is_none();
                        (
                            i.key_pressed(egui::Key::PageUp),
                            i.key_pressed(egui::Key::PageDown),
                            typed,
                            Keys {
                                enter: i.key_pressed(egui::Key::Enter),
                                escape: i.key_pressed(egui::Key::Escape),
                                backspace: i.key_pressed(egui::Key::Backspace),
                                delete: i.key_pressed(egui::Key::Delete),
                            },
                            none && i.key_pressed(egui::Key::V),
                        )
                    });
                    keys = k;
                    if pg_up {
                        o.go_to(o.viewer.page.saturating_sub(1));
                    }
                    if pg_down {
                        o.go_to(o.viewer.page + 1);
                    }
                    if v_key {
                        o.tools.set(Tool::Select);
                    }
                    let v = &mut o.viewer;
                    for c in typed.chars() {
                        match c {
                            '*' => v.fit_pending = true,
                            '+' | '=' => v.zoom_at(view, view.center(), 1.25),
                            '-' => v.zoom_at(view, view.center(), 0.8),
                            '/' => v.zoom_at(view, view.center(), 1.0 / v.zoom),
                            _ => continue,
                        }
                        v.last_input = Instant::now();
                    }
                }
                let v = &mut o.viewer;
                if v.fit_pending {
                    v.fit(view);
                    v.fit_pending = false;
                }

                // Pan: right or middle drag, wheel, Shift+wheel. Zoom: Ctrl+wheel at the cursor.
                if resp.dragged_by(egui::PointerButton::Secondary)
                    || resp.dragged_by(egui::PointerButton::Middle)
                {
                    v.offset += resp.drag_delta();
                    v.last_input = Instant::now();
                }
                if resp.hovered() {
                    let (zoom, scroll, cursor) = ctx
                        .input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
                    if zoom != 1.0 {
                        v.zoom_at(view, cursor.unwrap_or(view.center()), zoom);
                        v.last_input = Instant::now();
                    }
                    if scroll != Vec2::ZERO {
                        v.offset += scroll;
                        v.last_input = Instant::now();
                    }
                }

                o.tools.handle(&mut o.doc, &o.viewer, view, &resp, keys);
                let hover = resp.hover_pos();
                o.tools.running_text = o.tools.running(
                    &o.doc,
                    o.viewer.page,
                    hover.map(|h| o.viewer.to_page(view, h)),
                );
                if matches!(o.tools.tool, Tool::Draw(_) | Tool::Calibrate) && resp.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                }

                let file_page = o.doc.pages[o.viewer.page].file_page_index as usize;
                if let Err(e) = o.viewer.refresh(&ctx, view, ppp, file_page) {
                    failed = Some(e.to_string());
                }
                let painter = ui.painter_at(view);
                o.viewer.paint_plan(&painter, view);
                o.viewer
                    .paint_takeoffs(&painter, view, &o.doc, o.tools.selected);
                o.tools.paint(&painter, &o.viewer, view, &o.doc, hover);
            });
        if let Some(e) = failed {
            self.error = Some(e);
        }
        if start_open {
            self.request(&ctx, Pending::Open);
        }

        if let Some(o) = &mut self.open {
            o.conds.windows(&ctx, &mut o.doc, &mut o.tools);
            o.calibration_dialog(&ctx);
        }
        self.unsaved_prompt(&ctx);
    }
}
