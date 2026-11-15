//! Gears: the Gear Generator as a native command dialog (MODEL › CREATE › Gears).
//!
//! The set is drawn first, in an ordinary sketch of the design, with the app's own sketch tools:
//! circles are pitch circles (tangent circles mesh; a line from one centre to another says two
//! circles mesh too), lines are shafts, a point is the origin (a bevel's apex), and construction
//! curves are helpers, never gears or shafts (`gear::sketch_entities`, `sketch_rules::read`).
//! Starting Gears with the sketch open finishes it and takes it, as Extrude takes its profile;
//! with its curves selected, or the sketch picked in the timeline, it takes that. The dialog
//! adds the type and the few settings a drawing can't show, and OK makes the set with
//! `gear.create {sketch}`: the engine reads the sketch, and the set follows it afterwards (a
//! changed dimension regenerates it in place, `gear::follow_sketches`).
//!
//! The picks are the dialog's list: every circle, line, point, 3D sketch curve, model edge or
//! face picked (the native Geometry input: filters, box select and pick order as every dialog
//! has them) is a numbered row with its icon and name, lit in the view while hovered, with its
//! own role: a gear (spur, helical, herringbone, bevel, worm, wheel, rack, ring, face gear), a
//! shaft (one solid shaft through the gears on it, which then turn together) or a bearing (a
//! point on a picked shaft line, sized from the bearing library to fit the shaft). The rows are
//! sent as `gear.create`'s `selection`, each with its role and its module or teeth; the engine
//! works the meshes out from the drawing's tangencies, angles and distances and says per row
//! what each came to (`gear.calculate`'s `members`), which the rows show: module, teeth, Ø, what
//! it meshes with, the shaft it shares (a compound gear), or what is wrong with it. The types
//! a sketch can't hold (spline, elliptical gears, cycloidal drive, differential) and the set
//! families read as a whole (hypoid, crossed helicals, bevel planetary, sprocket, pulley,
//! cycloidal) take the sketch as one drawing; the first four are made from their rows alone.
//!
//! The preview is the set's abstract shape drawn over the viewport: each member's shaft and
//! pitch surface in its colour (the gear crate's schematic of what `gear.calculate` worked out),
//! no teeth; the teeth are built on OK, or as the live preview when Preview Teeth is on. What the
//! engine refuses is said under the form in its own words, and a circle it names is drawn in
//! the warning colour in the view. Edit Gears (a set's timeline folder) reopens the dialog on the
//! set; OK then updates it in place (`gear.update`), keeping its components and joints.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::plan::Kind as GearKind;
use solidcraft_gear::simple::{Role, Shape};
use solidcraft_gear::sketch_rules;

use crate::SolveApp;
use crate::dialogs::{Kind, combo, form_row};
use crate::ui_kit::icons::paint_icon;
use crate::selection::{AXES, CURVES, EDGES, FACES, FEATURES, SKETCH_POINTS, SelInput};
use crate::ui_kit::{self, choice_row, hint_row};

/// The Type row: tooth styles for parallel shafts (the sketch decides single gear, pair,
/// internal, planetary set or train), the other families a sketch holds, then the ones made from
/// their numbers alone.
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

/// What a picked row can be: `gear.create`'s `selection` roles, with their labels.
const ROLES: [(&str, &str); 13] = [
    ("spur", "Spur"),
    ("helical", "Helical"),
    ("herringbone", "Herringbone"),
    ("bevel", "Bevel"),
    ("spiral_bevel", "Spiral Bevel"),
    ("worm", "Worm"),
    ("wheel", "Worm Wheel"),
    ("rack", "Rack"),
    ("internal", "Ring Gear"),
    ("face", "Face Gear"),
    ("shaft", "Shaft"),
    ("bearing", "Bearing"),
    ("", "Not Used"),
];
const SHAFT_ROLE: usize = 10;
const BEARING_ROLE: usize = 11;
const UNUSED_ROLE: usize = 12;

/// The types whose picks are rows with roles (the rest take the sketch as one drawing).
fn by_rows(k: GearKind) -> bool {
    matches!(k, GearKind::Spur | GearKind::Helical | GearKind::Herringbone | GearKind::Bevel | GearKind::SpiralBevel | GearKind::Worm | GearKind::Rack | GearKind::FaceGear)
}

/// The role a gear of type `k` has by default.
fn role_of(k: GearKind) -> usize {
    match k {
        GearKind::Helical => 1,
        GearKind::Herringbone => 2,
        GearKind::Bevel => 3,
        GearKind::SpiralBevel => 4,
        GearKind::Worm => 5,
        GearKind::Rack => 7,
        GearKind::FaceGear => 9,
        _ => 0,
    }
}

/// What a pick is, for its row's icon, name and roles.
#[derive(Clone, Debug, PartialEq)]
enum Geo {
    /// A circle of a sketch (its id, centre and radius in the sketch's plane).
    Circle { sketch: u64, id: String, c: solidcraft_gear::V2, r: f64 },
    /// A straight line of a sketch (its ends in the sketch's plane).
    Line { sketch: u64, id: String, a: solidcraft_gear::V2, b: solidcraft_gear::V2 },
    /// A point of a sketch (where it is in the sketch's plane, when it is in it).
    Point { sketch: u64, id: String, at: Option<solidcraft_gear::V2> },
    /// Another curve of a sketch: a 3D sketch's line or arc, an arc.
    Curve { sketch: u64, id: String },
    /// A model edge, face or axis.
    Model,
    /// A whole sketch (picked in the timeline).
    Sketch(u64),
    Other,
}

impl Geo {
    fn icon(&self) -> &'static str {
        match self {
            Geo::Circle { .. } => "circle",
            Geo::Line { .. } => "line",
            Geo::Point { .. } => "point",
            Geo::Curve { .. } => "arc3",
            Geo::Model => "select_faces_edges",
            Geo::Sketch(_) => "sketch",
            Geo::Other => "gear_generator",
        }
    }

    fn sketch(&self) -> Option<u64> {
        match self {
            Geo::Circle { sketch, .. } | Geo::Line { sketch, .. } | Geo::Point { sketch, .. } | Geo::Curve { sketch, .. } | Geo::Sketch(sketch) => Some(*sketch),
            _ => None,
        }
    }
}

