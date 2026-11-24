//! The Gears dialog's form (GEAR_UI_SPEC.md §1–§5), laid out as tabbed operation dialogs are: a row
//! of icon tabs at the top, the tab's name and the **status line** (always what to do next, or
//! why the last pick was refused) under it, and the tab's titled groups scrolling below; OK and
//! Cancel stay at the foot. A group a checkbox switches on (the housing cutout, a hub, the tooth
//! chamfer, the keep-out, a differential) shows its rows only while it is on.
//!
//! - **Gears**: the type and **New gear**, a **card** per gear (its colour, name and type; while
//!   it is picked its checklist; once complete one line with its teeth and module; its messages
//!   with what to do; click a card to make it the one picks go to, × to remove it), and under the
//!   cards the selected gear's own settings and size (Size by, Module / Teeth, Face, and its Teeth:
//!   Straight / Helical / Herringbone with the helix angle). A bevel or
//!   hypoid pair has the switch for a differential in its ring gear here. Under the cards, the
//!   **blocks**: gears put in a block share its module, its shaft (Make shaft: one per axis, fused to its gears) or bore (Bore only),
//!   face width and, under More, helix, pressure angle, backlash and tooth chamfer.
//! - **Teeth**: the selected bevel's style and spiral, each gear's profile shift; the
//!   tooth form every gear takes (pressure angle, backlash, tip rounding, XY compensation); the
//!   tooth chamfer.
//! - **Shafts & Bores**: the shafts (solid rods along an axis: each block shaft, fused to its
//!   gears or turning free) and their bearing points, kept apart from each gear's bore (the hole
//!   through it: its Ø and where it comes from), and the selected gear's bore and hub.
//! - **Links & Motion**: the links (each drawn in the view; hovering a row lights it), the
//!   suggested ones, a link by hand, the drive and the ratio between two gears.
//! - **Housing & Output**: the housing cutout, the operation, Targets, keep-out and assembly.
//! - **Differential** (while a ring gear carries one): its style, pinions, module, half-shafts
//!   and facts.
//! - **Results**: each gear's numbers, every warning, the planner's notes and facts.
//!
//! The selected gear (its card on the Gears tab, or the Gear row at the top of Teeth and Shafts)
//! is the one the per-gear tabs show; choosing it there selects it as clicking its card does.
//! The data and the request are `dialogs_gear`'s; this file only draws and edits them, with the
//! app's ui_kit rows, fields, buttons, sections and hover texts.

use solidcraft_engine::Sel;
use solidcraft_gear::items::ItemKind;
use solidcraft_theme::Tokens;

use crate::SolveApp;
use crate::dialogs::OP_LABELS;
use crate::dialogs_gear::{self as dg, Gear, Msg, Tab};
use crate::dialogs_gear_items::{DiffForm, FACE_PRESETS};
use crate::dialogs_gear_links::{GearSettings, item_id};
use crate::selection::SelInput;
use crate::ui_kit::{self, BackingTooltipExt as _, choice_row, hint_row};

const TIP_TYPE: &str = "What the next gear will be. Each type lists the sketch pieces it needs.";
const TIP_NEW: &str = "Start a new gear of this type, then click the sketch pieces it asks for in the view.";
const TIP_CARD: &str = "Click to make this the gear your picks go to. Hover to light its geometry.";
const TIP_SIZE_BY: &str = "Set the module (tooth size) or the tooth count; the other follows from the pitch circle.";
const TIP_MODULE: &str = "Tooth size in mm (pitch Ø ÷ teeth). Meshing gears share one module: set it on one gear and its mates follow. Auto: the largest standard module giving the smallest meshing gear 17 teeth or more.";
const TIP_TEETH: &str = "Number of teeth. The pitch circle stays as drawn, so the module follows; its mates take the same module.";
const TIP_TEETH_FREE: &str = "Number of teeth of this gear. No circle gives its size, so the teeth and the module are both yours: the pitch diameter is module × teeth, and the two gears' teeth set the ratio.";
const TIP_PAIR_MODULE: &str = "The module of both gears of the pair: gears mesh only at one module. Each gear's teeth are set below.";
const TIP_FACE: &str = "How far the teeth reach from the sketch plane along the axis, on this side. Auto: 5 × module each side (10 × module in all); A only / B only: 10 × module on that side. The value is per side: 10 mm on each side gives a 20 mm face.";
const TIP_PRESET: &str =
    "Symmetric: equal both sides. A only / B only: the gear grows out of one face of the sketch (side A is the green arrow in the view).";
const TIP_HELIX: &str = "Tooth angle to the axis. 0 = straight. Meshing gears get equal angles of opposite hand.";
const TIP_HAND: &str = "Left or right helix. The mating gear is set to the other hand for you.";
const TIP_TOOTH_STYLE: &str = "Straight teeth, helical (slanted: quieter, with axial thrust), or herringbone (two opposite helices meeting in the middle: no axial thrust). Meshing gears take opposite hands, worked out.";
const TIP_V_BEVEL: &str = "Straight: straight teeth. Spiral: curved teeth, smoother and quieter. Chevron: the spiral changes hand at mid-face, so the axial thrust cancels.";
const TIP_SHIFT: &str = "Worked out for you: it holds the drawn centre distance and clears undercut on small gears.";
const TIP_BORE: &str = "The bore: the hole through this gear. Auto: a bore circle drawn on the gear, else one that fits its shaft line when one is picked, else its block's Bore only Ø; none on a made shaft (the gear and the shaft are one body). Ø: this diameter. A drawn bore circle wins.";
const TIP_SPIRAL: &str = "Spiral bevel/hypoid tooth angle at mid-face. 35° is typical.";
const TIP_STARTS: &str = "Thread starts on the worm. More starts = faster, less self-locking.";
const TIP_DRIVER: &str =
    "The gear you turn. Every other gear's speed and direction follow from it. — : none; the assembly is driven from the first gear.";
const TIP_RPM: &str = "Optional: fills speed and torque for every gear in Results.";
const TIP_ACCEPT: &str = "Make every suggested link real. Suggested: circles that touch in the drawing (mesh) or gears on one axis (same shaft).";
const TIP_OPERATION: &str = "New Body: separate bodies. Join/Cut/Intersect: combine with the Targets.";
const TIP_KEEP_OUT: &str = "Cut the space the gears sweep (plus the gap) out of the Targets, e.g. a housing.";
const TIP_GAP: &str = "Extra room around the swept teeth when cutting the housing.";
const TIP_ASSEMBLY: &str = "Each shaft and its gears become a component on a revolve joint, linked to its mates by the gear ratio.";
const TIP_TORQUE: &str = "Optional: the driver's torque in N·m. Fills torque for every gear in Results (losses included).";
const TIP_RIM: &str = "Rim thickness outside the tooth roots. Auto: 2.5 × module.";
const TIP_BACK: &str = "Material behind the rack's tooth roots. Auto from the module.";
const TIP_CARRIER: &str = "Make a carrier body: the planets turn on its pins. Off: the planets have no carrier.";
const TIP_SHAFT_D: &str = "The shaft's diameter (a solid rod along this line). Auto: from the smallest root of the gears it carries.";
const TIP_HUB: &str = "A boss on the gear's face round its bore. A drawn hub circle sets its Ø.";
const TIP_HUB_D: &str = "Hub diameter (mm), smaller than the tooth roots. Auto: 1.6 × the bore (or about half the root Ø with no bore).";
const TIP_HUB_LEN: &str = "How far the hub reaches past the face on this side. Both empty: half the face width on side A.";
const TIP_BORE_D: &str = "Bore diameter (mm). Must stay inside the tooth roots.";
const TIP_CHAMFER: &str = "A 45° chamfer off the tooth tips' edges on the end faces of every gear in the set (mm across, as deep): spur, helical, herringbone (outer ends), compound layers and ring gears. Bevel, worm, hypoid, crossed helical, rack and planetary gears stay plain (a note says so). Empty: none.";
const TIP_CUTOUT: &str = "One more body, \"Housing cutout\": all the space the drivetrain needs to turn (the teeth's swept envelopes, the shafts through the walls) and a bearing seat at every bearing point. Model a housing block and cut this body out of it.";
const TIP_BEARING: &str = "One bearing size for every seat (bore × OD × width, mm).";
const TIP_EQUATIONS: &str =
    "Exact involute curves instead of fitted splines: slower, smoother. Only for a set made before Gears features (a folder of features).";
const TIP_MESH: &str = "Mesh: the two gears' teeth engage; their distance is checked.";
const TIP_SHAFT: &str = "Same shaft: the two gears turn together.";
const TIP_PICKER: &str = "The gear these settings are for. Choosing one selects it, as clicking its card on the Gears tab does: your picks go to it.";
const TIP_DIFF: &str = "Put a differential in this pair's ring gear (the hypoid's ring, the bevel pair's larger gear): a case rigid with the ring, two side gears with a half-shaft each on the ring's axis, and pinions on the case. Its settings are on the Differential tab.";
const TIP_BLOCK: &str = "A block puts gears under shared settings: set the module, the shaft or bore, or the face width once and every gear in it follows. A gear's own value (in its settings below) wins over the block's.";
const TIP_BLOCK_GEARS: &str = "The gears in this block. A gear is in one block at most: ticking it here takes it out of any other.";
const TIP_BLOCK_MODULE: &str = "One module for every gear in the block, so they can mesh. Auto: the largest standard module giving the smallest gear in the block 17 teeth or more.";
const TIP_BLOCK_SHAFT_MODE: &str = "What the block's Ø makes. Make shaft: a shaft (a solid rod) along each axis its gears are on, the gears on it fused to it, one body per axis: no bore is cut. Bore only: every gear gets a bore (a hole) of the Ø and no shaft is made, for a rod of your own. None: the gears keep their own bores.";
const TIP_BLOCK_SHAFT: &str =
    "The shafts' diameter. Each runs over its gears' faces and a little past them, or out to the bearing points on its axis.";
