//! `assembly: true`: a gear set made a complete assembly from its plan's [`Assembly`]. Every
//! member's body becomes a component of its own (`component.from_bodies`), the keep-out
//! (clearance) body a grounded, see-through component "<set> keep-out"; each member that moves
//! gets a revolute joint on its shaft (a slider for a rack, a rigid joint for a spline's hub on
//! its shaft) to the world, or to its carrier; and each mesh a motion link (`joint.gear`,
//! `joint.rack_pinion`) with the plan's ratio and direction, so driving one gear turns the
//! others. Made inside `gear.create`/`gear.update`, so in their undo step, and moved into the
//! set's timeline folder.
//!
//! An update takes the set's assembly down first ([`remove`]: its joints and links, then its
//! components with the transfers that moved the bodies in, which puts the bodies back where the
//! set's features make them) and makes it again for the new set, so there is only ever one set
//! of components and joints.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use solidcraft_gear::axes::Axis;
use solidcraft_gear::plan::{Assembly, AssemblyLink, Kind, Plan};

use crate::schema::gear::{GearAssembly, GearAssemblyMember};
use crate::{EngineError, Result, Session};

/// What a set's assembly was made of, kept with the set's record so an update can take it down.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct AssemblyRecord {
    /// The components made (the members', then the keep-out's).
    #[serde(default)]
    pub(super) components: Vec<u64>,
    /// The joints made.
    #[serde(default)]
    pub(super) joints: Vec<u64>,
}

/// Whether the request asks for the set as an assembly.
pub(super) fn wanted(request: &Value) -> bool {
    request.get("assembly").and_then(Value::as_bool) == Some(true)
}

/// The keep-out component's colour and opacity.
const KEEP_OUT_COLOR: &str = "#5b9bd5";
const KEEP_OUT_OPACITY: f64 = 0.25;

fn occurrence_of(s: &Session, component: u64) -> Option<u64> {
    s.doc.occurrence_of(component).map(|o| o.id)
}