/// What `sel` is.
fn geo(app: &SolveApp, sel: &Sel) -> Geo {
    let curve = |sketch: u64, id: &str| -> Geo {
        let Ok(gs) = solidcraft_engine::gear_sketch(&app.session, sketch) else { return Geo::Curve { sketch, id: id.to_string() } };
        if let Some((_, c)) = gs.circles.iter().find(|(k, _)| k == id) {
            return Geo::Circle { sketch, id: id.to_string(), c: c.c, r: c.r };
        }
        if let Some((_, l)) = gs.lines.iter().find(|(k, _)| k == id) {
            return Geo::Line { sketch, id: id.to_string(), a: l.a, b: l.b };
        }
        Geo::Curve { sketch, id: id.to_string() }
    };
    let point = |sketch: u64, id: &str| -> Geo {
        let state = crate::open_meshing::session_world(&app.session);
        let at = state.sketch(sketch).and_then(|ss| ss.sketch.point(ss.sketch.point_index(id)?)).map(|p| solidcraft_gear::V2::new(p.x, p.y));
        Geo::Point { sketch, id: id.to_string(), at }
    };
    match sel {
        Sel::SketchCurve { id, sketch } => match sketch.or(app.session.active_sketch).or_else(|| crate::dialogs::picks_sketch(app, std::slice::from_ref(sel), None)) {
            Some(sk) => curve(sk, id),
            None => Geo::Other,
        },
        Sel::ExternalSketchCurve { sketch, id } => curve(*sketch, id),
        Sel::ExternalSketchPoint { sketch, id } => point(*sketch, id),
        Sel::SketchPoint { id } => app.session.active_sketch.map_or(Geo::Other, |sk| point(sk, id)),
        Sel::Edge { .. } | Sel::Face { .. } | Sel::Axis { .. } => Geo::Model,
        Sel::Feature { id } if app.session.doc.sketch(*id).is_ok() => Geo::Sketch(*id),
        _ => Geo::Other,
    }
}

/// Why a point can't be a bearing, or `None` when it can: it must lie on a shaft line picked
/// with it.
fn bearing_refused(point: &Geo, all: &[Geo]) -> Option<&'static str> {
    let Geo::Point { sketch, at: Some(p), .. } = point else { return Some("a bearing is a point on a picked shaft line") };
    let on = all.iter().any(|g| match g {
        Geo::Line { sketch: s, a, b, .. } if s == sketch => {
            let l = sketch_rules::Line { a: *a, b: *b };
            let t = (*p - *a).dot(l.dir());
            l.distance(*p) < 1e-6 * l.len().max(1.0) && t > -1e-9 && t < l.len() + 1e-9
        }
        _ => false,
    });
    (!on).then_some("a bearing is a point on a picked shaft line: pick the line too, or put the point on it (Coincident)")
}

/// A row's role when it is first picked: a circle the dialog's gear type, a line a shaft (a
/// gear's shaft for the types drawn with lines), a point on a picked line a bearing.
fn default_role(k: GearKind, g: &Geo, all: &[Geo]) -> usize {
    match g {
        Geo::Circle { .. } => role_of(k),
        Geo::Line { .. } | Geo::Curve { .. } | Geo::Model if !parallel(k) => role_of(k),
        Geo::Line { .. } | Geo::Curve { .. } => SHAFT_ROLE,
        Geo::Model => role_of(k),
        Geo::Point { .. } if bearing_refused(g, all).is_none() => BEARING_ROLE,
        _ => UNUSED_ROLE,
    }
}

/// One picked row: what it is, its role, its teeth when the rows are sized by teeth.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub sel: Sel,
    pub role: usize,
    pub teeth: String,
}

/// A row's name: the pick as the input's chips name it, with the sketch it is in (a set may be
/// drawn across several sketches on different planes).
fn row_name(app: &SolveApp, g: &Geo, label: &str) -> String {
    let sketch = match g {
        Geo::Sketch(_) => None,
        other => other.sketch(),
    };
    match sketch.and_then(|id| app.session.doc.feature(id)) {
        Some(f) if !label.contains(&f.name) => format!("{label} · {}", f.name),
        _ => label.to_string(),
    }
}

/// What `gear.calculate` said of one row (its `members` entry).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Member {
    pub module: Option<f64>,
    pub teeth: Option<f64>,
    pub pitch_diameter: Option<f64>,
    pub face_width: Option<f64>,
    pub meshes_with: Vec<usize>,
    pub shaft: Option<usize>,
    pub cluster: Option<String>,
    pub error: Option<String>,
}

/// `gear.calculate`'s `members`, by row.
fn members_of(r: &Value) -> BTreeMap<usize, Member> {
    let num = |m: &Value, k: &str| m.get(k).and_then(Value::as_f64);
    r.get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let index = m.get("index")?.as_u64()? as usize;
            let member = Member {
                module: num(m, "module"),
                teeth: num(m, "teeth"),
                pitch_diameter: num(m, "pitch_diameter"),
                face_width: num(m, "face_width"),
                meshes_with: m.get("meshes_with").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).map(|i| i as usize).collect(),
                shaft: m.get("shaft").and_then(Value::as_u64).map(|i| i as usize),
                cluster: m.get("cluster").filter(|c| !c.is_null()).map(|c| c.as_str().map(str::to_string).unwrap_or_else(|| c.to_string())),
                error: m.get("error").and_then(Value::as_str).map(str::to_string),
            };
            Some((index, member))
        })
        .collect()
}

/// What a row shows under its name: its worked-out size, what it meshes with, the shaft it
/// shares, or what is wrong with it.
fn member_line(m: &Member) -> (String, bool) {
    if let Some(e) = &m.error {
        return (e.clone(), true);
    }
    let mut parts = Vec::new();
    if let Some(v) = m.module {
        parts.push(format!("m {}", fmt(v)));
    }
    if let Some(v) = m.teeth {
        parts.push(format!("z {}", fmt(v)));
    }
    if let Some(v) = m.pitch_diameter {
        parts.push(format!("Ø{}", fmt(v)));
    }
    if let Some(v) = m.face_width {
        parts.push(format!("b {}", fmt(v)));
    }
    if !m.meshes_with.is_empty() {
        parts.push(format!("meshes {}", m.meshes_with.iter().map(|i| (i + 1).to_string()).collect::<Vec<_>>().join(", ")));
    }
    if let Some(s) = m.shaft {
        parts.push(format!("on shaft {}", s + 1));
    }
    if let Some(c) = &m.cluster {
        parts.push(format!("compound {c}"));
    }
    (parts.join(" · "), false)
}

/// A differential's styles and drives, as `gear.create`'s `differential` names them.
const DIFF_STYLES: [(&str, &str); 4] = [("open", "Open"), ("biased", "Biased"), ("torsen_b", "Torsen B"), ("torsen_a", "Torsen A")];
const DIFF_DRIVES: [(&str, &str); 5] = [("none", "None"), ("spiral_bevel", "Spiral Bevel"), ("bevel", "Bevel"), ("hypoid", "Hypoid"), ("helical", "Helical")];
/// Which of a differential's bodies are made (`differential.parts`).
const DIFF_PARTS: [(&str, &str); 7] =
    [("side_a", "Side Gear A"), ("side_b", "Side Gear B"), ("pinions", "Pinions"), ("pins", "Pins"), ("case", "Case"), ("ring", "Ring Gear"), ("drive_pinion", "Drive Pinion")];

