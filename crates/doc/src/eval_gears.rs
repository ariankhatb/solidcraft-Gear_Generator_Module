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
//! kernel's check refuses is kept with a warning saying why. Lofts' and tapers' sections go in
//! with their splines' pieces meeting at one speed ([`section_loop`]): on the sketch's fit, the
//! cones that trim flat bevels and hypoids crossed every tooth side with a kink SMLib refuses
//! ("Bad CurveCusp").

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock, PoisonError};

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
    let planned = solidcraft_profile::scope("gear", "plan (feature)", || planner(&GearsInput { doc, state: st, params, sketches: &ids }));
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
    let (built, _) = kernel::validating(kernel::Validation::Off, || solidcraft_profile::scope("gear", "build", || build(&plan, &mut notes)));
    let (members, keep_out, copies) = built?;
    // Members made as moved copies of others, with the body each was moved from: checked as that
    // one is (a rigid move changes nothing the check reads).
    let moved: Vec<(Body, Body)> = copies.iter().enumerate().filter_map(|(m, c)| Some((members[m].1.clone(), members.get((*c)?)?.1.clone()))).collect();
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
        // Each body's check on a thread of its own (they read nothing of each other's).
        let fresh: Vec<&ModelBody> = st.bodies.iter().filter(|b| !before.iter().any(|old| old.same_shape(&b.body))).collect();
        let _span = solidcraft_profile::span("gear", "check");
        // A moved copy whose original is among them takes the original's verdict.
        let original = |b: &ModelBody| moved.iter().find(|(copy, _)| copy.same_shape(&b.body)).and_then(|(_, from)| fresh.iter().position(|o| o.body.same_shape(from)));
        let of: Vec<Option<usize>> = fresh.iter().map(|b| original(b)).collect();
        let own: Vec<&ModelBody> = fresh.iter().zip(&of).filter(|(_, o)| o.is_none()).map(|(b, _)| *b).collect();
        let own_verdicts = kernel::map_on_threads(&own, THREADS, |b| {
            let _span = solidcraft_profile::span("gear check", solidcraft_profile::intern(&b.name));
            checked(&b.body)
        });
        let mut by_body = own.iter().zip(own_verdicts);
        let mut verdicts: Vec<Option<Option<String>>> = Vec::with_capacity(fresh.len());
        for o in &of {
            verdicts.push(match o {
                None => by_body.next().and_then(|(_, v)| v),
                Some(_) => None,
            });
        }
        for (i, o) in of.iter().enumerate() {
            if let Some(j) = o {
                verdicts[i] = verdicts[*j].clone();
                if let Some(v) = &verdicts[i] {
                    remember(&CHECKED, (fresh[i].body.clone(), v.clone()));
                }
            }
        }
        for (b, verdict) in fresh.iter().zip(verdicts) {
            match verdict {
                Some(None) => {}
                Some(Some(why)) => notes.push(refused_note(&b.name, &why)),
                None => notes.push(refused_note(&b.name, "the check stopped on an internal error")),
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

/// A set's bodies as built: each rigid member (its generator name and body), the keep-out body,
/// and for each member the earlier member it is a moved copy of (the same gears the same way
/// round each other, elsewhere).
type Built = (Vec<(String, Body)>, Option<(String, Body)>, Vec<Option<usize>>);

/// Builds the plan: each rigid member (its generator name and body) and the keep-out body. A gear
/// that can't be made fails the set; a step that only changes a gear (a hub, bore or trim) is left
/// out with a note, and so is the keep-out body.
///
/// A body planned the same as one before it but for where it stands (a drivetrain's front and
/// back axles, a pair of equal bevels) is not made again: the first is moved there
/// ([`moved_to`]); and so is a rigid member whose bodies are such copies, the same way round
/// each other, of an earlier member's.
fn build(plan: &Plan, notes: &mut Vec<String>) -> Result<Built> {
    let jobs: Vec<&Planned> = plan.bodies.iter().chain(plan.clearance.as_ref()).collect();
    let shapes: Vec<Option<u64>> = jobs.iter().map(|b| rigid(&b.frame).then(|| shape_key(b))).collect();
    let source: Vec<Option<usize>> = (0..jobs.len()).map(|i| shapes[i].and_then(|k| (0..i).find(|j| shapes[*j] == Some(k)))).collect();
    // Every body of a shape of its own (and the keep-out body) on a thread of its own: each is
    // made from its own plan alone, and the kernel answers the same on any thread.
    let own: Vec<usize> = (0..jobs.len()).filter(|i| source[*i].is_none()).collect();
    let own_jobs: Vec<&Planned> = own.iter().map(|i| jobs[*i]).collect();
    let made_own = kernel::map_on_threads(&own_jobs, THREADS, |b| made_body(b));
    let mut made: Vec<Option<(Result<Body>, Vec<String>)>> = (0..jobs.len()).map(|_| None).collect();
    for (i, r) in own.iter().zip(made_own) {
        made[*i] = Some(r.unwrap_or_else(|| (Err(stopped()), Vec::new())));
    }
    for i in 0..jobs.len() {
        let Some(from) = source[i] else { continue };
        let copy = match &made[from] {
            Some((Ok(body), said)) => match moved_to(body, &jobs[from].frame, &jobs[i].frame) {
                Ok(body) => (Ok(body), said.iter().map(|n| n.replace(&jobs[from].name, &jobs[i].name)).collect()),
                // (Made where it stands after all.)
                Err(_) => made_body(jobs[i]),
            },
            Some((Err(e), said)) => (Err(DocError::Invalid(e.to_string())), said.clone()),
            None => (Err(stopped()), Vec::new()),
        };
        made[i] = Some(copy);
    }
    let mut made = made.into_iter().map(|m| m.unwrap_or_else(|| (Err(stopped()), Vec::new())));
    let mut gears: Vec<Body> = Vec::with_capacity(plan.bodies.len());
    for b in &plan.bodies {
        let (body, said) = made.next().unwrap_or_else(|| (Err(stopped()), Vec::new()));
        notes.extend(said);
        gears.push(body.map_err(|e| DocError::Invalid(format!("{}: {e}", b.name)))?);
    }
    let span = solidcraft_profile::span("gear", "join members");
    let rigids = solid::members(plan);
    let keys: Vec<Option<u64>> = rigids.iter().map(|r| member_key(plan, &r.bodies, &shapes)).collect();
    let mut members: Vec<(String, Body)> = Vec::new();
    let mut copies: Vec<Option<usize>> = Vec::new();
    for (m, r) in rigids.iter().enumerate() {
        // A member of moved copies: the earlier one like it, moved (no union made again).
        let earlier = keys[m].and_then(|key| (0..m).find(|j| keys[*j] == Some(key))).filter(|_| r.bodies.iter().all(|k| source[*k].is_some()));
        let moved = earlier.and_then(|j| moved_to(&members[j].1, &plan.bodies[rigids[j].bodies[0]].frame, &plan.bodies[r.bodies[0]].frame).ok());
        let body = match (moved, &r.bodies[..]) {
            (Some(body), _) => body,
            (None, [one]) => gears[*one].clone(),
            (None, ks) => {
                let parts: Vec<&Body> = ks.iter().map(|k| &gears[*k]).collect();
                kernel::union_all(&parts).map_err(|e| DocError::Invalid(format!("{}: its gears could not be joined ({e})", r.name)))?
            }
        };
        copies.push(earlier);
        members.push((r.name.clone(), body));
    }
    drop(span);
    let keep_out = match &plan.clearance {
        Some(c) => {
            let (body, said) = made.next().unwrap_or_else(|| (Err(stopped()), Vec::new()));
            notes.extend(said);
            match body {
                Ok(b) => Some((c.name.clone(), b)),
                Err(e) => {
                    notes.push(format!("{}: the keep-out body could not be made ({e}); the gears are made without it", c.name));
                    None
                }
            }
        }
        None => None,
    };
    Ok((members, keep_out, copies))
}

/// What a planned body is known by apart from where it stands: its [`plan_key`] with the frame
/// the world's and no name.
fn shape_key(b: &Planned) -> u64 {
    hashed(&plan_key(&Planned { name: String::new(), frame: Frame::at([0.0; 3]), steps: b.steps.clone() }))
}

/// What a rigid member made of the plan's `bodies` is known by apart from where it stands: each
/// body's shape ([`shape_key`]) and where it stands from the first, to 1e-9 mm. `None` when a
/// body has no shape key (its frame not a rigid placing).
fn member_key(plan: &Plan, bodies: &[usize], shapes: &[Option<u64>]) -> Option<u64> {
    let first = plan.bodies.get(*bodies.first()?)?.frame;
    let back = inverse(&placing(&first));
    let mut text = String::new();
    for k in bodies {
        let m = times(&placing(&plan.bodies.get(*k)?.frame), &back);
        text.push_str(&format!("{};", shapes.get(*k).copied().flatten()?));
        for v in m.iter().flatten() {
            let near = format!("{v:.9}");
            text.push_str(if near.parse::<f64>() == Ok(0.0) { "0" } else { &near });
            text.push(',');
        }
    }
    Some(hashed(&text))
}

/// Whether a frame's axes are unit and square to each other (a rigid placing).
fn rigid(f: &Frame) -> bool {
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    (dot(f.x, f.x) - 1.0).abs() <= 1e-12 && (dot(f.z, f.z) - 1.0).abs() <= 1e-12 && dot(f.x, f.z).abs() <= 1e-12
}

type Mat = [[f64; 4]; 4];

/// The matrix that places a frame's own coordinates in the world (rows: x, y, z, origin; a point
/// is a row times it, as the kernel's transforms take it).
fn placing(f: &Frame) -> Mat {
    let (x, y, z, o) = (f.x, f.y(), f.z, f.origin);
    [[x[0], x[1], x[2], 0.0], [y[0], y[1], y[2], 0.0], [z[0], z[1], z[2], 0.0], [o[0], o[1], o[2], 1.0]]
}

/// The inverse of a rigid [`placing`].
fn inverse(m: &Mat) -> Mat {
    let mut out = [[0.0; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = m[j][i];
        }
        out[3][i] = -(0..3).map(|k| m[3][k] * m[i][k]).sum::<f64>();
    }
    out[3][3] = 1.0;
    out
}

/// `a` then `b`, for row points.
fn times(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| a[i][k] * b[k][j]).sum()))
}

