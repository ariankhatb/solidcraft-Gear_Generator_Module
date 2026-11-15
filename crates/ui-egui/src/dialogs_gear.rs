//! The Gear Generator as a native command dialog (stage 1: parallel shafts).
//!
//! The set is drawn in an ordinary sketch of the design: its circles are the pitch circles,
//! drawn, dimensioned and constrained with the app's own sketch tools. The dialog picks that
//! sketch (its curves, or the sketch in the timeline; a sketch being edited when Gears is
//! started is finished and taken, as Extrude takes its profile), the tooth style and the few
//! settings a drawing can't show, and makes the set with `gear.create {sketch}`: the engine reads
//! the sketch (`sketch_rules`) and the set follows it afterwards (a changed dimension
//! regenerates it in place, `gear::follow_sketches`). What the set comes to is shown from
//! `gear.calculate` on the same parameters, and the set itself as the dialog's live preview in
//! the viewport, as every command dialog previews.
//!
//! Stage 1 covers spur, helical and herringbone teeth (the sketch decides single gear, pair,
//! internal or planetary); the other families and every setting are still in the Gear
//! Generator window, one button away.

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::plan::Kind as GearKind;
use solidcraft_gear::sketch_rules;

use crate::SolveApp;
use crate::dialogs::Kind;
use crate::selection::{CURVES, FEATURES, SKETCH_POINTS, SelInput};
use crate::ui_kit::{self, choice_row, hint_row};

/// The tooth styles for parallel shafts, as the Type row lists them.
pub const STYLES: [(GearKind, &str); 3] = [(GearKind::Spur, "Spur"), (GearKind::Helical, "Helical"), (GearKind::Herringbone, "Herringbone")];

/// What the dialog asks the app to do once its frame is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Draw a new sketch for the set (`gear.sketch`) and open it for editing.
    NewSketch,
    /// Open the set's sketch for editing.
    EditSketch,
    /// The Gear Generator window (the other families, every setting).
    Window,
}

/// A sketch opened from the dialog: when its editing ends, the dialog comes back on it.
#[derive(Clone, Debug, PartialEq)]
pub struct AfterSketch {
    pub sketch: u64,
    /// The sketch has been seen open (its editing has started).
    pub opened: bool,
    pub style: usize,
    pub group: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gear {
    /// An index into [`STYLES`].
    pub style: usize,
    /// The sketch taken when the dialog opened (a sketch being edited, or the set's own); the
    /// Sketch input's picks win over it.
    pub sketch: Option<u64>,
    /// Editing this set (its timeline folder): OK updates it.
    pub group: Option<u64>,
    /// Empty: worked out (a standard module for the circles; 10 × module).
    pub module: String,
    pub face_width: String,
    pub helix: String,
    pub assembly: bool,
    /// The last request worked out and what it came to (a summary, or the engine's refusal).
    pub calc: Option<(String, Result<Vec<String>, String>)>,
    pub action: Option<Action>,
}

impl Gear {
    pub fn title(&self) -> &'static str {
        "GEARS"
    }

    pub fn previews(&self) -> bool {
        true
    }
}

fn new(sketch: Option<u64>) -> Gear {
    Gear {
        style: 0,
        sketch,
        group: None,
        module: String::new(),
        face_width: String::new(),
        helix: "20 deg".into(),
        assembly: false,
        calc: None,
        action: None,
    }
}

/// The dialog for `gear.create`.
pub fn start(app: &SolveApp, id: &str) -> Option<(Kind, Vec<SelInput>)> {
    if id != "gear.create" {
        return None;
    }
    let inputs = vec![SelInput::new("Sketch", CURVES | SKETCH_POINTS | FEATURES, true)];
    // A set drawn in a sketch, selected: the dialog on it, with what it was made with.
    if let Some(g) = solidcraft_engine::gear_selected_set(&app.session).and_then(|set| for_set(&set)) {
        return Some((Kind::Gear(g), inputs));
    }
    let mut g = new(None);
    if let Some(after) = &app.gear_after_sketch {
        g.sketch = Some(after.sketch);
        g.style = after.style;
        g.group = after.group;
    }
    Some((Kind::Gear(g), inputs))
}

/// The dialog on a set made from a sketch, with what it was made with.
pub fn for_set(set: &solidcraft_engine::schema::gear::GearSet) -> Option<Gear> {
    let sketch = set.sketch?;
    let r = &set.request;
    let kind = r.get("type").and_then(Value::as_str).unwrap_or("spur");
    let helix = r.get("helix_angle").and_then(Value::as_f64).unwrap_or(0.0);
    let herringbone = kind == "herringbone" || r.get("herringbone").and_then(Value::as_bool) == Some(true);
    let style = if herringbone {
        2
    } else if helix.abs() > 1e-9 || kind == "helical" {
        1
    } else {
        0
    };
    // Only the parallel families are this dialog's yet.
    if !matches!(kind, "spur" | "helical" | "herringbone" | "pair" | "internal" | "planetary") {
        return None;
    }
    let mut g = new(Some(sketch));
    g.style = style;
    g.group = Some(set.group);
    if helix.abs() > 1e-9 {
        g.helix = format!("{} deg", crate::gear_view3d::fmt(helix.abs()));
    }
    if let Some(m) = r.get("module").and_then(Value::as_f64) {
        g.module = format!("{} mm", crate::gear_view3d::fmt(m));
    }
    if let Some(f) = r.get("face_width").and_then(Value::as_f64) {
        g.face_width = format!("{} mm", crate::gear_view3d::fmt(f));
    }
    g.assembly = r.get("assembly").and_then(Value::as_bool) == Some(true);
    Some(g)
}

