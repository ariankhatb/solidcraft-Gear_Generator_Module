//! A gear set's drawing as a native sketch, and sets that follow their sketch.
//!
//! `gear.sketch` draws a mode's picture (`sketch_rules::template`: pitch circles, shaft lines,
//! the origin or apex point) as an ordinary sketch in the design, held by the app's own
//! constraints and dimensions as the sketch rules read it: a pair's circles tangent, a planet
//! tangent to its sun and ring, a worm's or rack's line tangent to the wheel, a bevel's shafts
//! meeting at the apex with the meeting angle as an angle dimension, the pitch diameters, a
//! hypoid's or crossed set's offset. It is edited like any other sketch (Edit Sketch, the
//! Dimension tool, Parameters).
//!
//! A set made from a sketch (`gear.create {sketch}`, or `sketch: "new"` / `{new: plane}` to draw
//! it in the same step) keeps a print of what the sketch rules read from it. When a command
//! leaves the sketch reading differently (a dimension changed, Finish Sketch after an edit, a
//! parameter it uses), [`follow_sketches`] regenerates the set with `gear.update`, in place
//! where it can be (same bodies, features and joints), in that command's undo step.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solidcraft_gear::geom::V2;
use solidcraft_gear::plan::Kind;
use solidcraft_gear::sketch_rules::{self, Role};

use super::{Record, bad, kind_of, records, sketch_entities, spec_of};
use crate::schema::gear::{GearParams, GearSketchParams, GearSketchResult};
use crate::{Result, Session};

/// The ids the drawing's entities get.
const ORIGIN: &str = "gear_origin";

fn circle_id(i: usize) -> String {
    format!("pitch{}", i + 1)
}

fn line_id(i: usize) -> String {
    format!("shaft{}", i + 1)
}

fn centre_id(i: usize) -> String {
    format!("pitch{}_centre", i + 1)
}

/// `gear.sketch`.
pub(super) fn make(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.sketch";
    let q: GearSketchParams = serde_json::from_value(if p.is_null() { json!({}) } else { p.clone() }).map_err(|e| bad(cmd, e.to_string()))?;
    let made = draw(s, cmd, &q)?;
    serde_json::to_value(made).map_err(|e| bad(cmd, e.to_string()))
}

