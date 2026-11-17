//! The Gears dialog's form (GEAR_UI_SPEC.md §1–§5), top to bottom:
//!
//! 1. **Type** (the gear icons) and one primary **New gear** button, with what the type needs;
//! 2. the **status line**: always what to do next ("Gear B · Compound — click: shaft line"), or
//!    why the last pick was refused;
//! 3. one **card** per gear: its colour, name and type; while it is picked, its checklist (✓ done
//!    with the sketch and size of the pick, → next, ○ optional); once complete, one line with its
//!    teeth and module; its warnings and errors with what to do. Click a card to make it the one
//!    picks go to, × to remove it; hovering lights its geometry in the view;
//! 4. the active gear's **settings**: Size by, Module / Teeth (the other worked out, greyed),
//!    Face preset and sides, Helix, Hand and V, the type's own, and More (profile shift, bore,
//!    hub);
//! 5. **Drivetrain**: the links as rows, the suggested ones with one Accept button, a link added by
//!    hand, the driver, the ratio between two gears, and the node graph on request;
//! 6. **Output** (operation, targets, keep-out, assembly), then Results and Advanced, closed.
//!
//! The data and the request are `dialogs_gear`'s; this file only draws and edits them, with the
//! app's ui_kit rows, fields, buttons, sections and hover texts.

use solidcraft_engine::Sel;
use solidcraft_gear::items::ItemKind;
use solidcraft_theme::Tokens;

use crate::SolveApp;
use crate::dialogs::OP_LABELS;
use crate::dialogs_gear::{self as dg, Gear, Msg};
use crate::dialogs_gear_graph::{GearSettings, item_id};
use crate::dialogs_gear_items::FACE_PRESETS;
use crate::selection::SelInput;
use crate::ui_kit::{self, BackingTooltipExt as _, choice_row, hint_row};

const TIP_TYPE: &str = "What the next gear will be. Each type lists the sketch pieces it needs.";
const TIP_NEW: &str = "Start a new gear of this type, then click the sketch pieces it asks for in the view.";
const TIP_CARD: &str = "Click to make this the gear your picks go to. Hover to light its geometry.";
const TIP_SIZE_BY: &str = "Set the module (tooth size) or the tooth count; the other follows from the pitch circle.";
const TIP_MODULE: &str = "Tooth size in mm (pitch Ø ÷ teeth). Auto picks a standard module so the smallest gear has ≥17 teeth.";
const TIP_TEETH: &str = "Number of teeth. The pitch circle stays as drawn, so the module follows.";
const TIP_FACE: &str = "How far the teeth reach each side of the sketch plane, along the axis. Auto ≈ 10 × module, limited by the room on the shaft.";
const TIP_PRESET: &str = "Symmetric: equal both sides. A only / B only: the gear grows out of one face of the sketch (side A is the green arrow in the view).";
const TIP_HELIX: &str = "Tooth angle to the axis. 0 = straight. Meshing gears get equal angles of opposite hand.";
const TIP_HAND: &str = "Left or right helix. The mating gear is set to the other hand for you.";
const TIP_V: &str = "Herringbone: two opposite helices meeting in the middle — no axial thrust.";
const TIP_V_BEVEL: &str = "Chevron bevel: the spiral changes hand at mid-face.";
const TIP_SHIFT: &str = "Moves the tooth out to avoid undercut on small gears. Auto does it below 17 teeth.";
const TIP_BORE: &str = "Hole through the gear. Auto fits its shaft.";
const TIP_SPIRAL: &str = "Spiral bevel/hypoid tooth angle at mid-face. 35° is typical.";
const TIP_STARTS: &str = "Thread starts on the worm. More starts = faster, less self-locking.";
const TIP_DRIVER: &str = "The gear you turn. Every other gear's speed and direction follow from it.";
const TIP_RPM: &str = "Optional: fills speed and torque for every gear in Results.";
const TIP_ACCEPT: &str = "Make every suggested link real. Suggested = circles the drawing shows as touching or on one line.";
const TIP_OPERATION: &str = "New Body: separate bodies. Join/Cut/Intersect: combine with the Targets.";
const TIP_KEEP_OUT: &str = "Cut the space the gears sweep (plus the gap) out of the Targets, e.g. a housing.";
const TIP_GAP: &str = "Extra room around the swept teeth when cutting the housing.";
const TIP_ASSEMBLY: &str = "Each shaft and its gears become a component on a revolve joint, linked to its mates by the gear ratio.";
const TIP_EQUATIONS: &str = "Exact involute curves instead of fitted splines: slower, smoother.";
const TIP_MESH: &str = "Mesh: the two gears' teeth engage; their distance is checked.";
const TIP_SHAFT: &str = "Same shaft: the two gears turn together.";

