//! Conditions panel: the list with live quantities, and the New, Edit, Duplicate and
//! Delete actions. Picking a condition arms its drawing tool.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Vec2};
use rea_core::model::{Condition, ConditionStyle, CountShape, Id, ResultKind};
use rea_core::units::{format_feet_inches, parse_packed_imperial};

use crate::doc::{default_results, result_label, Doc, RESULTS};
use crate::quantities::SummaryRow;
use crate::tools::{Tool, Tools};

/// Packed feet-inch text for an entry box: 96 in is `800`, 102.5 in is `806.5`.
pub fn packed(inches: f64) -> String {
    let inches = (inches * 10_000.0).round() / 10_000.0;
    let feet = (inches / 12.0).floor();
    let rest = inches - feet * 12.0;
    let whole = rest.trunc();
    let frac = format!("{:.4}", rest - whole);
    let frac = frac.trim_end_matches('0').trim_start_matches('0');
    let frac = if frac == "." { "" } else { frac };
    if feet > 0.0 {
        format!("{feet:.0}{whole:02.0}{frac}")
    } else {
        format!("{whole:.0}{frac}")
    }
}

pub fn style_name(s: ConditionStyle) -> &'static str {
    match s {
        ConditionStyle::Linear => "Linear",
        ConditionStyle::Area => "Area",
        ConditionStyle::Count => "Count",
    }
}

fn swatch(ui: &mut egui::Ui, color: [u8; 3]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 2.0, Color32::from_rgb(color[0], color[1], color[2]));
}

/// Quantity text of a summary slot, for example `24.50 LF`.
pub fn qty_text(q: Option<(f64, &str)>) -> String {
    q.map_or(String::new(), |(q, u)| format!("{q:.2} {u}"))
}

