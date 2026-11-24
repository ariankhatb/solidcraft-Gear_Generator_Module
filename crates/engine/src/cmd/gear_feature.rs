//! `gear.create` and `gear.update` as one native timeline feature (`build: feature`, the
//! default): a `FeatureKind::Gears` holding the set's parameters and the sketches it is read
//! from. The design evaluates it like any other feature: it reads those sketches again through
//! [`plan_for`] (the same reading `gear.create` does, on the design as it stands before the
//! feature), plans the set and builds every body in memory (`solidcraft_doc::eval::gears`). An
//! edit to a driving sketch recomputes it in place; the set's components and joints stay on its
//! bodies, which keep their names (the feature's body names), and [`update`] moves the joints
//! onto the new shafts when a sketch moved them (`follow_sketches` runs it).
//!
//! The set keeps its record as a folder set does (`gear-generator/sets/<id>.json`), `feature`
//! the feature's id and `group` the same, or the folder holding the feature with its components'
//! transfers and joints when the set is an assembly (one entry in the timeline either way). Sets made before are folders of ordinary features; they stay
//! as they are, and `gear.update {group, build: "feature"}` turns one into the feature.

use serde_json::{Value, json};
use solidcore_record::Vec3;
use solidcraft_doc::{BodyName, FeatureKind, Operation, SketchId};
use solidcraft_gear::plan::{Plan, solid};

use super::{Asked, BodyRecord, RECORD_VERSION, Read, Reads, Record, ask_read, assembly, axis_of, facts, keep, notes, params_of, reply};
use crate::params::bad;
use crate::schema::gear::{GearAxis, GearResult};
use crate::{EngineError, Result, Session};

/// Installs [`plan_for`] as the design's Gears planner.
pub(crate) fn install() {
    solidcraft_doc::set_gears_planner(plan_for);
}

/// Whether a request is made as one Gears feature: `build` (default `feature`), except teeth
/// drawn as formulas, which are features driven by the design's parameters.
pub(super) fn wanted(cmd: &str, request: &Value) -> Result<bool> {
    let formulas =
        request.get("tooth_curves").and_then(Value::as_str) == Some("equations") || request.get("parametric").and_then(Value::as_bool) == Some(true);
    match request.get("build").and_then(Value::as_str) {
        None | Some("feature") => Ok(!formulas),
        Some("features") => Ok(false),
        Some(other) => Err(bad(cmd, format!("`build` must be feature or features, not `{other}`"))),
    }
}

/// The design before a Gears feature, as it is evaluated: its sketches as solved there, axes
/// as given (`{origin, dir}` or X, Y, Z: the feature keeps other axes as lines).
struct Before<'a> {
    doc: &'a solidcraft_doc::Document,
    state: &'a solidcraft_doc::ModelState,
}

impl Reads for Before<'_> {
    fn sketch_id(&self, v: Option<&Value>, cmd: &str, what: &str) -> Result<u64> {
        let key = super::super::features::sketch_key(v).ok_or_else(|| bad(cmd, format!("`{what}` must be a sketch id or name")))?;
        match self.doc.find_feature(&key) {
            Some(f) if matches!(f.kind, FeatureKind::Sketch { .. }) => Ok(f.id),
            _ => Err(bad(cmd, format!("no sketch `{key}`"))),
        }
    }
    fn solved(&self, id: u64) -> Option<solidcraft_doc::SolvedSketch> {
        self.state.sketch(id).cloned()
    }
    fn sketch_name(&self, id: u64) -> Option<String> {
        self.doc.feature(id).map(|f| f.name.clone())
    }
    fn axis_line(&self, p: &Value, cmd: &str) -> Result<(Vec3, Vec3)> {
        match p.get("axis") {
            Some(o @ Value::Object(_)) => super::super::features::axis_line_object(o).map_err(|why| bad(cmd, why)),
            Some(Value::String(a)) if a.eq_ignore_ascii_case("x") => Ok((Vec3::ZERO, Vec3::X)),
            Some(Value::String(a)) if a.eq_ignore_ascii_case("y") => Ok((Vec3::ZERO, Vec3::Y)),
            Some(Value::String(a)) if a.eq_ignore_ascii_case("z") => Ok((Vec3::ZERO, Vec3::Z)),
            _ => Err(bad(cmd, "a Gears feature keeps its axes as sketch points and lines or {origin, dir}")),
        }
    }
}