/// What each type needs, for the line under the type list.
fn needs(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Spur | ItemKind::Helical | ItemKind::Herringbone => "Needs: a pitch circle. Optional: its shaft line.",
        ItemKind::Internal => "Needs: the ring's pitch circle. Optional: its pinion's circle inside it.",
        ItemKind::Rack => "Needs: the rack's pitch line. Optional: its pinion's circle.",
        ItemKind::Compound => "Needs: a shaft line and two or more circles on it (in parallel sketches).",
        ItemKind::Planetary => "Needs: the sun's and the ring's circles, then one planet's circle.",
        ItemKind::Bevel => "Needs: two circles on crossing axes, or two axis lines and a circle, or (side view) its two shaft lines, then the apex point where they meet.",
        ItemKind::Hypoid => "Needs: the ring gear's circle and the pinion's (offset) axis line.",
        ItemKind::Worm => "Needs: the wheel's pitch circle and the worm's axis line.",
        ItemKind::CrossedHelical => "Needs: two circles on skew axes.",
        ItemKind::Shaft => "Needs: a shaft line. Optional: bearing points on it.",
    }
}

/// What a slot's sketch piece decides about the gear (its hover text on the card).
fn drives(what: &str) -> &'static str {
    if what.contains("circle") {
        "This circle places the gear: its centre is the gear's centre, its Ø the pitch Ø, and its sketch's normal the axis (side A). Change the sketch and the gear follows."
    } else if what.contains("apex") {
        "This point is where the two shafts meet (the cones' apex)."
    } else if what.contains("bearing") {
        "This point on the shaft line places a bearing seat."
    } else if what.contains("contact") {
        "This line is where the two cones touch: its angle sets the ratio, its length the cone distance."
    } else {
        "This line is the axis: its direction (from its first point) is side A, and a shaft's length follows it."
    }
}

/// The dialog's form. True when Enter was pressed in a field.
pub fn rows(app: &mut SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize, hover: &mut Option<(usize, Sel)>) -> bool {
    let mut enter = false;
    if *input == 0 {
        dg::sync_picks(app, k, inputs);
    }
    dg::refresh(app, k, inputs);
    let t = Tokens::get();
    ui.spacing_mut().item_spacing.y = t.space_4;
    if k.legacy.is_none() {
        header(ui, k, inputs, input);
    }
    status(app, ui, k, *input);
    legacy(app, ui, k);
    let mut edit = None;
    for m in &k.set_msgs {
        message(ui, m);
        if let Some(sk) = loose_sketch(app, &m.text)
            && ui_kit::button(ui, "Edit sketch", ui_kit::Button::Normal).backing_on_hover_text("Open the sketch to dimension it; the dialog comes back when you finish it.").clicked()
        {
            edit = Some(sk);
        }
    }
    if let Some(sk) = edit {
        k.action = Some(dg::Action::EditSketch(sk));
    }
    if k.items.is_empty() && k.legacy.is_none() {
        guide(ui);
    }
    cards(app, ui, k, inputs, input, hover);
    if let Some(a) = k.active.filter(|a| *a < k.items.len()) {
        enter |= settings(ui, k, a);
    }
    enter |= drivetrain(app, ui, k);
    enter |= output(ui, k, inputs, input);
    results(app, ui, k);
    enter |= advanced(ui, k);
    enter
}

/// The type list and the New gear button, with what the type needs.
fn header(ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize) {
    let options: Vec<(&str, &str)> = dg::TYPES.iter().map(|(kind, icon)| (kind.label(), *icon)).collect();
    ui_kit::row(ui, "Type", |ui| ui_kit::dropdown_icons(ui, "gear_type", &mut k.new_kind, &options)).backing_on_hover_text(TIP_TYPE);
    // The type's picture beside what it needs.
    let t = Tokens::get();
    ui.horizontal_top(|ui| {
        let (icon, _) = ui.allocate_exact_size(egui::vec2(t.space_24 + t.space_8, t.space_24 + t.space_8), egui::Sense::hover());
        ui_kit::icons::paint_icon(ui.painter(), icon, dg::icon_of(k.new_type()), 1.0);
        ui.add(egui::Label::new(egui::RichText::new(needs(k.new_type())).size(t.font_small).color(t.form_dim)).wrap());
    });
    let new = ui_kit::wide_primary_button(ui, "+  New gear").backing_on_hover_text(TIP_NEW);
    solidcraft_ui_registry::publish_target("gear:new", new.rect);
    if new.clicked() {
        dg::new_gear(k, inputs);
        *input = 0;
    }
}