/// Tooth styles: the sketch's circles decide the set.
fn parallel(k: GearKind) -> bool {
    matches!(k, GearKind::Spur | GearKind::Helical | GearKind::Herringbone)
}

/// One value row: its `gear.create` parameter (a dotted path into nested ones), its label, what
/// it is, its hint, and whether it is a whole count.
#[derive(Clone, Copy)]
struct Field {
    key: &'static str,
    label: &'static str,
    kind: ValueKind,
    hint: &'static str,
    whole: bool,
}

const fn f(key: &'static str, label: &'static str, kind: ValueKind, hint: &'static str) -> Field {
    Field { key, label, kind, hint, whole: false }
}

const fn n(key: &'static str, label: &'static str, hint: &'static str) -> Field {
    Field { key, label, kind: ValueKind::Unitless, hint, whole: true }
}

const MODULE: Field = f("module", "Module", ValueKind::Length, "auto: a standard module");
const FACE: Field = f("face_width", "Face Width", ValueKind::Length, "auto");
const HELIX: Field = f("helix_angle", "Helix Angle", ValueKind::Angle, "deg");
const SPIRAL: Field = f("spiral_angle", "Spiral Angle", ValueKind::Angle, "deg");
const SHAFT: Field = f("shaft_angle", "Shaft Angle", ValueKind::Angle, "90 deg");