/// The set a Gears feature asks for, read from the design before it.
fn plan_for(input: &solidcraft_doc::GearsInput<'_>) -> std::result::Result<Plan, String> {
    let mut params = input.params.clone();
    // The sketches by the ids the feature keeps (a pasted or renumbered copy follows them).
    if !input.sketches.is_empty() && params.get("sketch").is_some() {
        params["sketch"] = match (&params["sketch"], input.sketches) {
            (Value::Array(_), ids) => json!(ids),
            (_, [one]) => json!(one),
            (_, ids) => json!(ids),
        };
    }
    match ask_read(&Before { doc: input.doc, state: input.state }, "gears", &params) {
        Ok(Read::Set(_, plan)) => Ok(plan),
        Ok(Read::Wrong { error, .. }) => Err(error),
        Err(EngineError::BadParams { msg, .. }) => Err(msg),
        Err(e) => Err(e.to_string()),
    }
}

/// The request as the feature keeps it: sketches by id, every axis that is not a sketch's point
/// or line as the line it is now (`{origin, dir}`), with the sketches it reads.
fn frozen(s: &Session, cmd: &str, mut request: Value) -> Result<(Value, Vec<SketchId>)> {
    let mut sketches: Vec<u64> = Vec::new();
    let mut add = |id: u64| {
        if !sketches.contains(&id) {
            sketches.push(id);
        }
    };
    match request.get("sketch").cloned() {
        Some(Value::Array(list)) => {
            let mut ids = Vec::new();
            for v in &list {
                let id = s.sketch_id(Some(v), cmd, "sketch")?;
                add(id);
                ids.push(json!(id));
            }
            request["sketch"] = Value::Array(ids);
        }
        Some(v @ (Value::Number(_) | Value::String(_))) => {
            let id = s.sketch_id(Some(&v), cmd, "sketch")?;
            add(id);
            request["sketch"] = json!(id);
        }
        _ => {}
    }
    let line = |a: solidcraft_gear::axes::Axis| json!({"origin": a.origin, "dir": a.dir});
    for key in ["axis1", "axis2"] {
        let Some(v) = request.get(key).cloned().filter(|v| !v.is_null()) else { continue };
        if let Some(sk) = v.get("sketch").filter(|_| v.get("point").is_some() || v.get("curve").is_some()) {
            add(s.sketch_id(Some(sk), cmd, key)?);
            continue;
        }
        request[key] = line(axis_of(s, &GearAxis(v), cmd, key)?);
    }
    // A picked thing (of `selection`, or an item's `components`): a sketch's by its id, any other
    // as the line it is now.
    let mut fix = |what: &str, it: &mut Value| -> Result<()> {
        if let Some(sk) = it.get("sketch").cloned().filter(|v| !v.is_null()) {
            let id = s.sketch_id(Some(&sk), cmd, what)?;
            add(id);
            it["sketch"] = json!(id);
            return Ok(());
        }
        let v = if let Some(e) = it.get("edge").filter(|v| !v.is_null()) {
            json!({"edge": e})
        } else if let Some(f) = it.get("face").filter(|v| !v.is_null()) {
            json!({"face": f})
        } else {
            match it.get("axis") {
                Some(a) if !a.is_null() => a.clone(),
                _ => return Ok(()),
            }
        };
        let a = axis_of(s, &GearAxis(v), cmd, what)?;
        if let Some(o) = it.as_object_mut() {
            o.remove("edge");
            o.remove("face");
            o.insert("axis".into(), line(a));
        }
        Ok(())
    };
    if let Some(items) = request.get_mut("selection").and_then(Value::as_array_mut) {
        for (i, it) in items.iter_mut().enumerate() {
            fix(&format!("selection[{i}]"), it)?;
        }
    }
    if let Some(items) = request.get_mut("items").and_then(Value::as_array_mut) {
        for (i, item) in items.iter_mut().enumerate() {
            if let Some(cs) = item.get_mut("components").and_then(Value::as_array_mut) {
                for (k, c) in cs.iter_mut().enumerate() {
                    fix(&format!("items[{i}].components[{k}]"), c)?;
                    // Its sketch's name now, for an error to name it by should it be deleted.
                    if let Some(name) = c.get("sketch").and_then(Value::as_u64).and_then(|id| s.doc.feature(id)).map(|f| f.name.clone()) {
                        c["sketch_name"] = json!(name);
                    }
                }
            }
        }
    }
    Ok((request, sketches.into_iter().map(SketchId).collect()))
}

