//! The Gears feature ([`FeatureKind::Gears`]): a whole gear set or drivetrain built in one step.
//!
//! Each evaluation reads the set again from the design as it stands before the feature (its
//! sketches, through the planner the app installs: [`set_planner`]), plans it with
//! `solidcraft_gear`, and builds every body of the plan in memory with the kernel calls the
//! Extrude, Sweep, Loft, Revolve and Coil features use, each section and profile a region of its
//! own (no sketch is made): every step of a body on the body it makes, by the kernel's boolean
//! (bores, keyways, hubs, trims), then each rigid member's bodies unioned
//! ([`solidcraft_gear::plan::solid`]). The members (and the keep-out body) are new bodies under
//! the feature's body names, or tools that join, cut or intersect its targets; the keep-out body
//! can cut other bodies instead (`keep_out_cuts`: a housing's cavity in the same step).
//!
//! The steps inside run unchecked, as the generator's timeline features were built; each body
//! the feature keeps is then checked once (unless the design's checks are off), and a body the
//! kernel's check refuses is kept with a warning saying why: SMLib refuses the curved trims of
//! flat bevels ("Bad CurveCusp", a warning SMLib itself calls one) and some hypoid edges.

use std::sync::OnceLock;

use serde_json::Value;
use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_gear::geom::{Outline, Region, Seg};
use solidcraft_gear::plan::{Body as Planned, Frame, Op, Plan, Step, solid};
use solidcraft_kernel::{self as kernel, Body, BoolOp};

use super::{ModelBody, ModelState};
use crate::lineage::Traced;
use crate::{DocError, Document, Feature, FeatureKind, Operation, Result};

/// What a planner reads a set from: the design, the model before the feature, the set's
/// parameters and its sketches (feature ids).
pub struct GearsInput<'a> {
    pub doc: &'a Document,
    pub state: &'a ModelState,
    pub params: &'a Value,
    pub sketches: &'a [u64],
}