const TIP_BLOCK_BORE: &str = "The bore (the hole through each gear) of every gear in the block that leaves its own bore on Auto.";
const TIP_BLOCK_FACE: &str = "The whole face width of every gear in the block, half each side of the sketch plane (A only / B only gears: all on that side). Empty: each gear's own, or Auto.";
const TIP_BLOCK_HELIX: &str = "Helix angle for every spur, ring, rack, compound and planetary gear in the block whose teeth are on Auto, and the spiral angle of its straight bevel pairs; meshing gears get opposite hands. A gear's own helix wins (a bevel pair's own spiral angle; 0 keeps it straight). Hypoids keep their own spiral.";
const TIP_BLOCK_TEETH: &str = "The teeth of every gear in the block that leaves its own on Auto: helical, or herringbone (two opposite helices: no axial thrust), at the block's helix angle. Its bevel pairs get spiral teeth at that angle (chevron for herringbone). Straight: each gear's own teeth.";
const TIP_BLOCK_PA: &str = "Pressure angle of the block's gears (20° is standard). Empty: the set's (Teeth tab).";
const TIP_BLOCK_BACKLASH: &str = "Backlash between the block's meshing gears. Empty: the set's (Teeth tab).";
const TIP_BLOCK_CHAMFER: &str = "A 45° tooth chamfer on every gear in the block that takes one, in place of the set's (Teeth tab). Empty: the set's.";
const TIP_DIFF_STYLE: &str = "Open: bevel side gears and spider pinions. Biased: uneven side gears, so it biases torque. Torsen B: helical elements (a planetary). Torsen A: worm elements.";
const TIP_DIFF_PINIONS: &str = "Spider pinions on the case, 1 to 6. Auto: 2.";
const TIP_DIFF_ELEMENTS: &str = "Element gear pairs, 1 to 8. Auto: 3 (Torsen B) or 2 (Torsen A).";
const TIP_DIFF_MODULE: &str = "The side gears' and pinions' module (mm). Auto: the largest whose case is at most 0.6 × the ring's pitch Ø.";
const TIP_DIFF_AXLE: &str = "Each half-shaft's reach from the differential's middle along the ring's axis (mm). Auto: 1.5 × the ring's pitch Ø, or to the ends of a Shaft drawn on that axis.";
const TIP_DIFF_AXLE_D: &str = "The half-shafts' diameter (mm). Auto: that Shaft's, else 0.4 × the side gears' pitch Ø.";

/// What each type needs, for the line under the type list.
fn needs(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Spur | ItemKind::Helical | ItemKind::Herringbone => "Needs: a pitch circle. Optional: its shaft line.",
        ItemKind::Internal => "Needs: the ring's pitch circle. Optional: its pinion's circle inside it.",
        ItemKind::Rack => "Needs: the rack's pitch line. Optional: its pinion's circle.",
        ItemKind::Compound => "Needs: a shaft line and two or more circles on it (in parallel sketches).",
        ItemKind::Planetary => "Needs: the sun's and the ring's circles, then one planet's circle.",
        ItemKind::Bevel => {
            "Needs: two circles on crossing axes, or two axis lines and a circle, or (side view) the apex point and the two shaft lines from it, in any order."
        }
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

/// The type list and the New gear button, with what the type needs.
fn header(ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize) {
    let options: Vec<(&str, &str)> = dg::TYPES.iter().map(|(kind, icon)| (kind.label(), *icon)).collect();
    ui_kit::row(ui, "Type", |ui| ui_kit::dropdown_pictures(ui, "gear_type", &mut k.new_kind, &options)).backing_on_hover_text(TIP_TYPE);
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
            ui.add(
                egui::Label::new(egui::RichText::new(format!("{}  {text}", if bad { "⚠" } else { "→" })).size(t.font_label).color(ink).strong())
                    .wrap(),
            );
        })
        .response;
    solidcraft_ui_registry::publish_rect("gear:status", r.rect);
    solidcraft_ui_registry::publish_rect(&format!("gear:status:{text}"), r.rect);
}

/// An old set: what it is, and the button that makes its circles gears.
fn legacy(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear) {
    let Some(l) = k.legacy.clone() else { return };
    ui_kit::text(
        ui,
        &format!("Made with the earlier Gears dialog: {}. It still follows its sketch; OK keeps it as it is.", l.what),
        ui_kit::Text::Note("gear_legacy"),
    );
    if l.convertible {
        let r = ui_kit::button(ui, "Convert to gears", ui_kit::Button::Normal).backing_on_hover_text(
            "Make each of its circles a gear of the new kind, to change them one by one. The set is replaced by the gears on OK.",
        );
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
                egui::Label::new(
                    egui::RichText::new("Then link them on the Links & Motion tab: touching circles are suggested for you.")
                        .size(t.font_small)
                        .color(t.form_dim),
                )
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
            let r = k.links.replies().iter().find(|r| (r.item, r.gear) == (i, g))?;
            (r.teeth > 0).then(|| {
                let name = if n > 1 { format!("{} ", k.gear_name((i, g))) } else { String::new() };
                let b = k.built.iter().find(|b| (b.item, b.gear) == (i, g));
                let bore = b.and_then(|b| b.bore).map(|d| format!(" · bore {}", dg::fmt(d))).unwrap_or_default();
                let hub = b.and_then(|b| b.hub).map(|d| format!(" · hub {}", dg::fmt(d))).unwrap_or_default();
                format!("{name}z{} m{}{bore}{hub}", r.teeth, dg::fmt(r.module))
            })
        })
        .collect();
    if parts.is_empty() { "complete".into() } else { parts.join(" · ") }
}

/// One card per gear.
fn cards(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize, hover: &mut Option<(usize, Sel)>) {
    let t = Tokens::get();
    let len = dg::len_of(app);
    let (mut activate, mut remove, mut edit, mut use_detail, mut drop_detail) = (None, None, None, None, None);
    let mut fix: Option<(usize, dg::Fix)> = None;
    let mut hovered = None;
    let clicked_at = ui.input(|i| if i.pointer.primary_clicked() { i.pointer.interact_pos() } else { None });
    for i in 0..k.items.len() {
        let it = k.items[i].clone();
        let active = k.active == Some(i) && *input == 0;
        let complete = it.complete(app);
        let lit = k.card_hovered == Some(i) || k.link_hovered.is_some_and(|l| l.0 == i || l.2 == i);
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
                    // A card's identity icon, at the kit's card size (as `status_header`).
                    let (icon, _) = ui.allocate_exact_size(egui::vec2(t.form_icon_size, t.form_icon_size), egui::Sense::hover());
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
                            let line = summary(k, i);
                            ui.add(egui::Label::new(egui::RichText::new(line.clone()).size(t.font_small).color(t.form_dim)).truncate()).backing_on_hover_text(line);
                        }
                    });
                });
                // While it is picked (or incomplete): its checklist.
                if active || !complete {
                    // A compound's circles are its gears: "A1 pitch circle (first layer)".
                    let mut circle_no = 0;
                    for row in it.slot_rows(app, &len) {
                        let layer_name = (it.kind == ItemKind::Compound && row.what == "pitch circle").then(|| {
                            circle_no += 1;
                            let layer = match circle_no {
                                1 => "first layer".to_string(),
                                2 => "second layer".to_string(),
                                n => format!("layer {n}"),
                            };
                            format!("{}{circle_no} pitch circle ({layer})", item_id(i))
                        });
                        let (mark, ink) = match (&row.filled, row.next, row.required) {
                            (Some(_), _, _) => ("✓", t.form_success),
                            (None, true, _) => ("→", t.form_primary),
                            (None, false, true) => ("●", t.form_ink),
                            (None, false, false) => ("○", t.form_dim),
                        };
                        ui.horizontal(|ui| {
                            ui.add_space(t.space_10);
                            ui.label(egui::RichText::new(mark).size(t.font_label).color(ink).strong());
                            let what = match (row.required || row.filled.is_some(), row.repeats) {
                                _ if layer_name.is_some() => layer_name.clone().unwrap_or_default(),
                                (true, _) => row.what.to_string(),
                                (false, true) => format!("{} (optional, repeats)", row.what),
                                (false, false) => format!("{} (optional)", row.what),
                            };
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
                // Its gears' bore and hub circles: those the drawing has (taken by themselves for a
                // gear made here, with × to take one out; offered with Use on a reopened one). A
                // gear's rows show only where its sketch has such a circle.
                if (active || !complete) && solidcraft_gear::items::takes_details(it.kind) && it.gear_count(app) > 0 {
                    let picked = it.detail_things(app);
                    let n = it.gear_count(app);
                    for g in 0..n {
                        for (slot, what) in solidcraft_gear::items::DETAIL_SLOTS {
                            let hub = slot == "hub";
                            let name = if n > 1 { format!("{} {what}", k.gear_name((i, g))) } else { what.to_string() };
                            let mine = picked.iter().find(|d| d.0 == g && d.1 == hub);
                            let offer = k.details_suggested.iter().find(|d| (d.item, d.gear, d.hub) == (i, g, hub));
                            if mine.is_none() && offer.is_none() {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                ui.add_space(t.space_10);
                                let (mark, ink) = if mine.is_some() { ("✓", t.form_success) } else { ("○", t.form_dim) };
                                ui.label(egui::RichText::new(mark).size(t.font_label).color(ink).strong());
                                ui.label(egui::RichText::new(name.clone()).size(t.font_label).color(t.form_ink))
                                    .backing_on_hover_text("A circle drawn on the gear's centre, inside its pitch circle: the smaller is its bore (a hole through it), the larger its hub.");
                                if let Some((_, _, th, sel)) = mine {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let x = ui_kit::compact_button(ui, "×", "Take this circle out: the gear gets no bore or hub from it", true, false);
                                        solidcraft_ui_registry::publish_target(&format!("gear:drop:{}:{g}:{slot}", item_id(i)), x.rect);
                                        button_hit |= x.hovered();
                                        if x.clicked() {
                                            drop_detail = Some((i, sel.clone()));
                                        }
                                        ui.add(egui::Label::new(egui::RichText::new(crate::dialogs_gear_items::thing_label(app, th, &len)).size(t.font_small).color(t.form_dim)).truncate());
                                    });
                                } else if let Some(d) = offer {
                                    ui.label(egui::RichText::new(format!("Ø{}", len(d.diameter))).size(t.font_small).color(t.form_dim));
                                    let b = ui_kit::button(ui, "Use", ui_kit::Button::Normal)
                                        .backing_on_hover_text("The drawing has this circle on the gear's centre: use it.");
                                    solidcraft_ui_registry::publish_target(&format!("gear:use:{}:{g}:{slot}", item_id(i)), b.rect);
                                    button_hit |= b.hovered();
                                    if b.clicked() {
                                        use_detail = Some(d.clone());
                                    }
                                }
                            });
                        }
                    }
                }
                // A pick whose geometry is gone: named in red, kept until it is re-picked.
                for gone in it.missing(app) {
                    message(ui, &Msg { text: format!("Missing: {gone} (deleted or renamed?). Click this card, then its replacement in the view."), error: true });
                }
                for m in k.item_msgs.get(i).into_iter().flatten() {
                    message(ui, m);
                    // What the message suggests, one click each.
                    let fixes = dg::fixes_in(k, &m.text);
                    if !fixes.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for f in fixes {
                                let b = ui_kit::button(ui, f.label(k), ui_kit::Button::Normal).backing_on_hover_text("Change the setting to this; the set is worked out again.");
                                button_hit |= b.hovered();
                                if b.clicked() {
                                    fix = Some((i, f.clone()));
                                }
                            }
                        });
                    }
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
    if let Some(i) = hovered
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
    if let Some(d) = use_detail {
        dg::use_detail(app, k, &d, inputs);
    }
    if let Some((i, sel)) = drop_detail {
        dg::drop_detail(k, i, &sel, inputs);
    }
    if let Some((i, f)) = fix {
        dg::apply_fix(k, i, &f, inputs);
    }
    if let Some(sk) = edit {
        k.action = Some(dg::Action::EditSketch(sk));
    }
}