/// What the feature's bodies do (`operation`, `targets`) and the bodies its keep-out cuts.
fn tools_of(s: &Session, cmd: &str, request: &Value) -> Result<(Operation, Vec<BodyName>, Vec<BodyName>)> {
    let operation = super::super::features::operation(request, cmd)?;
    let state = s.model.state();
    let names = |key: &str| -> Result<Vec<BodyName>> {
        let Some(v) = request.get(key).filter(|v| !v.is_null()) else { return Ok(Vec::new()) };
        let list: Vec<&str> = v
            .as_array()
            .and_then(|a| a.iter().map(Value::as_str).collect())
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a list of body names")))?;
        list.iter()
            .map(|n| if state.body(n).is_some() { Ok(BodyName(n.to_string())) } else { Err(bad(cmd, format!("`{key}`: no body `{n}`"))) })
            .collect()
    };
    let (targets, cuts) = (names("targets")?, names("keep_out_cuts")?);
    if operation != Operation::NewBody && targets.is_empty() {
        return Err(bad(cmd, "a set that joins, cuts or intersects needs `targets`: the bodies it works on"));
    }
    Ok((operation, targets, cuts))
}

/// The keep-out body's component in the assembly: a set of gear items makes its housing cutout a
/// plain body (no component, no joint); an older set keeps its grounded keep-out component.
fn keep_out_component<'a>(request: &Value, name: Option<&'a str>) -> Option<&'a str> {
    if request.get("items").is_some_and(Value::is_array) { None } else { name }
}

/// What the feature puts out, in order, by the generator's names: each rigid member, then the
/// keep-out body when it is a body of its own (a new set that cuts nothing with it).
fn generated(plan: &Plan, operation: Operation, cuts: &[BodyName]) -> (Vec<String>, Option<String>) {
    let members: Vec<String> = solid::members(plan).into_iter().map(|r| r.name).collect();
    let keep_out = plan.clearance.as_ref().filter(|_| operation == Operation::NewBody && cuts.is_empty()).map(|c| c.name.clone());
    (members, keep_out)
}

/// Names no body has yet for `base` names (`Gear z20 (2)` where `Gear z20` is taken), apart from
/// the feature's own bodies (`own`).
fn fresh(s: &Session, bases: &[String], own: Option<u64>) -> Vec<String> {
    let mut taken: std::collections::BTreeSet<String> =
        s.model.state().bodies.iter().filter(|b| Some(b.feature) != own).map(|b| b.name.clone()).collect();
    bases
        .iter()
        .map(|base| {
            let mut n = base.clone();
            let mut k = 2;
            while taken.contains(&n) {
                n = format!("{base} ({k})");
                k += 1;
            }
            taken.insert(n.clone());
            n
        })
        .collect()
}

/// The plan the set's assembly is made of ([`rigid_plan`]) and the member that drives it: the
/// drive gear's for a drivetrain of items (its assembly rooted there), else `assembly_driver`.
fn assembly_plan(asked: &Asked, plan: &Plan, request: &Value) -> (Plan, Option<usize>) {
    let mut rigid = rigid_plan(plan);
    let Some(it) = &asked.items else { return (rigid, assembly::driver(request)) };
    // (The drive's rigid member, as a member of the assembly.)
    let driver = super::items::driver(it, plan).and_then(|r| rigid.assembly.members.iter().position(|m| m.body == r));
    if let Some(d) = driver {
        rigid.assembly = rigid.assembly.rooted(d);
    }
    (rigid, driver)
}