/// The sketch the set is drawn in: the Sketch input's (a curve or point of it, or the sketch
/// in the timeline), else the one the dialog opened on.
fn sketch_of(app: &SolveApp, k: &Gear, inputs: &[SelInput]) -> Option<u64> {
    let items = inputs.first().map(|i| i.items.as_slice()).unwrap_or(&[]);
    // (Editing a set, a feature picked is one of its own, not the sketch it is drawn in.)
    let feature = items.iter().find_map(|x| match x {
        Sel::Feature { id } if k.group.is_none() && app.session.doc.sketch(*id).is_ok() => Some(*id),
        _ => None,
    });
    let curves: Vec<Sel> = items.iter().filter(|x| matches!(x, Sel::SketchCurve { .. })).cloned().collect();
    feature.or_else(|| (!curves.is_empty()).then(|| crate::dialogs::picks_sketch(app, &curves, None)).flatten()).or(k.sketch)
}

/// BRIDGE until the gear-train reading lands in the engine (`gear.create {sketch, type:
/// "spur"}` with any number of circles): two circles touching outside are a pair, one inside
/// another internal, three about one centre planetary, as the sketch rules recognise them.
fn kind_for(app: &SolveApp, sketch: u64, style: GearKind) -> GearKind {
    let drawn = solidcraft_engine::gear_sketch(&app.session, sketch).map(|g| g.user_drawn());
    match drawn.ok().and_then(|d| sketch_rules::recognize(&d)) {
        Some(k @ (GearKind::Pair | GearKind::Internal | GearKind::Planetary)) => k,
        _ => style,
    }
}

fn eval(app: &SolveApp, text: &str, kind: ValueKind) -> Result<Option<f64>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    app.session.doc.eval(text, kind).map(Some).map_err(|e| e.to_string())
}

/// `gear.create`'s parameters for what the dialog holds.
fn request(app: &SolveApp, k: &Gear, inputs: &[SelInput]) -> Result<Value, String> {
    let sketch = sketch_of(app, k, inputs).ok_or("Pick the sketch the gears are drawn in (its circles are the pitch circles), or press New Sketch")?;
    let style = STYLES.get(k.style).map_or(GearKind::Spur, |s| s.0);
    let kind = kind_for(app, sketch, style);
    let mut p = json!({"type": kind.name(), "sketch": sketch});
    if style != GearKind::Spur {
        let helix = eval(app, &k.helix, ValueKind::Angle).map_err(|e| format!("Helix Angle: {e}"))?.unwrap_or(20.0);
        p["helix_angle"] = json!(helix);
        if style == GearKind::Herringbone && kind != GearKind::Herringbone {
            p["herringbone"] = json!(true);
        }
    }
    if let Some(m) = eval(app, &k.module, ValueKind::Length).map_err(|e| format!("Module: {e}"))? {
        p["module"] = json!(m);
    }
    if let Some(f) = eval(app, &k.face_width, ValueKind::Length).map_err(|e| format!("Face Width: {e}"))? {
        p["face_width"] = json!(f);
    }
    if k.assembly {
        p["assembly"] = json!(true);
    }
    Ok(p)
}

pub fn commands(app: &SolveApp, k: &Gear, inputs: &[SelInput], _extra: &Map<String, Value>) -> Result<Vec<(String, Value)>, String> {
    let mut p = request(app, k, inputs)?;
    Ok(vec![match k.group {
        Some(group) => {
            // Laid over what the set was made with: what is auto now is taken away.
            p["group"] = json!(group);
            for key in ["module", "face_width", "helix_angle", "herringbone", "assembly"] {
                if p.get(key).is_none() {
                    p[key] = Value::Null;
                }
            }
            ("gear.update".to_string(), p)
        }
        None => ("gear.create".to_string(), p),
    }])
}