/// A row of segments under a label.
fn segments(ui: &mut egui::Ui, label: &str, selected: &mut usize, options: &[&str], tip: &str) -> bool {
    let mut changed = false;
    ui.scope(|ui| {
        ui_kit::row(ui, label, |ui| changed = ui_kit::segmented(ui, selected, options, ui_kit::Segments { dense: true, ..Default::default() }))
    })
    .response
    .backing_on_hover_text(tip);
    changed
}

/// A field row with its hover text.
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str, tip: &str) -> bool {
    let mut enter = false;
    ui.scope(|ui| enter = hint_row(ui, label, value, hint)).response.backing_on_hover_text(tip);
    enter
}

// ---------------------------------------------------------------------------------------------
// The tab row and the groups
// ---------------------------------------------------------------------------------------------

/// A tab's handle (its target in the UI registry): `gear:tab:<key>`.
pub(crate) fn tab_handle(tab: Tab) -> &'static str {
    match tab {
        Tab::Gears => "gear:tab:gears",
        Tab::Teeth => "gear:tab:teeth",
        Tab::Shafts => "gear:tab:shafts",
        Tab::Motion => "gear:tab:motion",
        Tab::Housing => "gear:tab:housing",
        Tab::Differential => "gear:tab:differential",
        Tab::Results => "gear:tab:results",
    }
}

/// Whether a tab has something to look at: an error or a missing pick on Gears, a warning on
/// Results (a dot on its icon).
fn tab_flag(app: &SolveApp, k: &Gear, tab: Tab) -> Option<egui::Color32> {
    let t = Tokens::get();
    match tab {
        Tab::Gears if k.items.iter().any(|it| !it.missing(app).is_empty()) || k.item_msgs.iter().flatten().any(|m| m.error) => Some(t.error),
        Tab::Results if !k.set_msgs.is_empty() || k.item_msgs.iter().any(|m| !m.is_empty()) => Some(t.warning),
        _ => None,
    }
}

/// The row of icon tabs: one per tab shown, its name on hover and
/// as the heading under the row.
fn tab_row(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear) {
    let t = Tokens::get();
    let shown = dg::tabs(k);
    let options: Vec<ui_kit::IconSegment> =
        shown.iter().map(|tab| ui_kit::IconSegment { key: tab_handle(*tab), icon: tab.icon(), tip: tab.tip().to_string() }).collect();
    let mut at = shown.iter().position(|x| *x == k.tab).unwrap_or(0);
    let left = ui.cursor().min;
    let width = ui.available_width();
    if ui_kit::segmented_icons(ui, &mut at, &options, &[]) {
        k.tab = shown.get(at).copied().unwrap_or_default();
    }
    // A dot on a tab with something to look at.
    let w = width / shown.len().max(1) as f32;
    for (n, tab) in shown.iter().enumerate() {
        if let Some(ink) = tab_flag(app, k, *tab) {
            let c = egui::pos2(left.x + w * (n as f32 + 1.0) - t.space_6, left.y + t.space_6);
            ui.painter().circle_filled(c, t.space_2 + 1.0, ink);
        }
    }
    ui_kit::section_heading(ui, k.tab.label());
}

/// A group switched on by the checkbox at its head: its rows show, indented
/// under a rule, only while it is on. Its checkbox's handle is `gear:group:<key>`.
fn check_group<R>(ui: &mut egui::Ui, key: &str, title: &str, on: &mut bool, tip: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let t = Tokens::get();
    ui.add_space(t.space_2);
    let r = ui_kit::checkbox(ui, title, on).backing_on_hover_text(tip);
    solidcraft_ui_registry::publish_target(&format!("gear:group:{key}"), r.rect);
    if !*on {
        return None;
    }
    let inner = ui.indent(("gear_group", key), body);
    let rect = inner.response.rect;
    let x = rect.left() - t.space_8;
    ui.painter().vline(x, rect.top()..=rect.bottom(), egui::Stroke::new(1.0, t.dialog_rule));
    Some(inner.inner)
}

/// The scroll areas of every tab back at their tops (the dialog just opened).
fn scrolls_to_top(ui: &egui::Ui) {
    for tab in Tab::ALL {
        let id = ui.make_persistent_id(egui::IdSalt::new(scroll_salt(tab)));
        egui::scroll_area::State::default().store(ui.ctx(), id);
    }
}

fn scroll_salt(tab: Tab) -> String {
    format!("gear_tab_body_{}", tab.key())
}

/// The dialog's form: the tab row and the status line stay at the top, the tab's groups scroll
/// under them. True when Enter was pressed in a field.
pub fn rows(
    app: &mut SolveApp,
    ui: &mut egui::Ui,
    k: &mut Gear,
    inputs: &mut [SelInput],
    input: &mut usize,
    hover: &mut Option<(usize, Sel)>,
) -> bool {
    let mut enter = false;
    if *input == 0 {
        dg::sync_picks(app, k, inputs);
    }
    dg::refresh_in(app, k, inputs, Some(ui.ctx()));
    dg::auto_details(app, k, inputs);
    let t = Tokens::get();
    ui.spacing_mut().item_spacing.y = t.space_4;
    k.tab = dg::shown_tab(k);
    tab_row(app, ui, k);
    // Opened (create or Edit): every tab from its top.
    if k.scroll == dg::Scroll::Top {
        scrolls_to_top(ui);
        k.scroll = dg::Scroll::None;
    }
    status(app, ui, k, *input);
    let tab = k.tab;
    // The tab's groups scroll under the tab row and the status line, down to the dialog's foot.
    let room = (ui.clip_rect().bottom() - ui.cursor().top() - t.space_12 - ui.spacing().item_spacing.y - 1.0).max(t.control_height * 4.0);
    egui::ScrollArea::vertical().id_salt(scroll_salt(tab)).max_height(room).min_scrolled_height(room).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = t.space_4;
        match tab {
            Tab::Gears => enter |= gears_tab(app, ui, k, inputs, input, hover),
            Tab::Teeth => enter |= teeth_tab(ui, k, inputs, input),
            Tab::Shafts => enter |= shafts_tab(app, ui, k, inputs, input),
            Tab::Motion => enter |= motion_tab(app, ui, k),
            Tab::Housing => enter |= housing_tab(app, ui, k, inputs, input),
            Tab::Differential => enter |= differential_tab(ui, k, inputs, input),
            Tab::Results => results_tab(app, ui, k),
        }
    });
    // What isn't drawn lights nothing.
    if tab != Tab::Gears {
        k.card_hovered = None;
    }
    if tab != Tab::Motion {
        k.link_hovered = None;
    }
    enter
}

// ---------------------------------------------------------------------------------------------
// Gears
// ---------------------------------------------------------------------------------------------