/// The settings a type's drawing can't show (empty: worked out).
fn fields(k: GearKind, diff_drive: bool) -> Vec<Field> {
    match k {
        GearKind::Spur => vec![MODULE, FACE],
        GearKind::Helical | GearKind::Herringbone => vec![HELIX, MODULE, FACE],
        // The sketch gives the shaft angle; the sizes are settings.
        GearKind::Bevel => vec![MODULE, n("teeth", "Teeth", "gear 1"), n("teeth2", "Teeth 2", "gear 2"), FACE],
        GearKind::SpiralBevel => vec![MODULE, n("teeth", "Teeth", "gear 1"), n("teeth2", "Teeth 2", "gear 2"), SPIRAL, FACE],
        GearKind::BevelPlanetary => vec![MODULE, n("teeth", "Sun Teeth", "auto"), n("teeth2", "Planet Teeth", "auto"), n("planets", "Planets", "auto"), FACE],
        GearKind::FaceGear => vec![MODULE, n("teeth", "Pinion Teeth", "auto"), FACE],
        GearKind::Worm => vec![n("starts", "Starts", "1"), MODULE, FACE],
        GearKind::Rack | GearKind::Cycloidal => vec![MODULE, FACE],
        // The sketch gives the ring gear's Ø and the offset.
        GearKind::Hypoid => vec![SHAFT, n("teeth", "Pinion Teeth", "auto"), SPIRAL, FACE],
        GearKind::CrossedHelical => vec![SHAFT, f("helix_angle", "Helix Angle 1", ValueKind::Angle, "half the shaft angle"), MODULE, FACE],
        GearKind::Sprocket | GearKind::Pulley => vec![FACE],
        GearKind::Spline => vec![MODULE, n("teeth", "Teeth", "auto"), f("face_width", "Length", ValueKind::Length, "auto")],
        GearKind::Elliptical => vec![MODULE, n("teeth", "Teeth", "auto"), f("eccentricity", "Eccentricity", ValueKind::Unitless, "0.3"), FACE],
        GearKind::CycloidalDrive => vec![
            n("pins", "Pins", "auto"),
            f("pin_circle", "Pin Circle Ø", ValueKind::Length, "auto"),
            f("pin_diameter", "Pin Ø", ValueKind::Length, "auto"),
            f("eccentricity", "Eccentricity", ValueKind::Length, "auto"),
            f("face_width", "Thickness", ValueKind::Length, "auto"),
        ],
        GearKind::Differential => {
            let mut v = vec![
                f("differential.module", "Module", ValueKind::Length, "auto"),
                n("differential.side_teeth", "Side Gear Teeth", "auto"),
                n("differential.side_teeth2", "Side Gear B Teeth", "as A"),
                n("differential.pinion_teeth", "Pinion Teeth", "auto"),
                n("differential.pinions", "Pinions", "auto"),
                f("differential.axle_diameter", "Axle Ø", ValueKind::Length, "auto"),
                f("differential.face_width", "Face Width", ValueKind::Length, "auto"),
            ];
            if diff_drive {
                v.push(n("differential.ring_teeth", "Ring Gear Teeth", "auto"));
                v.push(n("differential.drive_pinion_teeth", "Drive Pinion Teeth", "auto"));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// The Advanced section's rows (every type but the differential, which has its own).
const ADVANCED: [Field; 10] = [
    f("pressure_angle", "Pressure Angle", ValueKind::Angle, "20 deg"),
    f("backlash", "Backlash", ValueKind::Length, "0"),
    f("profile_shift", "Profile Shift 1", ValueKind::Unitless, "0"),
    f("profile_shift2", "Profile Shift 2", ValueKind::Unitless, "0"),
    f("bore.diameter", "Bore Ø 1", ValueKind::Length, "none"),
    f("bore2.diameter", "Bore Ø 2", ValueKind::Length, "none"),
    f("hub_diameter", "Hub Ø", ValueKind::Length, "none"),
    f("hub_length", "Hub Length", ValueKind::Length, "none"),
    f("tip_round", "Tip Rounding", ValueKind::Unitless, "× module"),
    f("xy_compensation", "XY Compensation", ValueKind::Length, "0"),
];

/// The member colours of the preview (a member's index picks one, round the list).
const PALETTE: [egui::Color32; 6] = [
    egui::Color32::from_rgb(70, 140, 220),
    egui::Color32::from_rgb(230, 140, 50),
    egui::Color32::from_rgb(80, 170, 100),
    egui::Color32::from_rgb(190, 90, 170),
    egui::Color32::from_rgb(200, 180, 60),
    egui::Color32::from_rgb(120, 170, 190),
];

/// What the dialog asks the app to do once its frame is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Draw a new sketch for the set (`gear.sketch`) and open it for editing.
    NewSketch,
    /// Open the set's sketch for editing.
    EditSketch,
}

/// A sketch opened from the dialog: when its editing ends, the dialog comes back on it.
#[derive(Clone, Debug, PartialEq)]
pub struct AfterSketch {
    pub sketch: u64,
    /// The sketch has been seen open (its editing has started).
    pub opened: bool,
    pub dialog: Gear,
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
    /// The set's request when editing: what the dialog's rows are laid over (its shafts picked,
    /// its axes, every setting the rows don't show stay as they were).
    pub base: Map<String, Value>,
    /// The value rows, by parameter path, as typed (empty: worked out).
    pub values: BTreeMap<String, String>,
    pub assembly: bool,
    /// A keep-out body round the set (its bearings' seats, when bearings are picked).
    pub clearance: bool,
    /// Teeth from the equations (fitted curves otherwise).
    pub equations: bool,
    /// An index into `solidcraft_gear::hub::BEARINGS`: the size of the bearings picked.
    pub bearing: usize,
    /// The differential's style and drive (indices into [`DIFF_STYLES`], [`DIFF_DRIVES`]), the
    /// bodies it makes ([`DIFF_PARTS`]) and which pinions (numbers; empty: all).
    pub diff_style: usize,
    pub diff_drive: usize,
    pub diff_parts: [bool; 7],
    pub which_pinions: String,
    /// The picked rows, in pick order (kept in step with the Geometry input).
    pub picks: Vec<Pick>,
    /// The gears are sized by their teeth (each row's), the module worked out; otherwise by the
    /// module, the teeth worked out.
    pub by_teeth: bool,
    /// What the engine said of each row last time.
    pub members: BTreeMap<usize, Member>,
    /// The live preview builds the teeth (off: the abstract shape in the viewport only).
    pub teeth: bool,
    /// The last request worked out and what it came to (a summary, or the engine's refusal).
    pub calc: Option<(String, Result<Vec<String>, String>)>,
    /// The abstract shape of the last set worked out: polylines in the world, by member.
    pub shape: Vec<(usize, Vec<[f64; 3]>)>,
    /// Circles the engine's refusal is about, in the world, to draw in the warning colour.
    pub flagged: Vec<Vec<[f64; 3]>>,
    pub action: Option<Action>,
}

impl Gear {
    pub fn title(&self) -> &'static str {
        "GEARS"
    }

    pub fn previews(&self) -> bool {
        self.teeth
    }

    pub fn gear_kind(&self) -> GearKind {
        TYPES.get(self.kind).map_or(GearKind::Spur, |t| t.0)
    }

    pub fn value_mut(&mut self, key: &str) -> &mut String {
        self.values.entry(key.to_string()).or_default()
    }

    fn has_drive(&self) -> bool {
        self.diff_drive != 0
    }
}

fn new(sketch: Option<u64>) -> Gear {
    let mut values = BTreeMap::new();
    values.insert("helix_angle".to_string(), "20 deg".to_string());
    let bearing = solidcraft_gear::hub::BEARINGS.iter().position(|b| b.0 == "608").unwrap_or(0);
    Gear {
        kind: 0,
        sketch,
        group: None,
        base: Map::new(),
        values,
        assembly: false,
        clearance: false,
        equations: false,
        bearing,
        diff_style: 0,
        diff_drive: 1,
        diff_parts: [true; 7],
        which_pinions: String::new(),
        picks: Vec::new(),
        by_teeth: false,
        members: BTreeMap::new(),
        teeth: false,
        calc: None,
        shape: Vec::new(),
        flagged: Vec::new(),
        action: None,
    }
}

/// The one pick input: circles, lines, points and curves of sketches (3D ones too), model
/// edges, faces and axes, or a whole sketch picked in the timeline.
fn inputs() -> Vec<SelInput> {
    vec![SelInput::new("Geometry", CURVES | SKETCH_POINTS | EDGES | FACES | AXES | FEATURES, true)]
}

/// The dialog for `gear.create`.
pub fn start(app: &SolveApp, id: &str) -> Option<(Kind, Vec<SelInput>)> {
    if id != "gear.create" {
        return None;
    }
    // A set selected: the dialog on it, with what it was made with.
    if let Some(g) = solidcraft_engine::gear_selected_set(&app.session).map(|set| for_set(&set)) {
        return Some((Kind::Gear(g), inputs()));
    }
    let g = match &app.gear_after_sketch {
        Some(after) => Gear { sketch: Some(after.sketch), calc: None, shape: Vec::new(), flagged: Vec::new(), action: None, ..after.dialog.clone() },
        None => new(None),
    };
    Some((Kind::Gear(g), inputs()))
}

/// Whether a timeline folder is a gear set (Edit Gears opens the dialog on it).
pub fn is_set(app: &SolveApp, group: u64) -> bool {
    solidcraft_engine::gear_sets(&app.session).iter().any(|set| set.group == group)
}

/// Opens the dialog on the gear set of a timeline folder.
pub fn open_set(app: &mut SolveApp, group: u64) -> Result<(), String> {
    let sets = solidcraft_engine::gear_sets(&app.session);
    let set = sets.iter().find(|set| set.group == group).ok_or_else(|| "That folder is not a gear set".to_string())?;
    let g = for_set(set);
    app.start("gear.create");
    match app.dialog.as_mut().map(|d| &mut d.kind) {
        Some(Kind::Gear(k)) => {
            *k = g;
            Ok(())
        }
        _ => Err("The Gears dialog could not open (finish what is open first)".into()),
    }
}

/// The Type row's entry for a set's `type` (a pair, internal or planetary set is the tooth
/// style its helix gives).
fn type_index(kind: &str, helix: f64, herringbone: bool) -> usize {
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
        other => GearKind::ALL.iter().copied().find(|k| k.name() == other).unwrap_or(GearKind::Spur),
    };
    TYPES.iter().position(|t| t.0 == k).unwrap_or(0)
}

/// A number as a row shows it.
pub(crate) fn fmt(v: f64) -> String {
    let r = (v * 1e4).round() / 1e4;
    let s = format!("{r:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn shown(v: f64, kind: ValueKind) -> String {
    match kind {
        ValueKind::Length => format!("{} mm", fmt(v)),
        ValueKind::Angle => format!("{} deg", fmt(v)),
        _ => fmt(v),
    }
}

/// The value at a dotted path of a request.
fn at_path<'a>(r: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let mut v = r.get(parts.next()?)?;
    for p in parts {
        v = v.get(p)?;
    }
    Some(v)
}

/// Sets the value at a dotted path (making the objects on the way).
fn put_path(r: &mut Map<String, Value>, path: &str, value: Value) {
    match path.split_once('.') {
        None => {
            r.insert(path.to_string(), value);
        }
        Some((head, rest)) => {
            let entry = r.entry(head.to_string()).or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            if let Some(o) = entry.as_object_mut() {
                put_path(o, rest, value);
            }
        }
    }
}

/// The dialog on a set, with what it was made with.
pub fn for_set(set: &solidcraft_engine::schema::gear::GearSet) -> Gear {
    let mut r = set.request.as_object().cloned().unwrap_or_default();
    r.remove("ui");
    let helix = r.get("helix_angle").and_then(Value::as_f64).unwrap_or(0.0);
    let herringbone = r.get("herringbone").and_then(Value::as_bool) == Some(true);
    let mut g = new(set.sketch);
    g.kind = type_index(r.get("type").and_then(Value::as_str).unwrap_or("spur"), helix, herringbone);
    g.group = Some(set.group);
    let flag = |key: &str| r.get(key).and_then(Value::as_bool) == Some(true);
    g.assembly = flag("assembly");
    g.equations = r.get("tooth_curves").and_then(Value::as_str) == Some("equations");
    g.clearance = r.get("clearance_body").is_some_and(|c| !c.is_null());
    if let Some(d) = r.get("differential") {
        g.diff_style = DIFF_STYLES.iter().position(|s| d.get("style").and_then(Value::as_str) == Some(s.0)).unwrap_or(0);
        g.diff_drive = DIFF_DRIVES.iter().position(|s| d.get("drive").and_then(Value::as_str) == Some(s.0)).unwrap_or(1);
        for (i, (key, _)) in DIFF_PARTS.iter().enumerate() {
            g.diff_parts[i] = d.get("parts").and_then(|p| p.get(*key)).and_then(Value::as_bool).unwrap_or(true);
        }
        if let Some(w) = d.get("parts").and_then(|p| p.get("which_pinions")).and_then(Value::as_array) {
            g.which_pinions = w.iter().filter_map(Value::as_u64).map(|n| n.to_string()).collect::<Vec<_>>().join(", ");
        }
    }
    if let Some(b) = r.get("selection").and_then(Value::as_array).and_then(|s| s.iter().find_map(|i| i.get("bearing")?.as_str())) {
        g.bearing = solidcraft_gear::hub::BEARINGS.iter().position(|x| x.0 == b).unwrap_or(g.bearing);
    }
    for fd in fields(g.gear_kind(), g.has_drive()).into_iter().chain(ADVANCED) {
        if let Some(v) = at_path(&r, fd.key).and_then(Value::as_f64) {
            g.values.insert(fd.key.to_string(), shown(if fd.key == "helix_angle" { v.abs() } else { v }, fd.kind));
        }
    }
    g.base = r;
    g
}

/// The sketch the set is drawn in: the first picked sketch geometry's (or the sketch picked in
/// the timeline), else the one the dialog opened on.
fn sketch_of(app: &SolveApp, k: &Gear, inputs: &[SelInput]) -> Option<u64> {
    let items = inputs.first().map(|i| i.items.as_slice()).unwrap_or(&[]);
    // (Editing a set, a feature picked is one of its own, not the sketch it is drawn in.)
    items.iter().filter(|x| k.group.is_none() || !matches!(x, Sel::Feature { .. })).find_map(|x| geo(app, x).sketch()).or(k.sketch)
}

/// The picked rows as they stand: the Geometry input's items in order, each with the role and
/// teeth it was given (new ones their default), and what each is.
fn rows_now(app: &SolveApp, k: &Gear, inputs: &[SelInput]) -> Vec<(Pick, Geo)> {
    let items = inputs.first().map(|i| i.items.as_slice()).unwrap_or(&[]);
    let geos: Vec<Geo> = items.iter().map(|x| geo(app, x)).collect();
    items
        .iter()
        .zip(&geos)
        .map(|(sel, g)| {
            let pick = k.picks.iter().find(|p| p.sel == *sel).cloned().unwrap_or_else(|| Pick { sel: sel.clone(), role: default_role(k.gear_kind(), g, &geos), teeth: String::new() });
            (pick, g.clone())
        })
        .collect()
}

/// The value rows as `gear.create` numbers (counts whole), and what is wrong with one.
fn numbers(app: &SolveApp, k: &Gear, rows: &[Field], out: &mut Map<String, Value>) -> Result<(), String> {
    for fd in rows {
        let text = k.values.get(fd.key).map(String::as_str).unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let v = app.session.doc.eval(text, fd.kind).map_err(|e| format!("{}: {e}", solidcraft_ui_text::tr_label(fd.label)))?;
        put_path(out, fd.key, if fd.whole { json!(v.round().max(0.0) as u64) } else { json!(v) });
    }
    Ok(())
}

/// A selection item's locator (`gear.create`'s `selection`), or `None` for a pick it can't name.
fn locator(app: &SolveApp, x: &Sel) -> Option<Value> {
    let v = |p: solidcore_record::Vec3| json!([p.x, p.y, p.z]);
    match x {
        Sel::SketchCurve { id, sketch } => {
            let sk = sketch.or(app.session.active_sketch).or_else(|| crate::dialogs::picks_sketch(app, std::slice::from_ref(x), None))?;
            Some(json!({"sketch": sk, "curve": id}))
        }
        Sel::ExternalSketchCurve { sketch, id } => Some(json!({"sketch": sketch, "curve": id})),
        Sel::ExternalSketchPoint { sketch, id } => Some(json!({"sketch": sketch, "point": id})),
        Sel::SketchPoint { id } => Some(json!({"sketch": app.session.active_sketch?, "point": id})),
        Sel::Edge { point, .. } => Some(json!({"edge": v(*point)})),
        Sel::Face { point, .. } => Some(json!({"face": v(*point)})),
        Sel::Axis { name } => Some(json!({"axis": name})),
        _ => None,
    }
}

/// What the dialog's rows say, as `gear.create` parameters (without where the set goes).
fn settings(app: &SolveApp, k: &Gear) -> Result<Map<String, Value>, String> {
    let gk = k.gear_kind();
    let mut p = Map::new();
    p.insert("type".into(), json!(gk.name()));
    numbers(app, k, &fields(gk, k.has_drive()), &mut p)?;
    if gk == GearKind::Differential {
        let d = p.entry("differential").or_insert_with(|| json!({}));
        d["style"] = json!(DIFF_STYLES[k.diff_style.min(3)].0);
        d["drive"] = json!(DIFF_DRIVES[k.diff_drive.min(4)].0);
        // A plain case: no assembly windows, no flange and bolts.
        d["windows"] = json!(false);
        d["ring_bolts"] = json!(false);
        let mut parts = Map::new();
        for (i, (key, _)) in DIFF_PARTS.iter().enumerate() {
            parts.insert(key.to_string(), json!(k.diff_parts[i]));
        }
        let which: Vec<u64> = k.which_pinions.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|t| t.trim().parse::<u64>().ok()).filter(|n| *n > 0).collect();
        if !which.is_empty() {
            parts.insert("which_pinions".into(), json!(which));
        }
        d["parts"] = Value::Object(parts);
    } else {
        numbers(app, k, &ADVANCED, &mut p)?;
    }
    if parallel(gk) {
        if gk == GearKind::Spur {
            p.remove("helix_angle");
        } else if !p.contains_key("helix_angle") {
            p.insert("helix_angle".into(), json!(20.0));
        }
    }
    if k.assembly {
        p.insert("assembly".into(), json!(true));
    }
    if k.equations {
        p.insert("tooth_curves".into(), json!("equations"));
    }
    if k.clearance {
        p.insert("clearance_body".into(), json!({}));
    }
    Ok(p)
}

