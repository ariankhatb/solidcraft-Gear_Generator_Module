//! `assembly: true`: a gear set made a complete assembly from its plan's [`Assembly`]. Every
//! member's body becomes a component of its own (`component.from_bodies`), the keep-out
//! (clearance) body a grounded, see-through component "<set> keep-out"; each member that moves
//! gets a revolute joint on its shaft (a slider for a rack, a rigid joint for a spline's hub on
//! its shaft) to the world, or to its carrier; and each mesh a motion link (`joint.gear`,
//! `joint.rack_pinion`) with the plan's ratio and direction, so driving one gear turns the
//! others. Made inside `gear.create`/`gear.update`, so in their undo step, and moved into the
//! set's timeline folder.
//!
//! An update keeps the assembly where it stands ([`sync`]): members are matched by their bodies'
//! names (which `gear.update` keeps), so a member's component keeps its id and name (its body is
//! regenerated inside it: the Body Transfer that moved it in finds it again by name), its joint
//! keeps its id and value and has its two frames moved onto the new shaft, and each link keeps
//! its place and is given the new ratio. Only members that appear get a component and a joint,
//! and only members that are gone lose theirs. Joints the user made to the set's components, and
//! anything else built on them, are left alone.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use solidcore_record::Vec3;
use solidcraft_doc::Mat;
use solidcraft_doc::joints::Snap;
use solidcraft_gear::axes::Axis;
use solidcraft_gear::plan::{Assembly, AssemblyLink, Kind, Plan};

use crate::schema::gear::{GearAssembly, GearAssemblyMember};
use crate::{EngineError, Result, Session};

/// One member as made: its body, component and joint, and how world points became each joint
/// side's component coordinates when the joint was made (so an update can move the joint's
/// frames onto a moved shaft, whatever the parts' poses are now).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct MemberRecord {
    pub(super) body: String,
    pub(super) component: u64,
    #[serde(default)]
    pub(super) joint: Option<u64>,
    /// The joint's type as joint.create takes it (revolute, slider, rigid).
    #[serde(default)]
    pub(super) kind: String,
    /// The occurrence the joint's side a is on (0: the world).
    #[serde(default)]
    pub(super) base: u64,
    /// World → side a's and side b's component coordinates, as they were placed when made.
    #[serde(default = "identity")]
    pub(super) a_inv: Mat,
    #[serde(default = "identity")]
    pub(super) b_inv: Mat,
    /// The shaft it turned about when last made or updated (origin, direction; world): a member
    /// whose body is named afresh is found again by its shaft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) axis: Option<[[f64; 3]; 2]>,
    /// The generated shaft it is (an index into `Plan::shafts`, the dialog's `shafts[].index`):
    /// a member keeps its component and joint by it when its body is named afresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) shaft: Option<usize>,
}

fn identity() -> Mat {
    solidcraft_doc::IDENTITY
}

/// What a set's assembly was made of, kept with the set's record.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct AssemblyRecord {
    /// The members, in the order of the set's bodies.
    #[serde(default)]
    pub(super) members: Vec<MemberRecord>,
    /// The keep-out body's name and its component.
    #[serde(default)]
    pub(super) keep_out: Option<(String, u64)>,
}

impl AssemblyRecord {
    /// Whether `component` is one of the set's (a member's or the keep-out).
    pub(super) fn holds(&self, component: u64) -> bool {
        self.members.iter().any(|m| m.component == component) || self.keep_out.as_ref().is_some_and(|k| k.1 == component)
    }
    fn components(&self) -> Vec<u64> {
        self.members.iter().map(|m| m.component).chain(self.keep_out.as_ref().map(|k| k.1)).collect()
    }
    fn joints(&self) -> Vec<u64> {
        self.members.iter().filter_map(|m| m.joint).collect()
    }
}

/// Whether the request asks for the set as an assembly.
pub(super) fn wanted(request: &Value) -> bool {
    request.get("assembly").and_then(Value::as_bool) == Some(true)
}

/// The member the request drives the assembly from (`assembly_driver`, 1-based), 0-based.
pub(super) fn driver(request: &Value) -> Option<usize> {
    request.get("assembly_driver").and_then(Value::as_u64).filter(|d| *d >= 1).and_then(|d| usize::try_from(d - 1).ok())
}