/// Gears: the type and New gear, the set's messages, the cards, and the selected gear's own
/// settings and size.
fn gears_tab(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize, hover: &mut Option<(usize, Sel)>) -> bool {
    let mut enter = false;
    if k.legacy.is_none() {
        header(ui, k, inputs, input);
    }
    legacy(app, ui, k);
    let mut edit = None;
    for m in &k.set_msgs {
        message(ui, m);
        if let Some(sk) = loose_sketch(app, &m.text)
            && ui_kit::button(ui, "Edit sketch", ui_kit::Button::Normal)
                .backing_on_hover_text("Open the sketch to dimension it; the dialog comes back when you finish it.")
                .clicked()
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
    enter |= blocks_group(ui, k);
    if let Some(a) = k.active.filter(|a| *a < k.items.len()) {
        enter |= size_group(app, ui, k, a);
    }
    enter
}

/// The blocks: none yet, one button puts every gear in one; each block its name, its gears
/// (ticked), its module and shaft Ø, its face width and, under More, its helix, pressure angle,
/// backlash and tooth chamfer, laid out two to a row. Its handles are `gear:block:all`,
/// `gear:block:new` and `gear:block:<n>:remove`.
fn blocks_group(ui: &mut egui::Ui, k: &mut Gear) -> bool {
    if k.items.is_empty() || k.legacy.is_some() {
        return false;
    }
    let mut enter = false;
    let n = k.links.blocks().len();
    let title = if n == 0 { "Blocks".to_string() } else { format!("Blocks · {n}") };
    ui_kit::section(ui, "gear_blocks", &title, true, |ui| {
        if n == 0 {
            ui_kit::text(
                ui,
                "Put the gears in a block to set their module, their shaft or bore, and their face width once for all of them.",
                ui_kit::Text::Note("gear_blocks_none"),
            );
            let r = ui_kit::button(ui, "Put all gears in one block", ui_kit::Button::Normal).backing_on_hover_text(TIP_BLOCK);
            solidcraft_ui_registry::publish_target("gear:block:all", r.rect);
            if r.clicked() {
                let all: Vec<usize> = (0..k.items.len()).collect();
                k.links.add_block(&all);
            }
            return;
        }
        let mut remove = None;
        for b in 0..n {
            enter |= block_rows(ui, k, b, &mut remove);
        }
        if let Some(b) = remove {
            k.links.remove_block(b);
        }
        let loose: Vec<usize> = (0..k.items.len()).filter(|i| k.links.block_of(*i).is_none()).collect();
        let r = ui_kit::button(ui, "New block", ui_kit::Button::Enabled(!loose.is_empty()))
            .backing_on_hover_text("Another block, holding the gears in no block yet.");
        solidcraft_ui_registry::publish_target("gear:block:new", r.rect);
        if r.clicked() {
            k.links.add_block(&loose);
        }
    });
    enter
}

/// Block `b`'s rows (see [`blocks_group`]); its × sets `remove`.
fn block_rows(ui: &mut egui::Ui, k: &mut Gear, b: usize, remove: &mut Option<usize>) -> bool {
    let t = Tokens::get();
    let mut enter = false;
    let color = dg::block_color(b);
    let title = k.links.blocks()[b].title(b);
    ui.add_space(t.space_2);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("●  {title}")).size(t.font_label).strong().color(color)).backing_on_hover_text(TIP_BLOCK);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui_kit::compact_button(ui, "×", "Remove this block: its gears keep their own settings", true, false);
            solidcraft_ui_registry::publish_target(&format!("gear:block:{}:remove", b + 1), r.rect);
            if r.clicked() {
                *remove = Some(b);
            }
        });
    });
    // Its gears, ticked.
    let count = k.items.len();
    let mut toggled = None;
    ui.scope(|ui| {
        ui_kit::row(ui, "Gears", |ui| {
            ui.horizontal_wrapped(|ui| {
                for i in 0..count {
                    let was = k.links.blocks()[b].members.contains(&i);
                    let mut on = was;
                    ui_kit::checkbox(ui, &item_id(i), &mut on);
                    if on != was {
                        toggled = Some((i, on));
                    }
                }
            });
        })
    })
    .response
    .backing_on_hover_text(TIP_BLOCK_GEARS);
    match toggled {
        Some((i, true)) => k.links.put_in_block(b, i),
        Some((i, false)) => k.links.take_out(i),
        None => {}
    }
    let shafts: Vec<String> = k
        .block_shafts
        .iter()
        .zip(solidcraft_gear::blocks::shaft_ids(&k.block_shafts))
        .filter(|(s, _)| s.block == b)
        .map(|(s, id)| {
            let gears = s.gears.iter().map(|g| k.gear_name((g.item, g.gear))).collect::<Vec<_>>().join(" + ");
            let name = id.unwrap_or_else(|| s.existing.map(|i| format!("{}'s line", item_id(i))).unwrap_or_default());
            format!("{name}: {gears} ({} mm)", dg::fmt((s.length() * 10.0).round() / 10.0))
        })
        .collect();
    let Some(bl) = k.links.block_mut(b) else { return enter };
    enter |= field(ui, "Name", &mut bl.name, &format!("Block {}", b + 1), "The block's name, in the view and the lists.");
    let mut mode = bl.shaft_mode.min(2);
    segments(ui, "Shaft", &mut mode, &["Make shaft", "Bore only", "None"], TIP_BLOCK_SHAFT_MODE);
    bl.shaft_mode = mode;
    ui.columns(2, |c| {
        enter |= field(&mut c[0], "Module", &mut bl.module, "auto", TIP_BLOCK_MODULE);
        match mode {
            0 => enter |= field(&mut c[1], "Shaft Ø", &mut bl.shaft_d, "mm", TIP_BLOCK_SHAFT),
            1 => enter |= field(&mut c[1], "Bore Ø", &mut bl.shaft_d, "mm", TIP_BLOCK_BORE),
            _ => {}
        }
    });
    ui.columns(2, |c| {
        enter |= field(&mut c[0], "Face width", &mut bl.face, "each gear's", TIP_BLOCK_FACE);
    });
    let d = bl.shaft_d.trim().to_string();
    let said = match (mode, d.is_empty(), shafts.len()) {
        (2, ..) | (_, true, _) => None,
        (1, ..) => Some(format!("Every gear in the block bored to {d}; no shaft is made (the rod is your own).")),
        (_, _, 0) => Some("No shaft yet: the block's gears aren't complete.".to_string()),
        (_, _, 1) => Some(format!("1 shaft, {}. Its gears are fused to it (one body): no bore.", shafts[0])),
        (_, _, n) => Some(format!("{n} shafts: {}. The gears on each are fused to it (one body): no bore.", shafts.join(", "))),
    };
    if let Some(said) = said {
        ui.add(egui::Label::new(egui::RichText::new(said).size(t.font_small).color(t.form_dim)).wrap());
    }
    let more = bl.has_more();
    ui_kit::section(ui, ("gear_block_more", b), "More", more, |ui| {
        // The block's Teeth, as each gear's (Straight: each gear's own teeth).
        let (style, changed) = teeth_toggle(ui, &mut bl.helix, &mut bl.herringbone, TIP_BLOCK_TEETH);
        enter |= changed;
        if style > 0 {
            enter |= field(ui, "Helix angle", &mut bl.helix, "20 deg", TIP_BLOCK_HELIX);
        } else {
            ui_kit::caption(ui, "Teeth", "Each gear's own teeth (straight unless it sets them).");
        }
        ui.columns(2, |c| {
            enter |= field(&mut c[0], "Pressure angle", &mut bl.pressure_angle, "the set's", TIP_BLOCK_PA);
        });
        ui.columns(2, |c| {
            enter |= field(&mut c[0], "Backlash", &mut bl.backlash, "the set's", TIP_BLOCK_BACKLASH);
            enter |= field(&mut c[1], "Tooth chamfer", &mut bl.chamfer, "the set's", TIP_BLOCK_CHAMFER);
        });
    });
    enter
}

/// Which of item `a`'s gears the rows below are for (a segment per gear, when it makes several).
fn gear_switch(ui: &mut egui::Ui, k: &mut Gear, a: usize) -> usize {
    let n = k.gear_counts.get(a).copied().unwrap_or(1);
    if n > 1 {
        let names: Vec<String> =
            (0..n).map(|g| crate::dialogs_gear_items::gear_part(k.items[a].kind, g).map_or_else(|| k.gear_name((a, g)), str::to_string)).collect();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut tab = k.gear_tab.min(n - 1);
        segments(ui, "Gear", &mut tab, &labels, "Each gear of this item has its own size, face, helix, bore and hub.");
        k.gear_tab = tab;
    }
    k.gear_tab.min(n.saturating_sub(1))
}

/// A group's title for item `a`: "Gear B · Spur".
fn item_title(k: &Gear, a: usize) -> String {
    format!("Gear {} · {}", item_id(a), k.items[a].label())
}