/// The keys the dialog's rows own: on an update, one left empty is taken away (back to auto).
fn owned(k: &Gear) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = fields(k.gear_kind(), true).iter().chain(ADVANCED.iter()).map(|f| f.key).filter(|key| !key.contains('.')).collect();
    keys.extend(["assembly", "tooth_curves", "clearance_body", "herringbone", "bore", "bore2", "differential"]);
    keys
}

/// The picked rows as `gear.create`'s `selection`: each its locator, role, and the module or
/// teeth sizing it (a whole sketch picked is the `sketch` itself, not a row).
fn selection_of(app: &SolveApp, k: &Gear, rows: &[(Pick, Geo)], module: Option<f64>, face: Option<f64>) -> Result<Vec<Value>, String> {
    let geos: Vec<Geo> = rows.iter().map(|(_, g)| g.clone()).collect();
    let mut out = Vec::new();
    for (i, (pick, g)) in rows.iter().enumerate() {
        if pick.role == UNUSED_ROLE || matches!(g, Geo::Sketch(_)) {
            continue;
        }
        let mut item = locator(app, &pick.sel).ok_or_else(|| format!("row {}: that pick can't be part of a set: pick a sketch circle, line or point, a straight or round edge, a cylinder face or an axis", i + 1))?;
        item["role"] = json!(ROLES[pick.role].0);
        item["index"] = json!(i);
        if pick.role == BEARING_ROLE {
            if let Some(why) = bearing_refused(g, &geos) {
                return Err(format!("row {}: {why}", i + 1));
            }
            if let Some(b) = solidcraft_gear::hub::BEARINGS.get(k.bearing) {
                item["bearing"] = json!(b.0);
            }
        } else if pick.role != SHAFT_ROLE {
            if k.by_teeth {
                let t = pick.teeth.trim();
                if !t.is_empty() {
                    let v = app.session.doc.eval(t, ValueKind::Unitless).map_err(|e| format!("row {}: teeth: {e}", i + 1))?;
                    item["teeth"] = json!(v.round().max(1.0) as u64);
                }
            } else if let Some(m) = module {
                item["module"] = json!(m);
            }
            if let Some(f) = face {
                item["face_width"] = json!(f);
            }
        }
        out.push(item);
    }
    Ok(out)
}

