//! The Gear Generator as a native command dialog.
//!
//! The set is drawn in an ordinary sketch of the design, as the sketch rules read it
//! (`solidcraft_gear::sketch_rules`): circles are pitch circles, lines are shafts, a point is the
//! origin (a bevel's apex). It is drawn, dimensioned and constrained with the app's own sketch
//! tools: a pair's circles tangent, a worm's line tangent to its wheel, a bevel's shafts meeting
//! at the apex with their angle as an angle dimension, a hypoid's offset as a distance. The
//! dialog picks that sketch (its curves, or the sketch in the timeline; a sketch being edited
//! when Gears is started is finished and taken, as Extrude takes its profile), the type and the
//! few settings a drawing can't show, and makes the set with `gear.create {sketch}`: the engine
//! reads the sketch and the set follows it afterwards (a changed dimension regenerates it in
//! place, `gear::follow_sketches`). What the set comes to is summarised from `gear.calculate` on
//! the same parameters, and the set itself is the dialog's live preview in the viewport, as
//! every command dialog previews. New Sketch draws the type's starting picture with
//! `gear.sketch` (native constraints and dimensions) and opens it in the sketch editor; Finish
//! brings the dialog back on it.
//!
//! The types a sketch can't hold (spline, elliptical gears, cycloidal drive, differential) are
//! set up in the Gear Generator window, one button away.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::plan::Kind as GearKind;
use solidcraft_gear::sketch_rules;

use crate::SolveApp;
use crate::dialogs::Kind;
use crate::selection::{CURVES, FEATURES, SKETCH_POINTS, SelInput};
use crate::ui_kit::{self, choice_row, hint_row};

/// The Type row: tooth styles for parallel shafts (the sketch decides single gear, pair,
/// internal or planetary), then the other families.
pub const TYPES: [(GearKind, &str); 18] = [
    (GearKind::Spur, "Spur"),
    (GearKind::Helical, "Helical"),
    (GearKind::Herringbone, "Herringbone"),
    (GearKind::Bevel, "Bevel"),
    (GearKind::SpiralBevel, "Spiral Bevel"),
    (GearKind::BevelPlanetary, "Bevel Planetary"),
    (GearKind::FaceGear, "Face Gear"),
    (GearKind::Worm, "Worm"),
    (GearKind::Rack, "Rack"),
    (GearKind::Hypoid, "Hypoid"),
    (GearKind::CrossedHelical, "Crossed Helical"),
    (GearKind::Sprocket, "Sprocket"),
    (GearKind::Pulley, "Timing Pulley"),
    (GearKind::Cycloidal, "Cycloidal"),
    (GearKind::Differential, "Differential"),
    (GearKind::Spline, "Spline"),
    (GearKind::Elliptical, "Elliptical"),
    (GearKind::CycloidalDrive, "Cycloidal Drive"),
];

/// Tooth styles: the sketch's circles decide the set.
fn parallel(k: GearKind) -> bool {
    matches!(k, GearKind::Spur | GearKind::Helical | GearKind::Herringbone)
}