/// The selected gear's group on the Gears tab: the type's own settings, then per gear Size by,
/// Module / Teeth and Face; a bevel or hypoid pair's differential switch.
fn size_group(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, a: usize) -> bool {
    let mut enter = false;
    let title = item_title(k, a);
    let kind = k.items[a].kind;
    let n = k.gear_counts.get(a).copied().unwrap_or(1);
    let replies: Vec<_> = k.links.replies().iter().filter(|r| r.item == a).cloned().collect();
    // The block this gear is in, if any: its module is the one the gears in it share.
    let in_block = k.links.block_of(a).map(|b| {
        let bl = &k.links.blocks()[b];
        (b, bl.title(b), bl.members.iter().map(|i| item_id(*i)).collect::<Vec<_>>().join(", "), bl.face.trim().to_string())
    });
    ui_kit::section(ui, ("gear_settings", a), &title, true, |ui| {
        // A card just made active: its settings come into view.
        if k.scroll == dg::Scroll::Settings {
            ui.scroll_to_cursor(Some(egui::Align::Center));
            k.scroll = dg::Scroll::None;
        }
        let it = &mut k.items[a];
        // The type's own settings.
        match kind {
            ItemKind::Shaft => {
                enter |= field(ui, "Shaft Ø", &mut it.shaft_d, "auto", TIP_SHAFT_D);
                return;
            }
            ItemKind::Internal => enter |= field(ui, "Rim", &mut it.rim, "auto: 2.5 × module", TIP_RIM),
            ItemKind::Worm => {
                enter |= field(ui, "Starts", &mut it.starts, "auto", TIP_STARTS);
                ui_kit::caption(ui, "Starts", "The wheel is throated: its rim wraps round the worm.");
            }
            ItemKind::Rack => enter |= field(ui, "Back height", &mut it.back_height, "auto", TIP_BACK),
            ItemKind::Planetary => {
                enter |= field(ui, "Planets", &mut it.planets, "auto", "How many planets: Auto fits as many as there is room for, 3 at most.");
                ui_kit::checkbox(ui, "Carrier", &mut it.carrier).backing_on_hover_text(TIP_CARRIER);
            }
            _ => {}
        }
        // No circle sizes the gears (a bevel pair drawn as a side view): the pair's one module here
        // (they must share it to mesh), each gear's teeth below.
        let free = crate::dialogs_gear_items::unsized_gears(kind, &k.items[a].things(app));
        let pair_module = !free.is_empty() && free.iter().all(|f| *f) && in_block.is_none();
        for (s, f) in k.items[a].settings_mut(n).iter_mut().zip(&free) {
            s.both = *f;
        }
        if pair_module {
            let gears = k.items[a].settings_mut(n);
            let mut m = gears[0].module.clone();
            enter |= field(ui, "Module", &mut m, "auto", TIP_PAIR_MODULE);
            for s in gears.iter_mut() {
                s.module.clone_from(&m);
            }
        }
        let g = gear_switch(ui, k, a);
        let reply = replies.iter().find(|r| r.gear == g);
        let s: &mut GearSettings = &mut k.items[a].settings_mut(n)[g];
        let block = in_block.as_ref().and_then(|(b, title, members, face)| {
            Some(BlockRows { module: &mut k.links.block_mut(*b)?.module, title: title.clone(), members: members.clone(), face: face.clone() })
        });
        enter |= size_rows(ui, s, reply, block, pair_module);
        if takes_tooth_style(kind) {
            let s: &mut GearSettings = &mut k.items[a].settings_mut(n)[g];
            enter |= tooth_style_rows(ui, s, kind);
        }
        if dg::takes_differential(kind) && dg::differential_supported() {
            diff_switch(ui, k, a);
        }
    });
    enter
}

/// The block a gear is in, for its size rows: the block's module (edited in place), its name, its
/// gears and its face width as typed.
struct BlockRows<'a> {
    module: &'a mut String,
    title: String,
    members: String,
    face: String,
}

/// One gear's size rows: Size by, Module / Teeth (the other greyed as worked out), Face (its
/// block's face width where it gives none). A gear no circle sizes takes its teeth and a module
/// both (`module_above`: the pair's module is typed above the gear switch).
fn size_rows(
    ui: &mut egui::Ui,
    s: &mut GearSettings,
    reply: Option<&solidcraft_gear::items::GearReply>,
    mut block: Option<BlockRows<'_>>,
    module_above: bool,
) -> bool {
    let mut enter = false;
    let worked = |v: Option<String>| v.map_or_else(|| "— until the gear is complete".to_string(), |v| format!("{v} (worked out)"));
    if s.both {
        if let Some(b) = block.as_mut() {
            enter |= field(
                ui,
                "Module",
                b.module,
                "auto",
                "This gear is in a block: its gears share this one module, so they can mesh. Change it here or on the block.",
            );
        } else if !module_above {
            enter |= field(ui, "Module", &mut s.module, "auto", TIP_MODULE);
        }
        enter |= field(ui, "Teeth", &mut s.teeth, "auto", TIP_TEETH_FREE);
    } else {
        let mut size = usize::from(s.by_teeth);
        segments(ui, "Size by", &mut size, &["Module", "Teeth"], TIP_SIZE_BY);
        s.by_teeth = size == 1;
        if s.by_teeth {
            enter |= field(ui, "Teeth", &mut s.teeth, "from the circle", TIP_TEETH);
            ui_kit::caption(ui, "Teeth", &format!("Module: {}", worked(reply.filter(|r| r.module > 0.0).map(|r| dg::fmt(r.module)))));
        } else if let Some(b) = block.as_mut() {
            // In a block: the block's module is the one its gears share (decision 2).
            enter |= field(
                ui,
                "Module",
                b.module,
                "auto",
                "This gear is in a block: its gears share this one module, so they can mesh. Change it here or on the block.",
            );
            ui_kit::caption(
                ui,
                "Module",
                &format!("{}: gears {} · teeth: {}", b.title, b.members, worked(reply.filter(|r| r.teeth > 0).map(|r| r.teeth.to_string()))),
            );
        } else {
            enter |= field(ui, "Module", &mut s.module, "auto", TIP_MODULE);
            ui_kit::caption(ui, "Module", &format!("Teeth: {}", worked(reply.filter(|r| r.teeth > 0).map(|r| r.teeth.to_string()))));
        }
    }
    let labels: Vec<&str> = FACE_PRESETS.iter().map(|p| p.2).collect();
    segments(ui, "Face", &mut s.face_preset, &labels, TIP_PRESET);
    // Left empty in a block with a face width: the block's (half each side).
    let from_block = block.as_ref().filter(|b| !b.face.is_empty()).map(|b| format!("{}: {} in all", b.title, b.face));
    let auto = from_block.as_deref().unwrap_or("auto");
    match s.face_preset {
        0 => enter |= field(ui, "Each side", &mut s.face_a, auto, TIP_FACE),
        1 => enter |= field(ui, "Side A", &mut s.face_a, auto, TIP_FACE),
        2 => enter |= field(ui, "Side B", &mut s.face_b, auto, TIP_FACE),
        _ => {
            enter |= field(ui, "Side A", &mut s.face_a, auto, TIP_FACE);
            enter |= field(ui, "Side B", &mut s.face_b, auto, TIP_FACE);
        }
    }
    enter
}