/// Whether two axes are one line (either way round).
fn coaxial(a: &Axis, b: &Axis) -> bool {
    let d = [b.origin[0] - a.origin[0], b.origin[1] - a.origin[1], b.origin[2] - a.origin[2]];
    let along = d[0] * a.dir[0] + d[1] * a.dir[1] + d[2] * a.dir[2];
    let off = [d[0] - along * a.dir[0], d[1] - along * a.dir[1], d[2] - along * a.dir[2]];
    let par = a.dir[0] * b.dir[0] + a.dir[1] * b.dir[1] + a.dir[2] * b.dir[2];
    (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt() < 1e-6 && par.abs() > 1.0 - 1e-9
}

/// The member member `i` turns on (its carrier, or the carrier of the planets it meshes with
/// when it turns on the carrier's axis: a sun or ring of an epicyclic set turns against its
/// carrier, so its link to the planets, which turn on the carrier, is between turns relative to
/// it); `None`: the world.
fn base_of(a: &Assembly, i: usize) -> Option<usize> {
    let m = &a.members[i];
    if m.carrier.is_some() {
        return m.carrier;
    }
    a.links.iter().find_map(|l| {
        let (x, y) = l.members();
        let other = if x == i { y } else if y == i { x } else { return None };
        let c = a.members.get(other)?.carrier?;
        (c != i && a.members.get(c).is_some_and(|cm| coaxial(&cm.axis, &m.axis))).then_some(c)
    })
}

/// Makes the assembly of a set just built: `names` are its gear bodies' names in plan order,
/// `keep_out` its clearance body's name, `set` its name, `group` its timeline folder.
#[allow(clippy::too_many_arguments)]
pub(super) fn make(
    s: &mut Session,
    plan: &Plan,
    kind: Kind,
    set: &str,
    names: &[String],
    keep_out: Option<&str>,
    group: Option<u64>,
    warnings: &mut Vec<String>,
) -> Result<(AssemblyRecord, GearAssembly)> {
    let a = &plan.assembly;
    let n = a.members.len();
    let mut bodies: Vec<String> = Vec::new();
    for m in &a.members {
        match names.get(m.body) {
            Some(name) => bodies.push(name.clone()),
            None => return Err(EngineError::Other(format!("assembly: no body {} in the set", m.body + 1))),
        }
    }
    if let Some(k) = keep_out {
        bodies.push(k.to_string());
    }
    if bodies.is_empty() {
        warnings.push("assembly: the set has nothing to assemble".into());
        return Ok((AssemblyRecord::default(), GearAssembly::default()));
    }
    if s.model.is_stale() {
        s.refresh();
    }
    // One component per body, in order.
    let r = s.execute("component.from_bodies", &json!({"bodies": bodies}))?;
    let made: Vec<(u64, u64)> = r
        .get("components")
        .and_then(Value::as_array)
        .map(|cs| cs.iter().filter_map(|c| Some((c.get("component")?.as_u64()?, c.get("feature")?.as_u64()?))).collect())
        .unwrap_or_default();
    if made.len() != bodies.len() {
        return Err(EngineError::Other("assembly: not every body became a component".into()));
    }
    let components: Vec<u64> = made.iter().map(|m| m.0).collect();
    let transfers: Vec<u64> = made.iter().map(|m| m.1).collect();
    let mut occs = Vec::new();
    for c in &components {
        occs.push(occurrence_of(s, *c).ok_or_else(|| EngineError::Other(format!("assembly: component {c} is not placed")))?);
    }
    // Held members are grounded, the rest free to move on their joints (a design's first part
    // is grounded by itself: each is set as it should be).
    for (i, m) in a.members.iter().enumerate() {
        s.execute("occurrence.ground", &json!({"occurrence": occs[i], "grounded": m.grounded}))?;
    }
    let mut keep_out_component = None;
    if keep_out.is_some() {
        let (c, o) = (components[n], occs[n]);
        keep_out_component = Some(c);
        s.execute("occurrence.ground", &json!({"occurrence": o, "grounded": true}))?;
        if let Err(e) = s.execute("component.rename", &json!({"component": c, "name": format!("{set} keep-out")})) {
            warnings.push(format!("assembly: the keep-out component keeps its body's name ({e})"));
        }
        if let Err(e) = s.execute("appearance.assign", &json!({"components": [c.to_string()], "color": KEEP_OUT_COLOR, "opacity": KEEP_OUT_OPACITY})) {
            warnings.push(format!("assembly: the keep-out component could not be made see-through ({e})"));
        }
    }

    // A joint per member that moves: on its shaft, to the world or its carrier.
    let slides: Vec<bool> = (0..n).map(|i| kind == Kind::Rack || a.links.iter().any(|l| matches!(l, AssemblyLink::RackPinion { rack, .. } if *rack == i))).collect();
    let rigid_to: Vec<Option<usize>> = (0..n).map(|i| a.links.iter().find_map(|l| matches!(l, AssemblyLink::Rigid { b, .. } if *b == i).then(|| l.members().0))).collect();
    let mut joint: Vec<Option<u64>> = vec![None; n];
    let mut joints = Vec::new();
    for (i, m) in a.members.iter().enumerate() {
        if m.grounded {
            continue;
        }
        let (kind_name, other) = match rigid_to[i] {
            Some(on) => ("rigid", occs[on]),
            None => (if slides[i] { "slider" } else { "revolute" }, base_of(a, i).map_or(0, |c| occs[c])),
        };
        let ax = &m.axis;
        let side = |occurrence: u64| json!({"occurrence": occurrence, "point": ax.origin, "z": ax.dir});
        let what = match kind_name {
            "slider" => "slide",
            "rigid" => "fixed",
            _ => "shaft",
        };
        let name = format!("{} {what}", bodies[i]);
        match s.execute("joint.create", &json!({"type": kind_name, "a": side(other), "b": side(occs[i]), "flip": true, "name": name})) {
            Ok(r) => {
                if let Some(id) = r.get("joint").and_then(Value::as_u64) {
                    joint[i] = Some(id);
                    joints.push(id);
                }
            }
            Err(e) => warnings.push(format!("assembly: {name} could not be made ({e})")),
        }
    }

    // A motion link per mesh, b driven by a.
    let mut links = Vec::new();
    for l in &a.links {
        if matches!(l, AssemblyLink::Rigid { .. }) {
            continue;
        }
        let (x, y) = l.members();
        let (Some(ja), Some(jb)) = (joint.get(x).copied().flatten(), joint.get(y).copied().flatten()) else {
            warnings.push(format!(
                "assembly: {} and {} are not linked: one of them has no joint",
                bodies.get(x).map_or("?", String::as_str),
                bodies.get(y).map_or("?", String::as_str)
            ));
            continue;
        };
        let r = match *l {
            AssemblyLink::Gear { ra, rb, internal, reverse, .. } => {
                s.execute("joint.gear", &json!({"a": ja, "b": jb, "ra": ra, "rb": rb, "internal": internal, "reverse": reverse}))
            }
            AssemblyLink::Worm { starts, wheel_teeth, .. } => s.execute("joint.gear", &json!({"a": ja, "b": jb, "ra": f64::from(starts), "rb": f64::from(wheel_teeth)})),
            AssemblyLink::RackPinion { pitch_radius, .. } => s.execute("joint.rack_pinion", &json!({"a": ja, "b": jb, "radius": pitch_radius})),
            AssemblyLink::Rigid { .. } => continue,
        };
        match r {
            Ok(v) => {
                if let Some(i) = v.get("link").and_then(Value::as_u64) {
                    links.push(i as usize);
                }
            }
            Err(e) => warnings.push(format!("assembly: {} and {} could not be linked ({e})", bodies[x], bodies[y])),
        }
    }

    // What the joints can't show.
    match kind {
        Kind::Elliptical => warnings.push("assembly: the elliptical pair is linked at its mean ratio (1:1): its true ratio changes through the turn, so the gears drift in and out of mesh as they are driven".into()),
        Kind::CycloidalDrive => warnings.push("assembly: a cycloidal drive's disc orbits on an eccentric input the set has no body for: it turns on its own axis only, inside the grounded pin ring".into()),
        Kind::Planetary | Kind::BevelPlanetary if a.members.iter().all(|m| m.carrier.is_none()) && a.links.len() > 1 => {
            warnings.push("assembly: the set has no carrier body, so the carrier is held: the planets turn on fixed shafts".into())
        }
        _ => {}
    }

    // In the set's folder, with its features.
    if let Some(g) = group
        && let Err(e) = s.execute("timeline.move_to_group", &json!({"features": transfers, "joints": joints, "group": g}))
    {
        warnings.push(format!("assembly: the components and joints stay outside the set's folder ({e})"));
    }

    let members = a
        .members
        .iter()
        .enumerate()
        .map(|(i, m)| GearAssemblyMember { body: bodies[i].clone(), component: components[i], occurrence: occs[i], joint: joint[i], grounded: m.grounded })
        .collect();
    let record = AssemblyRecord { components: components.clone(), joints: joints.clone() };
    Ok((record, GearAssembly { members, keep_out: keep_out_component, joints, links }))
}

/// Takes a set's assembly down: its joints (with their motion links), then its components with
/// what is in them (the transfers that moved the bodies in: the bodies go back where the set's
/// features make them). Joints made by hand to the set's components go too, and are named in a
/// warning.
pub(super) fn remove(s: &mut Session, rec: &AssemblyRecord, warnings: &mut Vec<String>) -> Result<()> {
    let ours: BTreeSet<u64> = rec.joints.iter().copied().collect();
    let components: Vec<u64> = rec.components.iter().copied().filter(|c| s.doc.components.iter().any(|k| k.id == *c)).collect();
    let occs: BTreeSet<u64> = s.doc.occurrences.iter().filter(|o| components.contains(&o.component)).map(|o| o.id).collect();
    let foreign: Vec<String> = s
        .doc
        .assembly
        .joints
        .iter()
        .filter(|j| !ours.contains(&j.id) && j.a.occurrences().chain(j.b.occurrences()).any(|o| occs.contains(&o)))
        .map(|j| j.name.clone())
        .collect();
    if !foreign.is_empty() {
        warnings.push(format!("assembly: the set's components were made again, and the joints made to them are gone: {}", foreign.join(", ")));
    }
    super::super::joints::delete_joints(s.doc_mut(), &rec.joints);
    for c in components {
        s.execute("component.delete", &json!({"component": c}))?;
    }
    s.refresh();
    Ok(())
}