/// The facts worth showing under the form: what the set came to.
fn summary(r: &Value) -> Vec<String> {
    let wanted = ["module", "ratio", "centre distance", "center distance", "gear 1 teeth", "gear 2 teeth", "teeth", "sun teeth", "planet teeth", "ring teeth"];
    let mut out: Vec<String> = r["facts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["label"].as_str().is_some_and(|l| wanted.contains(&l)))
        .map(|f| format!("{}: {}{}", f["label"].as_str().unwrap_or(""), crate::gear_view3d::fmt(f["value"].as_f64().unwrap_or(0.0)), if f["unit"] == "mm" { " mm" } else { "" }))
        .collect();
    out.extend(r["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|w| format!("⚠ {w}")));
    out
}

pub fn rows(app: &mut SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &[SelInput]) -> bool {
    let mut enter = false;
    let names: Vec<&str> = STYLES.iter().map(|s| s.1).collect();
    choice_row(ui, "Type", "gear_style", &mut k.style, &names);
    if STYLES.get(k.style).is_some_and(|s| s.0 != GearKind::Spur) {
        enter |= hint_row(ui, "Helix Angle", &mut k.helix, "deg");
    }
    enter |= hint_row(ui, "Module", &mut k.module, "auto: a standard module");
    enter |= hint_row(ui, "Face Width", &mut k.face_width, "auto: 10 × module");
    ui.checkbox(&mut k.assembly, solidcraft_ui_text::tr_label("Assembly")).on_hover_text("Each gear a component, turning on a joint, linked to its mates");
    ui.horizontal_wrapped(|ui| {
        if ui_kit::button(ui, solidcraft_ui_text::tr_label("New Sketch"), ui_kit::Button::Normal)
            .on_hover_text("Draw the set in a new sketch: a pitch circle to start from; add circles, dimension them and make them tangent")
            .clicked()
        {
            k.action = Some(Action::NewSketch);
        }
        if sketch_of(app, k, inputs).is_some()
            && ui_kit::button(ui, solidcraft_ui_text::tr_label("Edit Sketch"), ui_kit::Button::Normal).on_hover_text("Open the set's sketch to change its circles, dimensions and constraints").clicked()
        {
            k.action = Some(Action::EditSketch);
        }
    });
    // What the engine reads from the sketch: worked out once per change.
    match request(app, k, inputs) {
        Ok(p) => {
            let key = p.to_string();
            if k.calc.as_ref().is_none_or(|(c, _)| *c != key) {
                let r = app.session.execute("gear.calculate", &p).map(|r| summary(&r)).map_err(|e| e.to_string());
                k.calc = Some((key, r));
            }
            match &k.calc {
                Some((_, Ok(lines))) => {
                    for l in lines {
                        ui.label(egui::RichText::new(l).small());
                    }
                }
                Some((_, Err(e))) => {
                    ui.label(egui::RichText::new(e).small().color(ui.visuals().warn_fg_color));
                }
                None => {}
            }
        }
        Err(e) => {
            ui.label(egui::RichText::new(e).small().weak());
        }
    }
    ui_kit::section(ui, "gear_more", "More Types", false, |ui| {
        ui.label(egui::RichText::new("Bevel, worm, hypoid, rack, chain and belt, and every setting").small().weak());
        if ui_kit::button(ui, solidcraft_ui_text::tr_label("Gear Generator…"), ui_kit::Button::Normal).clicked() {
            k.action = Some(Action::Window);
        }
    });
    enter
}

/// Once a frame: what a Gear dialog asked for, and the dialog back on a sketch opened from it
/// once its editing ends.
pub fn tick(app: &mut SolveApp) {
    let asked = match app.dialog.as_mut() {
        Some(d) => match &mut d.kind {
            Kind::Gear(g) => g.action.take().map(|a| (a, g.style, g.group, g.sketch)),
            _ => None,
        },
        None => None,
    };
    if let Some((action, style, group, opened_on)) = asked {
        let picked = app.dialog.as_ref().and_then(|d| match &d.kind {
            Kind::Gear(g) => sketch_of(app, g, &d.inputs),
            _ => None,
        });
        crate::dialogs::cancel(app);
        app.dialog = None;
        match action {
            Action::Window => crate::gear_window::open(app),
            Action::NewSketch => {
                let kind = STYLES.get(style).map_or(GearKind::Spur, |s| s.0);
                match app.run("gear.sketch", json!({"type": kind.name()})) {
                    Ok(r) => {
                        if let Some(id) = r.get("sketch").and_then(Value::as_u64) {
                            app.gear_after_sketch = Some(AfterSketch { sketch: id, opened: false, style, group });
                            app.edit_sketch(id);
                        }
                    }
                    Err(e) => app.set_status(e, true),
                }
            }
            Action::EditSketch => {
                if let Some(id) = picked.or(opened_on) {
                    app.gear_after_sketch = Some(AfterSketch { sketch: id, opened: false, style, group });
                    app.edit_sketch(id);
                }
            }
        }
        return;
    }
    // The dialog back once the sketch opened from it is finished.
    let Some(after) = app.gear_after_sketch.as_mut() else { return };
    if matches!(app.dialog.as_ref().map(|d| &d.kind), Some(Kind::Gear(_))) {
        app.gear_after_sketch = None;
        return;
    }
    if app.session.active_sketch == Some(after.sketch) {
        after.opened = true;
        return;
    }
    if after.opened && app.session.active_sketch.is_none() && app.dialog.is_none() && app.tool.is_none() {
        app.start("gear.create");
        app.gear_after_sketch = None;
    }
}

#[cfg(test)]
#[path = "dialogs_gear_tests.rs"]
mod tests;