/// One value row: the `gear.create` parameter, its label, what it is, its hint.
type Field = (&'static str, &'static str, ValueKind, &'static str);

/// The settings a type's drawing can't show (empty: worked out).
fn fields(k: GearKind) -> Vec<Field> {
    let module: Field = ("module", "Module", ValueKind::Length, "auto: a standard module");
    let face: Field = ("face_width", "Face Width", ValueKind::Length, "auto");
    let helix: Field = ("helix_angle", "Helix Angle", ValueKind::Angle, "deg");
    let teeth: Field = ("teeth", "Teeth", ValueKind::Unitless, "gear 1");
    let teeth2: Field = ("teeth2", "Teeth 2", ValueKind::Unitless, "gear 2");
    let spiral: Field = ("spiral_angle", "Spiral Angle", ValueKind::Angle, "deg");
    let shaft: Field = ("shaft_angle", "Shaft Angle", ValueKind::Angle, "90 deg");
    match k {
        GearKind::Spur => vec![module, face],
        GearKind::Helical | GearKind::Herringbone => vec![helix, module, face],
        // The sketch gives the shaft angle; the sizes are settings.
        GearKind::Bevel => vec![module, teeth, teeth2, face],
        GearKind::SpiralBevel => vec![module, teeth, teeth2, spiral, face],
        GearKind::BevelPlanetary => vec![
            module,
            ("teeth", "Sun Teeth", ValueKind::Unitless, "auto"),
            ("teeth2", "Planet Teeth", ValueKind::Unitless, "auto"),
            ("planets", "Planets", ValueKind::Unitless, "auto"),
            face,
        ],
        GearKind::FaceGear => vec![module, ("teeth", "Pinion Teeth", ValueKind::Unitless, "auto"), face],
        GearKind::Worm => vec![("starts", "Starts", ValueKind::Unitless, "1"), module, face],
        GearKind::Rack | GearKind::Cycloidal => vec![module, face],
        // The sketch gives the ring gear's Ø and the offset.
        GearKind::Hypoid => vec![shaft, ("teeth", "Pinion Teeth", ValueKind::Unitless, "auto"), spiral, face],
        GearKind::CrossedHelical => vec![shaft, ("helix_angle", "Helix Angle 1", ValueKind::Angle, "half the shaft angle"), module, face],
        GearKind::Sprocket | GearKind::Pulley => vec![face],
        _ => Vec::new(),
    }
}

/// What the dialog asks the app to do once its frame is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Draw a new sketch for the set (`gear.sketch`) and open it for editing.
    NewSketch,
    /// Open the set's sketch for editing.
    EditSketch,
    /// The Gear Generator window (the types a sketch can't hold, every setting).
    Window,
}

/// A sketch opened from the dialog: when its editing ends, the dialog comes back on it.
#[derive(Clone, Debug, PartialEq)]
pub struct AfterSketch {
    pub sketch: u64,
    /// The sketch has been seen open (its editing has started).
    pub opened: bool,
    pub kind: usize,
    pub group: Option<u64>,
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gear {
    /// An index into [`TYPES`].
    pub kind: usize,
    /// The sketch taken when the dialog opened (a sketch being edited, or the set's own); the
    /// Sketch input's picks win over it.
    pub sketch: Option<u64>,
    /// Editing this set (its timeline folder): OK updates it.
    pub group: Option<u64>,
    /// The value rows ([`fields`]), by parameter, as typed (empty: worked out).
    pub values: BTreeMap<String, String>,
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
        sketch_rules::sketched(self.gear_kind())
    }

    pub fn gear_kind(&self) -> GearKind {
        TYPES.get(self.kind).map_or(GearKind::Spur, |t| t.0)
    }

    pub fn value_mut(&mut self, key: &str) -> &mut String {
        self.values.entry(key.to_string()).or_default()
    }
}

fn new(sketch: Option<u64>) -> Gear {
    let mut values = BTreeMap::new();
    values.insert("helix_angle".to_string(), "20 deg".to_string());
    Gear { kind: 0, sketch, group: None, values, assembly: false, calc: None, action: None }
}

fn inputs() -> Vec<SelInput> {
    vec![SelInput::new("Sketch", CURVES | SKETCH_POINTS | FEATURES, true)]
}

/// The dialog for `gear.create`.
pub fn start(app: &SolveApp, id: &str) -> Option<(Kind, Vec<SelInput>)> {
    if id != "gear.create" {
        return None;
    }
    // A set drawn in a sketch, selected: the dialog on it, with what it was made with.
    if let Some(g) = solidcraft_engine::gear_selected_set(&app.session).and_then(|set| for_set(&set)) {
        return Some((Kind::Gear(g), inputs()));
    }
    let mut g = new(None);
    if let Some(after) = &app.gear_after_sketch {
        g.sketch = Some(after.sketch);
        g.kind = after.kind;
        g.group = after.group;
        g.values = after.values.clone();
    }
    Some((Kind::Gear(g), inputs()))
}

/// The Type row's entry for a set's `type` (a pair, internal or planetary set is the tooth
/// style its helix gives).
fn type_index(kind: &str, helix: f64, herringbone: bool) -> Option<usize> {
    let k = match kind {
        "pair" | "internal" | "planetary" | "spur" | "helical" | "herringbone" => {
            if herringbone || kind == "herringbone" {
                GearKind::Herringbone
            } else if helix.abs() > 1e-9 || kind == "helical" {
                GearKind::Helical
            } else {
                GearKind::Spur
            }
        }
        other => *GearKind::ALL.iter().find(|k| k.name() == other)?,
    };
    TYPES.iter().position(|t| t.0 == k)
}

