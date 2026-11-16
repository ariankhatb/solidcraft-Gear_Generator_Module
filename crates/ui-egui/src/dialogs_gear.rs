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
//! The preview is the set's outline drawn over the viewport, never its teeth (they are solved
//! on OK only): each gear's pitch, tip and root circles at both faces with its face width
//! between them, a bevel's cones, the shafts as cylinders, in its colour (the gear crate's
//! schematic of what `gear.calculate` worked out), and each mesh's contact highlighted. What the
//! engine refuses is said under the form in its own words, and a circle it names is drawn in
//! the warning colour in the view. A circle's row is Auto until given a type: the engine says
//! what it came to ("Auto → Bevel"). Shafts are made for every gear axis (Generate Shafts; each
//! one a row of its own with its diameter, on or off; a picked line sets its length), and a
//! shaft with its gears is one body. Edit Gears (a set's timeline folder) reopens the dialog on the
//! set; OK then updates it in place (`gear.update`), keeping its components and joints.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::plan::Kind as GearKind;
use solidcraft_gear::simple::{Role, Shape};
use solidcraft_gear::sketch_rules;

use crate::SolveApp;
use crate::dialogs::{Kind, OP_LABELS, OPS, combo, form_row};
use crate::ui_kit::icons::paint_icon;
use crate::selection::{AXES, BODIES, CURVES, EDGES, FACES, FEATURES, SKETCH_POINTS, SelInput};
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
const ROLES: [(&str, &str); 14] = [
    ("auto", "Auto"),
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
const AUTO_ROLE: usize = 0;
const SHAFT_ROLE: usize = 11;
const BEARING_ROLE: usize = 12;
const UNUSED_ROLE: usize = 13;

/// Rows that take a helix angle (parallel-axis teeth, or Auto that may come to them) and rows
/// that take a spiral angle (bevels); both may be V (herringbone, or a chevron bevel).
fn helix_role(role: usize) -> bool {
    matches!(ROLES[role].0, "auto" | "spur" | "helical" | "herringbone" | "internal" | "rack")
}
fn spiral_role(role: usize) -> bool {
    matches!(ROLES[role].0, "bevel" | "spiral_bevel")
}

/// The types whose picks are rows with roles (the rest take the sketch as one drawing).
fn by_rows(k: GearKind) -> bool {
    matches!(k, GearKind::Spur | GearKind::Helical | GearKind::Herringbone | GearKind::Bevel | GearKind::SpiralBevel | GearKind::Worm | GearKind::Rack | GearKind::FaceGear)
}

/// The role a gear of type `k` has by default: Auto for the tooth styles (the engine works out
/// what each circle is from the drawing), the type itself otherwise.
fn role_of(k: GearKind) -> usize {
    let name = match k {
        GearKind::Bevel => "bevel",
        GearKind::SpiralBevel => "spiral_bevel",
        GearKind::Worm => "worm",
        GearKind::Rack => "rack",
        GearKind::FaceGear => "face",
        _ => "auto",
    };
    ROLES.iter().position(|r| r.0 == name).unwrap_or(AUTO_ROLE)
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

/// The geometry a projected (linked) copy stands for: a curve or point of another sketch,
/// followed through copies of copies; `None` for one drawn here.
fn source_of(app: &SolveApp, sel: &Sel) -> Option<Sel> {
    use solidcraft_engine::sketch::LinkSource;
    let state = crate::open_meshing::session_world(&app.session);
    let mut at = sel.clone();
    let mut found = None;
    for _ in 0..4 {
        let (sketch, id, curve) = match &at {
            Sel::SketchCurve { id, sketch } => (sketch.or(app.session.active_sketch)?, id.clone(), true),
            Sel::ExternalSketchCurve { sketch, id } => (*sketch, id.clone(), true),
            Sel::ExternalSketchPoint { sketch, id } => (*sketch, id.clone(), false),
            Sel::SketchPoint { id } => (app.session.active_sketch?, id.clone(), false),
            _ => break,
        };
        let Some(ss) = state.sketch(sketch) else { break };
        let link = if curve { ss.sketch.curves.iter().find(|c| c.id == id).and_then(|c| c.link.clone()) } else { ss.sketch.points.iter().find(|p| p.id == id).and_then(|p| p.link.clone()) };
        let Some(link) = link.and_then(|l| ss.sketch.link(&l)) else { break };
        let next = match &link.source {
            LinkSource::SketchCurve { sketch, curve } => Sel::SketchCurve { id: curve.clone(), sketch: Some(*sketch) },
            LinkSource::SketchPoint { sketch, point } => Sel::ExternalSketchPoint { sketch: *sketch, id: point.clone() },
            _ => break,
        };
        found = Some(next.clone());
        at = next;
    }
    found
}

/// What `sel` is (a projected copy: what it was projected from).
fn geo(app: &SolveApp, sel: &Sel) -> Geo {
    if let Some(src) = source_of(app, sel) {
        return geo_here(app, &src);
    }
    geo_here(app, sel)
}

fn geo_here(app: &SolveApp, sel: &Sel) -> Geo {
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
    /// Helix angle (parallel-axis rows) or spiral angle (bevels), as typed; empty: straight.
    pub angle: String,
    /// V teeth: herringbone, or a chevron bevel.
    pub v: bool,
}

/// A row's name: the pick as the input's chips name it, with the sketch it is in (a set may be
/// drawn across several sketches on different planes).
fn row_name(app: &SolveApp, g: &Geo, label: &str) -> String {
    let sketch = match g {
        Geo::Sketch(_) => None,
        other => other.sketch(),
    };
    let named = match sketch.and_then(|id| app.session.doc.feature(id)) {
        Some(f) if !label.contains(&f.name) => format!("{label} · {}", f.name),
        _ => label.to_string(),
    };
    match g {
        Geo::Circle { id, .. } | Geo::Line { id, .. } | Geo::Point { id, .. } | Geo::Curve { id, .. } if !label.starts_with(id.as_str()) => {
            format!("{named} (projected from {id})")
        }
        _ => named,
    }
}

/// What `gear.calculate` said of one row (its `members` entry).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Member {
    /// The role it came to, and whether the engine worked it out (an Auto row).
    pub role: Option<String>,
    pub inferred: bool,
    /// Its hand as worked out (a helical or spiral mate is the other hand): "LH" or "RH".
    pub hand: Option<String>,
    /// Its shaft angle to its mate, for a bevel worked out (degrees).
    pub angle: Option<f64>,
    pub module: Option<f64>,
    pub teeth: Option<f64>,
    pub pitch_diameter: Option<f64>,
    pub face_width: Option<f64>,
    pub meshes_with: Vec<usize>,
    pub shaft: Option<usize>,
    pub cluster: Option<String>,
    pub error: Option<String>,
}

/// A shaft the engine makes on a gear axis (`gear.calculate`'s `shafts`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShaftOut {
    pub index: usize,
    pub name: Option<String>,
    pub diameter: Option<f64>,
    pub length: Option<f64>,
    /// The rows of the gears on it.
    pub gears: Vec<usize>,
    /// The row of the line that sets its length, when one was picked.
    pub line: Option<usize>,
}