/// The status line: what to do next, in the accent colour; a refusal in the warning colour.
fn status(app: &SolveApp, ui: &mut egui::Ui, k: &Gear, input: usize) {
    let t = Tokens::get();
    let (text, bad) = if input == 1 {
        ("Click the bodies the gears act on (Targets). Click a gear's card to go back to picking gears.".to_string(), false)
    } else {
        dg::next_step(app, k)
    };
    let (fill, ink) = if bad { (t.form_error_fill, t.warning) } else { (t.selected_bg, t.selected_ink) };
    let r = egui::Frame::new()
        .fill(fill)
        .corner_radius(t.control_radius)
        .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_6 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(egui::RichText::new(format!("{}  {text}", if bad { "⚠" } else { "→" })).size(t.font_label).color(ink).strong()).wrap());
        })
        .response;
    solidcraft_ui_registry::publish_rect("gear:status", r.rect);
    solidcraft_ui_registry::publish_rect(&format!("gear:status:{text}"), r.rect);
}

/// An old set: what it is, and the button that makes its circles gears.
fn legacy(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear) {
    let Some(l) = k.legacy.clone() else { return };
    ui_kit::text(ui, &format!("Made with the earlier Gears dialog: {}. It still follows its sketch; OK keeps it as it is.", l.what), ui_kit::Text::Note("gear_legacy"));
    if l.convertible {
        let r = ui_kit::button(ui, "Convert to gears", ui_kit::Button::Normal)
            .backing_on_hover_text("Make each of its circles a gear of the new kind, to change them one by one. The set is replaced by the gears on OK.");
        solidcraft_ui_registry::publish_target("gear:convert", r.rect);
        if r.clicked() {
            match dg::convert_legacy(app, k) {
                Ok(_) => k.refused = None,
                Err(e) => k.refused = Some((None, e)),
            }
        }
    }
}

/// The empty state: three steps.
fn guide(ui: &mut egui::Ui) {
    let t = Tokens::get();
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, t.control_border))
        .corner_radius(t.control_radius)
        .inner_margin(egui::Margin::same(t.space_8 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (n, step) in [
                "Pick a type above (spur, compound, worm…).",
                "Press New gear, or just click circles in the view: box-select several to make one gear each.",
                "Click the sketch pieces the status line asks for. Each gear gets its own card and settings.",
            ]
            .iter()
            .enumerate()
            {
                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::new(format!("{}", n + 1)).size(t.font_label).strong().color(t.form_primary));
                    ui.add(egui::Label::new(egui::RichText::new(*step).size(t.font_label).color(t.form_ink)).wrap());
                });
            }
            ui.add(
                egui::Label::new(egui::RichText::new("Then link them under Drivetrain: touching circles are suggested for you.").size(t.font_small).color(t.form_dim))
                    .wrap(),
            );
        });
}

/// One message on a card: ⚠ a warning, ⛔ an error, with what to do.
fn message(ui: &mut egui::Ui, m: &Msg) {
    let t = Tokens::get();
    let (mark, ink) = if m.error { ("×", t.error) } else { ("⚠", t.warning) };
    ui.add(egui::Label::new(egui::RichText::new(format!("{mark} {}", m.text)).size(t.font_small).color(ink)).wrap());
}

/// The sketch a "sketch 'X' isn't fully defined" message names, to open it.
fn loose_sketch(app: &SolveApp, text: &str) -> Option<u64> {
    if !text.contains("isn't fully defined") {
        return None;
    }
    let start = text.find('\'')? + 1;
    let end = start + text[start..].find('\'')?;
    let name = &text[start..end];
    app.session.doc.features.iter().find(|f| f.name == name).map(|f| f.id)
}

/// A complete gear's one line: each of its gears' teeth and module once worked out.
fn summary(k: &Gear, i: usize) -> String {
    let n = k.gear_counts.get(i).copied().unwrap_or(0);
    let parts: Vec<String> = (0..n)
        .filter_map(|g| {
            let r = k.graph.replies().iter().find(|r| (r.item, r.gear) == (i, g))?;
            (r.teeth > 0).then(|| {
                let name = if n > 1 { format!("{} ", k.gear_name((i, g))) } else { String::new() };
                format!("{name}z{} m{}", r.teeth, dg::fmt(r.module))
            })
        })
        .collect();
    if parts.is_empty() { "complete".into() } else { parts.join(" · ") }
}