fn shown(v: f64, kind: ValueKind) -> String {
    let n = crate::gear_view3d::fmt(v);
    match kind {
        ValueKind::Length => format!("{n} mm"),
        ValueKind::Angle => format!("{n} deg"),
        _ => n,
    }
}

/// The dialog on a set made from a sketch, with what it was made with.
pub fn for_set(set: &solidcraft_engine::schema::gear::GearSet) -> Option<Gear> {
    let sketch = set.sketch?;
    let r = &set.request;
    let helix = r.get("helix_angle").and_then(Value::as_f64).unwrap_or(0.0);
    let herringbone = r.get("herringbone").and_then(Value::as_bool) == Some(true);
    let kind = type_index(r.get("type").and_then(Value::as_str).unwrap_or("spur"), helix, herringbone)?;
    let mut g = new(Some(sketch));
    g.kind = kind;
    g.group = Some(set.group);
    for (key, _, vk, _) in fields(g.gear_kind()) {
        if let Some(v) = r.get(key).and_then(Value::as_f64) {
            g.values.insert(key.to_string(), shown(if key == "helix_angle" { v.abs() } else { v }, vk));
        }
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
    if !parallel(style) {
        return style;
    }
    let drawn = solidcraft_engine::gear_sketch(&app.session, sketch).map(|g| g.user_drawn());
    match drawn.ok().and_then(|d| sketch_rules::recognize(&d)) {
        Some(k @ (GearKind::Pair | GearKind::Internal | GearKind::Planetary)) => k,
        _ => style,
    }
}

/// The value rows as `gear.create` numbers (counts whole), and what is wrong with one.
fn numbers(app: &SolveApp, k: &Gear) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (key, label, vk, _) in fields(k.gear_kind()) {
        let text = k.values.get(key).map(String::as_str).unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let v = app.session.doc.eval(text, vk).map_err(|e| format!("{}: {e}", solidcraft_ui_text::tr_label(label)))?;
        out.insert(key.to_string(), if matches!(vk, ValueKind::Unitless) { json!(v.round().max(0.0) as u64) } else { json!(v) });
    }
    Ok(out)
}

/// `gear.create`'s parameters for what the dialog holds.
fn request(app: &SolveApp, k: &Gear, inputs: &[SelInput]) -> Result<Value, String> {
    let gk = k.gear_kind();
    if !sketch_rules::sketched(gk) {
        return Err(format!("A {} isn't drawn in a sketch: set it up in the Gear Generator window (More Types)", gk.label()));
    }
    let sketch = sketch_of(app, k, inputs).ok_or("Pick the sketch the gears are drawn in (circles are pitch circles, lines are shafts), or press New Sketch")?;
    let kind = kind_for(app, sketch, gk);
    let mut p = Map::new();
    p.insert("type".into(), json!(kind.name()));
    p.insert("sketch".into(), json!(sketch));
    p.extend(numbers(app, k)?);
    if parallel(gk) {
        if gk == GearKind::Spur {
            p.remove("helix_angle");
        } else if !p.contains_key("helix_angle") {
            p.insert("helix_angle".into(), json!(20.0));
        }
        if gk == GearKind::Herringbone && kind != GearKind::Herringbone {
            p.insert("herringbone".into(), json!(true));
        }
    }
    if k.assembly {
        p.insert("assembly".into(), json!(true));
    }
    Ok(Value::Object(p))
}