/// Plans a set from its parameters and what they name in the design (the app's Gear Generator:
/// the same reading `gear.create` does).
pub type GearsPlanner = fn(&GearsInput<'_>) -> std::result::Result<Plan, String>;

static PLANNER: OnceLock<GearsPlanner> = OnceLock::new();

/// Installs the planner Gears features are planned with (once; later calls change nothing).
pub fn set_planner(planner: GearsPlanner) {
    let _ = PLANNER.set(planner);
}

/// The Gears feature `f` on the model `st`.
pub(super) fn eval(doc: &Document, f: &Feature, st: &mut ModelState, warning: &mut Option<String>) -> Result<()> {
    let FeatureKind::Gears { params, sketches, operation, targets, keep_out_cuts } = &f.kind else {
        return Err(DocError::Invalid("not a gear set".into()));
    };
    let planner = PLANNER.get().ok_or_else(|| DocError::Invalid("gear sets are planned by the Gear Generator, which is not running here".into()))?;
    let ids: Vec<u64> = sketches.iter().map(|s| s.0).collect();
    let planned = planner(&GearsInput { doc, state: st, params, sketches: &ids });
    // A sketch the set reads that isn't there: the planner's error names the picks it held (the
    // item, its slot, the sketch); else the sketch.
    if let Some(gone) = ids.iter().find(|id| st.sketch(**id).is_none()) {
        return Err(match planned {
            Err(e) => DocError::Invalid(e),
            Ok(_) => {
                let name = doc.feature(*gone).map_or_else(|| format!("sketch {gone}"), |f| format!("sketch '{}'", f.name));
                DocError::Unknown(format!("{name} (it must come earlier in the timeline)"))
            }
        });
    }
    let plan = planned.map_err(DocError::Invalid)?;
    let checks = super::EVAL_VALIDATION.get() != kernel::Validation::Off;
    let mut notes: Vec<String> = Vec::new();
    // Everything inside runs unchecked; what the feature keeps is checked below.
    let (built, _) = kernel::validating(kernel::Validation::Off, || build(&plan, &mut notes));
    let (members, keep_out) = built?;
    let cut_by_keep_out = !keep_out_cuts.is_empty();
    if cut_by_keep_out && keep_out.is_none() {
        notes.push("the set has no keep-out body to cut the housing with: ask for one (clearance body)".into());
    }
    // Names: the feature's own (kept across edits, so components and joints stay on their
    // bodies), else the generator's.
    let mut named: Vec<(String, Body)> = Vec::new();
    let outputs = members.into_iter().chain(keep_out.clone().filter(|_| !cut_by_keep_out));
    // A set that keeps its bodies' generator names (`body_keys`, beside its body names) names
    // each body by its own, whatever its place in this plan; others by place.
    let keys: Option<Vec<&str>> = params.get("body_keys").and_then(Value::as_array).map(|a| a.iter().map(|k| k.as_str().unwrap_or("")).collect());
    for (k, (generated, body)) in outputs.enumerate() {
        let slot = match &keys {
            Some(keys) => keys.iter().position(|key| *key == generated),
            None => Some(k),
        };
        let name = match slot.and_then(|k| f.body_names.get(k)).filter(|n| !n.is_empty() && st.body(n).is_none()) {
            Some(n) => n.clone(),
            None => super::unique_body_name(st, &generated),
        };
        named.push((name, body));
    }
    let before: Vec<Body> = st.bodies.iter().map(|b| b.body.clone()).collect();
    let (applied, _) = kernel::validating(kernel::Validation::Off, || -> Result<()> {
        if *operation == Operation::NewBody {
            for (name, body) in &named {
                st.bodies.push(ModelBody::new(name.clone(), body.clone(), f.id));
            }
        } else {
            let tools = named.iter().map(|(_, b)| Traced::unnamed(b.clone())).collect();
            super::apply_traced(doc, st, f, tools, *operation, &crate::plain(targets))?;
        }
        if let (true, Some((_, ko))) = (cut_by_keep_out, &keep_out) {
            super::apply_traced(doc, st, f, vec![Traced::unnamed(ko.clone())], Operation::Cut, &crate::plain(keep_out_cuts))?;
        }
        Ok(())
    });
    applied?;
    if checks {
        for b in st.bodies.iter().filter(|b| !before.iter().any(|old| old.same_shape(&b.body))) {
            if let Err(e) = kernel::check_kept(&b.body) {
                notes.push(refused_note(&b.name, &e.to_string()));
            }
        }
    }
    if !notes.is_empty() {
        super::append_warning(warning, notes.join("; "));
    }
    Ok(())
}

/// What a body the kernel's check refuses is told as: the body and the check's reason, and what
/// is known of it.
fn refused_note(name: &str, why: &str) -> String {
    let known = if why.contains("CurveCusp") {
        " (SMLib's curvature warning on a flat bevel's curved trim: the body is kept as built)"
    } else if why.contains("proto edge") || why.contains("proto-edge") {
        " (SMLib's edge build on a hypoid's teeth: the body is kept as built)"
    } else {
        ""
    };
    format!("{name}: the kernel's check refuses it: {why}{known}")
}

/// Builds the plan: each rigid member (its generator name and body) and the keep-out body. A gear
/// that can't be made fails the set; a step that only changes a gear (a hub, bore or trim) is left
/// out with a note, and so is the keep-out body.
#[allow(clippy::type_complexity)]
fn build(plan: &Plan, notes: &mut Vec<String>) -> Result<(Vec<(String, Body)>, Option<(String, Body)>)> {
    let mut gears: Vec<Body> = Vec::with_capacity(plan.bodies.len());
    for b in &plan.bodies {
        gears.push(build_body(b, notes).map_err(|e| DocError::Invalid(format!("{}: {e}", b.name)))?);
    }
    let mut members = Vec::new();
    for r in solid::members(plan) {
        let parts: Vec<&Body> = r.bodies.iter().filter_map(|k| gears.get(*k)).collect();
        let body = match parts[..] {
            [one] => one.clone(),
            _ => kernel::union_all(&parts).map_err(|e| DocError::Invalid(format!("{}: its gears could not be joined ({e})", r.name)))?,
        };
        members.push((r.name, body));
    }
    let keep_out = match &plan.clearance {
        Some(c) => match build_body(c, notes) {
            Ok(b) => Some((c.name.clone(), b)),
            Err(e) => {
                notes.push(format!("{}: the keep-out body could not be made ({e}); the gears are made without it", c.name));
                None
            }
        },
        None => None,
    };
    Ok((members, keep_out))
}

/// One planned body: its steps in order, each on what the ones before made.
fn build_body(b: &Planned, notes: &mut Vec<String>) -> Result<Body> {
    let mut made: Option<Body> = None;
    for (i, step) in b.steps.iter().enumerate() {
        let op = solid::op_of(step);
        let tries = solid::attempts(step);
        let mut failed = None;
        for (k, (attempt, nudge)) in tries.iter().enumerate() {
            let tool = match (solid::mirrored_half(b, i, attempt), &made) {
                (Some(z), Some(lower)) => {
                    let plane = Plane { origin: v3(b.frame.world([0.0, 0.0, z])), x: v3(b.frame.x), y: v3(b.frame.y()) };
                    kernel::transform_matrix(lower, super::mirror_matrix(&plane)).map_err(DocError::from)
                }
                _ => tool(&b.frame, attempt, *nudge),
            };
            match tool.and_then(|t| combine(made.as_ref(), t, op)) {
                Ok(body) => {
                    if k > 0
                        && let (Step::Loft { sections: all, .. }, Step::Loft { sections: used, .. }) = (step, attempt)
                    {
                        notes.push(format!("{}: the loft through all {} sections failed; built through {} of them", b.name, all.len(), used.len()));
                    }
                    made = Some(body);
                    failed = None;
                    break;
                }
                Err(e) => failed = Some(e),
            }
        }
        if let Some(e) = failed {
            if op == Op::New || made.is_none() {
                return Err(e);
            }
            if solid::trims(step) {
                notes.push(format!("{}: the back and front cones could not be trimmed ({e}); the teeth run on a little past them", b.name));
            } else {
                notes.push(format!("{}: step {} ({}) could not be made ({e}); it is left out", b.name, i + 1, op_name(op)));
            }
        }
    }
    made.ok_or_else(|| DocError::Invalid("the plan has no steps".into()))
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::New => "new",
        Op::Join => "join",
        Op::Cut => "cut",
        Op::Intersect => "intersect",
    }
}