/// Where the set goes: the picked rows (`selection`, with the sketch they are in), or, for the
/// types read as a whole, the sketch as one drawing. Editing a set, nothing picked keeps where
/// it is.
fn placement(app: &SolveApp, k: &Gear, inputs: &[SelInput], p: &mut Map<String, Value>) -> Result<(), String> {
    let gk = k.gear_kind();
    if !sketch_rules::sketched(gk) {
        return Ok(());
    }
    let rows = rows_now(app, k, inputs);
    let sketch = sketch_of(app, k, inputs);
    if by_rows(gk) && rows.iter().any(|(_, g)| !matches!(g, Geo::Sketch(_))) {
        let module = p.get("module").and_then(Value::as_f64);
        let face = p.get("face_width").and_then(Value::as_f64);
        let sel = selection_of(app, k, &rows, (!k.by_teeth).then_some(module).flatten(), face)?;
        if k.by_teeth {
            // The teeth drive: the module is worked out.
            p.remove("module");
        }
        if !sel.iter().any(|i| !matches!(i["role"].as_str(), Some("shaft" | "bearing"))) {
            return Err("Give at least one row a gear role (Spur, Bevel, Worm...): the shafts and bearings go with gears".into());
        }
        // Each entry names its own sketch: a set may be drawn across several sketches.
        p.insert("selection".into(), Value::Array(sel));
        p.insert("sketch".into(), Value::Null);
        p.insert("picks".into(), Value::Null);
        return Ok(());
    }
    match sketch {
        Some(sketch) => {
            p.insert("sketch".into(), json!(sketch));
            p.insert("selection".into(), Value::Null);
            Ok(())
        }
        None if k.group.is_some() => Ok(()),
        None => Err("Pick the circles and lines the gears are drawn with (or the sketch), or shafts to put gears on, or press New Sketch".into()),
    }
}

/// The parameters that make the set as the dialog holds it (the whole of it, when editing: the
/// set's request with the rows laid over it).
fn request(app: &SolveApp, k: &Gear, inputs: &[SelInput]) -> Result<Value, String> {
    let mut p = settings(app, k)?;
    placement(app, k, inputs, &mut p)?;
    if k.group.is_none() {
        p.retain(|_, v| !v.is_null());
        return Ok(Value::Object(p));
    }
    let mut whole = k.base.clone();
    for key in owned(k) {
        whole.remove(key);
    }
    for (key, v) in p {
        if v.is_null() {
            whole.remove(&key);
        } else {
            whole.insert(key, v);
        }
    }
    Ok(Value::Object(whole))
}