/// A bevel or hypoid pair's switch: a differential in its ring gear, set up on its own tab.
fn diff_switch(ui: &mut egui::Ui, k: &mut Gear, a: usize) {
    let mut on = k.items[a].diff.is_some();
    check_group(ui, "differential", "Differential in the ring gear", &mut on, TIP_DIFF, |ui| {
        let b = ui_kit::button(ui, "Differential tab  →", ui_kit::Button::Normal)
            .backing_on_hover_text("Its style, pinions, module and half-shafts, and what it came to.");
        solidcraft_ui_registry::publish_target("gear:to_differential", b.rect);
        if b.clicked() {
            k.tab = Tab::Differential;
        }
    });
    match (on, k.items[a].diff.is_some()) {
        (true, false) => k.items[a].diff = Some(DiffForm::default()),
        (false, true) => k.items[a].diff = None,
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// The selected gear on the other tabs
// ---------------------------------------------------------------------------------------------

/// The gear a tab's rows are for: the selected one, chosen here too (choosing one selects it, as
/// clicking its card does: picks go to it). `None`: no gear selected (the set's own rows still
/// show).
fn item_picker(ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize, id: &str) -> Option<usize> {
    if k.items.is_empty() {
        ui_kit::text(ui, "No gears yet: make one on the Gears tab.", ui_kit::Text::Note("gear_no_items"));
        return None;
    }
    let mut labels = vec!["—".to_string()];
    labels.extend(k.items.iter().enumerate().map(|(i, it)| format!("{} · {}", item_id(i), it.label())));
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let was = k.active.filter(|a| *a < k.items.len()).map_or(0, |a| a + 1);
    let mut at = was;
    choice_row(ui, "Gear", &format!("gear_pick_{id}"), &mut at, &labels).backing_on_hover_text(TIP_PICKER);
    if at != was {
        match at.checked_sub(1) {
            Some(i) => {
                dg::make_active(k, i, inputs);
                *input = 0;
            }
            None => k.active = None,
        }
    }
    let a = k.active.filter(|a| *a < k.items.len());
    if a.is_none() {
        ui_kit::text(ui, "Choose a gear above (or click its card on the Gears tab) for its own settings.", ui_kit::Text::Note("gear_pick_one"));
    }
    a
}

/// Teeth: the selected gear's helix (or a bevel's style and spiral) and profile shift; the tooth
/// form every gear takes; the tooth chamfer.
fn teeth_tab(ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize) -> bool {
    let mut enter = false;
    if let Some(a) = item_picker(ui, k, inputs, input, "teeth") {
        let kind = k.items[a].kind;
        let title = item_title(k, a);
        let replies: Vec<_> = k.links.replies().iter().filter(|r| r.item == a).cloned().collect();
        ui_kit::section(ui, ("gear_teeth", a), &title, true, |ui| {
            if kind == ItemKind::Shaft {
                ui_kit::text(ui, "A shaft has no teeth.", ui_kit::Text::Note("gear_shaft_teeth"));
                return;
            }
            if matches!(kind, ItemKind::Bevel | ItemKind::Hypoid) {
                let it = &mut k.items[a];
                choice_row(ui, "Style", &format!("gear_bevel_{a}"), &mut it.bevel_style, &["Straight", "Spiral", "Chevron"])
                    .backing_on_hover_text(TIP_V_BEVEL);
                if it.bevel_style > 0 || kind == ItemKind::Hypoid {
                    enter |= field(ui, "Spiral angle", &mut it.spiral, "35 deg", TIP_SPIRAL);
                }
            }
            let g = gear_switch(ui, k, a);
            let reply = replies.iter().find(|r| r.gear == g);
            // The shift is the planner's: it holds the drawn centre distance (read-only).
            let shift = reply
                .and_then(|r| r.profile_shift)
                .map_or_else(|| "worked out".to_string(), |x| format!("{} (worked out)", dg::fmt((x * 1000.0).round() / 1000.0)));
            ui_kit::value_row(ui, "Profile shift", &shift).backing_on_hover_text(TIP_SHIFT);
        });
    }
    ui_kit::section(ui, "gear_tooth_form", "Tooth form · every gear", true, |ui| {
        for (key, label, _, hint, tip) in dg::ADVANCED {
            enter |= field(ui, label, k.values.entry(key.to_string()).or_default(), hint, tip);
        }
        // Formula teeth are a folder of features: only for a set made that way, never a Gears feature.
        if k.legacy.is_some() {
            ui_kit::checkbox(ui, "Equation teeth", &mut k.equations).backing_on_hover_text(TIP_EQUATIONS);
        }
    });
    let mut on = k.chamfer_open || !k.chamfer.trim().is_empty();
    check_group(ui, "chamfer", "Tooth chamfer · every gear", &mut on, TIP_CHAMFER, |ui| {
        enter |= field(ui, "Size", &mut k.chamfer, "mm", TIP_CHAMFER);
        segments(
            ui,
            "Faces",
            &mut k.chamfer_faces,
            &["Both", "Side A", "Side B"],
            "Which end faces of every gear get the chamfer: side A is the green arrow in the view.",
        );
    });
    if !on {
        k.chamfer.clear();
    }
    k.chamfer_open = on;
    enter
}

/// Whether a gear of `kind` takes the Teeth toggle (a bevel, hypoid or worm has teeth of its own
/// kind; a shaft none).
fn takes_tooth_style(kind: ItemKind) -> bool {
    !matches!(kind, ItemKind::Bevel | ItemKind::Hypoid | ItemKind::Worm | ItemKind::Shaft)
}

/// A gear's Teeth toggle: Straight, Helical or Herringbone (two opposite helices), then its helix
/// angle and hand when they are not straight.
fn tooth_style_rows(ui: &mut egui::Ui, s: &mut GearSettings, kind: ItemKind) -> bool {
    let (style, mut enter) = teeth_toggle(ui, &mut s.helix, &mut s.v, TIP_TOOTH_STYLE);
    if style > 0 {
        enter |= field(ui, "Helix angle", &mut s.helix, "20 deg", TIP_HELIX);
        if kind == ItemKind::Planetary {
            segments(ui, "Hand", &mut s.hand, &["Auto", "Left", "Right"], TIP_HAND);
        } else {
            ui_kit::caption(ui, "Helix angle", "Hand: worked out, opposite across each mesh.");
        }
    }
    enter
}

/// Whether a typed helix angle is one (not empty or 0).
fn helical(h: &str) -> bool {
    !matches!(h.trim(), "" | "0" | "0 deg" | "0deg" | "0°")
}

/// The Teeth toggle (Straight / Helical / Herringbone) over a typed helix angle and V flag, as a
/// gear and a block both have it: Straight clears the angle, Helical or Herringbone gives 20°
/// where none is typed. Returns the style shown (0, 1, 2) and whether it changed.
pub(crate) fn teeth_toggle(ui: &mut egui::Ui, helix: &mut String, v: &mut bool, tip: &str) -> (usize, bool) {
    let was = teeth_style(helix, *v);
    let mut style = was;
    segments(ui, "Teeth", &mut style, &["Straight", "Helical", "Herringbone"], tip);
    if style != was {
        set_teeth_style(helix, v, style);
    }
    (style, style != was)
}

/// The Teeth toggle's style for a typed helix angle and V flag: 0 straight, 1 helical, 2
/// herringbone.
pub(crate) fn teeth_style(helix: &str, v: bool) -> usize {
    if v { 2 } else { usize::from(helical(helix)) }
}

/// The typed helix angle and V flag for Teeth style `style` (see [`teeth_toggle`]).
pub(crate) fn set_teeth_style(helix: &mut String, v: &mut bool, style: usize) {
    *v = style == 2;
    if style == 0 {
        helix.clear();
    } else if !helical(helix) {
        *helix = "20 deg".into();
    }
}

/// Shafts & Bores, kept apart: **Shafts** (solid rods along an axis: one row per shaft with its Ø ×
/// length, its gears, where it comes from and whether they are fused to it or turn free) and
/// the bearing points on them; **Gear bores** (the hole through each gear: its Ø and where it
/// comes from); then the selected gear's own bore and hub.
fn shafts_tab(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize) -> bool {
    let mut enter = false;
    let t = Tokens::get();
    shafts_made(app, ui, k);
    // The bearing points every shaft line carries: a seat each in the housing cutout.
    let points: Vec<(usize, usize)> = k
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| (i, it.things(app).iter().filter(|x| matches!(x, solidcraft_gear::items::Thing::Point { .. })).count()))
        .filter(|(_, n)| *n > 0)
        .collect();
    let total: usize = points.iter().map(|p| p.1).sum();
    ui_kit::section(ui, "gear_bearing_points", &format!("Bearing points · {total}"), true, |ui| {
        for (i, n) in &points {
            ui_kit::value_row(ui, &format!("Gear {}", item_id(*i)), &format!("{n} point{}", if *n == 1 { "" } else { "s" }));
        }
        let said = if total == 0 {
            "No bearing points yet: pick points on a shaft line (a gear's or a Shaft's) on the Gears tab."
        } else if k.cutout.on {
            "Each gets a bearing seat in the housing cutout (Housing & Output tab)."
        } else {
            "Turn on the housing cutout (Housing & Output tab) for a bearing seat at each."
        };
        ui.add(egui::Label::new(egui::RichText::new(said).size(t.font_small).color(t.form_dim)).wrap());
    });
    gear_bores(app, ui, k);
    if let Some(a) = item_picker(ui, k, inputs, input, "shafts") {
        let kind = k.items[a].kind;
        let title = format!("{} · bore and hub", item_title(k, a));
        let n = k.gear_counts.get(a).copied().unwrap_or(1);
        ui_kit::section(ui, ("gear_shafts", a), &title, true, |ui| {
            if kind == ItemKind::Shaft {
                enter |= field(ui, "Shaft Ø", &mut k.items[a].shaft_d, "auto", TIP_SHAFT_D);
                return;
            }
            let g = gear_switch(ui, k, a);
            let (_, from) = dg::bore_of(app, k, &k.block_shafts, a, g);
            let fused = dg::shaft_of(&k.block_shafts, a, g).is_some();
            let s: &mut GearSettings = &mut k.items[a].settings_mut(n)[g];
            let mut bore = usize::from(s.bore == 2);
            choice_row(ui, "Bore", &format!("gear_bore_{a}_{g}"), &mut bore, &["Auto", "Ø"]).backing_on_hover_text(TIP_BORE);
            s.bore = if bore == 1 {
                2
            } else if s.bore == 2 {
                0
            } else {
                s.bore
            };
            if s.bore == 2 && !fused {
                enter |= field(ui, "Bore Ø", &mut s.bore_d, "mm", TIP_BORE_D);
            } else {
                ui_kit::caption(ui, "Bore", &format!("Bore: {from}"));
            }
            let mut hub = s.hub;
            check_group(ui, &format!("hub:{}:{g}", item_id(a)), "Hub", &mut hub, TIP_HUB, |ui| {
                enter |= field(ui, "Hub Ø", &mut s.hub_d, "auto", TIP_HUB_D);
                enter |= field(ui, "Length A", &mut s.hub_a, "auto", TIP_HUB_LEN);
                enter |= field(ui, "Length B", &mut s.hub_b, "auto", TIP_HUB_LEN);
            });
            s.hub = hub;
        });
    }
    enter
}

/// **Shafts**: the shafts (solid rods) the set makes, one row per axis: each block shaft ("Shaft
/// S1": Ø × length, its gears, its block, fused or turning free), each shaft line on a block's
/// axis that takes its Ø, then each Shaft drawn by hand. With none, how to get them.
fn shafts_made(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear) {
    let t = Tokens::get();
    let drawn: Vec<usize> = (0..k.items.len()).filter(|i| k.items[*i].kind == ItemKind::Shaft).collect();
    let ids = solidcraft_gear::blocks::shaft_ids(&k.block_shafts);
    let n = ids.iter().flatten().count() + drawn.len();
    ui_kit::section(ui, "gear_shafts_made", &format!("Shafts · {n}"), true, |ui| {
        for (sh, id) in k.block_shafts.iter().zip(&ids) {
            let gears = sh.gears.iter().map(|g| k.gear_name((g.item, g.gear))).collect::<Vec<_>>().join(" + ");
            let block = k.links.blocks().get(sh.block).map(|b| b.title(sh.block)).unwrap_or_default();
            let name = match (id, sh.existing) {
                (Some(id), _) => format!("Shaft {id}"),
                (None, Some(i)) => format!("Gear {}'s line", item_id(i)),
                (None, None) => String::new(),
            };
            // Fused: one body with the shaft; free: the engine bores it to turn on the shaft.
            let free: Vec<String> = sh
                .gears
                .iter()
                .filter(|g| dg::bore_of(app, k, &k.block_shafts, g.item, g.gear).1.starts_with("free"))
                .map(|g| k.gear_name((g.item, g.gear)))
                .collect();
            let how = if free.is_empty() { "fused".to_string() } else { format!("{} turn free", free.join(", ")) };
            let what = format!("Ø{} × {} mm · gears {gears} · {block} · {how}", dg::fmt(sh.diameter), dg::fmt((sh.length() * 10.0).round() / 10.0));
            ui_kit::row(ui, &name, |ui| ui.label(egui::RichText::new(what).size(t.font_small).color(dg::block_color(sh.block))))
                .backing_on_hover_text(
                    "A shaft from the block's Make shaft: a solid rod along this axis, the gears on it fused to it (one body), so they have no bore.",
                );
        }
        for i in &drawn {
            let d = k.items[*i].shaft_d.trim();
            let what = format!("Ø{} · a drawn shaft line", if d.is_empty() { "auto" } else { d });
            ui_kit::value_row(ui, &format!("Shaft {}", item_id(*i)), &what);
        }
        if n == 0 && k.block_shafts.is_empty() {
            let said = "No shafts yet. Easiest: on the Gears tab put the gears in a block, choose Make shaft and give its Ø: each axis gets a shaft, its gears fused to it. Or draw a shaft line and add a Shaft on it.";
            ui.add(egui::Label::new(egui::RichText::new(said).size(t.font_small).color(t.form_dim)).wrap());
        }
    });
}

/// **Gear bores**: the bore (the hole through the gear) of each gear, one row each: its Ø (or
/// none) and where it comes from.
fn gear_bores(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear) {
    let t = Tokens::get();
    let gears = k.all_gears();
    ui_kit::section(ui, "gear_bores", &format!("Gear bores · {}", gears.len()), true, |ui| {
        for (i, g) in gears {
            if k.items.get(i).is_some_and(|it| it.kind == ItemKind::Shaft) {
                continue;
            }
            let (d, from) = dg::bore_of(app, k, &k.block_shafts, i, g);
            let d = d.map_or_else(|| "—".to_string(), |d| format!("Ø{}", dg::fmt(d)));
            ui_kit::row(ui, &format!("Gear {}", k.gear_name((i, g))), |ui| {
                ui.label(egui::RichText::new(format!("{d} · {from}")).size(t.font_small).color(t.form_ink))
            })
            .backing_on_hover_text(TIP_BORE);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Links & Motion
// ---------------------------------------------------------------------------------------------

/// Links & Motion: the links as rows ("A ⟷ B  mesh  1.50 : 1", "B ═ C  same shaft"; each drawn
/// in the view, the hovered row lit there), the suggested ones (dim) with one Accept button, a
/// link added by hand, the driver with its rpm and torque, and the ratio between two gears.
fn motion_tab(_app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear) -> bool {
    let mut enter = false;
    let gears = k.all_gears();
    k.link_hovered = None;
    if gears.len() < 2 && k.links.all().is_empty() {
        ui_kit::text(
            ui,
            "Links join two gears or more: make them on the Gears tab. Touching circles and circles on one shaft are suggested here.",
            ui_kit::Text::Note("gear_no_links"),
        );
        return false;
    }
    let t = Tokens::get();
    let links = k.links.all();
    let suggested = k.links.suggested().to_vec();
    let title = format!("Links · {}", links.len());
    ui_kit::section(ui, "gear_drivetrain", &title, true, |ui| {
        let mut drop = None;
        if links.is_empty() && suggested.is_empty() {
            ui_kit::text(
                ui,
                "No links yet: add one below. Touching circles and circles on one shaft are suggested here.",
                ui_kit::Text::Note("gear_no_links"),
            );
        }
        ui_kit::text(ui, "Each link is drawn in the view between its gears; hover a row to light it.", ui_kit::Text::Note("gear_links_in_view"));
        for (n, l) in links.iter().enumerate() {
            let (a, ag, b, bg, kind) = l;
            let (x, y) = ((*a, *ag), (*b, *bg));
            let note = k.links.note_of(l).cloned();
            let row = ui.horizontal(|ui| {
                let joint = if *kind == "shaft" { "═" } else { "⟷" };
                ui.label(
                    egui::RichText::new(format!("{}  {joint}  {}", k.gear_name(x), k.gear_name(y))).size(t.font_label).strong().color(t.form_ink),
                );
                let what = if *kind == "shaft" { "same shaft" } else { "mesh" };
                ui.label(egui::RichText::new(what).size(t.font_small).color(t.form_dim)).backing_on_hover_text(if *kind == "shaft" {
                    TIP_SHAFT
                } else {
                    TIP_MESH
                });
                if let Some(r) = note.as_ref().and_then(|n| n.ratio).filter(|_| *kind == "mesh") {
                    let turn = if r < 0.0 { " opposite" } else { " same way" };
                    ui.label(egui::RichText::new(format!("{} : 1", dg::fmt_ratio(r.abs()))).size(t.font_label).color(t.form_ink));
                    ui.label(egui::RichText::new(turn).size(t.font_small).color(t.form_dim));
                }
                if let Some(blk) = k.links.block_of(*a).filter(|blk| Some(*blk) == k.links.block_of(*b)) {
                    ui.label(egui::RichText::new(format!("● {}", k.links.blocks()[blk].title(blk))).size(t.font_small).color(dg::block_color(blk)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui_kit::compact_button(ui, "×", "Remove this link", true, false).clicked() {
                        drop = Some((x, y, *kind));
                    }
                });
            });
            solidcraft_ui_registry::publish_target(&format!("gear:link:{}", n + 1), row.response.rect);
            if ui.rect_contains_pointer(row.response.rect) {
                k.link_hovered = Some(*l);
            }
            if let Some(e) = note.and_then(|n| n.error.map(|e| (e, n.ratio))) {
                message(ui, &Msg { text: dg::worded(&e.0), error: true });
            }
        }
        if let Some((x, y, kind)) = drop {
            k.links.remove_link(x, y, kind);
        }
        if !suggested.is_empty() {
            for s in &suggested {
                let joint = if s.kind == "shaft" { "═" } else { "⟷" };
                let what = if s.kind == "shaft" { "same shaft" } else { "mesh" };
                ui.label(
                    egui::RichText::new(format!("{}  {joint}  {}   {what} · suggested", k.gear_name((s.a, s.a_gear)), k.gear_name((s.b, s.b_gear))))
                        .size(t.font_label)
                        .italics()
                        .color(t.form_dim),
                );
            }
            let r = ui_kit::button(ui, format!("Accept suggested ({})", suggested.len()), ui_kit::Button::Normal).backing_on_hover_text(TIP_ACCEPT);
            solidcraft_ui_registry::publish_target("gear:accept", r.rect);
            if r.clicked() {
                k.links.accept_all();
            }
        }
    });
    // A link by hand: gear, kind, gear.
    let labels: Vec<String> = gears.iter().map(|g| k.gear_name(*g)).collect();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    if !gears.is_empty() {
        let (mut x, mut kind, mut y) = k.link_add;
        x = x.min(gears.len() - 1);
        y = y.min(gears.len() - 1);
        ui_kit::section(ui, "gear_add_link", "Add a link by hand", links.is_empty() && suggested.is_empty(), |ui| {
            choice_row(ui, "Gear", "gear_link_a", &mut x, &labels);
            segments(ui, "Link", &mut kind, &["Mesh", "Same shaft"], "Mesh: the teeth engage. Same shaft: the two gears turn together.");
            choice_row(ui, "With gear", "gear_link_b", &mut y, &labels);
            let add = ui_kit::button(ui, "Add link", ui_kit::Button::Enabled(x != y))
                .backing_on_hover_text("Link the two gears: mesh (teeth engage) or same shaft (they turn together).");
            solidcraft_ui_registry::publish_target("gear:add_link", add.rect);
            if add.clicked() && x != y {
                let ((a, ag), (b, bg)) = (gears[x], gears[y]);
                k.links.accept(&crate::dialogs_gear_links::Suggested {
                    a,
                    a_gear: ag,
                    b,
                    b_gear: bg,
                    kind: if kind == 1 { "shaft".into() } else { "mesh".into() },
                });
            }
        });
        k.link_add = (x, kind, y);
    }
    let mut with_none: Vec<&str> = vec!["—"];
    with_none.extend(labels.iter().copied());
    // The driver, its rpm and torque.
    ui_kit::section(ui, "gear_drive", "Drive", true, |ui| {
        let mut at = k.drive_item.and_then(|d| gears.iter().position(|x| *x == d)).map_or(0, |p| p + 1);
        choice_row(ui, "Driver", "gear_driver", &mut at, &with_none).backing_on_hover_text(TIP_DRIVER);
        k.drive_item = at.checked_sub(1).and_then(|p| gears.get(p).copied());
        if k.drive_item.is_some() {
            enter |= field(ui, "Input rpm", &mut k.rpm, "rpm, optional", TIP_RPM);
            enter |= field(ui, "Input torque", &mut k.torque, "N·m, optional", TIP_TORQUE);
        }
    });
    // The ratio between two gears.
    ui_kit::section(ui, "gear_ratio", "Ratio between two gears", true, |ui| {
        for (n, (key, title)) in [("gear_ratio_a", "Ratio from"), ("gear_ratio_b", "to")].into_iter().enumerate() {
            let mut at = k.ratio_pick[n].and_then(|p| gears.iter().position(|x| *x == p)).map_or(0, |p| p + 1);
            choice_row(ui, title, key, &mut at, &with_none)
                .backing_on_hover_text("The ratio is the second gear's speed over the first's, with the direction and the path between them.");
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
    });
    enter
}

// ---------------------------------------------------------------------------------------------
// Housing & Output
// ---------------------------------------------------------------------------------------------

/// Housing & Output: the housing cutout (off by default: one body holding the space the
/// drivetrain needs to turn, with a bearing seat at every bearing point), then the operation,
/// Targets, keep-out and assembly.
fn housing_tab(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize) -> bool {
    let mut enter = false;
    let t = Tokens::get();
    let points: usize =
        k.items.iter().map(|it| it.things(app).iter().filter(|x| matches!(x, solidcraft_gear::items::Thing::Point { .. })).count()).sum();
    let mut on = k.cutout.on;
    check_group(ui, "housing_cutout", "Housing cutout", &mut on, TIP_CUTOUT, |ui| {
        let mut labels: Vec<String> =
            solidcraft_gear::hub::BEARINGS.iter().map(|b| format!("{} — {} × {} × {}", b.0, dg::fmt(b.1), dg::fmt(b.2), dg::fmt(b.3))).collect();
        labels.push("Custom…".into());
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        choice_row(ui, "Bearing", "gear_cutout_bearing", &mut k.cutout.bearing, &labels).backing_on_hover_text(TIP_BEARING);
        if k.cutout.bearing >= solidcraft_gear::hub::BEARINGS.len() {
            enter |= field(ui, "Bearing OD", &mut k.cutout.od, "mm", "Custom bearing outside diameter.");
            enter |= field(ui, "Bearing width", &mut k.cutout.width, "mm", "Custom bearing width.");
            enter |= field(
                ui,
                "Bearing bore",
                &mut k.cutout.bore,
                "optional",
                "Custom bearing bore; used to check it against the shaft (empty skips the check).",
            );
        }
        let said = match points {
            0 => "No bearing points yet: pick points on the gears' shaft lines (or a Shaft's) for the seats.".to_string(),
            1 => "1 bearing point: a seat there.".to_string(),
            n => format!("{n} bearing points: a seat at each."),
        };
        ui.add(egui::Label::new(egui::RichText::new(said).size(t.font_small).color(t.form_dim)).wrap());
        ui_kit::section(ui, "gear_cutout_more", "Gaps and fit", false, |ui| {
            enter |= field(
                ui,
                "Radial gap",
                &mut k.cutout.radial,
                "0.5 mm",
                "Gap around every tip envelope and shaft. For printed housings 0.3–0.6 mm is typical.",
            );
            enter |= field(ui, "Axial gap", &mut k.cutout.axial, "0.5 mm", "Gap past each gear's faces along its axis.");
            enter |= field(
                ui,
                "Seat fit",
                &mut k.cutout.fit,
                "0",
                "How much larger than the bearing's OD each seat is, on the diameter: 0.1–0.2 for a printed slip fit, 0 or less for a press fit.",
            );
            enter |= field(
                ui,
                "Shaft run-on",
                &mut k.cutout.run_on,
                "2 × face",
                "How far each shaft's clearance runs past its gear on each side, enough to pass through a housing wall.",
            );
        });
    });
    k.cutout.on = on;
    ui_kit::section(ui, "gear_output", "Output", true, |ui| {
        ui_kit::checkbox(ui, "Assembly (joints and gear links)", &mut k.assembly).backing_on_hover_text(TIP_ASSEMBLY);
        choice_row(ui, "Operation", "gear_op", &mut k.operation, &OP_LABELS).backing_on_hover_text(TIP_OPERATION);
        let mut keep_out = k.cut_keep_out;
        check_group(ui, "keep_out", "Cut keep-out from Targets", &mut keep_out, TIP_KEEP_OUT, |ui| {
            enter |= field(ui, "Keep-out gap", &mut k.keep_out_gap, "auto", TIP_GAP);
        });
        k.cut_keep_out = keep_out;
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

// ---------------------------------------------------------------------------------------------
// Differential
// ---------------------------------------------------------------------------------------------

/// The differential styles' short names (the full ones are `diff::Style::label`).
const DIFF_STYLES: [&str; 4] = ["Open", "Biased", "Torsen B", "Torsen A"];

/// Differential: the differential a bevel or hypoid pair's ring gear carries (`differential`):
/// its style, pinions (element pairs for a Torsen), module and half-shafts, and its facts.
fn differential_tab(ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], input: &mut usize) -> bool {
    let mut enter = false;
    let t = Tokens::get();
    let with: Vec<usize> = (0..k.items.len()).filter(|i| k.items[*i].diff.is_some()).collect();
    let Some(mut a) = dg::diff_item(k) else {
        ui_kit::text(
            ui,
            "No ring gear carries a differential: switch one on under a bevel or hypoid gear on the Gears tab.",
            ui_kit::Text::Note("gear_no_diff"),
        );
        return false;
    };
    if with.len() > 1 {
        let labels: Vec<String> = with.iter().map(|i| item_title(k, *i)).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut at = with.iter().position(|i| *i == a).unwrap_or(0);
        choice_row(ui, "Ring gear", "gear_diff_pick", &mut at, &labels).backing_on_hover_text("The gear whose differential the rows below are for.");
        if with[at] != a {
            a = with[at];
            dg::make_active(k, a, inputs);
            *input = 0;
        }
    }
    let id = item_id(a);
    let title = item_title(k, a);
    let mut on = true;
    ui_kit::section(ui, ("gear_diff", a), &title, true, |ui| {
        let r = ui_kit::checkbox(ui, "Differential in the ring gear", &mut on).backing_on_hover_text(TIP_DIFF);
        solidcraft_ui_registry::publish_target("gear:group:differential", r.rect);
        let Some(d) = k.items[a].diff.as_mut() else { return };
        choice_row(ui, "Style", &format!("gear_diff_style_{a}"), &mut d.style, &DIFF_STYLES).backing_on_hover_text(TIP_DIFF_STYLE);
        ui_kit::caption(ui, "Style", d.style().label());
        let bevel = d.style().bevel();
        let auto = match d.style() {
            solidcraft_gear::diff::Style::TorsenB => "auto: 3",
            _ => "auto: 2",
        };
        let (label, tip) = if bevel { ("Pinions", TIP_DIFF_PINIONS) } else { ("Element pairs", TIP_DIFF_ELEMENTS) };
        enter |= field(ui, label, &mut d.pinions, auto, tip);
        enter |= field(ui, "Module", &mut d.module, "auto", TIP_DIFF_MODULE);
    });
    if !on {
        k.items[a].diff = None;
        k.tab = Tab::Gears;
        return enter;
    }
    ui_kit::section(ui, ("gear_diff_axles", a), "Half-shafts", true, |ui| {
        if let Some(d) = k.items[a].diff.as_mut() {
            enter |= field(ui, "Length", &mut d.axle_length, "auto: 1.5 × ring Ø", TIP_DIFF_AXLE);
            enter |= field(ui, "Ø", &mut d.axle_d, "auto", TIP_DIFF_AXLE_D);
        }
    });
    // What the engine says of it: its facts ("differential Gear F: …") and its messages.
    let prefix = format!("differential Gear {id}: ");
    let facts: Vec<&(String, f64, String)> = k.facts.iter().filter(|f| f.0.starts_with(&prefix)).collect();
    ui_kit::section(ui, ("gear_diff_facts", a), &format!("Facts · {}", facts.len()), true, |ui| {
        if facts.is_empty() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("Worked out once the pair is complete: module, teeth, case, bearings and half-shaft lengths.")
                        .size(t.font_small)
                        .color(t.form_dim),
                )
                .wrap(),
            );
        }
        for (label, value, unit) in facts {
            let v = if unit.is_empty() { dg::fmt(*value) } else { format!("{} {unit}", dg::fmt(*value)) };
            ui_kit::value_row(ui, label.trim_start_matches(&prefix), &v);
        }
        for m in k.item_msgs.get(a).into_iter().flatten().filter(|m| m.text.to_lowercase().contains("differential")) {
            message(ui, m);
        }
    });
    enter
}

// ---------------------------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------------------------

/// Results: one row per gear, every warning (by gear), the planner's notes and its facts.
fn results_tab(app: &SolveApp, ui: &mut egui::Ui, k: &Gear) {
    let len = dg::len_of(app);
    let replies = k.links.replies();
    if replies.is_empty() && k.notes.is_empty() && k.set_msgs.is_empty() {
        ui_kit::text(ui, "Nothing worked out yet: complete a gear on the Gears tab.", ui_kit::Text::Note("gear_no_results"));
    }
    if !replies.is_empty() {
        ui_kit::section(ui, "gear_results", &format!("Gears · {}", replies.len()), true, |ui| {
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
                ui_kit::value_row(ui, &k.gear_name((r.item, r.gear)), &parts.join(" · "))
                    .backing_on_hover_text(crate::dialogs_gear_links::reply_tip(r));
            }
        });
    }
    let said: usize = k.set_msgs.len() + k.item_msgs.iter().map(Vec::len).sum::<usize>();
    if said > 0 {
        ui_kit::section(ui, "gear_warnings", &format!("Warnings · {said}"), true, |ui| {
            for m in &k.set_msgs {
                message(ui, m);
            }
            for (i, ms) in k.item_msgs.iter().enumerate() {
                for m in ms {
                    message(ui, &Msg { text: format!("Gear {}: {}", item_id(i), m.text), error: m.error });
                }
            }
        });
    }
    if !k.notes.is_empty() {
        ui_kit::section(ui, "gear_notes", "Notes", true, |ui| {
            for note in &k.notes {
                ui_kit::text(ui, &dg::worded(note), ui_kit::Text::Note("gear_note"));
            }
        });
    }
    if !k.facts.is_empty() {
        ui_kit::section(ui, "gear_facts", &format!("Facts · {}", k.facts.len()), false, |ui| {
            for (label, value, unit) in &k.facts {
                let v = if unit.is_empty() { dg::fmt(*value) } else { format!("{} {unit}", dg::fmt(*value)) };
                ui_kit::value_row(ui, label, &v);
            }
        });
    }
}