/// A step's tool on what is made so far (`None`: nothing yet), by the kernel's boolean.
fn combine(made: Option<&Body>, tool: Body, op: Op) -> Result<Body> {
    let Some(made) = made else {
        return if op == Op::New { Ok(tool) } else { Err(DocError::Invalid(format!("a {} step before the body is made", op_name(op)))) };
    };
    let bop = match op {
        Op::New | Op::Join => BoolOp::Union,
        Op::Cut => BoolOp::Cut,
        Op::Intersect => BoolOp::Intersect,
    };
    kernel::boolean(made, &tool, bop)?.ok_or_else(|| DocError::Invalid(format!("the {} leaves nothing", op_name(op))))
}

fn v3(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

fn v2(p: solidcraft_gear::V2) -> Vec2 {
    Vec2::new(p.x, p.y)
}

/// The plane z = `z` of a frame (its x and y).
fn plane_at(f: &Frame, z: f64) -> Plane {
    Plane { origin: v3(f.world([0.0, 0.0, z])), x: v3(f.x), y: v3(f.y()) }
}

/// The plane through a frame's axis: x radial, y along the axis (a revolve's and a coil's).
fn axial_plane(f: &Frame) -> Plane {
    Plane { origin: v3(f.origin), x: v3(f.x), y: v3(f.z) }
}

/// An outline as a kernel loop: lines and arcs as they are, a fit-point spline as the cubic
/// pieces a sketch's spline through the same points is ([`solidcraft_sketch::fit_beziers`]).
fn outline_loop(o: &Outline) -> Loop2 {
    if let [Seg::Circle(c, r)] = &o.segs[..] {
        return Loop2::circle(v2(*c), *r);
    }
    let mut segs = Vec::with_capacity(o.segs.len());
    for s in &o.segs {
        match s {
            Seg::Line(a, b) => segs.push(Seg2::Line { a: v2(*a), b: v2(*b) }),
            Seg::Arc { c, a, b, ccw } => {
                let (ta, tb) = ((*a - *c).angle(), (*b - *c).angle());
                let tau = std::f64::consts::TAU;
                let turn = |d: f64| if d <= 1e-12 { d + tau } else { d };
                let sweep = if *ccw { turn((tb - ta).rem_euclid(tau)) } else { -turn((ta - tb).rem_euclid(tau)) };
                segs.push(Seg2::Arc { center: v2(*c), radius: a.dist(*c), start: ta, sweep });
            }
            Seg::Spline(pts) => {
                let p: Vec<Vec2> = pts.iter().map(|q| v2(*q)).collect();
                segs.extend(solidcraft_sketch::fit_beziers(&p).into_iter().map(|[p0, p1, p2, p3]| Seg2::Cubic { p0, p1, p2, p3 }));
            }
            Seg::Circle(c, r) => segs.extend(Loop2::circle(v2(*c), *r).segs),
        }
    }
    Loop2 { segs }
}

/// A region as kernel regions: each outer outline with the holes inside it.
fn regions_of(r: &Region) -> Vec<Region2> {
    solid::region_parts(r).into_iter().map(|(o, holes)| Region2 { outer: outline_loop(&r.outer[o]), holes: holes.iter().map(|h| outline_loop(&r.holes[*h])).collect() }).collect()
}

/// One body of several (a region of more than one outline): their union.
fn one(bodies: Vec<Body>) -> Result<Body> {
    if bodies.len() > 1 {
        return Ok(kernel::union_all(&bodies.iter().collect::<Vec<_>>())?);
    }
    bodies.into_iter().next().ok_or_else(|| DocError::Invalid("the step makes nothing".into()))
}

/// The tool a step makes in frame `f` (a revolved blank moved `nudge` along the axis).
fn tool(f: &Frame, step: &Step, nudge: f64) -> Result<Body> {
    // Each section is the first one scaled and turned (a taper's), on its plane along the axis.
    let taper = |o: &Outline, z0: f64, height: f64, twist: f64, scale: f64, rim: Option<f64>| -> Result<Body> {
        let n = if twist.abs() <= 1e-12 { 2 } else { solidcraft_gear::plan::taper_sections(o, twist, scale) };
        let sections: Vec<(Plane, Region2)> = (0..n)
            .map(|i| {
                let t = i as f64 / (n - 1) as f64;
                let k = 1.0 + (scale - 1.0) * t;
                let section = outline_loop(&o.scaled(k).rotated(solidcraft_gear::plan::taper_turn(twist, scale, k)));
                let region = match rim {
                    Some(r) => Region2 { outer: Loop2::circle(Vec2::ZERO, r), holes: vec![section] },
                    None => Region2 { outer: section, holes: Vec::new() },
                };
                (plane_at(f, z0 + height * t), region)
            })
            .collect();
        Ok(kernel::loft_regions(&sections)?)
    };
    match step {
        Step::Placed { frame, step } => tool(&f.compose(frame), step, nudge),
        Step::Prism { region, z0, height, twist, .. } => {
            let plane = plane_at(f, *z0);
            let regions = regions_of(region);
            if twist.abs() < 1e-12 {
                return one(kernel::extrude(&plane, &regions, 0.0, *height)?);
            }
            let path = [kernel::PathSeg::Line { a: plane.origin, b: plane.origin + v3(f.z) * *height }];
            let ctl = kernel::SweepControl { twist: *twist, ..kernel::SweepControl::default() };
            one(regions.iter().map(|r| kernel::sweep_with(&plane, r, &path, &ctl)).collect::<kernel::Result<Vec<_>>>()?)
        }
        Step::Taper { outline, z0, height, twist, scale, .. } => taper(outline, *z0, *height, *twist, *scale, None),
        Step::RingTaper { outline, rim, z0, height, twist, scale, .. } => taper(outline, *z0, *height, *twist, *scale, Some(*rim)),
        Step::Loft { sections, .. } => {
            let s: Vec<(Plane, Region2)> = sections.iter().map(|(o, h)| (plane_at(f, *h), Region2 { outer: outline_loop(o), holes: Vec::new() })).collect();
            Ok(kernel::loft_regions(&s)?)
        }
        Step::Revolve { profile, .. } => {
            let pts: Vec<Vec2> = solid::nudged(profile, nudge).into_iter().map(v2).collect();
            let region = Region2 { outer: Loop2::polygon(&pts), holes: Vec::new() };
            one(kernel::revolve(&axial_plane(f), &[region], Vec2::ZERO, Vec2::new(0.0, 1.0), std::f64::consts::TAU)?)
        }
        Step::Coil { section, pitch, height, left, .. } => {
            let pts: Vec<Vec2> = section.iter().map(|q| v2(*q)).collect();
            let region = Region2 { outer: Loop2::polygon(&pts), holes: Vec::new() };
            let k = v3(f.z);
            // A left-handed coil turns about −axis and climbs along +axis (as the Coil feature).
            let (axis, p) = if *left { (k * -1.0, -*pitch) } else { (k, *pitch) };
            Ok(kernel::sweep_helix(&axial_plane(f), &region, v3(f.origin), axis, p, height / pitch)?)
        }
    }
}

#[cfg(test)]
#[path = "eval_gears_tests.rs"]
mod tests;