/// A body made in frame `from`, moved to stand in frame `to` the same way.
fn moved_to(body: &Body, from: &Frame, to: &Frame) -> Result<Body> {
    Ok(kernel::transform_matrix(body, times(&inverse(&placing(from)), &placing(to)))?)
}

/// A text's 64-bit hash: what the caches know a plan by.
fn hashed(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Most threads a set's gears are made and checked on at once.
const THREADS: usize = 16;

/// What a body's job ends with when the kernel stopped on an internal error (its thread's panic).
fn stopped() -> DocError {
    DocError::Invalid("the kernel stopped on an internal error".into())
}

/// How many bodies [`MADE`] and [`CHECKED`] keep: a large set's, with room for an undo.
const KEPT: usize = 64;

/// Bodies made before, by the planned body each was made from ([`plan_key`]), with the notes
/// making it gave. A set evaluated again (an edit to one gear, an undo,
/// another item added) makes only the bodies whose plan changed; the others are the bodies made
/// before, which are what making them again gives: the kernel answers the same on any thread
/// and after any other work.
static MADE: Mutex<VecDeque<(u64, Body, Vec<String>)>> = Mutex::new(VecDeque::new());

/// SMLib's verdict on bodies checked before (`None`: valid; else why it refuses), so a body made
/// before ([`MADE`]) is not checked again.
static CHECKED: Mutex<VecDeque<(Body, Option<String>)>> = Mutex::new(VecDeque::new());

/// Keeps `entry` at the front of `cache` (the most recently used first), at most [`KEPT`].
fn remember<T>(cache: &Mutex<VecDeque<T>>, entry: T) {
    let mut c = cache.lock().unwrap_or_else(PoisonError::into_inner);
    c.push_front(entry);
    c.truncate(KEPT);
}

/// What `take` reads of the entry of `cache` that is `wanted`, which moves to the front (used
/// again).
fn recall<T, R>(cache: &Mutex<VecDeque<T>>, wanted: impl Fn(&T) -> bool, take: impl Fn(&T) -> R) -> Option<R> {
    let mut c = cache.lock().unwrap_or_else(PoisonError::into_inner);
    let at = c.iter().position(wanted)?;
    let entry = c.remove(at)?;
    let r = take(&entry);
    c.push_front(entry);
    Some(r)
}

/// One planned body made ([`build_body`]), or the body made from the same plan before, with the
/// notes making it gave.
fn made_body(b: &Planned) -> (Result<Body>, Vec<String>) {
    // (By the key's hash: a body's plan can be megabytes of text.)
    let key = hashed(&plan_key(b));
    if let Some((body, notes)) = recall(&MADE, |(k, ..)| *k == key, |(_, body, notes)| (body.clone(), notes.clone())) {
        return (Ok(body), notes);
    }
    let mut notes = Vec::new();
    let body = build_body(b, &mut notes);
    if let Ok(body) = &body {
        remember(&MADE, (key, body.clone(), notes.clone()));
    }
    (body, notes)
}

/// What a planned body is known by in [`MADE`]: its debug text with every decimal number to 9
/// decimal places (1e-9 mm or rad). A set's planner works the teeth of every gear out again for
/// an edit to any one of them, and gives the gears that didn't change the same numbers but for
/// the last bit or two (1e-15 mm); a gear whose plan differs that little is the same gear (the
/// kernel's tolerances are far coarser), and is not made again.
fn plan_key(b: &Planned) -> String {
    let text = format!("{b:?}");
    let mut key = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(c) = rest.chars().next() {
        let after_word = key.chars().next_back().is_some_and(|p| p.is_alphanumeric() || p == '_');
        let starts_number = c.is_ascii_digit() || (c == '-' && rest[1..].starts_with(|d: char| d.is_ascii_digit()));
        if starts_number && !after_word {
            let len = rest
                .char_indices()
                .find(|&(i, d)| !(d.is_ascii_digit() || d == '.' || d == 'e' || (matches!(d, '-' | '+') && (i == 0 || rest[..i].ends_with('e')))))
                .map_or(rest.len(), |(i, _)| i);
            let token = &rest[..len];
            match token.parse::<f64>() {
                Ok(v) if token.contains(['.', 'e']) => {
                    let near = format!("{v:.9}");
                    key.push_str(if near.parse::<f64>() == Ok(0.0) { "0" } else { &near });
                }
                _ => key.push_str(token),
            }
            rest = &rest[len..];
        } else {
            key.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    key
}

/// SMLib's check of a body the feature keeps ([`kernel::check_kept`]): `None` when it passes, else
/// why it refuses. A body checked before is not checked again.
fn checked(body: &Body) -> Option<String> {
    if let Some(verdict) = recall(&CHECKED, |(b, _)| b.same_shape(body), |(_, v)| v.clone()) {
        return verdict;
    }
    let verdict = kernel::check_kept(body).err().map(|e| e.to_string());
    remember(&CHECKED, (body.clone(), verdict.clone()));
    verdict
}

/// One planned body: its steps in order, each on what the ones before made; or, the same solid,
/// with fewer booleans on its teeth ([`solidcraft_gear::plan::fused`]: through holes in the
/// teeth's regions, the hub and shafts a blank of their own joined in one union). A fused build
/// that fails or leaves a step out is dropped for the plan's own.
fn build_body(b: &Planned, notes: &mut Vec<String>) -> Result<Body> {
    let _span = solidcraft_profile::span("gear body", solidcraft_profile::intern(&b.name));
    if let Some(f) = solidcraft_gear::plan::fused::fused(b) {
        let mut said = Vec::new();
        let made = build_fused(b, &f, &mut said);
        if let (Ok(body), true) = (made, said.is_empty()) {
            return Ok(body);
        }
    }
    run_steps(b, 0, None, notes)
}

/// [`build_body`]'s fused build: the teeth, the blank joined to them, the cuts left.
fn build_fused(b: &Planned, f: &solidcraft_gear::plan::fused::Fused, notes: &mut Vec<String>) -> Result<Body> {
    let _span = solidcraft_profile::span("gear", "fused build");
    let part = |steps: &[Step]| Planned { name: b.name.clone(), frame: b.frame, steps: steps.to_vec() };
    let teeth = run_steps(&part(&f.teeth), 0, None, notes)?;
    let joined = if f.blank.is_empty() {
        teeth
    } else {
        let blank = run_steps(&part(&f.blank), 0, None, notes)?;
        combine(Some(&teeth), blank, Op::Join)?
    };
    if f.after.is_empty() {
        return Ok(joined);
    }
    // The cuts after the body's own solids as planned (what a step reads of the body before it,
    // the tooth chamfers' reach, is the planned body's).
    let solids = b.steps.iter().take_while(|s| matches!(s.op(), Op::New | Op::Join)).count();
    let rest = Planned { steps: b.steps[..solids].iter().chain(&f.after).cloned().collect(), ..part(&[]) };
    run_steps(&rest, solids, Some(joined), notes)
}

/// The steps of `b` from step `from` on in order, each on what the ones before made (`start`:
/// what the steps before `from` made).
fn run_steps(b: &Planned, from: usize, start: Option<Body>, notes: &mut Vec<String>) -> Result<Body> {
    let mut made: Option<Body> = start;
    let mut skip = false;
    for (i, step) in b.steps.iter().enumerate().skip(from) {
        if std::mem::take(&mut skip) {
            continue;
        }
        // Both ends' tooth chamfers of a gear: one intersect in place of two cuts (the same
        // solid), else the two cuts as planned.
        if let (Some(lower), Some(one)) = (&made, solidcraft_gear::plan::chamfer::chamfers_as_one(b, i))
            && let Ok(body) = tool(&b.frame, &one, 0.0).and_then(|t| combine(Some(lower), t, Op::Intersect))
        {
            made = Some(body);
            skip = true;
            continue;
        }
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

/// A section of a loft or taper as a kernel loop: [`outline_loop`]'s curves, but each run of
/// fit-point splines that meet tangent to each other one natural cubic spline on equal parameter
/// steps, one per piece of their fits ([`natural_beziers`]), through points spread along the fits
/// ([`graded_points`]), so that its pieces meet at the same speed.
///
/// The kernel gives each piece of a loop a parameter span of its own. A sketch's fit spline is
/// smooth on its chord lengths, so on the kernel's spans its speed jumps by the ratio of the two
/// chords at every fit point (and from one spline to the next: a bevel tooth's fillet to its
/// flank, whose two fits with free ends also meet half a degree apart), and so does a loft's
/// wall across that point's line between the sections: the wall is tangent there, its parameters
/// are not. A boolean's curve crossing such a line at a slant (a flat bevel's back and front
/// cones, a hypoid's toe and heel cones, across every tooth side) turns sharply there in the
/// wall's parameters, which SMLib's check refuses ("Bad CurveCusp", several per tooth) and takes
/// a minute or two a body to look into. With the same speed on both sides of every join, the
/// walls' parameters run on smoothly too. The curves stay within a few µm of the fits (an
/// extruded wall is crossed only along its own lines by the cylinders and planes a spur's steps
/// cut it with, so prisms keep the fit as it is).
fn section_loop(o: &Outline) -> Loop2 {
    let fits: Vec<Option<Vec<[Vec2; 4]>>> = o
        .segs
        .iter()
        .map(|s| match s {
            Seg::Spline(pts) if pts.len() >= 2 => Some(solidcraft_sketch::fit_beziers(&pts.iter().map(|q| v2(*q)).collect::<Vec<_>>())).filter(|f| !f.is_empty()),
            _ => None,
        })
        .collect();
    // Whether the fit `a` runs on into the fit `b`: from the same point in the same direction.
    let tangent = |a: &[[Vec2; 4]], b: &[[Vec2; 4]]| -> bool {
        let (Some(a), Some(b)) = (a.last(), b.first()) else { return false };
        let (ta, tb) = (a[3] - a[2], b[1] - b[0]);
        let (la, lb) = (length(ta), length(tb));
        let scale = length(a[3] - a[0]).max(length(b[3] - b[0]));
        length(a[3] - b[0]) <= 1e-9 * scale && la > 0.0 && lb > 0.0 && (ta.x * tb.y - ta.y * tb.x).abs() <= TANGENT * la * lb && ta.x * tb.x + ta.y * tb.y > 0.0
    };
    let mut segs = Vec::with_capacity(o.segs.len());
    let mut i = 0;
    while i < o.segs.len() {
        let Some(Some(first)) = fits.get(i) else {
            segs.extend(outline_loop(&Outline { segs: vec![o.segs[i].clone()] }).segs);
            i += 1;
            continue;
        };
        let mut chain = first.clone();
        // (Not on round past the outline's last segment: its first piece starts where it did.)
        while let Some(Some(next)) = fits.get(i + 1) {
            if !tangent(&chain, next) {
                break;
            }
            chain.extend_from_slice(next);
            i += 1;
        }
        let points = graded_points(&chain);
        let t: Vec<f64> = (0..points.len()).map(|k| k as f64).collect();
        segs.extend(natural_beziers(&points, &t).into_iter().map(|[p0, p1, p2, p3]| Seg2::Cubic { p0, p1, p2, p3 }));
        i += 1;
    }
    Loop2 { segs }
}

/// Points on a chain of cubic Bézier pieces (the fits of a run of splines), one at each end and
/// one between each two pieces, spread along it as the pieces' own joins are but with the
/// spacing graded: each step's length the pieces' own, smoothed on its logarithm with its
/// neighbours' ([`GRADING`] passes). The fits of a fillet and a flank space their points
/// differently (densely round the fillet), and the spline on equal steps through points whose
/// spacing jumps from one to the other bends sharply there; graded, the fillet keeps its points
/// close together and the steps grow smoothly into the flank's.
fn graded_points(chain: &[[Vec2; 4]]) -> Vec<Vec2> {
    const SAMPLES: usize = 64;
    let at = |b: &[Vec2; 4], u: f64| {
        let v = 1.0 - u;
        b[0] * (v * v * v) + b[1] * (3.0 * v * v * u) + b[2] * (3.0 * v * u * u) + b[3] * (u * u * u)
    };
    // The length along the chain at each sample of each piece.
    let mut along = vec![0.0];
    let mut last = chain[0][0];
    for b in chain {
        for k in 1..=SAMPLES {
            let q = at(b, k as f64 / SAMPLES as f64);
            along.push(along[along.len() - 1] + length(q - last));
            last = q;
        }
    }
    let n = chain.len();
    let total = along[n * SAMPLES];
    let mut steps: Vec<f64> = (0..n).map(|k| (along[(k + 1) * SAMPLES] - along[k * SAMPLES]).max(1e-12 * total).ln()).collect();
    for _ in 0..GRADING {
        let was = steps.clone();
        for k in 0..n {
            let (l, r) = (was[k.saturating_sub(1)], was[(k + 1).min(n - 1)]);
            steps[k] = 0.25 * l + 0.5 * was[k] + 0.25 * r;
        }
    }
    let sum: f64 = steps.iter().map(|e| e.exp()).sum();
    let mut points = Vec::with_capacity(n + 1);
    points.push(chain[0][0]);
    let (mut j, mut want) = (0, 0.0);
    for e in &steps[..n - 1] {
        want += total * e.exp() / sum;
        while j + 1 < n * SAMPLES && along[j + 1] < want {
            j += 1;
        }
        let f = if along[j + 1] > along[j] { (want - along[j]) / (along[j + 1] - along[j]) } else { 0.0 };
        points.push(at(&chain[j / SAMPLES], ((j % SAMPLES) as f64 + f) / SAMPLES as f64));
    }
    points.push(chain[n - 1][3]);
    points
}

/// How many times [`graded_points`] smooths the steps' lengths with their neighbours'.
const GRADING: usize = 2;

/// How far apart (the sine of the angle) the directions of two splines may be where they meet for
/// [`section_loop`] to take them as one: the fits of a bevel tooth's fillet and of its flank,
/// each on its own with free ends, meet half a degree apart where the tooth's sides are tangent.
const TANGENT: f64 = 0.02;

/// A vector's length.
fn length(v: Vec2) -> f64 {
    v.x.hypot(v.y)
}

/// The cubic Bézier pieces of the natural cubic spline through `p` at the parameters `t` (one
/// per point, increasing; the second derivative zero at both ends), each piece on its own span.
/// On the points' own chord lengths, the pieces of [`solidcraft_sketch::fit_beziers`].
fn natural_beziers(p: &[Vec2], t: &[f64]) -> Vec<[Vec2; 4]> {
    let n = p.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| t[i + 1] - t[i]).collect();
    // The second derivatives at the fit points (Thomas' algorithm on the tridiagonal system).
    let mut m = vec![Vec2::ZERO; n + 1];
    if n >= 2 {
        let mut diag = vec![0.0; n + 1];
        let mut rhs = vec![Vec2::ZERO; n + 1];
        for i in 1..n {
            diag[i] = 2.0 * (h[i - 1] + h[i]);
            rhs[i] = ((p[i + 1] - p[i]) * (1.0 / h[i]) - (p[i] - p[i - 1]) * (1.0 / h[i - 1])) * 6.0;
        }
        for i in 2..n {
            let w = h[i - 1] / diag[i - 1];
            diag[i] -= w * h[i - 1];
            rhs[i] = rhs[i] - rhs[i - 1] * w;
        }
        m[n - 1] = rhs[n - 1] * (1.0 / diag[n - 1]);
        for i in (1..n - 1).rev() {
            m[i] = (rhs[i] - m[i + 1] * h[i]) * (1.0 / diag[i]);
        }
    }
    (0..n)
        .map(|i| {
            let chord = (p[i + 1] - p[i]) * (1.0 / h[i]);
            let start = chord - (m[i] * 2.0 + m[i + 1]) * (h[i] / 6.0);
            let end = chord + (m[i] + m[i + 1] * 2.0) * (h[i] / 6.0);
            [p[i], p[i] + start * (h[i] / 3.0), p[i + 1] - end * (h[i] / 3.0), p[i + 1]]
        })
        .collect()
}

/// A region as kernel regions: each outer outline with the holes inside it.
fn regions_of(r: &Region) -> Vec<Region2> {
    solid::region_parts(r).into_iter().map(|(o, holes)| Region2 { outer: outline_loop(&r.outer[o]), holes: holes.iter().map(|h| outline_loop(&r.holes[*h])).collect() }).collect()
}

/// One body of several (a region of more than one outline): their union, side by side when
/// they are apart (lightening holes, a cross's corners).
fn one(bodies: Vec<Body>) -> Result<Body> {
    if bodies.len() > 1 {
        if let Ok(Some(apart)) = kernel::union_apart(&bodies) {
            return Ok(apart);
        }
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
                let (section, t) = solidcraft_gear::plan::taper_section(o, twist, scale, i, n);
                let section = section_loop(&section);
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
            let s: Vec<(Plane, Region2)> = sections.iter().map(|(o, h)| (plane_at(f, *h), Region2 { outer: section_loop(o), holes: Vec::new() })).collect();
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