/// One card per gear.
fn cards(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize, hover: &mut Option<(usize, Sel)>) {
    let t = Tokens::get();
    let len = dg::len_of(app);
    let (mut activate, mut remove, mut edit) = (None, None, None);
    let mut hovered = None;
    let clicked_at = ui.input(|i| if i.pointer.primary_clicked() { i.pointer.interact_pos() } else { None });
    for i in 0..k.items.len() {
        let it = k.items[i].clone();
        let active = k.active == Some(i) && *input == 0;
        let complete = it.complete(app);
        let lit = k.card_hovered == Some(i) || k.graph_hovered == Some(i);
        let stroke = if active {
            egui::Stroke::new(1.5, t.form_primary)
        } else if lit {
            egui::Stroke::new(1.5, dg::item_color(i))
        } else {
            egui::Stroke::new(1.0, t.control_border)
        };
        let mut button_hit = false;
        let card = egui::Frame::new()
            .fill(if active { t.selected_bg } else { t.control_fill })
            .stroke(stroke)
            .corner_radius(t.control_radius)
            .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_6 as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(egui::vec2(t.space_10, t.space_16), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), t.space_4 + 1.0, dg::item_color(i));
                    let (icon, _) = ui.allocate_exact_size(egui::vec2(t.space_16, t.space_16), egui::Sense::hover());
                    ui_kit::icons::paint_icon(ui.painter(), icon, dg::icon_of(it.kind), 1.0);
                    ui.label(egui::RichText::new(format!("Gear {}", item_id(i))).size(t.font_label).strong().color(t.form_ink));
                    ui.label(egui::RichText::new(it.label()).size(t.font_label).color(t.form_dim));
                    if k.drive_item.is_some_and(|(d, _)| d == i) {
                        ui.label(egui::RichText::new("▶ driver").size(t.font_small).color(t.form_primary));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let x = ui_kit::compact_button(ui, "×", "Remove this gear (its links go too)", true, false);
                        solidcraft_ui_registry::publish_target(&format!("gear:remove:{}", item_id(i)), x.rect);
                        if x.clicked() {
                            remove = Some(i);
                            button_hit = true;
                        }
                        button_hit |= x.hovered();
                        if complete && !active {
                            ui.add(egui::Label::new(egui::RichText::new(summary(k, i)).size(t.font_small).color(t.form_dim)).truncate());
                        }
                    });
                });
                // While it is picked (or incomplete): its checklist.
                if active || !complete {
                    for row in it.slot_rows(app, &len) {
                        let (mark, ink) = match (&row.filled, row.next, row.required) {
                            (Some(_), _, _) => ("✓", t.form_success),
                            (None, true, _) => ("→", t.form_primary),
                            (None, false, true) => ("●", t.form_ink),
                            (None, false, false) => ("○", t.form_dim),
                        };
                        ui.horizontal(|ui| {
                            ui.add_space(t.space_10);
                            ui.label(egui::RichText::new(mark).size(t.font_label).color(ink).strong());
                            let what = if row.required || row.filled.is_some() { row.what.to_string() } else { format!("{} (optional)", row.what) };
                            let l = ui.label(egui::RichText::new(what).size(t.font_label).color(if row.next { t.form_primary } else { t.form_ink }));
                            if row.filled.is_none() {
                                l.backing_on_hover_text(format!("Still needed: {}. Click one in the view.\n{}", row.what, drives(row.what)));
                            } else {
                                l.backing_on_hover_text(drives(row.what));
                            }
                            if let Some(p) = &row.filled {
                                ui.add(egui::Label::new(egui::RichText::new(p).size(t.font_small).color(t.form_dim)).truncate());
                            } else if row.next {
                                ui.label(egui::RichText::new("click one in the view").size(t.font_small).color(t.form_dim));
                            }
                        });
                    }
                }
                // A pick whose geometry is gone: named in red, kept until it is re-picked.
                for gone in it.missing(app) {
                    message(ui, &Msg { text: format!("Missing: {gone} (deleted or renamed?). Click this card, then its replacement in the view."), error: true });
                }
                for m in k.item_msgs.get(i).into_iter().flatten() {
                    message(ui, m);
                    if let Some(sk) = loose_sketch(app, &m.text) {
                        let b = ui_kit::button(ui, "Edit sketch", ui_kit::Button::Normal).backing_on_hover_text("Open the sketch to dimension it; the dialog comes back when you finish it.");
                        button_hit |= b.hovered();
                        if b.clicked() {
                            edit = Some(sk);
                        }
                    }
                }
            })
            .response;
        let rect = card.rect;
        solidcraft_ui_registry::publish_target(&format!("gear:card:{}", item_id(i)), rect);
        let sense = ui.interact(rect, ui.scope_id().with(("gear_card", i)), egui::Sense::hover());
        if sense.hovered() {
            hovered = Some(i);
            sense.backing_on_hover_text(TIP_CARD);
        }
        if !button_hit && clicked_at.is_some_and(|p| rect.contains(p)) {
            activate = Some(i);
        }
    }
    k.card_hovered = hovered;
    if let Some(i) = hovered.or(k.graph_hovered)
        && let Some(first) = k.items.get(i).and_then(|it| it.picks.first())
    {
        *hover = Some((0, first.clone()));
    }
    if let Some(i) = remove {
        dg::remove_item(k, i, inputs);
    } else if let Some(i) = activate {
        dg::make_active(k, i, inputs);
        *input = 0;
    }
    if let Some(sk) = edit {
        k.action = Some(dg::Action::EditSketch(sk));
    }
}