/// The set's assembly as its bodies stand: one member per rigid member.
fn rigid_plan(plan: &Plan) -> Plan {
    let rigid = solid::members(plan);
    let mut out = plan.clone();
    out.assembly = solid::assembly(plan, &rigid);
    out
}

/// The folder an assembled set's feature, component transfers and joints go in (one entry in
/// the timeline): made round the feature.
fn assembly_folder(s: &mut Session, name: &str, feature: u64) -> Result<u64> {
    let r = s.execute("timeline.group", &json!({"name": name, "features": [feature]}))?;
    r.get("group").and_then(Value::as_u64).ok_or_else(|| EngineError::Other("the set's folder was not made".into()))
}

/// The feature's evaluation failed: its error, as a feature's.
fn failed(s: &Session, id: u64) -> Option<EngineError> {
    let r = s.model.result(id)?;
    r.error.clone().map(|e| EngineError::of_feature(r, e))
}

/// `gear.create` as one Gears feature.
#[allow(clippy::too_many_arguments)]
pub(super) fn create(
    s: &mut Session,
    cmd: &str,
    asked: &Asked,
    plan: &Plan,
    name: String,
    request: Value,
    mut warnings: Vec<String>,
    sketch_print: Option<String>,
) -> Result<Value> {
    let (mut request, sketches) = frozen(s, cmd, request)?;
    let (operation, targets, cuts) = tools_of(s, cmd, &request)?;
    let (members, keep_out) = generated(plan, operation, &cuts);
    let bases: Vec<String> = members.iter().cloned().chain(keep_out.clone()).collect();
    let names = fresh(s, &bases, None);
    if asked.items.is_some() {
        request["body_keys"] = json!(bases);
    }
    super::prime_items(&*s, cmd, &request, asked, plan);
    let kind = FeatureKind::Gears { params: request.clone(), sketches, operation, targets, keep_out_cuts: cuts };
    let made = super::super::features::add_feature(s, &json!({"name": name, "body_names": names}), kind)?;
    let id = made.get("feature").and_then(Value::as_u64).ok_or_else(|| EngineError::Other("the Gears feature was not made".into()))?;
    if let Some(w) = made.get("warning").and_then(Value::as_str) {
        warnings.push(w.to_string());
    }
    let member_names: Vec<String> = names.iter().take(members.len()).cloned().collect();
    let keep_out_name = keep_out.as_ref().and_then(|_| names.get(members.len()).cloned());
    let mut built = None;
    let mut group = id;
    if assembly::wanted(&request) {
        let folder = assembly_folder(s, &name, id)?;
        group = folder;
        let (rigid, driver) = assembly_plan(asked, plan, &request);
        built = Some(assembly::sync(
            s,
            &rigid,
            asked.spec.kind,
            &name,
            &member_names,
            keep_out_component(&request, keep_out_name.as_deref()),
            Some(folder),
            None,
            driver,
            &mut warnings,
        )?);
    }
    let (assembly_record, assembly_made) = built.map_or((None, None), |(r, m)| (Some(r), Some(m)));
    let bodies = names.iter().map(|n| BodyRecord { name: n.clone(), steps: Vec::new(), auto: Some(n.clone()) }).collect();
    let rec = Record {
        version: RECORD_VERSION,
        group,
        name,
        request,
        params: params_of(&asked.spec),
        bodies,
        assembly: assembly_record,
        sketch_print,
        feature: Some(id),
    };
    keep(s, &rec, &mut warnings);
    let mut r = GearResult {
        bodies: names.clone(),
        group: Some(group),
        feature: Some(id),
        clearance: keep_out_name,
        assembly: assembly_made,
        facts: facts(asked, plan),
        warnings,
        notes: notes(asked),
        members: asked.members.clone(),
        ..GearResult::default()
    };
    if let Some(it) = &asked.items {
        super::items::fill(&mut r, it, plan, Some(&names));
    }
    reply(cmd, r)
}