pub fn commands(app: &SolveApp, k: &Gear, inputs: &[SelInput], _extra: &Map<String, Value>) -> Result<Vec<(String, Value)>, String> {
    let mut p = request(app, k, inputs)?;
    Ok(vec![match k.group {
        Some(group) => {
            // Laid over what the set was made with: what is auto now is taken away.
            p["group"] = json!(group);
            for key in fields(k.gear_kind()).iter().map(|f| f.0).chain(["herringbone", "assembly"]) {
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
    let wanted = [
        "module",
        "ratio",
        "centre distance",
        "center distance",
        "shaft angle",
        "gear 1 teeth",
        "gear 2 teeth",
        "teeth",
        "sun teeth",
        "planet teeth",
        "ring teeth",
        "pinion teeth",
        "wheel teeth",
    ];
    let mut out: Vec<String> = r["facts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["label"].as_str().is_some_and(|l| wanted.contains(&l)))
        .map(|f| {
            let unit = match f["unit"].as_str() {
                Some("mm") => " mm",
                Some("deg") => "°",
                _ => "",
            };
            format!("{}: {}{unit}", f["label"].as_str().unwrap_or(""), crate::gear_view3d::fmt(f["value"].as_f64().unwrap_or(0.0)))
        })
        .collect();
    out.extend(r["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|w| format!("⚠ {w}")));
    out
}

pub fn rows(app: &mut SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &[SelInput]) -> bool {
    let mut enter = false;
    let names: Vec<&str> = TYPES.iter().map(|t| t.1).collect();
    let before = k.kind;
    choice_row(ui, "Type", "gear_type", &mut k.kind, &names);
    if k.kind != before {
        k.calc = None;
    }
    let gk = k.gear_kind();
    if !sketch_rules::sketched(gk) {
        ui.label(egui::RichText::new(format!("A {} isn't drawn in a sketch: it is set up in the Gear Generator window", gk.label())).small().weak());
        if ui_kit::button(ui, solidcraft_ui_text::tr_label("Gear Generator…"), ui_kit::Button::Normal).clicked() {
            k.action = Some(Action::Window);
        }
        return enter;
    }
    for (key, label, _, hint) in fields(gk) {
        enter |= hint_row(ui, label, k.value_mut(key), hint);
    }
    ui.checkbox(&mut k.assembly, solidcraft_ui_text::tr_label("Assembly")).on_hover_text("Each gear a component, turning on a joint, linked to its mates");
    ui.horizontal_wrapped(|ui| {
        if ui_kit::button(ui, solidcraft_ui_text::tr_label("New Sketch"), ui_kit::Button::Normal)
            .on_hover_text("Draw the set in a new sketch, from the type's starting picture with its constraints and dimensions")
            .clicked()
        {
            k.action = Some(Action::NewSketch);
        }
        if sketch_of(app, k, inputs).is_some()
            && ui_kit::button(ui, solidcraft_ui_text::tr_label("Edit Sketch"), ui_kit::Button::Normal)
                .on_hover_text("Open the set's sketch to change its circles, lines, dimensions and constraints")
                .clicked()
        {
            k.action = Some(Action::EditSketch);
        }
    });
    // What the engine reads from the sketch: worked out once per change.
    match request(app, k, inputs) {
        Ok(p) => {
            let key = format!("{}@{}", p, app.session.revision);
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
    ui_kit::section(ui, "gear_more", "More", false, |ui| {
        ui.label(egui::RichText::new("Bores, hubs, tooth tuning, strength and every other setting").small().weak());
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
            Kind::Gear(g) => g.action.take().map(|a| (a, g.clone())),
            _ => None,
        },
        None => None,
    };
    if let Some((action, g)) = asked {
        let picked = app.dialog.as_ref().and_then(|d| sketch_of(app, &g, &d.inputs));
        crate::dialogs::cancel(app);
        app.dialog = None;
        let after = |sketch: u64| AfterSketch { sketch, opened: false, kind: g.kind, group: g.group, values: g.values.clone() };
        match action {
            Action::Window => crate::gear_window::open(app),
            Action::NewSketch => {
                let sizes = numbers(app, &g).unwrap_or_default();
                match app.run("gear.sketch", json!({"type": g.gear_kind().name(), "gear": Value::Object(sizes)})) {
                    Ok(r) => {
                        if let Some(id) = r.get("sketch").and_then(Value::as_u64) {
                            app.gear_after_sketch = Some(after(id));
                            app.edit_sketch(id);
                        }
                    }
                    Err(e) => app.set_status(e, true),
                }
            }
            Action::EditSketch => {
                if let Some(id) = picked.or(g.sketch) {
                    app.gear_after_sketch = Some(after(id));
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