/// A row of segments under a label.
fn segments(ui: &mut egui::Ui, label: &str, selected: &mut usize, options: &[&str], tip: &str) -> bool {
    let mut changed = false;
    ui.scope(|ui| ui_kit::row(ui, label, |ui| changed = ui_kit::segmented(ui, selected, options, ui_kit::Segments { dense: true, ..Default::default() }))).response.backing_on_hover_text(tip);
    changed
}

/// A field row with its hover text.
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str, tip: &str) -> bool {
    let mut enter = false;
    ui.scope(|ui| enter = hint_row(ui, label, value, hint)).response.backing_on_hover_text(tip);
    enter
}

/// The active gear's settings (§3): the type's own, then per gear (a tab per gear when it makes
/// several): Size by, Module / Teeth, Face, Helix / Hand / V, and More.
fn settings(ui: &mut egui::Ui, k: &mut Gear, a: usize) -> bool {
    let mut enter = false;
    let title = format!("Gear {} settings", item_id(a));
    let n = k.gear_counts.get(a).copied().unwrap_or(1);
    let kind = k.items[a].kind;
    let names: Vec<String> = (0..n).map(|g| crate::dialogs_gear_items::gear_part(k.items[a].kind, g).map_or_else(|| k.gear_name((a, g)), str::to_string)).collect();
    let replies: Vec<_> = k.graph.replies().iter().filter(|r| r.item == a).cloned().collect();
    // The module block this gear is in, if any: its module is the one the gears in it share.
    let blocks = k.graph.blocks().into_iter().enumerate().find(|(_, (_, m))| m.contains(&a)).map(|(b, (_, m))| (b, m.iter().map(|i| item_id(*i)).collect::<Vec<_>>().join(", ")));
    ui_kit::section(ui, ("gear_settings", a), &title, true, |ui| {
        let it = &mut k.items[a];
        // The type's own settings.
        match kind {
            ItemKind::Shaft => {
                enter |= hint_row(ui, "Shaft Ø", &mut it.shaft_d, "auto");
                ui_kit::checkbox(ui, "Keyway", &mut it.keyway);
                return;
            }
            ItemKind::Internal => enter |= hint_row(ui, "Rim", &mut it.rim, "auto: 2.5 × module"),
            ItemKind::Worm => {
                enter |= field(ui, "Starts", &mut it.starts, "1", TIP_STARTS);
                ui_kit::checkbox(ui, "Throated wheel", &mut it.throated);
            }
            ItemKind::Bevel | ItemKind::Hypoid => {
                choice_row(ui, "Style", &format!("gear_bevel_{a}"), &mut it.bevel_style, &["Straight", "Spiral", "Chevron"]).backing_on_hover_text(TIP_V_BEVEL);
                if it.bevel_style > 0 || kind == ItemKind::Hypoid {
                    enter |= field(ui, "Spiral angle", &mut it.spiral, "35 deg", TIP_SPIRAL);
                }
            }
            ItemKind::Rack => enter |= hint_row(ui, "Back height", &mut it.back_height, "auto"),
            ItemKind::Planetary => {
                enter |= field(ui, "Planets", &mut it.planets, "auto", "How many planets: Auto fits as many as there is room for, 3 at most.");
                ui_kit::checkbox(ui, "Carrier", &mut it.carrier);
            }
            _ => {}
        }
        // Which gear the rows below are for.
        if n > 1 {
            let labels: Vec<&str> = names.iter().map(String::as_str).collect();
            let mut tab = k.gear_tab.min(n - 1);
            segments(ui, "Gear", &mut tab, &labels, "Each gear of this item has its own size, face and helix.");
            k.gear_tab = tab;
        }
        let g = k.gear_tab.min(n.saturating_sub(1));
        let reply = replies.iter().find(|r| r.gear == g);
        let s: &mut GearSettings = &mut it.settings_mut(n)[g];
        let block = blocks.as_ref().and_then(|(b, names)| Some((k.graph.block_module_mut(*b)?, names.clone())));
        enter |= gear_rows(ui, s, kind, reply, block);
        enter |= more_rows(ui, s, (a, g));
    });
    enter
}