/// A generated shaft as the dialog holds it: on or off, its diameter as typed (empty: auto).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaftRow {
    pub enabled: bool,
    pub diameter: String,
}

fn shafts_of(r: &Value) -> Vec<ShaftOut> {
    r.get("shafts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some(ShaftOut {
                index: v.get("index")?.as_u64()? as usize,
                name: v.get("name").and_then(Value::as_str).map(str::to_string),
                diameter: v.get("diameter").and_then(Value::as_f64),
                length: v.get("length").and_then(Value::as_f64),
                gears: v.get("gears").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).map(|i| i as usize).collect(),
                line: v.get("line").and_then(Value::as_u64).map(|i| i as usize),
            })
        })
        .collect()
}

/// A generated shaft's row text: "Shaft 2: input, Ø8, 3 gears".
fn shaft_line(s: &ShaftOut) -> String {
    let mut t = format!("Shaft {}", s.index + 1);
    if let Some(n) = &s.name {
        t.push_str(&format!(": {n}"));
    }
    t.push_str(&match s.diameter {
        Some(d) => format!(", Ø{}", fmt(d)),
        None => ", Ø auto".into(),
    });
    let n = s.gears.len();
    t.push_str(&format!(", {n} gear{}", if n == 1 { "" } else { "s" }));
    if let Some(l) = s.length {
        t.push_str(&format!(", {} long", fmt(l)));
    }
    if let Some(r) = s.line {
        t.push_str(&format!(" (row {})", r + 1));
    }
    t
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
                role: m.get("role").and_then(Value::as_str).map(str::to_string),
                inferred: m.get("inferred").and_then(Value::as_bool) == Some(true),
                hand: m.get("hand").and_then(Value::as_str).map(str::to_string),
                angle: num(m, "shaft_angle"),
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
    if m.inferred
        && let Some(role) = &m.role
    {
        let label = ROLES.iter().find(|r| r.0 == role).map_or(role.as_str(), |r| r.1);
        parts.push(match m.angle {
            Some(a) => format!("Auto → {label} {}°", fmt(a)),
            None => format!("Auto → {label}"),
        });
    }
    if let Some(h) = &m.hand {
        parts.push(h.clone());
    }
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
        parts.push(format!("one body with shaft {}", s + 1));
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
    /// Shafts made on every gear axis, and each generated one's own setting (by its index).
    pub generate_shafts: bool,
    pub shaft_rows: BTreeMap<usize, ShaftRow>,
    /// The shafts the engine worked out last time.
    pub shafts: Vec<ShaftOut>,
    /// Each mesh's contact, in the world: a line or (two equal ends) a point.
    pub contacts: Vec<[[f64; 3]; 2]>,
    /// What the set does to the Targets bodies (an index into the dialogs' `OPS`: a new body,
    /// joined to them, cut from them, intersected), and whether its keep-out is cut from them
    /// (a housing).
    pub operation: usize,
    pub cut_keep_out: bool,
    /// The row of the gear that drives the set (the rest follow it).
    pub driver: usize,
    /// The last request worked out and what it came to (a summary, or the engine's refusal).
    pub calc: Option<(String, Result<Vec<String>, String>)>,
    /// The abstract shape of the last set worked out: polylines in the world, by member.
    pub shape: Vec<(usize, Vec<[f64; 3]>)>,
    /// Circles the engine's refusal is about, in the world, to draw in the warning colour.
    pub flagged: Vec<Vec<[f64; 3]>>,
    pub action: Option<Action>,
    /// Gear items (each one gear, compound gear or pair, from the geometry picked for it), the
    /// one picks go to, the type the next one is made as, and the graph linking them.
    pub items: Vec<crate::dialogs_gear_graph::GearItem>,
    pub active: Option<usize>,
    pub new_kind: usize,
    pub graph: crate::dialogs_gear_graph::Graph,
    /// What the engine says of the links, and the links the drawing suggests.
    pub link_errors: Vec<String>,
    pub suggested: Vec<(usize, usize, String)>,
}