/// The names the set's bodies have now: the feature's body names (a rename changes them), else
/// the record's.
pub(super) fn current_names(s: &Session, id: u64, rec: &Record) -> Vec<String> {
    let stored = s.doc.feature(id).map(|f| f.body_names.clone()).unwrap_or_default();
    rec.bodies.iter().enumerate().map(|(k, b)| stored.get(k).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| b.name.clone())).collect()
}

/// A folder set turned into one Gears feature: the folder's first feature becomes it, in its
/// place in the history (before the transfers that move the set's bodies into components), and
/// its other sketches and features go. Returns the feature's id.
fn convert(s: &mut Session, cmd: &str, rec: &Record) -> Result<u64> {
    let own = rec.features();
    let first = s.doc.features.iter().find(|f| own.contains(&f.id)).map(|f| f.id).ok_or_else(|| bad(cmd, "the set's features are gone"))?;
    let rest: Vec<u64> = own.into_iter().filter(|f| *f != first && s.doc.feature(*f).is_some()).collect();
    let name = rec.name.clone();
    if let Some(f) = s.doc_mut().feature_mut(first) {
        f.kind = FeatureKind::Gears {
            params: json!({}),
            sketches: Vec::new(),
            operation: Operation::NewBody,
            targets: Vec::new(),
            keep_out_cuts: Vec::new(),
        };
        f.name = name;
        f.body_names.clear();
        f.suppressed = false;
    }
    if !rest.is_empty() {
        s.execute("timeline.delete", &json!({"features": rest}))?;
    }
    Ok(first)
}

/// Every sketch the feature reads comes before it in the timeline: one drawn after it (an item
/// added with a new sketch) is moved to just before it.
fn sketches_first(s: &mut Session, cmd: &str, id: u64, sketches: &[SketchId]) -> Result<()> {
    for sk in sketches {
        let at = |s: &Session, f: u64| s.doc.features.iter().position(|x| x.id == f);
        if let (Some(fp), Some(sp)) = (at(s, id), at(s, sk.0))
            && sp > fp
        {
            s.execute("timeline.reorder", &json!({"feature": sk.0, "position": fp}))
                .map_err(|e| bad(cmd, format!("sketch {} comes after the Gears feature and can't be moved before it ({e})", sk.0)))?;
        }
    }
    Ok(())
}