/// One gear's main rows: Size by, Module / Teeth (the other greyed as worked out), Face, Helix.
fn gear_rows(ui: &mut egui::Ui, s: &mut GearSettings, kind: ItemKind, reply: Option<&solidcraft_gear::items::GearReply>, block: Option<(&mut String, String)>) -> bool {
    let mut enter = false;
    let mut size = usize::from(s.by_teeth);
    segments(ui, "Size by", &mut size, &["Module", "Teeth"], TIP_SIZE_BY);
    s.by_teeth = size == 1;
    let worked = |v: Option<String>| v.map_or_else(|| "— until the gear is complete".to_string(), |v| format!("{v} (worked out)"));
    if s.by_teeth {
        enter |= field(ui, "Teeth", &mut s.teeth, "from the circle", TIP_TEETH);
        ui_kit::caption(ui, "Teeth", &format!("Module: {}", worked(reply.filter(|r| r.module > 0.0).map(|r| dg::fmt(r.module)))));
    } else if let Some((module, members)) = block {
        // In a module block: the block's module is the one its gears share (decision 2).
        enter |= field(ui, "Module", module, "auto", "This gear is in a Module Block: its gears share this one module, so they can mesh. Change it here or in the graph.");
        ui_kit::caption(ui, "Module", &format!("Shared by gears {members} · teeth: {}", worked(reply.filter(|r| r.teeth > 0).map(|r| r.teeth.to_string()))));
    } else {
        enter |= field(ui, "Module", &mut s.module, "auto", TIP_MODULE);
        ui_kit::caption(ui, "Module", &format!("Teeth: {}", worked(reply.filter(|r| r.teeth > 0).map(|r| r.teeth.to_string()))));
    }
    let labels: Vec<&str> = FACE_PRESETS.iter().map(|p| p.2).collect();
    segments(ui, "Face", &mut s.face_preset, &labels, TIP_PRESET);
    match s.face_preset {
        0 => enter |= field(ui, "Each side", &mut s.face_a, "auto", TIP_FACE),
        1 => enter |= field(ui, "Side A", &mut s.face_a, "auto", TIP_FACE),
        2 => enter |= field(ui, "Side B", &mut s.face_b, "auto", TIP_FACE),
        _ => {
            enter |= field(ui, "Side A", &mut s.face_a, "auto", TIP_FACE);
            enter |= field(ui, "Side B", &mut s.face_b, "auto", TIP_FACE);
        }
    }
    if !matches!(kind, ItemKind::Bevel | ItemKind::Hypoid | ItemKind::Worm) {
        enter |= field(ui, "Helix angle", &mut s.helix, "0 = straight", TIP_HELIX);
        if !s.helix.trim().is_empty() && s.helix.trim() != "0" && s.helix.trim() != "0 deg" {
            segments(ui, "Hand", &mut s.hand, &["Auto", "Left", "Right"], TIP_HAND);
            ui_kit::checkbox(ui, "V teeth (herringbone)", &mut s.v).backing_on_hover_text(TIP_V);
        }
    }
    enter
}

/// More (closed): profile shift, bore, hub.
fn more_rows(ui: &mut egui::Ui, s: &mut GearSettings, (a, g): (usize, usize)) -> bool {
    let mut enter = false;
    ui_kit::section(ui, ("gear_more", a, g), "More", false, |ui| {
        enter |= field(ui, "Profile shift", &mut s.profile_shift, "auto", TIP_SHIFT);
        choice_row(ui, "Bore", &format!("gear_bore_{a}_{g}"), &mut s.bore, &["Auto", "None", "Ø"]).backing_on_hover_text(TIP_BORE);
        if s.bore == 2 {
            enter |= field(ui, "Bore Ø", &mut s.bore_d, "mm", TIP_BORE);
        }
        ui_kit::checkbox(ui, "Hub", &mut s.hub);
        if s.hub {
            enter |= hint_row(ui, "Hub Ø", &mut s.hub_d, "auto");
            enter |= hint_row(ui, "Hub length A", &mut s.hub_a, "0");
            enter |= hint_row(ui, "Hub length B", &mut s.hub_b, "0");
        }
    });
    enter
}