/// The keep-out component's colour and opacity.
const KEEP_OUT_COLOR: &str = "#5b9bd5";
const KEEP_OUT_OPACITY: f64 = 0.25;

fn occurrence_of(s: &Session, component: u64) -> Option<u64> {
    s.doc.occurrence_of(component).map(|o| o.id)
}

fn component_exists(s: &Session, c: u64) -> bool {
    s.doc.components.iter().any(|k| k.id == c)
}

fn joint_exists(s: &Session, j: u64) -> bool {
    s.doc.assembly.joints.iter().any(|k| k.id == j)
}

/// World → an occurrence's component coordinates, where it is placed now (identity: the world).
fn world_to_local(s: &Session, occurrence: u64) -> Mat {
    if occurrence == 0 {
        return identity();
    }
    let t = solidcraft_doc::joints::occurrence_world(&s.doc, occurrence);
    solidcraft_doc::mat_inverse(&t).unwrap_or_else(identity)
}

fn v3(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
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

/// Whether two shafts (origin, direction) are one line.
fn same_shaft(a: &[[f64; 3]; 2], b: &Axis) -> bool {
    coaxial(&Axis { origin: a[0], dir: a[1] }, b)
}

/// Makes the assembly of a set just built, or brings the one it had (`old`) up to date with it in
/// place: `names` are its gear bodies' names in plan order, `keep_out` its clearance body's
/// name, `set` its name, `group` its timeline folder, `driver` the member the user drives
/// (0-based; `None`: the plan's own driver, member 1).
///
/// The whole system is one tree of joints: one revolute joint per shaft (the member of a compound
/// cluster nearest the driver; the others rigid on it), and a motion link per mesh, each driven
/// from the end nearer the driver. Loops of meshes whose ratios don't agree
/// (`Plan::assembly_issues`) are left out and named in the warnings.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync(
    s: &mut Session,
    plan: &Plan,
    kind: Kind,
    set: &str,
    names: &[String],
    keep_out: Option<&str>,
    group: Option<u64>,
    old: Option<AssemblyRecord>,
    driver: Option<usize>,
    warnings: &mut Vec<String>,
) -> Result<(AssemblyRecord, GearAssembly)> {
    // Driven from the member asked for: the tree turned to point away from it.
    let rerooted;
    let a: &Assembly = match driver.filter(|d| *d > 0 && *d < plan.assembly.members.len()) {
        Some(d) => {
            rerooted = plan.assembly.rooted(d);
            &rerooted
        }
        None => &plan.assembly,
    };
    if driver.is_some_and(|d| d >= plan.assembly.members.len()) {
        warnings.push(format!("assembly: there is no member {} to drive: member 1 drives", driver.map_or(0, |d| d + 1)));
    }
    let issues: Vec<String> = plan.assembly_issues.iter().map(|i| format!("assembly: {}", i.message)).collect();
    warnings.extend(issues.iter().cloned());
    let n = a.members.len();
    let mut bodies: Vec<String> = Vec::new();
    for m in &a.members {
        match names.get(m.body) {
            Some(name) => bodies.push(name.clone()),
            None => return Err(EngineError::Other(format!("assembly: no body {} in the set", m.body + 1))),
        }
    }
    if s.model.is_stale() {
        s.refresh();
    }
    let old = old.unwrap_or_default();

    // What was there and is still wanted: by body name (an update keeps the names), then by
    // place among the members for a body named afresh (a set rebuilt with another member
    // count names its bodies from the plan); what is gone comes down.
    let mut kept: Vec<Option<MemberRecord>> = vec![None; n];
    let mut gone_components = Vec::new();
    let mut gone_joints = Vec::new();
    let mut left: Vec<Option<MemberRecord>> = old.members.into_iter().map(Some).collect();
    for slot in &mut left {
        let Some(m) = slot.as_ref() else { continue };
        if !component_exists(s, m.component) {
            gone_joints.extend(m.joint);
            *slot = None;
            continue;
        }
        if let Some(i) = bodies.iter().position(|b| *b == m.body).filter(|i| kept[*i].is_none()) {
            kept[i] = slot.take();
        }
    }
    // Bodies named afresh move into the component of the member they take the place of: the one
    // on the same shaft (a gear added or taken away elsewhere in the system leaves the others
    // where they are), else the one at the same place among the members.
    let mut rehoused: Vec<(usize, u64)> = Vec::new();
    let mut claim: Vec<Option<usize>> = vec![None; left.len()];
    // The shaft each new member is (`Plan::shafts`), the dialog's stable index for it.
    let shaft_of = |i: usize| plan.shafts.iter().position(|sh| sh.body == a.members[i].body);
    for i in (0..n).filter(|i| kept[*i].is_none()) {
        let Some(k_shaft) = shaft_of(i) else { continue };
        let same = left.iter().enumerate().position(|(k, slot)| claim[k].is_none() && slot.as_ref().is_some_and(|m| !bodies.contains(&m.body) && m.shaft == Some(k_shaft)));
        if let Some(k) = same {
            claim[k] = Some(i);
        }
    }
    for i in 0..n {
        if kept[i].is_some() || claim.contains(&Some(i)) {
            continue;
        }
        let on_shaft = left.iter().enumerate().position(|(k, slot)| {
            claim[k].is_none() && slot.as_ref().is_some_and(|m| !bodies.contains(&m.body) && m.axis.as_ref().is_some_and(|x| same_shaft(x, &a.members[i].axis)))
        });
        if let Some(k) = on_shaft {
            claim[k] = Some(i);
        }
    }
    for i in 0..n {
        if kept[i].is_none() && !claim.contains(&Some(i)) && i < left.len() && claim[i].is_none() && left[i].as_ref().is_some_and(|m| !bodies.contains(&m.body)) {
            claim[i] = Some(i);
        }
    }
    for (k, slot) in left.iter_mut().enumerate() {
        if let Some(i) = claim[k] {
            if let Some(m) = slot.take() {
                let (home, old_body) = (m.component, m.body.clone());
                rehoused.push((i, home));
                kept[i] = Some(MemberRecord { body: bodies[i].clone(), ..m });
                // The transfer that moved the old body in has nothing to move now.
                let stale: Vec<u64> = s
                    .doc
                    .features
                    .iter()
                    .filter(|f| matches!(&f.kind, solidcraft_doc::FeatureKind::BodyTransfer { to, bodies, .. } if *to == home && bodies.iter().any(|b| *b == old_body.as_str())))
                    .map(|f| f.id)
                    .collect();
                if !stale.is_empty() {
                    s.execute("timeline.delete", &json!({"features": stale}))?;
                }
            }
        }
    }
    for m in left.into_iter().flatten() {
        gone_joints.extend(m.joint);
        gone_components.push(m.component);
    }
    let mut keep_out_kept = None;
    if let Some((body, c)) = old.keep_out {
        if keep_out == Some(body.as_str()) && component_exists(s, c) {
            keep_out_kept = Some(c);
        } else {
            gone_components.push(c);
        }
    }
    // A member that is now held loses its joint.
    for (i, m) in a.members.iter().enumerate() {
        if let Some(k) = kept[i].as_mut()
            && m.grounded
            && let Some(j) = k.joint.take()
        {
            gone_joints.push(j);
        }
    }
    if !gone_joints.is_empty() {
        super::super::joints::delete_joints(s.doc_mut(), &gone_joints);
    }
    for c in gone_components {
        if component_exists(s, c) {
            s.execute("component.delete", &json!({"component": c}))?;
        }
    }

    // A component for each body that has none yet.
    let mut fresh: Vec<String> = (0..n).filter(|i| kept[*i].is_none()).map(|i| bodies[i].clone()).collect();
    let new_keep_out = keep_out.filter(|_| keep_out_kept.is_none()).map(str::to_string);
    fresh.extend(new_keep_out.clone());
    let mut transfers = Vec::new();
    let mut made_for: Vec<(String, u64)> = Vec::new();
    if s.model.is_stale() {
        s.refresh();
    }
    for (i, c) in &rehoused {
        match s.execute("component.move_bodies", &json!({"bodies": [bodies[*i].clone()], "component": c})) {
            Ok(r) => transfers.extend(r.get("features").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64)),
            Err(e) => return Err(EngineError::Other(format!("assembly: {} could not go into its component ({e})", bodies[*i]))),
        }
    }
    if !fresh.is_empty() {
        if s.model.is_stale() {
            s.refresh();
        }
        let r = s.execute("component.from_bodies", &json!({"bodies": fresh}))?;
        let made: Vec<(u64, u64)> = r
            .get("components")
            .and_then(Value::as_array)
            .map(|cs| cs.iter().filter_map(|c| Some((c.get("component")?.as_u64()?, c.get("feature")?.as_u64()?))).collect())
            .unwrap_or_default();
        if made.len() != fresh.len() {
            return Err(EngineError::Other("assembly: not every body became a component".into()));
        }
        for (name, (c, f)) in fresh.iter().zip(made) {
            made_for.push((name.clone(), c));
            transfers.push(f);
        }
    }
    let made_component = |body: &str| made_for.iter().find(|(b, _)| b == body).map(|(_, c)| *c);
    let mut components = Vec::new();
    let mut occs = Vec::new();
    for i in 0..n {
        let c = kept[i].as_ref().map(|k| k.component).or_else(|| made_component(&bodies[i]));
        let c = c.ok_or_else(|| EngineError::Other(format!("assembly: {} has no component", bodies[i])))?;
        components.push(c);
        occs.push(occurrence_of(s, c).ok_or_else(|| EngineError::Other(format!("assembly: component {c} is not placed")))?);
    }
    // New members are grounded or freed as the plan says (a design's first part is grounded by
    // itself); kept ones keep what they are (the user may have grounded one), unless now held.
    for (i, m) in a.members.iter().enumerate() {
        if kept[i].is_none() || m.grounded {
            s.execute("occurrence.ground", &json!({"occurrence": occs[i], "grounded": m.grounded}))?;
        }
    }
    let keep_out_component = match (keep_out_kept, &new_keep_out) {
        (Some(c), _) => Some(c),
        (None, Some(body)) => {
            let c = made_component(body);
            if let Some(c) = c {
                let o = occurrence_of(s, c).unwrap_or(0);
                s.execute("occurrence.ground", &json!({"occurrence": o, "grounded": true}))?;
                if let Err(e) = s.execute("component.rename", &json!({"component": c, "name": format!("{set} keep-out")})) {
                    warnings.push(format!("assembly: the keep-out component keeps its body's name ({e})"));
                }
                if let Err(e) = s.execute("appearance.assign", &json!({"components": [c.to_string()], "color": KEEP_OUT_COLOR, "opacity": KEEP_OUT_OPACITY})) {
                    warnings.push(format!("assembly: the keep-out component could not be made see-through ({e})"));
                }
            }
            c
        }
        (None, None) => None,
    };

    // A joint per member that moves: on its shaft, to the world or its carrier. A kept joint of
    // the same type on the same side keeps its id, name and value, its frames moved onto the
    // shaft as it is now.
    let slides: Vec<bool> = (0..n).map(|i| kind == Kind::Rack || a.links.iter().any(|l| matches!(l, AssemblyLink::RackPinion { rack, .. } if *rack == i))).collect();
    let rigid_to: Vec<Option<usize>> = (0..n).map(|i| a.links.iter().find_map(|l| matches!(l, AssemblyLink::Rigid { b, .. } if *b == i).then(|| l.members().0))).collect();
    let mut records: Vec<MemberRecord> = Vec::new();
    let mut new_joints = Vec::new();
    let mut moved = false;
    for (i, m) in a.members.iter().enumerate() {
        let mut rec = kept[i].clone().unwrap_or(MemberRecord {
            body: bodies[i].clone(),
            component: components[i],
            joint: None,
            kind: String::new(),
            base: 0,
            a_inv: identity(),
            b_inv: identity(),
            axis: None,
            shaft: None,
        });
        rec.body = bodies[i].clone();
        rec.component = components[i];
        rec.axis = Some([m.axis.origin, m.axis.dir]);
        rec.shaft = plan.shafts.iter().position(|sh| sh.body == m.body);
        if m.grounded {
            rec.joint = None;
            records.push(rec);
            continue;
        }
        let (kind_name, other) = match rigid_to[i] {
            Some(on) => ("rigid", occs[on]),
            None => (if slides[i] { "slider" } else { "revolute" }, base_of(a, i).map_or(0, |c| occs[c])),
        };
        let ax = &m.axis;
        let same = rec.joint.filter(|j| joint_exists(s, *j)).filter(|_| rec.kind == kind_name && rec.base == other);
        if let Some(j) = same {
            // Both frames onto the shaft, each in its side's own coordinates.
            let at = |inv: &Mat| Snap::Point { point: solidcraft_doc::apply_point(inv, v3(ax.origin)), z: Some(solidcraft_doc::apply_vector(inv, v3(ax.dir))), x: None };
            let (sa, sb) = (at(&rec.a_inv), at(&rec.b_inv));
            if let Some(joint) = s.doc_mut().assembly.joints.iter_mut().find(|k| k.id == j)
                && (joint.a.snap != sa || joint.b.snap != sb)
            {
                joint.a.snap = sa;
                joint.b.snap = sb;
                moved = true;
            }
            records.push(rec);
            continue;
        }
        if let Some(j) = rec.joint.take()
            && joint_exists(s, j)
        {
            super::super::joints::delete_joints(s.doc_mut(), &[j]);
        }
        let (a_inv, b_inv) = (world_to_local(s, other), world_to_local(s, occs[i]));
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
                    rec.joint = Some(id);
                    new_joints.push(id);
                }
            }
            Err(e) => warnings.push(format!("assembly: {name} could not be made ({e})")),
        }
        rec.kind = kind_name.to_string();
        rec.base = other;
        rec.a_inv = a_inv;
        rec.b_inv = b_inv;
        records.push(rec);
    }
    if moved {
        let _ = super::super::joints::resolve(s);
    }

    // A motion link per mesh, b driven by a: one between the same two joints keeps its place
    // and takes the new ratio; ours that no mesh wants any more go.
    // A member rigid on another (a gear of a cluster built as a body of its own) turns with the
    // member that holds the cluster's revolute joint: its meshes are linked to that joint.
    let own_joint: Vec<Option<u64>> = records.iter().map(|r| r.joint).collect();
    let turning_member = |mut i: usize| {
        for _ in 0..n {
            match rigid_to.get(i).copied().flatten() {
                Some(on) if on != i => i = on,
                _ => break,
            }
        }
        i
    };
    let joint: Vec<Option<u64>> = (0..n).map(|i| own_joint[turning_member(i)]).collect();
    let ours: BTreeSet<u64> = own_joint.iter().flatten().copied().collect();
    let wanted_pairs: Vec<(u64, u64)> = a
        .links
        .iter()
        .filter(|l| !matches!(l, AssemblyLink::Rigid { .. }))
        .filter_map(|l| {
            let (x, y) = l.members();
            Some((joint.get(x).copied().flatten()?, joint.get(y).copied().flatten()?))
        })
        .collect();
    // A link between our joints that the tree doesn't have (no mesh there now, or one driven
    // from its other end since the driver changed) goes; the others are redefined in place.
    s.doc_mut().assembly.links.retain(|l| !(ours.contains(&l.a) && ours.contains(&l.b)) || wanted_pairs.contains(&(l.a, l.b)));
    // Members the motion has reached, from the driver on: a rack and pinion is driven from the
    // end reached first.
    let mut reached = vec![false; n];
    if let Some(r) = driver.filter(|d| *d < n).or(Some(0)).filter(|_| n > 0) {
        reached[r] = true;
    }
    let mut links_made = Vec::new();
    for l in &a.links {
        let (x0, y0) = l.members();
        let rack_first = matches!(l, AssemblyLink::RackPinion { .. }) && reached.get(y0).copied().unwrap_or(false) && !reached.get(x0).copied().unwrap_or(false);
        for e in [x0, y0] {
            if let Some(r) = reached.get_mut(e) {
                *r = true;
            }
        }
        if matches!(l, AssemblyLink::Rigid { .. }) {
            continue;
        }
        let (x, y) = if rack_first { (y0, x0) } else { (x0, y0) };
        let (Some(ja), Some(jb)) = (joint.get(x).copied().flatten(), joint.get(y).copied().flatten()) else {
            warnings.push(format!(
                "assembly: {} and {} are not linked: one of them has no joint",
                bodies.get(x).map_or("?", String::as_str),
                bodies.get(y).map_or("?", String::as_str)
            ));
            continue;
        };
        let mut p = match *l {
            AssemblyLink::Gear { ra, rb, internal, reverse, .. } => json!({"a": ja, "b": jb, "ra": ra, "rb": rb, "internal": internal, "reverse": reverse}),
            AssemblyLink::Worm { starts, wheel_teeth, .. } => json!({"a": ja, "b": jb, "ra": f64::from(starts), "rb": f64::from(wheel_teeth)}),
            AssemblyLink::RackPinion { pitch_radius, .. } => json!({"a": ja, "b": jb, "radius": pitch_radius}),
            AssemblyLink::Rigid { .. } => continue,
        };
        if let Some(i) = s.doc.assembly.links.iter().position(|k| k.a == ja && k.b == jb) {
            p["link"] = json!(i);
        }
        let cmd = if matches!(l, AssemblyLink::RackPinion { .. }) { "joint.rack_pinion" } else { "joint.gear" };
        match s.execute(cmd, &p) {
            Ok(_) => links_made.push((ja, jb)),
            Err(e) => warnings.push(format!("assembly: {} and {} could not be linked ({e})", bodies[x], bodies[y])),
        }
    }
    let links: Vec<usize> = links_made.iter().filter_map(|(ja, jb)| s.doc.assembly.links.iter().position(|k| k.a == *ja && k.b == *jb)).collect();

    // What the joints can't show.
    match kind {
        Kind::Elliptical => warnings.push("assembly: the elliptical pair is linked at its mean ratio (1:1): its true ratio changes through the turn, so the gears drift in and out of mesh as they are driven".into()),
        Kind::CycloidalDrive => warnings.push("assembly: a cycloidal drive's disc orbits on an eccentric input the set has no body for: it turns on its own axis only, inside the grounded pin ring".into()),
        Kind::Planetary | Kind::BevelPlanetary if a.members.iter().all(|m| m.carrier.is_none()) && a.links.len() > 1 => {
            warnings.push("assembly: the set has no carrier body, so the carrier is held: the planets turn on fixed shafts".into())
        }
        _ => {}
    }

    // What is new goes in the set's folder, with its features.
    if let Some(g) = group
        && (!transfers.is_empty() || !new_joints.is_empty())
        && let Err(e) = s.execute("timeline.move_to_group", &json!({"features": transfers, "joints": new_joints, "group": g}))
    {
        warnings.push(format!("assembly: the components and joints stay outside the set's folder ({e})"));
    }

    let members = a
        .members
        .iter()
        .enumerate()
        .map(|(i, m)| GearAssemblyMember { body: bodies[i].clone(), component: components[i], occurrence: occs[i], joint: records[i].joint, grounded: m.grounded, axis: [m.axis.origin, m.axis.dir] })
        .collect();
    let keep_out_record = keep_out_component.and_then(|c| keep_out.map(|b| (b.to_string(), c)));
    let joints = records.iter().filter_map(|r| r.joint).collect();
    let record = AssemblyRecord { members: records, keep_out: keep_out_record };
    Ok((record, GearAssembly { members, keep_out: keep_out_component, joints, links, driver: driver.filter(|d| *d < n).unwrap_or(0) + 1, issues }))
}

/// Takes a set's assembly down (`assembly` switched off on an update): its joints (with their
/// motion links), then its components with what is in them (the transfers that moved the bodies
/// in: the bodies go back where the set's features make them). Joints made by hand to the set's
/// components go with them, and are named in a warning.
pub(super) fn remove(s: &mut Session, rec: &AssemblyRecord, warnings: &mut Vec<String>) -> Result<()> {
    let mine = rec.joints();
    let ours: BTreeSet<u64> = mine.iter().copied().collect();
    let components: Vec<u64> = rec.components().into_iter().filter(|c| component_exists(s, *c)).collect();
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
        warnings.push(format!("assembly: the set is no longer an assembly, and the joints made to its components are gone: {}", foreign.join(", ")));
    }
    super::super::joints::delete_joints(s.doc_mut(), &mine);
    for c in components {
        s.execute("component.delete", &json!({"component": c}))?;
    }
    s.refresh();
    Ok(())
}