pub fn commands(app: &SolveApp, k: &Gear, inputs: &[SelInput], _extra: &Map<String, Value>) -> Result<Vec<(String, Value)>, String> {
    let mut p = request(app, k, inputs)?;
    Ok(vec![match k.group {
        Some(group) => {
            // Everything the set is is in these parameters.
            p["group"] = json!(group);
            p["reset"] = json!(true);
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
            format!("{}: {}{unit}", f["label"].as_str().unwrap_or(""), fmt(f["value"].as_f64().unwrap_or(0.0)))
        })
        .collect();
    out.extend(r["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|w| format!("⚠ {w}")));
    out
}

fn value_rows(ui: &mut egui::Ui, k: &mut Gear, rows: &[Field]) -> bool {
    let mut enter = false;
    for fd in rows {
        enter |= hint_row(ui, fd.label, k.value_mut(fd.key), fd.hint);
    }
    enter
}

/// The picked rows: number, icon and name (as the input's own chips name them), role, and what
/// the engine worked out for it; hovering lights the geometry in the view; a row moves up or is
/// taken out with its buttons.
fn pick_rows(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], hover: &mut Option<(usize, Sel)>, labels: &[String]) -> bool {
    let mut enter = false;
    let rows = rows_now(app, k, inputs);
    let geos: Vec<Geo> = rows.iter().map(|(_, g)| g.clone()).collect();
    let (mut up, mut remove, mut lit) = (None, None, None);
    let t = solidcraft_theme::Tokens::get();
    for (i, (pick, g)) in rows.iter().enumerate() {
        let mut pick = pick.clone();
        let row = ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}", i + 1)).small().weak());
            let (r, _) = ui.allocate_exact_size(egui::vec2(t.space_24, t.space_24), egui::Sense::hover());
            paint_icon(ui.painter(), r, g.icon(), 1.0);
            ui.label(row_name(app, g, labels.get(i).map(String::as_str).unwrap_or("")));
            if !matches!(g, Geo::Sketch(_)) && by_rows(k.gear_kind()) {
                // A point not on a picked line can't be a bearing: the list leaves it out.
                let refused = bearing_refused(g, &geos);
                let roles: Vec<usize> = (0..ROLES.len()).filter(|r| *r != BEARING_ROLE || refused.is_none()).collect();
                let labels: Vec<&str> = roles.iter().map(|r| ROLES[*r].1).collect();
                let mut at = roles.iter().position(|r| *r == pick.role).unwrap_or(roles.len() - 1);
                combo(ui, &format!("gear_role_{i}"), &labels, &mut at);
                pick.role = roles[at.min(roles.len() - 1)];
                if let (Some(why), Geo::Point { .. }) = (refused, g) {
                    ui.label(egui::RichText::new("⊘").weak()).on_hover_text(why);
                }
                if k.by_teeth && pick.role < SHAFT_ROLE {
                    enter |= ui.add(egui::TextEdit::singleline(&mut pick.teeth).desired_width(48.0).hint_text("teeth")).lost_focus();
                }
            }
            if i > 0 && ui_kit::button(ui, "↑", ui_kit::Button::Normal).on_hover_text("Move up").clicked() {
                up = Some(i);
            }
            if ui_kit::button(ui, "×", ui_kit::Button::Normal).on_hover_text("Remove").clicked() {
                remove = Some(i);
            }
        });
        if row.response.hovered() {
            lit = Some((0, pick.sel.clone()));
        }
        if let Some(m) = k.members.get(&i) {
            let (text, bad) = member_line(m);
            if !text.is_empty() {
                let color = if bad { ui.visuals().warn_fg_color } else { ui.visuals().weak_text_color() };
                ui.label(egui::RichText::new(text).small().color(color));
            }
        }
        match k.picks.iter_mut().find(|p| p.sel == pick.sel) {
            Some(p) => *p = pick,
            None => k.picks.push(pick),
        }
    }
    if lit.is_some() || hover.as_ref().is_some_and(|(i, _)| *i == 0) {
        *hover = lit;
    }
    if let Some(items) = inputs.first_mut().map(|i| &mut i.items) {
        if let Some(i) = up.filter(|i| *i > 0 && *i < items.len()) {
            items.swap(i - 1, i);
        }
        if let Some(i) = remove.filter(|i| *i < items.len()) {
            let sel = items.remove(i);
            k.picks.retain(|p| p.sel != sel);
        }
    }
    enter
}