/// Drivetrain: the links as rows ("A ⟷ B  mesh  1.50 : 1", "B ═ C  same shaft"), the suggested
/// ones (dim) with one Accept button, a link added by hand, the driver with its rpm and torque,
/// the ratio between two gears, and the graph on request.
fn drivetrain(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear) -> bool {
    let mut enter = false;
    let gears = k.all_gears();
    if gears.len() < 2 && k.graph.links().is_empty() {
        return false;
    }
    let t = Tokens::get();
    let links = k.graph.links();
    let suggested = k.graph.suggested().to_vec();
    let title = format!("Drivetrain · {} link{}", links.len(), if links.len() == 1 { "" } else { "s" });
    ui_kit::section(ui, "gear_drivetrain", &title, true, |ui| {
        let mut drop = None;
        if links.is_empty() && suggested.is_empty() {
            ui_kit::text(ui, "No links yet: add one below. Touching circles and circles on one shaft are suggested here.", ui_kit::Text::Note("gear_no_links"));
        }
        for (a, ag, b, bg, kind) in &links {
            let (x, y) = ((*a, *ag), (*b, *bg));
            let note = k.graph.notes().iter().find(|n| n.kind == *kind && (((n.a, n.a_gear) == x && (n.b, n.b_gear) == y) || ((n.a, n.a_gear) == y && (n.b, n.b_gear) == x))).cloned();
            ui.horizontal(|ui| {
                let joint = if *kind == "shaft" { "═" } else { "⟷" };
                ui.label(egui::RichText::new(format!("{}  {joint}  {}", k.gear_name(x), k.gear_name(y))).size(t.font_label).strong().color(t.form_ink));
                let what = if *kind == "shaft" { "same shaft" } else { "mesh" };
                ui.label(egui::RichText::new(what).size(t.font_small).color(t.form_dim)).backing_on_hover_text(if *kind == "shaft" { TIP_SHAFT } else { TIP_MESH });
                if let Some(r) = note.as_ref().and_then(|n| n.ratio).filter(|_| *kind == "mesh") {
                    let turn = if r < 0.0 { " opposite" } else { " same way" };
                    ui.label(egui::RichText::new(format!("{} : 1", dg::fmt_ratio(r.abs()))).size(t.font_label).color(t.form_ink));
                    ui.label(egui::RichText::new(turn).size(t.font_small).color(t.form_dim));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui_kit::compact_button(ui, "×", "Remove this link", true, false).clicked() {
                        drop = Some((x, y, *kind));
                    }
                });
            });
            if let Some(e) = note.and_then(|n| n.error.map(|e| (e, n.ratio))) {
                message(ui, &Msg { text: dg::worded(&e.0), error: true });
            }
        }
        if let Some((x, y, kind)) = drop {
            k.graph.remove_link(x, y, kind);
        }
        if !suggested.is_empty() {
            for s in &suggested {
                let joint = if s.kind == "shaft" { "═" } else { "⟷" };
                let what = if s.kind == "shaft" { "same shaft" } else { "mesh" };
                ui.label(egui::RichText::new(format!("{}  {joint}  {}   {what} · suggested", k.gear_name((s.a, s.a_gear)), k.gear_name((s.b, s.b_gear)))).size(t.font_label).italics().color(t.form_dim));
            }
            let r = ui_kit::button(ui, format!("Accept suggested ({})", suggested.len()), ui_kit::Button::Normal).backing_on_hover_text(TIP_ACCEPT);
            solidcraft_ui_registry::publish_target("gear:accept", r.rect);
            if r.clicked() {
                k.graph.accept_all();
            }
        }
        // A link by hand: gear, kind, gear.
        let labels: Vec<String> = gears.iter().map(|g| k.gear_name(*g)).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let (mut x, mut kind, mut y) = k.link_add;
        x = x.min(gears.len() - 1);
        y = y.min(gears.len() - 1);
        ui_kit::section(ui, "gear_add_link", "Add a link by hand", links.is_empty() && suggested.is_empty(), |ui| {
            choice_row(ui, "Gear", "gear_link_a", &mut x, &labels);
            segments(ui, "Link", &mut kind, &["Mesh", "Same shaft"], "Mesh: the teeth engage. Same shaft: the two gears turn together.");
            choice_row(ui, "With gear", "gear_link_b", &mut y, &labels);
            let add = ui_kit::button(ui, "Add link", ui_kit::Button::Enabled(x != y)).backing_on_hover_text("Link the two gears: mesh (teeth engage) or same shaft (they turn together).");
            solidcraft_ui_registry::publish_target("gear:add_link", add.rect);
            if add.clicked() && x != y {
                let ((a, ag), (b, bg)) = (gears[x], gears[y]);
                k.graph.accept(&crate::dialogs_gear_graph::Suggested { a, a_gear: ag, b, b_gear: bg, kind: if kind == 1 { "shaft".into() } else { "mesh".into() } });
            }
        });
        k.link_add = (x, kind, y);
        // The driver, its rpm and torque.
        let mut at = k.drive_item.and_then(|d| gears.iter().position(|x| *x == d)).map_or(0, |p| p + 1);
        let mut with_none: Vec<&str> = vec!["—"];
        with_none.extend(labels.iter().copied());
        choice_row(ui, "Driver", "gear_driver", &mut at, &with_none).backing_on_hover_text(TIP_DRIVER);
        k.drive_item = at.checked_sub(1).and_then(|p| gears.get(p).copied());
        if k.drive_item.is_some() {
            enter |= field(ui, "Input rpm", &mut k.rpm, "optional", TIP_RPM);
            enter |= field(ui, "Input torque", &mut k.torque, "N·m, optional", TIP_RPM);
        }
        // The ratio between two gears.
        for (n, (key, title)) in [("gear_ratio_a", "Ratio from"), ("gear_ratio_b", "to")].into_iter().enumerate() {
            let mut at = k.ratio_pick[n].and_then(|p| gears.iter().position(|x| *x == p)).map_or(0, |p| p + 1);
            choice_row(ui, title, key, &mut at, &with_none).backing_on_hover_text("The ratio is the second gear's speed over the first's, with the direction and the path between them.");
            k.ratio_pick[n] = at.checked_sub(1).and_then(|p| gears.get(p).copied());
        }
        match &k.ratio {
            Some((_, Ok(lines))) => {
                for l in lines {
                    let (label, value) = l.split_once('\t').unwrap_or(("", l.as_str()));
                    ui_kit::row(ui, label, |ui| ui.label(egui::RichText::new(value).size(t.font_label).strong().color(t.form_ink)));
                }
            }
            Some((_, Err(e))) => message(ui, &Msg { text: e.clone(), error: false }),
            None => {}
        }
        for note in &k.notes {
            ui_kit::text(ui, &dg::worded(note), ui_kit::Text::Note("gear_note"));
        }
        // The graph, on request.
        ui_kit::checkbox(ui, "Show graph", &mut k.show_graph).backing_on_hover_text("The same links as a node graph: drag from a ● Mesh pin to another to mesh two gears; right-click for module blocks.");
        if k.show_graph {
            let len = dg::len_of(app);
            let size = egui::vec2(ui.available_width().max(240.0), 260.0);
            let marks = crate::dialogs_gear_graph::Marks { driver: k.drive_item, picked: k.ratio_pick, len: &len };
            let out = crate::dialogs_gear_graph::graph_ui(ui, &k.items, &k.gear_counts, &mut k.graph, marks, size, "sc_gear_graph_docked");
            if out.driver.is_some() {
                k.drive_item = out.driver;
            }
            if let Some(c) = out.clicked {
                dg::pick_for_ratio(k, c);
            }
            k.graph_hovered = out.hovered;
        } else {
            k.graph_hovered = None;
        }
    });
    enter
}