impl Gear {
    pub fn title(&self) -> &'static str {
        "GEARS"
    }

    /// No kernel preview: the teeth are solved on OK only (the viewport shows the outline).
    pub fn previews(&self) -> bool {
        false
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
        generate_shafts: true,
        shaft_rows: BTreeMap::new(),
        shafts: Vec::new(),
        contacts: Vec::new(),
        operation: 0,
        cut_keep_out: false,
        driver: 0,
        calc: None,
        shape: Vec::new(),
        flagged: Vec::new(),
        action: None,
        items: Vec::new(),
        active: None,
        new_kind: 0,
        graph: Default::default(),
        link_errors: Vec::new(),
        suggested: Vec::new(),
    }
}

/// The pick input (circles, lines, points and curves of sketches, 3D ones too, model edges,
/// faces and axes, or a whole sketch picked in the timeline) and the bodies the set's operation
/// and keep-out act on.
fn inputs() -> Vec<SelInput> {
    vec![SelInput::new("Geometry", CURVES | SKETCH_POINTS | EDGES | FACES | AXES | FEATURES, true), SelInput::new("Targets", BODIES, true)]
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
    solidcraft_engine::gear_sets(&app.session).iter().any(|set| owns(set, group))
}

/// Whether `id` is the set's timeline folder or its Gears feature (the two ids differ).
fn owns(set: &solidcraft_engine::schema::gear::GearSet, id: u64) -> bool {
    set.group == id || set.feature == Some(id)
}