/// `gear.update` of a Gears feature (or of a folder set asked to become one): the feature's
/// parameters set and the design evaluated again; the bodies keep their names where the set has
/// as many, those still called what the generator named them follow the new set's names, and
/// the assembly is brought up to date where it stands.
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    s: &mut Session,
    cmd: &str,
    mut rec: Record,
    request: Value,
    asked: &Asked,
    plan: &Plan,
    mut warnings: Vec<String>,
    sketch_print: Option<String>,
) -> Result<Value> {
    if !wanted(cmd, &request)? {
        return Err(bad(cmd, "this set is one Gears feature: its teeth can't be drawn as formulas, and it can't go back to a folder of features"));
    }
    // Nothing asked but to follow the sketches (`follow_sketches`, a bare `gear.update {group}`):
    // the bodies keep their names, as the feature keeps them through a sketch edit.
    let bare = request == rec.request;
    let names_before = super::current_names(s, &rec);
    let auto_before: Vec<String> = rec.bodies.iter().map(|b| b.auto.clone().unwrap_or_else(|| b.name.clone())).collect();
    let before = s.doc.clone();
    let converted = rec.feature.is_none();
    let id = match rec.feature {
        Some(id) if s.doc.feature(id).is_some_and(|f| matches!(f.kind, FeatureKind::Gears { .. })) => id,
        Some(_) => return Err(bad(cmd, "the set's Gears feature is gone")),
        None => convert(s, cmd, &rec)?,
    };
    let (mut request, sketches) = frozen(s, cmd, request)?;
    let (operation, targets, cuts) = tools_of(s, cmd, &request)?;
    let (members, keep_out) = generated(plan, operation, &cuts);
    let bases: Vec<String> = members.iter().cloned().chain(keep_out.clone()).collect();
    let items = asked.items.is_some();
    // Bodies keep their names where the set still has as many; another set of members is named
    // afresh. A drivetrain of items: each body whose item and gear are still there keeps its name.
    let names = if items {
        super::items::carry_names(s, id, &rec.request, &names_before, &request, &bases)
    } else if names_before.len() == bases.len() {
        names_before.clone()
    } else {
        fresh(s, &bases, Some(id))
    };
    if items {
        request["body_keys"] = json!(bases);
    }
    sketches_first(s, cmd, id, &sketches)?;
    super::prime_items(&*s, cmd, &request, asked, plan);
    if let Some(f) = s.doc_mut().feature_mut(id) {
        f.kind = FeatureKind::Gears { params: request.clone(), sketches, operation, targets, keep_out_cuts: cuts };
        f.body_names = names.clone();
    }
    s.refresh();
    if let Some(e) = failed(s, id) {
        s.doc = before;
        s.refresh();
        return Err(e);
    }
    rec.bodies = names.iter().map(|n| BodyRecord { name: n.clone(), steps: Vec::new(), auto: items.then(|| n.clone()) }).collect();
    // (Bodies of items are named by item and gear, which an update doesn't change.)
    if bare && !items && names_before.len() == bases.len() {
        for (b, auto) in rec.bodies.iter_mut().zip(&auto_before) {
            b.auto = Some(auto.clone());
        }
    } else if !items {
        let auto: &[String] = if names_before.len() == bases.len() { &auto_before } else { &names };
        super::follow_auto_names(s, &mut rec, &bases, &names, auto, &mut warnings);
    }
    let member_names: Vec<String> = rec.bodies.iter().take(members.len()).map(|b| b.name.clone()).collect();
    let keep_out_name = keep_out.as_ref().and_then(|_| rec.bodies.get(members.len()).map(|b| b.name.clone()));
    let mut assembly_made = None;
    // (Its members' bodies by the names they have now: `records` follows renames.)
    let old_assembly = rec.assembly.take();
    // The set's folder: the one it has (a folder set's, or an assembled set's), else none.
    let mut group = Some(rec.group).filter(|g| s.doc.timeline_groups.iter().any(|t| t.id == *g));
    if assembly::wanted(&request) {
        let folder = match group {
            Some(g) => g,
            None => assembly_folder(s, &rec.name, id)?,
        };
        group = Some(folder);
        let (rigid, driver) = assembly_plan(asked, plan, &request);
        let (made, out) = assembly::sync(
            s,
            &rigid,
            asked.spec.kind,
            &rec.name,
            &member_names,
            keep_out_component(&request, keep_out_name.as_deref()),
            Some(folder),
            old_assembly,
            driver,
            &mut warnings,
        )?;
        rec.assembly = Some(made);
        assembly_made = Some(out);
    } else if let Some(old) = old_assembly {
        assembly::remove(s, &old, &mut warnings)?;
    }
    let old_key = super::record_key(rec.group);
    rec.group = group.unwrap_or(id);
    rec.feature = Some(id);
    rec.request = request;
    rec.params = params_of(&asked.spec);
    rec.sketch_print = sketch_print;
    if old_key != super::record_key(rec.group) {
        s.doc_mut().plugin_data.remove(&old_key);
    }
    keep(s, &rec, &mut warnings);
    s.refresh();
    if let Some(w) = s.model.result(id).and_then(|r| r.warning.clone()) {
        warnings.push(w);
    }
    let bodies: Vec<String> = rec.bodies.iter().map(|b| b.name.clone()).collect();
    let mut r = GearResult {
        bodies: bodies.clone(),
        group: Some(rec.group),
        feature: Some(id),
        clearance: keep_out_name,
        assembly: assembly_made,
        facts: facts(asked, plan),
        warnings,
        notes: notes(asked),
        updated: Some(if converted { "rebuilt" } else { "in_place" }.into()),
        rebuilt_because: converted.then(|| "the folder of features became one Gears feature".into()),
        ..GearResult::default()
    };
    if let Some(it) = &asked.items {
        super::items::fill(&mut r, it, plan, Some(&bodies));
    }
    reply(cmd, r)
}

#[cfg(test)]
#[path = "gear_feature_tests.rs"]
mod tests;