struct Editor {
    c: Condition,
    is_new: bool,
    /// Packed feet-inch text per size: height, thickness, width, depth.
    sizes: [String; 4],
    slope: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct ConditionsUi {
    /// Highlighted row.
    pub current: Option<Id>,
    editor: Option<Editor>,
    confirm_delete: Option<Id>,
}

impl ConditionsUi {
    fn edit(&mut self, c: Condition, is_new: bool) {
        self.editor = Some(Editor {
            sizes: [c.height, c.thickness, c.width, c.depth].map(packed),
            slope: c.slope.is_some(),
            c,
            is_new,
            error: None,
        });
    }

    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut Doc,
        rows: &[SummaryRow],
        tools: &mut Tools,
        page: usize,
    ) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                let c = doc.new_condition(ConditionStyle::Linear);
                self.edit(c, true);
            }
            let cur = self.current.and_then(|id| doc.condition(id)).cloned();
            ui.add_enabled_ui(cur.is_some(), |ui| {
                if ui.button("Edit").clicked() {
                    if let Some(c) = cur.clone() {
                        self.edit(c, false);
                    }
                }
                if ui.button("Duplicate").clicked() {
                    if let Some(c) = cur.as_ref().and_then(|c| doc.duplicate_condition(c.id)) {
                        self.current = Some(c.id);
                        doc.put_condition(c);
                    }
                }
                if ui.button("Delete").clicked() {
                    self.confirm_delete = cur.as_ref().map(|c| c.id);
                }
            });
        });
        ui.separator();
        if doc.conditions.is_empty() {
            ui.label("No conditions yet. Click New to add one.");
            return;
        }
        let by_id: HashMap<Id, &SummaryRow> = rows.iter().map(|r| (r.id, r)).collect();
        let mut order: Vec<&Condition> = doc.conditions.iter().collect();
        order.sort_by_key(|c| c.number);
        let mut picked = None;
        let mut open_editor = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for c in order {
                let qty = by_id
                    .get(&c.id)
                    .map_or(String::new(), |r| qty_text(r.qty[0]));
                let armed = tools.tool == Tool::Draw(c.id);
                ui.horizontal(|ui| {
                    swatch(ui, c.color);
                    let text = format!("{}  {}  ({})", c.number, c.name, style_name(c.style));
                    // Long names are cut short so the quantity stays readable.
                    let w = (ui.available_width() - 90.0).max(60.0);
                    let r = ui
                        .scope(|ui| {
                            ui.set_max_width(w);
                            ui.add(
                                egui::Button::selectable(self.current == Some(c.id) || armed, text)
                                    .truncate(),
                            )
                        })
                        .inner;
                    if r.clicked() {
                        picked = Some(c.id);
                    }
                    if r.double_clicked() {
                        open_editor = Some(c.clone());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(qty);
                    });
                });
            }
        });
        if let Some(id) = picked {
            self.current = Some(id);
            tools.set(Tool::Draw(id));
            tools.message = doc.can_draw(page).err();
        }
        if let Some(c) = open_editor {
            self.edit(c, false);
        }
    }

    /// Edit window and delete confirmation; call once per frame.
    pub fn windows(&mut self, ctx: &egui::Context, doc: &mut Doc, tools: &mut Tools) {
        if let Some(id) = self.confirm_delete {
            let Some(c) = doc.condition(id).cloned() else {
                self.confirm_delete = None;
                return;
            };
            let n = doc.takeoff_count(id);
            egui::Modal::new(egui::Id::new("confirm_delete")).show(ctx, |ui| {
                ui.heading("Delete condition");
                ui.label(format!(
                    "Delete condition {} {} and its {} takeoff{} on all pages?",
                    c.number,
                    c.name,
                    n,
                    if n == 1 { "" } else { "s" }
                ));
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        doc.delete_condition(id);
                        if tools.tool == Tool::Draw(id) {
                            tools.set(Tool::Select);
                        }
                        self.current = None;
                        self.confirm_delete = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_delete = None;
                    }
                });
            });
        }

        let Some(ed) = &mut self.editor else {
            return;
        };
        let has_takeoff = !ed.is_new && doc.takeoff_count(ed.c.id) > 0;
        let mut done = None;
        egui::Window::new(if ed.is_new {
            "New condition"
        } else {
            "Edit condition"
        })
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("cond_edit").num_columns(2).show(ui, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut ed.c.name);
                ui.end_row();
                ui.label("Number");
                ui.add(egui::DragValue::new(&mut ed.c.number).range(1..=999_999));
                ui.end_row();
                ui.label("Style");
                ui.add_enabled_ui(!has_takeoff, |ui| {
                    ui.horizontal(|ui| {
                        let before = ed.c.style;
                        for s in [
                            ConditionStyle::Linear,
                            ConditionStyle::Area,
                            ConditionStyle::Count,
                        ] {
                            ui.radio_value(&mut ed.c.style, s, style_name(s));
                        }
                        if ed.c.style != before {
                            ed.c.results = default_results(ed.c.style);
                            ed.c.count_shape =
                                (ed.c.style == ConditionStyle::Count).then_some(CountShape::Square);
                        }
                    })
                    .response
                    .on_disabled_hover_text("Style is fixed once the condition has takeoffs.");
                });
                ui.end_row();
                ui.label("Color");
                ui.color_edit_button_srgb(&mut ed.c.color);
                ui.end_row();

                let mut size = |ui: &mut egui::Ui, label: &str, i: usize| {
                    ui.label(label);
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut ed.sizes[i]).desired_width(70.0));
                        ui.weak(match parse_packed_imperial(&ed.sizes[i]) {
                            Ok(v) => format_feet_inches(v, 2),
                            Err(_) if ed.sizes[i].trim().is_empty() => "0".into(),
                            Err(_) => "not valid".into(),
                        });
                    });
                    ui.end_row();
                };
                size(ui, "Height", 0);
                size(ui, "Thickness", 1);
                if ed.c.style == ConditionStyle::Count {
                    size(ui, "Width", 2);
                    size(ui, "Depth", 3);
                }
                ui.label("Slope");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut ed.slope, "");
                    let (mut rise, mut run) = ed.c.slope.unwrap_or((4.0, 12.0));
                    ui.add_enabled_ui(ed.slope, |ui| {
                        ui.add(egui::DragValue::new(&mut rise).prefix("rise "));
                        ui.add(egui::DragValue::new(&mut run).prefix("run "));
                    });
                    ed.c.slope = ed.slope.then_some((rise, run));
                });
                ui.end_row();
                for slot in 0..3 {
                    ui.label(format!("Result {}", slot + 1));
                    let cur = ed.c.results[slot];
                    egui::ComboBox::from_id_salt(("result", slot))
                        .selected_text(cur.map_or("(none)", |k| result_label(ed.c.style, k)))
                        .show_ui(ui, |ui| {
                            if slot > 0 {
                                ui.selectable_value(&mut ed.c.results[slot], None, "(none)");
                            }
                            for r in RESULTS.iter().filter(|r| r.0 == ed.c.style) {
                                let k: Option<ResultKind> = Some(r.1);
                                ui.selectable_value(&mut ed.c.results[slot], k, r.3);
                            }
                        });
                    ui.end_row();
                }
            });
            if let Some(e) = &ed.error {
                ui.colored_label(Color32::RED, e);
            }
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    done = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    done = Some(false);
                }
            });
        });

        match done {
            Some(true) => {
                let mut c = ed.c.clone();
                let mut vals = [0.0; 4];
                for (i, v) in vals.iter_mut().enumerate() {
                    let s = ed.sizes[i].trim();
                    *v = if s.is_empty() {
                        0.0
                    } else {
                        match parse_packed_imperial(s) {
                            Ok(v) => v,
                            Err(e) => {
                                ed.error = Some(format!("Size: {e}"));
                                return;
                            }
                        }
                    };
                }
                [c.height, c.thickness, c.width, c.depth] = vals;
                if let Err(e) = doc.validate_condition(&c) {
                    ed.error = Some(e);
                    return;
                }
                let id = c.id;
                doc.put_condition(c);
                self.editor = None;
                self.current = Some(id);
                tools.set(Tool::Draw(id));
            }
            Some(false) => self.editor = None,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::packed;
    use rea_core::units::parse_packed_imperial;

    #[test]
    fn packed_round_trips_through_the_parser() {
        for v in [0.0, 6.0, 12.0, 96.0, 102.5, 130.0, 1206.0] {
            assert_eq!(parse_packed_imperial(&packed(v)), Ok(v), "{v}");
        }
        assert_eq!(packed(96.0), "800");
    }
}