/// Output: the operation, the Targets (when it needs them), the keep-out and the assembly.
fn output(ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize) -> bool {
    let mut enter = false;
    ui_kit::section(ui, "gear_output", "Output", true, |ui| {
        ui_kit::checkbox(ui, "Assembly (components, joints and gear ratios)", &mut k.assembly).backing_on_hover_text(TIP_ASSEMBLY);
        choice_row(ui, "Operation", "gear_op", &mut k.operation, &OP_LABELS).backing_on_hover_text(TIP_OPERATION);
        ui_kit::checkbox(ui, "Cut keep-out from Targets", &mut k.cut_keep_out).backing_on_hover_text(TIP_KEEP_OUT);
        if k.cut_keep_out {
            enter |= field(ui, "Keep-out gap", &mut k.keep_out_gap, "auto", TIP_GAP);
        }
        if k.operation != 0 || k.cut_keep_out {
            let n = inputs.get(1).map_or(0, |i| i.items.len());
            let text = if n == 0 { "↖ Select bodies".to_string() } else { format!("{n} selected") };
            let chip = ui_kit::row(ui, "Targets", |ui| ui_kit::selection_field(ui, &text, ui_kit::Selection { active: *input == 1, filled: n > 0 }));
            solidcraft_ui_registry::publish_target("gear:targets", chip.rect);
            if chip.clicked() {
                *input = 1;
            }
        }
    });
    enter
}

/// Results (closed): one row per gear.
fn results(app: &SolveApp, ui: &mut egui::Ui, k: &Gear) {
    let replies = k.graph.replies();
    if replies.is_empty() {
        return;
    }
    let t = Tokens::get();
    let len = dg::len_of(app);
    ui_kit::section(ui, "gear_results", &format!("Results · {} gears", replies.len()), false, |ui| {
        for r in replies {
            let mut parts = vec![format!("z{}", r.teeth), format!("m{}", dg::fmt(r.module)), format!("Ø{}", len(r.pitch_d))];
            if let Some(x) = r.ratio {
                parts.push(format!("{} : 1", dg::fmt_ratio(x.abs())));
            }
            if let Some(n) = r.rpm {
                parts.push(format!("{} rpm", dg::fmt((n * 10.0).round() / 10.0)));
            }
            if let Some(v) = r.torque {
                parts.push(format!("{} N·m", dg::fmt((v * 100.0).round() / 100.0)));
            }
            if let Some(c) = r.contact_ratio {
                parts.push(format!("contact {}", dg::fmt((c * 100.0).round() / 100.0)));
            }
            ui_kit::row(ui, &k.gear_name((r.item, r.gear)), |ui| {
                ui.add(egui::Label::new(egui::RichText::new(parts.join(" · ")).size(t.font_small).color(t.form_ink)).truncate())
            })
            .backing_on_hover_text(crate::dialogs_gear_graph::reply_tip(r));
        }
    });
}

/// Advanced (closed): the defaults every gear takes.
fn advanced(ui: &mut egui::Ui, k: &mut Gear) -> bool {
    let mut enter = false;
    ui_kit::section(ui, "gear_advanced", "Advanced", false, |ui| {
        for (key, label, _, hint, tip) in dg::ADVANCED {
            enter |= field(ui, label, k.values.entry(key.to_string()).or_default(), hint, tip);
        }
        ui_kit::checkbox(ui, "Equation teeth", &mut k.equations).backing_on_hover_text(TIP_EQUATIONS);
    });
    enter
}