/// Opens the dialog on the gear set of a timeline folder.
pub fn open_set(app: &mut SolveApp, group: u64) -> Result<(), String> {
    let sets = solidcraft_engine::gear_sets(&app.session);
    let set = sets.iter().find(|set| owns(set, group)).ok_or_else(|| "That folder is not a gear set".to_string())?;
    let g = for_set(set);
    let targets: Vec<Sel> = set.request.get("targets").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(|name| Sel::Body { name: name.to_string() }).collect();
    app.start("gear.create");
    match app.dialog.as_mut() {
        Some(d) if matches!(d.kind, Kind::Gear(_)) => {
            d.kind = Kind::Gear(g);
            if let Some(input) = d.inputs.get_mut(1) {
                input.items = targets;
            }
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
    g.operation = r.get("operation").and_then(Value::as_str).and_then(|o| OPS.iter().position(|x| *x == o)).unwrap_or(0);
    g.cut_keep_out = r.get("keep_out_cuts").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
    g.driver = r.get("assembly_driver").and_then(Value::as_u64).map_or(0, |d| (d as usize).saturating_sub(1));
    g.generate_shafts = r.get("shafts").and_then(|v| v.get("generate")).and_then(Value::as_bool).unwrap_or(true);
    for o in r.get("shafts").and_then(|v| v.get("overrides")).and_then(Value::as_array).into_iter().flatten() {
        if let Some(i) = o.get("index").and_then(Value::as_u64) {
            let diameter = o.get("diameter").and_then(Value::as_f64).map(|d| shown(d, ValueKind::Length)).unwrap_or_default();
            g.shaft_rows.insert(i as usize, ShaftRow { enabled: o.get("enabled").and_then(Value::as_bool) != Some(false), diameter });
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
            let pick = k.picks.iter().find(|p| p.sel == *sel).cloned().unwrap_or_else(|| Pick {
                sel: sel.clone(),
                role: default_role(k.gear_kind(), g, &geos),
                teeth: String::new(),
                angle: String::new(),
                v: false,
            });
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
        // (An angle evaluates in radians; gear.create takes degrees.)
        let v = if matches!(fd.kind, ValueKind::Angle) { v.to_degrees() } else { v };
        put_path(out, fd.key, if fd.whole { json!(v.round().max(0.0) as u64) } else { json!(v) });
    }
    Ok(())
}

/// A selection item's locator (`gear.create`'s `selection`), or `None` for a pick it can't name
/// (a projected copy: its source's).
/// `sel` as a `selection` locator (for the items' components).
pub(crate) fn locate(app: &SolveApp, sel: &Sel) -> Option<Value> {
    locator(app, sel)
}

/// Whether `sel` is a circle (true), a line or other curve (false), or neither.
pub(crate) fn geo_kind(app: &SolveApp, sel: &Sel) -> Option<bool> {
    match geo(app, sel) {
        Geo::Circle { .. } => Some(true),
        Geo::Line { .. } | Geo::Curve { .. } | Geo::Model => Some(false),
        _ => None,
    }
}

fn locator(app: &SolveApp, x: &Sel) -> Option<Value> {
    let src = source_of(app, x);
    let x = src.as_ref().unwrap_or(x);
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
    keys.extend(["assembly", "tooth_curves", "clearance_body", "herringbone", "bore", "bore2", "differential", "shafts", "operation", "targets", "keep_out_cuts", "assembly_driver"]);
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
            let a = pick.angle.trim();
            if !a.is_empty() && (helix_role(pick.role) || spiral_role(pick.role)) {
                let v = app.session.doc.eval(a, ValueKind::Angle).map_err(|e| format!("row {}: angle: {e}", i + 1))?;
                item[if spiral_role(pick.role) { "spiral_angle" } else { "helix_angle" }] = json!(v.to_degrees());
            }
            if pick.v && (helix_role(pick.role) || spiral_role(pick.role)) {
                item["herringbone"] = json!(true);
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
        let mut overrides = Vec::new();
        for (i, row) in &k.shaft_rows {
            let mut o = json!({"index": i, "enabled": row.enabled});
            if !row.diameter.trim().is_empty() {
                let d = app.session.doc.eval(row.diameter.trim(), ValueKind::Length).map_err(|e| format!("shaft {}: diameter: {e}", i + 1))?;
                o["diameter"] = json!(d);
            }
            overrides.push(o);
        }
        p.insert("shafts".into(), json!({"generate": k.generate_shafts, "overrides": overrides}));
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
    if k.items.is_empty() {
        placement(app, k, inputs, &mut p)?;
    } else {
        // Gear items and their links: each its own components.
        p.extend(crate::dialogs_gear_graph::items_request(app, &k.items, &k.graph)?);
        for key in ["sketch", "selection", "picks"] {
            p.insert(key.into(), Value::Null);
        }
    }
    let targets: Vec<String> = inputs.get(1).map(|i| i.items.as_slice()).unwrap_or(&[]).iter().filter_map(|x| match x {
        Sel::Body { name } => Some(name.clone()),
        _ => None,
    }).collect();
    p.insert("operation".into(), json!(OPS.get(k.operation).copied().unwrap_or("new")));
    if targets.is_empty() {
        if k.operation != 0 {
            return Err("Join, Cut and Intersect act on the Targets: pick the bodies".into());
        }
        if k.cut_keep_out {
            return Err("Cut Keep-Out cuts the set's keep-out from the Targets: pick the housing bodies".into());
        }
    } else {
        p.insert("targets".into(), json!(targets));
        if k.cut_keep_out {
            p.insert("keep_out_cuts".into(), json!(targets));
        }
    }
    // The gear turned (the rest follow it), by its place among the gears (1-based).
    if by_rows(k.gear_kind()) && k.assembly && k.driver > 0 {
        p.insert("assembly_driver".into(), json!(k.driver + 1));
    }
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

/// Generate Shafts, and a row per shaft the engine made on a gear axis: what it is, on or off,
/// its diameter (empty: auto). A shaft and its gears are one body.
fn shaft_rows(ui: &mut egui::Ui, k: &mut Gear) -> bool {
    let mut enter = false;
    ui.checkbox(&mut k.generate_shafts, solidcraft_ui_text::tr_label("Generate Shafts"))
        .on_hover_text("A shaft on every gear axis, one body with its gears (a picked line sets a shaft's length)");
    if !k.generate_shafts {
        return enter;
    }
    for s in k.shafts.clone() {
        let row = k.shaft_rows.entry(s.index).or_insert_with(|| ShaftRow { enabled: true, diameter: String::new() });
        ui.horizontal(|ui| {
            ui.checkbox(&mut row.enabled, "");
            ui.label(egui::RichText::new(shaft_line(&s)).small());
        });
        if row.enabled {
            enter |= hint_row(ui, "  Diameter", &mut row.diameter, "auto");
        }
    }
    enter
}

fn value_rows(ui: &mut egui::Ui, k: &mut Gear, rows: &[Field]) -> bool {
    let mut enter = false;
    for fd in rows {
        enter |= hint_row(ui, fd.label, k.value_mut(fd.key), fd.hint);
    }
    enter
}

/// The gear items: a type and Create New make one, the picks then go to it (the active item);
/// each row lists what its type still needs, and is made active or removed with its buttons.
fn item_rows(app: &SolveApp, ui: &mut egui::Ui, k: &mut Gear, inputs: &mut [SelInput]) {
    use crate::dialogs_gear_graph::{GearItem, ITEM_TYPES};
    let names: Vec<&str> = ITEM_TYPES.iter().map(|t| t.1).collect();
    ui.horizontal(|ui| {
        combo(ui, "gear_new_item", &names, &mut k.new_kind);
        if ui_kit::button(ui, solidcraft_ui_text::tr_label("Create New"), ui_kit::Button::Normal).on_hover_text("A new gear item of this type: then pick what defines it").clicked() {
            k.items.push(GearItem { kind: k.new_kind, picks: Vec::new() });
            k.active = Some(k.items.len() - 1);
            if let Some(input) = inputs.first_mut() {
                input.items.clear();
            }
        }
    });
    // The picks made go to the active item.
    if let (Some(a), Some(input)) = (k.active, inputs.first()) {
        let taken: Vec<Sel> = k.items.iter().enumerate().filter(|(i, _)| *i != a).flat_map(|(_, it)| it.picks.clone()).collect();
        if let Some(item) = k.items.get_mut(a) {
            item.picks = input.items.iter().filter(|s| !taken.contains(s)).cloned().collect();
        }
    }
    let (mut make_active, mut remove) = (None, None);
    for (i, it) in k.items.iter().enumerate() {
        let active = k.active == Some(i);
        ui.horizontal_wrapped(|ui| {
            let label = format!("Item {} · {}", i + 1, ITEM_TYPES.get(it.kind).map_or("", |t| t.1));
            if ui.selectable_label(active, egui::RichText::new(label).strong()).on_hover_text("Make it the item picks go to").clicked() {
                make_active = Some(i);
            }
            for (what, done) in it.checklist(app) {
                let mark = if done { "✓" } else { "○" };
                ui.label(egui::RichText::new(format!("{mark} {what}")).small().color(if done { ui.visuals().text_color() } else { ui.visuals().warn_fg_color }));
            }
            if ui_kit::button(ui, "×", ui_kit::Button::Normal).on_hover_text("Remove the item").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = make_active {
        k.active = Some(i);
        if let Some(input) = inputs.first_mut() {
            input.items = k.items[i].picks.clone();
        }
    }
    if let Some(i) = remove {
        k.items.remove(i);
        k.graph.remove_item(i);
        k.active = k.active.filter(|a| *a != i).map(|a| if a > i { a - 1 } else { a });
        if let Some(input) = inputs.first_mut() {
            input.items = k.active.map(|a| k.items[a].picks.clone()).unwrap_or_default();
        }
    }
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
    // Ten and more rows scroll in the panel, as the native picked-items list does.
    egui::ScrollArea::vertical().id_salt("gear_pick_rows").max_height(t.control_height * 10.5).auto_shrink([false, true]).show(ui, |ui| {
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
                if helix_role(pick.role) || spiral_role(pick.role) {
                    let hint = if spiral_role(pick.role) { "spiral°" } else { "helix°" };
                    enter |= ui.add(egui::TextEdit::singleline(&mut pick.angle).desired_width(48.0).hint_text(hint)).on_hover_text("0: straight teeth").lost_focus();
                    ui.checkbox(&mut pick.v, "V").on_hover_text(if spiral_role(pick.role) { "A chevron bevel" } else { "Herringbone: two opposite helices" });
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
    });
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
        item_rows(app, ui, k, inputs);
        if k.items.is_empty() {
            enter |= pick_rows(app, ui, k, inputs, hover, labels);
        }
        if by_rows(gk) {
            let mut size = usize::from(k.by_teeth);
            choice_row(ui, "Size By", "gear_size_by", &mut size, &["Module", "Teeth"]);
            k.by_teeth = size == 1;
            enter |= shaft_rows(ui, k);
            // Which gear drives (it is the one turned; the rest follow it).
            let gears: Vec<String> = rows_now(app, k, inputs)
                .iter()
                .enumerate()
                .filter(|(_, (p, _))| p.role < SHAFT_ROLE)
                .map(|(i, _)| format!("Row {}", i + 1))
                .collect();
            if gears.len() > 1 {
                let labels: Vec<&str> = gears.iter().map(String::as_str).collect();
                let mut at = k.driver.min(labels.len() - 1);
                choice_row(ui, "Driver", "gear_driver", &mut at, &labels);
                k.driver = at;
            }
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
    choice_row(ui, "Operation", "gear_op", &mut k.operation, &OP_LABELS);
    ui.checkbox(&mut k.cut_keep_out, solidcraft_ui_text::tr_label("Cut Keep-Out from Targets")).on_hover_text("Cut the room the set needs to turn out of the Targets (a housing)");
    ui.checkbox(&mut k.assembly, solidcraft_ui_text::tr_label("Assembly")).on_hover_text("Each gear a component, turning on a joint, linked to its mates");
    ui.checkbox(&mut k.clearance, solidcraft_ui_text::tr_label("Clearance Body"))
        .on_hover_text("A keep-out body round the set, with the bearings' seats when bearings are picked, to cut a housing with");
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
                // The engine's outlines of each gear; the gear crate's schematic of the set where
                // it gives none (a set read as one drawing).
                k.shape = match r.as_ref().map(outlines_of) {
                    Ok(o) if !o.is_empty() => o,
                    _ => r.as_ref().ok().and_then(|r| r.get("params")).map(shape_of).unwrap_or_default(),
                };
                k.members = r.as_ref().map(members_of).unwrap_or_default();
                (k.link_errors, k.suggested) = r.as_ref().map(crate::dialogs_gear_graph::link_notes).unwrap_or_default();
                k.shafts = r.as_ref().map(shafts_of).unwrap_or_default();
                k.contacts = r.as_ref().map(contacts_of).unwrap_or_default();
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

/// Each gear's outline `gear.calculate` gives (`outlines`): pitch, tip and root circles at its
/// back and front faces with four lines between the pitch circles; a bevel's pitch cone from
/// its heel to its toe (towards its apex).
fn outlines_of(r: &Value) -> Vec<(usize, Vec<[f64; 3]>)> {
    let p3 = |v: Option<&Value>| -> Option<[f64; 3]> {
        let a = v?.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
    };
    let mut out = Vec::new();
    for o in r.get("outlines").and_then(Value::as_array).into_iter().flatten() {
        let (Some(c), Some(ax)) = (p3(o.get("center")), p3(o.get("axis"))) else { continue };
        let num = |k: &str| o.get(k).and_then(Value::as_f64);
        let member = o.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let len = (ax[0] * ax[0] + ax[1] * ax[1] + ax[2] * ax[2]).sqrt().max(1e-12);
        let z = ax.map(|v| v / len);
        let x0 = if z[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let x = {
            let y = cross(z, x0);
            cross(y, z)
        };
        let xl = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt().max(1e-12);
        let x = x.map(|v| v / xl);
        let y = cross(z, x);
        let at = |r: f64, a: f64, h: f64| [0, 1, 2].map(|i| c[i] + r * (a.cos() * x[i] + a.sin() * y[i]) + h * z[i]);
        let ring = |r: f64, h: f64| (0..=48).map(|i| at(r, std::f64::consts::TAU * f64::from(i) / 48.0, h)).collect::<Vec<_>>();
        let fw = num("face_width").unwrap_or(0.0);
        let pitch = num("pitch_diameter").unwrap_or(0.0) / 2.0;
        // A bevel: the heel's circles, then the pitch cone to the toe's pitch circle.
        let toe = num("toe_pitch_diameter").map(|d| d / 2.0);
        let apex = p3(o.get("apex"));
        let (h1, r1) = match (toe, apex) {
            (Some(t), Some(a)) => {
                let along = [0, 1, 2].map(|i| a[i] - c[i]).iter().zip(z).map(|(d, zz)| d * zz).sum::<f64>();
                (along * (1.0 - t / pitch.max(1e-9)), t)
            }
            _ => (fw, pitch),
        };
        for key in ["tip_diameter", "root_diameter"] {
            if let Some(d) = num(key) {
                out.push((member, ring(d / 2.0, 0.0)));
                if toe.is_none() {
                    out.push((member, ring(d / 2.0, fw)));
                }
            }
        }
        out.push((member, ring(pitch, 0.0)));
        out.push((member, ring(r1, h1)));
        for q in 0..4 {
            let a = std::f64::consts::FRAC_PI_2 * f64::from(q);
            out.push((member, vec![at(pitch, a, 0.0), at(r1, a, h1)]));
        }
    }
    out
}

/// Each mesh's contact `gear.calculate` gives (`contacts`: {a, b} in the world; a point when they
/// are the same).
fn contacts_of(r: &Value) -> Vec<[[f64; 3]; 2]> {
    let p3 = |v: &Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
    };
    r.get("contacts").and_then(Value::as_array).into_iter().flatten().filter_map(|c| Some([p3(c.get("a")?)?, p3(c.get("b").unwrap_or(c.get("a")?))?])).collect()
}

/// The set's outline for parameters `gear.calculate` gave, in the world, from the gear crate's
/// schematic: each gear's pitch surface (rings at both faces and four lines along it: a bevel's
/// cones), its tip and root rings at both faces (thin), its shaft's axis, and the shafts as
/// cylinders. The second of each pair says whether it is drawn thin.
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
        // Tip, root and shaft surfaces: rings at their ends (thin), a shaft's four lines too.
        for p in &m.parts {
            let (z0, r0, z1, r1, along) = match (&p.role, &p.shape) {
                (Role::Tip | Role::Root, Shape::Cylinder { r, z0, z1 }) => (*z0, *r, *z1, *r, false),
                (Role::Tip | Role::Root, Shape::Cone { r0, z0, r1, z1, .. }) => (*z0, *r0, *z1, *r1, false),
                (Role::Shaft, Shape::Cylinder { r, z0, z1 }) => (*z0, *r, *z1, *r, true),
                _ => continue,
            };
            out.push((m.color, ring(m, r0, z0)));
            out.push((m.color, ring(m, r1, z1)));
            if along {
                for q in 0..4 {
                    let a = std::f64::consts::FRAC_PI_2 * f64::from(q);
                    out.push((m.color, vec![m.world([r0 * a.cos(), r0 * a.sin(), z0]), m.world([r1 * a.cos(), r1 * a.sin(), z1])]));
                }
            }
        }
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
    // Each mesh's contact: highlighted.
    let contact = egui::Color32::from_rgb(255, 210, 40);
    for [a, b] in &k.contacts {
        if let (Some(pa), Some(pb)) = (to(a), to(b)) {
            if pa.distance(pb) < 1.0 {
                painter.circle_filled(pa, 4.0, contact);
            } else {
                painter.line_segment([pa, pb], egui::Stroke::new(3.0, contact));
            }
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