/// Draws the sketch `q` asks for and finishes it.
pub(super) fn draw(s: &mut Session, cmd: &str, q: &GearSketchParams) -> Result<GearSketchResult> {
    if s.active_sketch.is_some() {
        return Err(bad(cmd, "finish the sketch being edited first"));
    }
    let mut gp: GearParams = match &q.gear {
        Some(v) if !v.is_null() => serde_json::from_value(v.clone()).map_err(|e| bad(cmd, format!("`gear`: {e}")))?,
        _ => GearParams::default(),
    };
    gp.kind = Some(q.kind);
    // Sizes only: where the set goes is the sketch's plane.
    gp.placement = None;
    gp.sketch = None;
    gp.picks = None;
    let kind = kind_of(q.kind);
    if !sketch_rules::sketched(kind) {
        return Err(bad(cmd, format!("a {} is not drawn from a sketch", kind.label())));
    }
    let spec = spec_of(&gp).map_err(|e| bad(cmd, e))?;
    let d = sketch_rules::template(kind, &spec);
    let mut create = json!({"plane": q.plane.clone().unwrap_or(json!("XY")), "project_edges": false});
    if let Some(n) = q.name.as_deref().filter(|n| !n.trim().is_empty()) {
        create["name"] = json!(n);
    }
    let sk = s.execute("sketch.create", &create)?.get("sketch").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "sketch.create gave no sketch"))?;
    let mut dims = BTreeMap::new();
    let mut constraints = Vec::new();
    let drawn = (|| -> Result<()> {
        let at = |v: V2| json!([v.x, v.y]);
        // The origin (a bevel's apex), on the sketch's own origin.
        let origin = d.points.first().copied().unwrap_or(V2::ZERO);
        s.execute("sketch.point", &json!({"point": at(origin), "id": ORIGIN}))?;
        for (i, c) in d.circles.iter().enumerate() {
            let centre = if c.c.dist(origin) < 1e-9 {
                json!(ORIGIN)
            } else {
                s.execute("sketch.point", &json!({"point": at(c.c), "id": centre_id(i), "connect": false}))?;
                json!(centre_id(i))
            };
            s.execute("sketch.circle.center", &json!({"center": centre, "radius": c.r, "id": circle_id(i)}))?;
        }
        for (i, l) in d.lines.iter().enumerate() {
            s.execute("sketch.line", &json!({"points": [at(l.a), at(l.b)], "ids": [line_id(i)]}))?;
        }
        let mut constrain = |s: &mut Session, id: &str, p: Value| -> Result<()> {
            let r = s.execute(id, &p)?;
            for c in r.get("constraints").and_then(Value::as_array).into_iter().flatten().chain(r.get("constraint")) {
                if let Some(c) = c.as_str() {
                    constraints.push(c.to_string());
                }
            }
            Ok(())
        };
        let mut dimension = |s: &mut Session, key: &str, p: Value| -> Result<()> {
            let r = s.execute("sketch.dimension", &p)?;
            if let Some(name) = r.get("param").and_then(Value::as_str) {
                dims.insert(key.to_string(), name.to_string());
            }
            Ok(())
        };
        let (c1, c2, c3) = (circle_id(0), circle_id(1), circle_id(2));
        let (l1, l2) = (line_id(0), line_id(1));
        let horizontal_centres = |s: &mut Session, constrain: &mut dyn FnMut(&mut Session, &str, Value) -> Result<()>| {
            constrain(s, "sketch.constraint.horizontal_vertical", json!({"points": [ORIGIN, centre_id(1)], "mode": "horizontal"}))
        };
        match kind {
            Kind::Pair | Kind::Cycloidal | Kind::Internal => {
                constrain(s, "sketch.constraint.tangent", json!({"a": c1, "b": c2}))?;
                horizontal_centres(s, &mut constrain)?;
                dimension(s, "diameter1", json!({"entities": [c1], "type": "diameter"}))?;
                dimension(s, "diameter2", json!({"entities": [c2], "type": "diameter"}))?;
            }
            Kind::Planetary => {
                constrain(s, "sketch.constraint.tangent", json!({"a": c1, "b": c2}))?;
                constrain(s, "sketch.constraint.tangent", json!({"a": c2, "b": c3}))?;
                horizontal_centres(s, &mut constrain)?;
                dimension(s, "diameter1", json!({"entities": [c1], "type": "diameter"}))?;
                dimension(s, "diameter2", json!({"entities": [c2], "type": "diameter"}))?;
            }
            Kind::Rack | Kind::Worm => {
                constrain(s, "sketch.constraint.tangent", json!({"a": c1, "b": l1}))?;
                constrain(s, "sketch.constraint.horizontal_vertical", json!({"line": l1, "mode": "horizontal"}))?;
                dimension(s, "diameter1", json!({"entities": [c1], "type": "diameter"}))?;
                if kind == Kind::Rack {
                    dimension(s, "length", json!({"entities": [l1], "type": "length"}))?;
                }
            }
            Kind::FaceGear => {
                constrain(s, "sketch.constraint.horizontal_vertical", json!({"line": l1, "mode": "horizontal"}))?;
                dimension(s, "diameter1", json!({"entities": [c1], "type": "diameter"}))?;
            }
            Kind::Hypoid | Kind::CrossedHelical => {
                constrain(s, "sketch.constraint.horizontal_vertical", json!({"line": l1, "mode": "horizontal"}))?;
                dimension(s, "diameter1", json!({"entities": [c1], "type": "diameter"}))?;
                dimension(s, "offset", json!({"entities": [ORIGIN, l1], "type": "distance"}))?;
            }
            Kind::Bevel | Kind::SpiralBevel | Kind::BevelPlanetary => {
                constrain(s, "sketch.constraint.horizontal_vertical", json!({"line": l1, "mode": "vertical"}))?;
                // The angle's text between the two shafts picks the sector that is the shaft angle.
                if let (Some(a), Some(b)) = (d.lines.first(), d.lines.get(1)) {
                    let far = |l: &sketch_rules::Line| if l.a.dist(origin) >= l.b.dist(origin) { l.a } else { l.b };
                    let (u, v) = (far(a) - origin, far(b) - origin);
                    let mid = origin + (u * (1.0 / u.len().max(1e-9)) + v * (1.0 / v.len().max(1e-9))) * (0.3 * u.len().min(v.len()));
                    dimension(s, "shaft_angle", json!({"entities": [l1, l2], "type": "angle", "text_at": at(mid)}))?;
                }
            }
            _ => {
                dimension(s, "diameter1", json!({"entities": [c1], "type": "diameter"}))?;
            }
        }
        Ok(())
    })();
    // Finished either way (a refusal undoes the whole command).
    s.execute("sketch.finish", &json!({}))?;
    drawn?;
    if s.model.is_stale() {
        s.refresh();
    }
    let gs = sketch_entities(s, sk).map_err(|e| bad(cmd, e))?;
    let picks = sketch_rules::auto_picks(kind, &gs.user_drawn())
        .iter()
        .zip(sketch_rules::slots(kind))
        .map(|(pick, slot)| {
            pick.and_then(|i| match slot.role {
                Role::Point => gs.points.get(i).map(|p| p.0.clone()),
                Role::Circle => gs.circles.get(i).map(|c| c.0.clone()),
                Role::Line => gs.lines.get(i).map(|l| l.0.clone()),
            })
        })
        .collect();
    Ok(GearSketchResult { sketch: sk, picks, dimensions: dims, constraints })
}