pub fn rows(app: &mut SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput], hover: &mut Option<(usize, Sel)>, labels: &[String]) -> bool {
    let mut enter = false;
    let names: Vec<&str> = TYPES.iter().map(|t| t.1).collect();
    choice_row(ui, "Type", "gear_type", &mut k.kind, &names);
    let gk = k.gear_kind();
    let sketched = sketch_rules::sketched(gk);
    if sketched {
        enter |= pick_rows(app, ui, k, inputs, hover, labels);
        if by_rows(gk) {
            let mut size = usize::from(k.by_teeth);
            choice_row(ui, "Size By", "gear_size_by", &mut size, &["Module", "Teeth"]);
            k.by_teeth = size == 1;
        }
    }
    if gk == GearKind::Differential {
        let styles: Vec<&str> = DIFF_STYLES.iter().map(|s| s.1).collect();
        choice_row(ui, "Style", "gear_diff_style", &mut k.diff_style, &styles);
        let drives: Vec<&str> = DIFF_DRIVES.iter().map(|s| s.1).collect();
        choice_row(ui, "Drive", "gear_diff_drive", &mut k.diff_drive, &drives);
    }
    let shown_fields: Vec<Field> = fields(gk, k.has_drive()).into_iter().filter(|f| !(k.by_teeth && by_rows(gk) && f.key == "module")).collect();
    enter |= value_rows(ui, k, &shown_fields);
    if gk == GearKind::Differential {
        let bevel = k.diff_style < 2;
        let drive = k.has_drive();
        ui_kit::section(ui, "gear_diff_parts", "Parts", true, |ui| {
            for (i, (key, label)) in DIFF_PARTS.iter().enumerate() {
                let shown = match *key {
                    "pins" => bevel,
                    "ring" | "drive_pinion" => drive,
                    _ => true,
                };
                if shown {
                    let label = if *key == "pinions" && !bevel { "Elements" } else { label };
                    ui.checkbox(&mut k.diff_parts[i], solidcraft_ui_text::tr_label(label));
                }
            }
            if k.diff_parts[2] {
                enter |= hint_row(ui, if bevel { "Which Pinions" } else { "Which Elements" }, &mut k.which_pinions, "all; or 1, 3");
            }
        });
    }
    if k.picks.iter().any(|p| p.role == BEARING_ROLE && inputs.first().is_some_and(|i| i.items.contains(&p.sel))) {
        let labels: Vec<String> = solidcraft_gear::hub::BEARINGS.iter().map(|b| format!("{} · {}×{}×{}", b.0, fmt(b.1), fmt(b.2), fmt(b.3))).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        form_row(ui, "Bearing", |ui| combo(ui, "gear_bearing", &labels, &mut k.bearing));
    }
    ui.checkbox(&mut k.assembly, solidcraft_ui_text::tr_label("Assembly")).on_hover_text("Each gear a component, turning on a joint, linked to its mates");
    ui.checkbox(&mut k.clearance, solidcraft_ui_text::tr_label("Clearance Body"))
        .on_hover_text("A keep-out body round the set, with the bearings' seats when bearings are picked, to cut a housing with");
    ui.checkbox(&mut k.teeth, solidcraft_ui_text::tr_label("Preview Teeth"))
        .on_hover_text("Build the teeth for the live preview (off: the set's shafts and pitch surfaces only, drawn at once)");
    if sketched {
        ui.horizontal_wrapped(|ui| {
            if ui_kit::button(ui, solidcraft_ui_text::tr_label("New Sketch"), ui_kit::Button::Normal)
                .on_hover_text("Draw the set in a new sketch, from the type's starting picture with its constraints and dimensions")
                .clicked()
            {
                k.action = Some(Action::NewSketch);
            }
            if sketch_of(app, k, &*inputs).is_some()
                && ui_kit::button(ui, solidcraft_ui_text::tr_label("Edit Sketch"), ui_kit::Button::Normal)
                    .on_hover_text("Open the set's sketch to change its circles, lines, dimensions and constraints")
                    .clicked()
            {
                k.action = Some(Action::EditSketch);
            }
        });
    }
    if gk != GearKind::Differential {
        ui_kit::section(ui, "gear_advanced", "Advanced", false, |ui| {
            enter |= value_rows(ui, k, &ADVANCED);
            ui.checkbox(&mut k.equations, solidcraft_ui_text::tr_label("Equation Teeth")).on_hover_text("Teeth drawn from their equations instead of fitted curves");
        });
    }
    // What the engine reads from the picks: worked out once per change.
    match request(app, k, &*inputs) {
        Ok(p) => {
            let key = format!("{}@{}", p, app.session.revision);
            if k.calc.as_ref().is_none_or(|(c, _)| *c != key) {
                let r = app.session.execute("gear.calculate", &p);
                k.shape = r.as_ref().ok().and_then(|r| r.get("params")).map(shape_of).unwrap_or_default();
                k.members = r.as_ref().map(members_of).unwrap_or_default();
                k.flagged = flagged(app, k, &*inputs, r.is_err());
                k.calc = Some((key, r.map(|r| summary(&r)).map_err(|e| e.to_string())));
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
            k.shape.clear();
            k.flagged.clear();
            ui.label(egui::RichText::new(e).small().weak());
        }
    }
    enter
}

/// The circles to draw in the warning colour, in the world: the rows the engine says something
/// is wrong with, and (when it refused the set) the circles its reading refuses (one meshing
/// with nothing, two joined by a centre line that don't touch).
fn flagged(app: &SolveApp, k: &Gear, inputs: &[SelInput], refused: bool) -> Vec<Vec<[f64; 3]>> {
    let Some(gs) = sketch_of(app, k, inputs).and_then(|id| solidcraft_engine::gear_sketch(&app.session, id).ok()) else { return Vec::new() };
    let rows = rows_now(app, k, inputs);
    let mut ids: Vec<String> = rows
        .iter()
        .enumerate()
        .filter(|(i, _)| k.members.get(i).is_some_and(|m| m.error.is_some()))
        .filter_map(|(_, (_, g))| if let Geo::Circle { id, .. } = g { Some(id.clone()) } else { None })
        .collect();
    if refused && let Some((_, circles)) = gs.mesh_problem() {
        ids.extend(circles.iter().filter_map(|i| gs.circles.get(*i)).map(|(id, _)| id.clone()));
    }
    let fr = gs.frame;
    let y = cross(fr.z, fr.x);
    gs.circles
        .iter()
        .filter(|(id, _)| ids.contains(id))
        .map(|(_, c)| {
            (0..=48)
                .map(|i| {
                    let a = std::f64::consts::TAU * f64::from(i) / 48.0;
                    let (u, v) = (c.c.x + c.r * a.cos(), c.c.y + c.r * a.sin());
                    [0, 1, 2].map(|k| fr.origin[k] + u * fr.x[k] + v * y[k])
                })
                .collect()
        })
        .collect()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The set's abstract shape for parameters `gear.calculate` gave: each member's shaft and pitch
/// surface (rings across it at its ends and four lines along it), in the world, from the gear
/// crate's schematic.
fn shape_of(params: &Value) -> Vec<(usize, Vec<[f64; 3]>)> {
    let Ok(gp) = serde_json::from_value::<solidcraft_engine::schema::gear::GearParams>(params.clone()) else { return Vec::new() };
    let Ok(spec) = solidcraft_engine::gear_spec_of(&gp) else { return Vec::new() };
    let sch = solidcraft_gear::simple::schematic(&spec);
    let mut out = Vec::new();
    let ring = |m: &solidcraft_gear::simple::Member, r: f64, z: f64| -> Vec<[f64; 3]> {
        (0..=48)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 48.0;
                m.world([r * a.cos(), r * a.sin(), z])
            })
            .collect()
    };
    for m in &sch.members {
        // The pitch surface as (z, r) stations along the shaft.
        let stations: Vec<[f64; 2]> = m
            .parts
            .iter()
            .filter(|p| p.role == Role::Pitch)
            .flat_map(|p| match &p.shape {
                Shape::Cylinder { r, z0, z1 } => vec![[*z0, *r], [*z1, *r]],
                Shape::Cone { r0, z0, r1, z1, .. } => vec![[*z0, *r0], [*z1, *r1]],
                Shape::Revolved { profile } => profile.iter().map(|q| [q[1], q[0]]).collect(),
                _ => Vec::new(),
            })
            .collect();
        let (lo, hi) = stations.iter().fold((f64::MAX, f64::MIN), |(a, b), s| (a.min(s[0]), b.max(s[0])));
        let reach = stations.iter().map(|s| s[1]).fold(m.pitch_diameter / 2.0, f64::max).max(1.0);
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (0.0, 0.0) };
        out.push((m.color, vec![m.world([0.0, 0.0, lo - 0.4 * reach]), m.world([0.0, 0.0, hi + 0.4 * reach])]));
        if let (Some(first), Some(last)) = (stations.first(), stations.last()) {
            out.push((m.color, ring(m, first[1], first[0])));
            out.push((m.color, ring(m, last[1], last[0])));
            for q in 0..4 {
                let a = std::f64::consts::FRAC_PI_2 * f64::from(q);
                out.push((m.color, stations.iter().map(|[z, r]| m.world([r * a.cos(), r * a.sin(), *z])).collect()));
            }
        }
        // A rim or body round it: its outer rings.
        for p in &m.parts {
            if let (Role::Body, Shape::Tube { r_out, z0, z1, .. }) = (p.role, &p.shape) {
                out.push((m.color, ring(m, *r_out, *z0)));
                out.push((m.color, ring(m, *r_out, *z1)));
            }
        }
    }
    out
}

/// The abstract shape drawn over the viewport while the dialog is open, and the circles a
/// refusal is about in the warning colour.
pub fn overlay(painter: &egui::Painter, proj: &crate::viewport::Proj, k: &Gear) {
    let to = |p: &[f64; 3]| proj.to_screen(solidcore_record::Vec3::new(p[0], p[1], p[2]));
    for (member, line) in &k.shape {
        let pts: Vec<egui::Pos2> = line.iter().filter_map(to).collect();
        if pts.len() == line.len() && pts.len() >= 2 {
            painter.add(egui::Shape::line(pts, egui::Stroke::new(if line.len() == 2 { 1.0 } else { 1.8 }, PALETTE[member % PALETTE.len()])));
        }
    }
    let warn = egui::Color32::from_rgb(230, 70, 60);
    for line in &k.flagged {
        let pts: Vec<egui::Pos2> = line.iter().filter_map(to).collect();
        if pts.len() == line.len() && pts.len() >= 2 {
            painter.add(egui::Shape::line(pts, egui::Stroke::new(3.0, warn)));
        }
    }
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
        let after = |sketch: u64| AfterSketch { sketch, opened: false, dialog: g.clone() };
        match action {
            Action::NewSketch => {
                let mut sizes = Map::new();
                let _ = numbers(app, &g, &fields(g.gear_kind(), g.has_drive()), &mut sizes);
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
