//! Simple mode's drawing and form kept with a set (in `gear.create`'s `ui`), so reopening the
//! Gear Generator on the set shows the drawing it was designed with: the drawn points, circles
//! and lines, the sketch frame, the picks, the turn, the model sketch it came from, and every
//! field of the form.

use serde_json::{Value, json};
use solidcraft_gear::geom::V2;
use solidcraft_gear::plan::{BevelPlanetMode, Frame, Kind, Spec};
use solidcraft_gear::simple::Simple;
use solidcraft_gear::sketch_rules::{Circle, Drawn, Line};

use crate::gear_simple::SimpleState;

fn v2(p: V2) -> Value {
    json!([p.x, p.y])
}

fn read_v2(v: &Value) -> Option<V2> {
    let a = v.as_array()?;
    let (x, y) = (a.first()?.as_f64()?, a.get(1)?.as_f64()?);
    (x.is_finite() && y.is_finite()).then(|| V2::new(x, y))
}

fn read_3(v: Option<&Value>) -> Option<[f64; 3]> {
    let a = v?.as_array()?;
    let mut out = [0.0; 3];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = a.get(i)?.as_f64().filter(|x| x.is_finite())?;
    }
    Some(out)
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// The form's settings as JSON (every field; `null` for "work it out").
fn settings_json(s: &Simple) -> Value {
    json!({
        "kind": s.kind.map(Kind::name),
        "shaft_angle": s.shaft_angle,
        "offset": s.offset,
        "ratio": s.ratio,
        "center_distance": s.center_distance,
        "pitch_diameter": s.pitch_diameter,
        "pitch_diameter2": s.pitch_diameter2,
        "ring_diameter": s.ring_diameter,
        "outer_diameter": s.outer_diameter,
        "cone_distance": s.cone_distance,
        "teeth": s.teeth,
        "module": s.module,
        "exact": s.exact,
        "face_width": s.face_width,
        "face_width2": s.face_width2,
        "helix": s.helix,
        "left_hand": s.left_hand,
        "herringbone": s.herringbone,
        "helix_split": s.helix_split,
        "spiral_angle": s.spiral_angle,
        "planets": s.planets,
        "bevel_planet_mode": s.bevel_planet_mode.map(BevelPlanetMode::name),
        "starts": s.starts,
        "length": s.length,
        "eccentricity": s.eccentricity,
        "bore": s.bore,
        "bore2": s.bore2,
        "chain": s.chain,
        "belt": s.belt,
        "printable": s.printable,
    })
}

#[allow(clippy::needless_update)]
fn read_settings(v: &Value) -> Simple {
    let f = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite());
    let u = |k: &str| v.get(k).and_then(Value::as_u64).and_then(|x| u32::try_from(x).ok());
    let b = |k: &str| v.get(k).and_then(Value::as_bool);
    let text = |k: &str| v.get(k).and_then(Value::as_str);
    Simple {
        kind: text("kind").and_then(Kind::parse),
        shaft_angle: f("shaft_angle"),
        offset: f("offset"),
        ratio: f("ratio"),
        center_distance: f("center_distance"),
        pitch_diameter: f("pitch_diameter"),
        pitch_diameter2: f("pitch_diameter2"),
        ring_diameter: f("ring_diameter"),
        outer_diameter: f("outer_diameter"),
        cone_distance: f("cone_distance"),
        teeth: u("teeth"),
        module: f("module"),
        exact: b("exact").unwrap_or(false),
        face_width: f("face_width"),
        face_width2: f("face_width2"),
        helix: f("helix"),
        left_hand: b("left_hand"),
        herringbone: b("herringbone").unwrap_or(false),
        helix_split: f("helix_split"),
        spiral_angle: f("spiral_angle"),
        planets: u("planets"),
        bevel_planet_mode: text("bevel_planet_mode").and_then(|n| BevelPlanetMode::ALL.iter().copied().find(|m| m.name() == n)),
        starts: u("starts"),
        length: f("length"),
        eccentricity: f("eccentricity"),
        bore: f("bore"),
        bore2: f("bore2"),
        chain: text("chain").and_then(|id| solidcraft_gear::sprocket::CHAINS.iter().find(|c| c.id == id).map(|c| c.id)),
        belt: text("belt").and_then(|id| solidcraft_gear::pulley::BELTS.iter().find(|c| c.id == id).map(|c| c.id)),
        printable: b("printable").unwrap_or(false),
        ..Simple::default()
    }
}

/// Simple mode's state as JSON, for the window's `ui` state.
pub(crate) fn to_json(st: &SimpleState) -> Value {
    let d = &st.drawn;
    json!({
        "points": d.points.iter().map(|p| v2(*p)).collect::<Vec<_>>(),
        "circles": d.circles.iter().map(|c| json!({"c": v2(c.c), "r": c.r})).collect::<Vec<_>>(),
        "lines": d.lines.iter().map(|l| json!({"a": v2(l.a), "b": v2(l.b)})).collect::<Vec<_>>(),
        "frame": {"origin": st.frame.origin, "x": st.frame.x, "z": st.frame.z},
        "source": st.source,
        "ids": [st.ids.0, st.ids.1, st.ids.2],
        "picks": st.picks,
        "auto": st.auto,
        "turn": st.turn,
        "template": st.template,
        "settings": settings_json(&st.settings),
    })
}

/// Simple mode's state from what [`to_json`] kept, for a window reopened on the set (`spec`:
/// the set as it was made, which the state starts from). `None` when nothing usable was kept.
pub(crate) fn from_json(v: &Value, spec: &Spec) -> Option<SimpleState> {
    let points: Vec<V2> = v.get("points")?.as_array()?.iter().filter_map(read_v2).collect();
    let circles: Vec<Circle> = v
        .get("circles")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|c| Some(Circle { c: read_v2(c.get("c")?)?, r: c.get("r")?.as_f64().filter(|r| r.is_finite() && *r > 0.0)? })).collect())
        .unwrap_or_default();
    let lines: Vec<Line> = v
        .get("lines")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|l| Some(Line { a: read_v2(l.get("a")?)?, b: read_v2(l.get("b")?)? })).collect())
        .unwrap_or_default();
    let mut st = SimpleState::new(spec);
    st.drawn = Drawn { points, circles, lines };
    if let Some(f) = v.get("frame")
        && let (Some(origin), Some(x), Some(z)) = (read_3(f.get("origin")), read_3(f.get("x")), read_3(f.get("z")))
    {
        st.frame = Frame { origin, x, z };
    }
    st.source = v.get("source").and_then(Value::as_u64);
    if let Some(ids) = v.get("ids").and_then(Value::as_array) {
        st.ids = (strings(ids.first()), strings(ids.get(1)), strings(ids.get(2)));
    }
    st.picks = v.get("picks").and_then(Value::as_array).map(|a| a.iter().map(|p| p.as_u64().map(|i| i as usize)).collect()).unwrap_or_default();
    st.auto = v.get("auto").and_then(Value::as_bool).unwrap_or(true);
    st.turn = read_3(v.get("turn")).unwrap_or([0.0; 3]);
    st.template = v.get("template").and_then(Value::as_bool).unwrap_or(false);
    if let Some(s) = v.get("settings") {
        st.settings = read_settings(s);
    }
    st.dirty = true;
    Some(st)
}