/// `gear.create`'s `sketch: "new"` (or `{new: plane}`): the drawing for the parameters, made
/// first in the same step; the request then names it and its picks.
pub(super) fn new_sketch(s: &mut Session, cmd: &str, p: &Value) -> Result<Option<Value>> {
    let plane = match p.get("sketch") {
        Some(Value::String(n)) if n == "new" => None,
        Some(Value::Object(o)) if o.contains_key("new") && o.len() == 1 => o.get("new").cloned().filter(|v| !v.is_null()),
        _ => return Ok(None),
    };
    let mut rest = p.as_object().cloned().unwrap_or_default();
    rest.remove("sketch");
    let params: GearParams = serde_json::from_value(Value::Object(rest)).map_err(|e| bad(cmd, e.to_string()))?;
    let kind = params.kind.ok_or_else(|| bad(cmd, "`sketch: \"new\"` needs `type`: the mode to draw"))?;
    let gear = serde_json::to_value(GearParams { sketch: None, picks: None, ..params }).map_err(|e| bad(cmd, e.to_string()))?;
    let made = draw(s, cmd, &GearSketchParams { kind, plane, name: None, gear: Some(gear) })?;
    let mut out = p.clone();
    out["sketch"] = json!(made.sketch);
    out["picks"] = json!(made.picks);
    // The drawing gives the sizes: the turns and placement are the sketch's.
    if let Some(o) = out.as_object_mut() {
        o.remove("placement");
    }
    Ok(Some(out))
}

/// A short, stable print of what the sketch rules read from a set's sketch (its plane and its
/// points, circles and lines with their ids), or `None` when the request names no sketch or
/// the sketch can't be read.
pub(super) fn sketch_print(s: &Session, request: &Value) -> Option<String> {
    let id = sketch_of(s, request)?;
    let gs = sketch_entities(s, id).ok()?;
    let mut text = String::new();
    let num = |v: f64| format!("{:.6}", if v.abs() < 5e-7 { 0.0 } else { v });
    let f = gs.frame;
    for v in f.origin.iter().chain(f.x.iter()).chain(f.z.iter()) {
        text.push_str(&num(*v));
        text.push(',');
    }
    for (k, p) in &gs.points {
        text.push_str(&format!("p{k}:{},{};", num(p.x), num(p.y)));
    }
    for (k, c) in &gs.circles {
        text.push_str(&format!("c{k}:{},{},{};", num(c.c.x), num(c.c.y), num(c.r)));
    }
    for (k, l) in &gs.lines {
        text.push_str(&format!("l{k}:{},{},{},{};", num(l.a.x), num(l.a.y), num(l.b.x), num(l.b.y)));
    }
    // FNV-1a: the same in every build (kept with the design).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{id}:{h:016x}"))
}

/// The sketch a set's request names (its `sketch`, or the sketch of the first shaft or
/// bearing picked in one in its `selection`), if it is still there.
pub(super) fn sketch_of(s: &Session, request: &Value) -> Option<u64> {
    let v = request
        .get("sketch")
        .filter(|v| !v.is_null())
        .or_else(|| request.get("selection")?.as_array()?.iter().find_map(|item| item.get("sketch").filter(|v| !v.is_null())))?;
    super::super::features::sketch_id(s, Some(v), "gear.update", "sketch").ok()
}

/// Whether the set's sketch reads differently from when the set was last made.
pub(super) fn sketch_changed(s: &Session, rec: &Record) -> bool {
    rec.sketch_print.is_some() && sketch_print(s, &rec.request).is_some_and(|now| Some(&now) != rec.sketch_print.as_ref())
}

/// Regenerates every set whose sketch reads differently than when it was made (see the module
/// docs): run by the session after a command that changed the design. Each update is a
/// command run inside that one, so it shares its undo step; one that fails leaves its set as it
/// was (and the set shows as needing an update). Returns what was done, for the reply.
pub fn follow_sketches(s: &mut Session) -> Vec<Value> {
    if s.active_sketch.is_some() || s.doc.marker.is_some_and(|m| m < s.doc.features.len()) {
        return Vec::new();
    }
    let linked: Vec<Record> = records(&s.doc).into_iter().filter(|r| r.sketch_print.is_some()).collect();
    if linked.is_empty() {
        return Vec::new();
    }
    if s.model.is_stale() {
        s.refresh();
    }
    let mut out = Vec::new();
    for rec in linked {
        if !sketch_changed(s, &rec) {
            continue;
        }
        match s.execute("gear.update", &json!({"group": rec.group})) {
            Ok(r) => out.push(json!({"group": rec.group, "name": rec.name, "updated": r.get("updated").cloned().unwrap_or(Value::Null)})),
            Err(e) => out.push(json!({"group": rec.group, "name": rec.name, "error": e.to_string()})),
        }
    }
    out
}
