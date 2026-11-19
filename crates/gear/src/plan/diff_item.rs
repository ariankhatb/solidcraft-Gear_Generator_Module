//! Differentials on a drivetrain's ring gears: a hypoid or bevel item with a
//! [`crate::items::DiffItem`] gets a whole differential round its ring gear's axis, planned by
//! the differential generator ([`crate::diff`], [`crate::plan::Kind::Differential`] with no drive
//! gear of its own) and placed where the drawing puts the ring:
//!
//! - **where**: its middle on the ring's axis where the pinion's axis passes it (a bevel's apex,
//!   a hypoid pinion's closest approach), as a real axle's differential sits under its pinion;
//! - **how big**: the largest module (side gears 16, pinions 10 for an open one: the style's own
//!   teeth) whose case fits inside the ring, its outside at most 0.6 × the ring's pitch diameter;
//! - **the case**: one body with its pinion pins, rigid with the ring (the ring's bore opened to
//!   the case's outside, or the case grown out to the ring's bore, so the two sit together as a
//!   bolted ring does);
//! - **the side gears**: each one body with its half-shaft, which runs from the gear out along
//!   the ring's axis through the case's axle bore (1.5 × the ring's pitch diameter from the
//!   middle, or to the ends of a Shaft item drawn on the axis, which then is the half-shafts);
//! - **the pinions** (or element gears): each turning on the case (its carrier).
//!
//! The assembly is the generator's: side gear A turns the pinions relative to the case, side gear
//! B follows them, and side gear A is left free (an open differential has two degrees of
//! freedom). Its joint is on the case, so with nothing else turning it, both side gears and their
//! half-shafts turn with the case: the car driving straight.

use crate::axes::{Axis, add, cross, dot, len, mul, num, square_to, sub};
use crate::diff::{Diff, DriveKind, Style};
use crate::geom::{Outline, Region, V2};
use crate::items::{DiffItem, GearItem, GearRef, ItemKind};
use crate::plan::{Assembly, AssemblyLink, AssemblyMember, Body, Frame, Kind, Op, Plan, SetMember, Spec, Step};

use super::{Planned, Unit};

type V = [f64; 3];

/// The case's outside diameter, at most this much of the ring's pitch diameter.
const CASE_OF_RING: f64 = 0.6;
/// The half-shafts' reach from the middle, Auto: this many ring pitch diameters.
const AXLE_OF_RING: f64 = 1.5;

/// What the differentials of the items come to: per ring gear its unit (or why it can't be
/// made), and the Shaft items that became half-shafts (not built as bars of their own).
#[derive(Default)]
pub(super) struct Made {
    pub units: Vec<(GearRef, Result<Unit, String>)>,
    pub axle_items: Vec<usize>,
}

/// The ring gear of a pair item: a hypoid's first gear, a bevel pair's larger one.
pub(super) fn ring_of(items: &[GearItem], i: usize, planned: &[Planned]) -> Option<(GearRef, GearRef)> {
    let it = items.get(i)?;
    let (a, b) = (GearRef::new(i, 0), GearRef::new(i, 1));
    match it.kind {
        ItemKind::Hypoid => Some((a, b)),
        ItemKind::Bevel => {
            let d = |r: GearRef| planned.iter().find(|p| p.gear == r).map_or(0.0, |p| p.pitch_d);
            Some(if d(b) > d(a) + 1e-9 { (b, a) } else { (a, b) })
        }
        _ => None,
    }
}

/// Every item's differential, planned (`planned`: the gears planned so far, their bodies where
/// the drawing puts them). Each ring's own body (in `units`) gets its bore opened to the case.
pub(super) fn plan_diffs(items: &[GearItem], planned: &[Planned], units: &mut [Unit]) -> Made {
    let mut out = Made::default();
    for (i, it) in items.iter().enumerate() {
        let Some(want) = &it.differential else { continue };
        let Some((ring_ref, pinion_ref)) = ring_of(items, i, planned) else {
            out.units.push((
                GearRef::new(i, 0),
                Err(format!(
                    "{} has a differential, which rides on a ring gear: a hypoid or bevel pair's; turn it off, or make the item a hypoid or bevel pair",
                    it.who()
                )),
            ));
            continue;
        };
        // A pair that isn't planned is refused already (its own error says why).
        let (Some(ring), Some(pinion)) = (planned.iter().find(|p| p.gear == ring_ref), planned.iter().find(|p| p.gear == pinion_ref)) else { continue };
        let axis = Axis { origin: ring.body.frame.origin, dir: ring.body.frame.z };
        // A Shaft item on the ring's axis: its line is the half-shafts.
        let shaft = items.iter().enumerate().find_map(|(k, s)| {
            let g = crate::items::shaft_geom(s).filter(|_| s.kind == ItemKind::Shaft)?;
            let (a, b) = g.line?;
            let on = len(cross(g.axis, axis.dir)) < 1e-3 && axis.distance(a) <= crate::items::tol(ring.pitch_d / 2.0) && axis.distance(b) <= crate::items::tol(ring.pitch_d / 2.0);
            on.then_some((k, a, b, s.shaft_diameter))
        });
        let made = diff_unit(items, i, want, ring, pinion, shaft.map(|(_, a, b, d)| (a, b, d)));
        if let Ok((_, cut)) = &made {
            if let Some((k, ..)) = shaft {
                out.axle_items.push(k);
            }
            bore_ring(units, ring_ref, cut);
        }
        out.units.push((ring_ref, made.map(|(u, _)| u)));
    }
    out
}

/// The ring's bore opened to the case's outside: a cut through the ring (its own coordinates),
/// radius and z range.
struct RingCut {
    r: f64,
    z: (f64, f64),
    /// More to cut from the ring (its tapped bolt holes), in its own coordinates.
    more: Vec<Step>,
}

/// The ring gear's body in the unit that keeps it (the first that plans it) bored out to `cut`.
fn bore_ring(units: &mut [Unit], ring: GearRef, cut: &RingCut) {
    for u in units.iter_mut() {
        let Some(k) = u.refs.iter().position(|r| *r == Some(ring)) else { continue };
        let Some(b) = u.plan.members.get(k).and_then(|m| m.body) else { continue };
        if let Some(body) = u.plan.bodies.get_mut(b) {
            body.steps.push(Step::Prism { region: disc(cut.r), z0: cut.z.0 - 1.0, height: cut.z.1 - cut.z.0 + 2.0, twist: 0.0, op: Op::Cut });
            body.steps.extend(cut.more.iter().cloned());
        }
        return;
    }
}

fn disc(r: f64) -> Region {
    Region { outer: vec![Outline::circle(V2::ZERO, r)], holes: vec![] }
}

/// A frame moved by the rigid motion that takes the differential's own frame (axle along z, its
/// middle at the origin) to `to`.
fn placed(f: &Frame, to: &Frame) -> Frame {
    let dir = |v: V| {
        let y = to.y();
        [0, 1, 2].map(|i| v[0] * to.x[i] + v[1] * y[i] + v[2] * to.z[i])
    };
    Frame { origin: to.world(f.origin), x: dir(f.x), z: dir(f.z) }
}

/// Material of a body along an axis: (least radius, z range) of its points, the axis through
/// `o` along `u` (world).
fn along(body: &Body, h: f64, o: V, u: V) -> (f64, f64, f64) {
    let s = crate::mesh3d::Solid::new(body, h);
    let (mut rmin, mut lo, mut hi) = (f64::MAX, f64::MAX, f64::MIN);
    for q in s.material_points(h) {
        let d = sub(s.world(q), o);
        let z = dot(d, u);
        rmin = rmin.min(len(sub(d, mul(u, z))));
        lo = lo.min(z);
        hi = hi.max(z);
    }
    (rmin, lo, hi)
}

/// The most radius of a body's material about the z axis (its points in world coordinates)
/// between `z0` and `z1`: how far out it reaches there.
fn reach_between(body: &Body, h: f64, z0: f64, z1: f64) -> f64 {
    let s = crate::mesh3d::Solid::new(body, h);
    s.material_points(h).into_iter().map(|q| s.world(q)).filter(|w| w[2] >= z0 && w[2] <= z1).map(|w| w[0].hypot(w[1])).fold(0.0, f64::max)
}

/// The settings for a differential of `module` (the style's own teeth and pinions unless asked).
fn diff_spec(want: &DiffItem, module: f64, axle: f64) -> Spec {
    let d = Diff {
        module,
        axle_diameter: axle,
        // The drivetrain's ring gear drives it: no drive, mount or keep-out of its own, every part.
        drive: DriveKind::None,
        ring_bolts: false,
        keep_out: false,
        parts: crate::diff::Parts::default(),
        ..want.settings.clone()
    };
    Spec { diff: d, name: String::new(), ..Spec::defaults_for(Kind::Differential) }
}

/// The half-shaft's Auto diameter for side gears of `module`: 0.4 × their pitch diameter, in
/// half millimetres (at least 1 mm).
fn auto_axle(want: &DiffItem, module: f64) -> f64 {
    let z = f64::from(want.settings.side_teeth.min(want.settings.side_b()));
    ((0.8 * module * z).floor() / 2.0).max(1.0)
}

/// The differential planned at `module`, and its case's outside radius.
fn try_module(want: &DiffItem, module: f64, axle: Option<f64>) -> Result<(Plan, f64), String> {
    let axle = axle.unwrap_or_else(|| auto_axle(want, module));
    let p = crate::plan::plan(&diff_spec(want, module, axle))?;
    let r_o = p.facts.iter().find(|f| f.label == "case outside diameter").map(|f| f.value / 2.0).ok_or("the differential has no case")?;
    Ok((p, r_o))
}

/// The differential round `ring` (driven by `pinion`): its unit (the ring's copy first, joined
/// to the case; the case with its pins; side gear A and B each with its half-shaft; the pinions)
/// and the cut that opens the ring's bore to the case.
fn diff_unit(items: &[GearItem], i: usize, want: &DiffItem, ring: &Planned, pinion: &Planned, shaft: Option<(V, V, Option<f64>)>) -> Result<(Unit, RingCut), String> {
    let it = &items[i];
    let name = it.who();
    let id = it.id.clone();
    let rf = ring.body.frame;
    let u = rf.z;
    let pf = pinion.body.frame;
    // The middle: on the ring's axis where the pinion's passes closest.
    let (w, b) = (sub(rf.origin, pf.origin), dot(u, pf.z));
    let s = if 1.0 - b * b > 1e-9 { (b * dot(pf.z, w) - dot(u, w)) / (1.0 - b * b) } else { 0.0 };
    let c = add(rf.origin, mul(u, s));
    // The ring's bore and where it lies along the axis (its own coordinates).
    let h_ring = (0.05 * ring.module).clamp(0.05, 0.5);
    let (r_bore, z0_ring, z1_ring) = along(&ring.body, h_ring, rf.origin, u);
    if !r_bore.is_finite() {
        return Err(format!("{name}'s ring gear has no material to carry a differential"));
    }
    // The size: the asked module, else the largest whose case fits inside the ring.
    let limit = CASE_OF_RING * ring.pitch_d / 2.0;
    let axle_asked = want.axle_diameter.or(shaft.and_then(|s| s.2)).filter(|d| *d > 0.0);
    let (p, r_o, module) = match want.module.filter(|m| *m > 0.0) {
        Some(m) => {
            let (p, r_o) = try_module(want, m, axle_asked).map_err(|e| format!("{name}'s differential: {e}"))?;
            (p, r_o, m)
        }
        None => {
            let mut m = (ring.pitch_d / 50.0).max(0.2);
            let mut best: Option<(Plan, f64, f64)> = None;
            let mut last = String::new();
            for _ in 0..6 {
                match try_module(want, m, axle_asked) {
                    Ok((p, r_o)) if r_o <= limit + 1e-9 => {
                        let bigger = best.as_ref().is_none_or(|b| m > b.2);
                        if bigger {
                            best = Some((p, r_o, m));
                        }
                        // Room to spare: a step bigger, while it still fits.
                        if r_o > 0.92 * limit {
                            break;
                        }
                        m = ((m * (limit / r_o).min(1.5)) * 20.0).floor() / 20.0;
                    }
                    Ok((_, r_o)) => {
                        last = format!("its case is Ø{} at module {} and the ring's room Ø{}", num(2.0 * r_o), num(m), num(2.0 * limit));
                        m = ((m * (limit / r_o).max(0.5) * 0.97) * 20.0).floor() / 20.0;
                    }
                    Err(e) => {
                        last = e;
                        m = (m * 0.85 * 20.0).floor() / 20.0;
                    }
                }
                if m < 0.2 || best.as_ref().is_some_and(|b| (b.2 - m).abs() < 1e-9) {
                    break;
                }
            }
            best.ok_or_else(|| format!("{name}'s ring gear (Ø{}) is too small for a differential ({last}): use a bigger ring, or give the differential a module", num(ring.pitch_d)))?
        }
    };
    let axle = axle_asked.unwrap_or_else(|| auto_axle(want, module));
    // The differential's own frame onto the ring's axis.
    let to = Frame { origin: c, x: square_to(u), z: u };
    // Bodies: case, side A, side B, the pinions or elements, the pins last (bevel styles).
    let n = p.bodies.len();
    let pins = p.bodies.last().filter(|b| b.name.contains("Pinion pins")).map(|_| n - 1);
    let carried: Vec<usize> = (3..pins.unwrap_or(n)).collect();
    if n < 3 + carried.len() || carried.is_empty() {
        return Err(format!("{name}'s differential didn't plan its gears"));
    }
    let mut case = p.bodies[0].clone();
    if let Some(k) = pins {
        // The pins are part of the case (the pinions turn on them).
        for (j, st) in p.bodies[k].steps.iter().enumerate() {
            let mut st = st.clone();
            if j == 0 {
                set_op(&mut st, Op::Join);
            }
            case.steps.push(st);
        }
    }
    // The ring sits on the case: the case grown out to a larger bore (over the ring's width).
    let z_ring = dot(sub(rf.origin, c), u);
    let (zr0, zr1) = (z_ring + z0_ring, z_ring + z1_ring);
    if r_bore > r_o + 1e-3 {
        case.steps.push(Step::Prism {
            region: Region { outer: vec![Outline::circle(V2::ZERO, r_bore)], holes: vec![Outline::circle(V2::ZERO, (r_o - 0.5).max(axle / 2.0 + 0.5))] },
            z0: zr0,
            height: zr1 - zr0,
            twist: 0.0,
            op: Op::Join,
        });
    }
    // The ring bolted to a flange on the case behind it (its back face: its own least z): tapped
    // holes in the ring, through holes in the flange, on one bolt circle between the case and
    // the ring's tooth roots ([`crate::diff::ring_bolt_layout`], as a differential's own drive).
    let r_fit = r_o.max(r_bore);
    let mut ring_holes = Vec::new();
    let mut bolt_facts = Vec::new();
    let mut bolt_warn = None;
    let h_side_case = (0.05 * module).clamp(0.05, 0.25);
    if want.settings.ring_bolts {
        let root = crate::diff::least_root(&ring.body, h_ring);
        let wall = p.facts.iter().find(|f| f.label == "case wall").map_or(2.0, |f| f.value);
        match crate::diff::ring_bolt_layout(&want.settings, r_fit, root, wall) {
            Some((nb, b, rbc, rf_out, t)) => {
                let phase = std::f64::consts::PI / f64::from(nb);
                ring_holes.push(Step::Prism { region: Region { outer: crate::hub::lightening_holes(nb, 0.85 * b, rbc, phase), holes: vec![] }, z0: z0_ring - 1.0, height: 1.5 * b + 1.0, twist: 0.0, op: Op::Cut });
                // The same holes seen from the case (its x is the differential frame's).
                let cx = square_to(u);
                let turn = dot(cross(cx, rf.x), u).atan2(dot(cx, rf.x));
                case.steps.push(Step::Prism { region: Region { outer: vec![Outline::circle(V2::ZERO, rf_out)], holes: vec![Outline::circle(V2::ZERO, (reach_between(&p.bodies[0], h_side_case, zr0 - t, zr0) - 0.3).max(axle / 2.0 + 0.6))] }, z0: zr0 - t, height: t, twist: 0.0, op: Op::Join });
                case.steps.push(Step::Prism { region: Region { outer: crate::hub::lightening_holes(nb, 1.1 * b, rbc, phase + turn), holes: vec![] }, z0: zr0 - t - 1.0, height: t + 1.0, twist: 0.0, op: Op::Cut });
                bolt_facts = vec![("ring bolts", f64::from(nb), crate::plan::Unit::None), ("ring bolt size (M)", b, crate::plan::Unit::None), ("ring bolt circle diameter", 2.0 * rbc, crate::plan::Unit::Mm), ("case flange diameter", 2.0 * rf_out, crate::plan::Unit::Mm), ("case flange thickness", t, crate::plan::Unit::Mm)];
            }
            None => {
                bolt_warn = Some(format!(
                    "no room for ring bolts between the case (Ø{}) and the ring's tooth roots (Ø{}): the ring sits on the case unbolted (fixed to it all the same); use a bigger ring or a smaller differential module",
                    num(2.0 * r_fit),
                    num(2.0 * root)
                ));
            }
        }
    }
    case.name = format!("Diff {id} case");
    // The half-shafts: each side gear's, from the gear out along the axle.
    let reach = want.axle_length.filter(|l| *l > 0.0).unwrap_or(AXLE_OF_RING * ring.pitch_d);
    let ends = shaft.map(|(a, b, _)| (dot(sub(a, c), u), dot(sub(b, c), u)));
    let mut sides = Vec::new();
    let h_side = (0.05 * module).clamp(0.02, 0.25);
    for (k, label) in [(1usize, "A"), (2, "B")] {
        let mut side = p.bodies[k].clone();
        let (_, lo, hi) = along(&side, h_side, [0.0; 3], [0.0, 0.0, 1.0]);
        let out_up = lo + hi > 0.0;
        let inner = if out_up { lo } else { hi };
        // Its outer end: the Shaft item's end on its side, else the reach asked.
        let drawn = ends.and_then(|(a, b)| {
            let far = if out_up { a.max(b) } else { a.min(b) };
            (far * if out_up { 1.0 } else { -1.0 } > inner.abs() + 1.0).then_some(far)
        });
        let outer = drawn.unwrap_or(if out_up { reach } else { -reach });
        // In the side gear's own coordinates (its z on the axle).
        let f = side.frame;
        let t = |z: f64| dot(sub([0.0, 0.0, z], f.origin), f.z);
        let (t0, t1) = (t(inner), t(outer));
        side.steps.push(Step::Prism { region: disc(axle / 2.0), z0: t0.min(t1), height: (t1 - t0).abs(), twist: 0.0, op: Op::Join });
        side.name = format!("Diff {id} side {label} + axle");
        sides.push((side, (outer.abs() - inner.abs()).max(0.0)));
    }
    // The unit: the ring's copy (dropped when merging: the ring planned already), the case, the
    // sides, the pinions; placed on the ring's axis.
    let mut bodies = vec![ring.body.clone(), case];
    bodies.extend(sides.iter().map(|(b, _)| b.clone()));
    for (j, &k) in carried.iter().enumerate() {
        let mut b = p.bodies[k].clone();
        b.name = format!("Diff {id} {} {}", if want.style().bevel() { "pinion" } else { "element" }, j + 1);
        bodies.push(b);
    }
    for b in bodies.iter_mut().skip(1) {
        b.frame = placed(&b.frame, &to);
    }
    // Members of the generator's assembly (case 0, side A 1, side B 2, the carried ones, the pins)
    // as this unit's (the ring 0, the case 1, ...); the pins are the case's.
    let map = |m: usize| -> Option<usize> {
        if Some(m) == pins {
            None
        } else if m < 3 {
            Some(m + 1)
        } else {
            carried.iter().position(|x| *x == m).map(|j| 4 + j)
        }
    };
    let mut asm = Assembly::default();
    asm.members.push(AssemblyMember { body: 0, axis: Axis { origin: rf.origin, dir: u }, grounded: false, carrier: None });
    for (k, b) in bodies.iter().enumerate().skip(1) {
        let old = (0..p.assembly.members.len()).find(|m| map(*m) == Some(k));
        let carrier = old.and_then(|m| p.assembly.members[m].carrier).and_then(map);
        asm.members.push(AssemblyMember { body: k, axis: Axis { origin: b.frame.origin, dir: b.frame.z }, grounded: false, carrier });
    }
    asm.links.push(AssemblyLink::Rigid { a: 0, b: 1 });
    for l in &p.assembly.links {
        let (a, b) = l.members();
        let (Some(na), Some(nb)) = (map(a), map(b)) else { continue };
        asm.links.push(match l.clone() {
            AssemblyLink::Gear { ra, rb, internal, reverse, .. } => AssemblyLink::Gear { a: na, b: nb, ra, rb, internal, reverse },
            AssemblyLink::Rigid { .. } => AssemblyLink::Rigid { a: na, b: nb },
            _ => continue,
        });
    }
    let mut plan = Plan { bodies, assembly: asm, ..Plan::default() };
    plan.members = vec![SetMember { module: ring.module, teeth: ring.teeth, pitch_diameter: ring.pitch_d, body: Some(0), ..SetMember::default() }];
    plan.facts = p.facts.clone();
    plan.fact("differential module", module, crate::plan::Unit::Mm);
    let ds = &want.settings;
    plan.fact("side gear A teeth", f64::from(ds.side_teeth), crate::plan::Unit::None);
    plan.fact("side gear B teeth", f64::from(if ds.style == Style::Biased { ds.side_b() } else { ds.side_teeth }), crate::plan::Unit::None);
    plan.fact(if want.style().bevel() { "pinion teeth" } else { "element teeth" }, f64::from(ds.pinion_teeth), crate::plan::Unit::None);
    plan.fact(if want.style().bevel() { "pinions" } else { "element gears" }, carried.len() as f64, crate::plan::Unit::None);
    plan.fact("half-shaft reach from the middle", reach, crate::plan::Unit::Mm);
    plan.fact("half-shaft diameter", axle, crate::plan::Unit::Mm);
    plan.fact("half-shaft A length (out of its side gear)", sides[0].1, crate::plan::Unit::Mm);
    plan.fact("half-shaft B length (out of its side gear)", sides[1].1, crate::plan::Unit::Mm);
    for (label, v, unit) in bolt_facts {
        plan.fact(label, v, unit);
    }
    plan.warnings.extend(bolt_warn);
    for w in p.warnings.iter().filter(|w| !w.starts_with("assembly:")) {
        plan.warn(w.clone());
    }
    let what = match want.style() {
        Style::Open => "an open differential",
        Style::Biased => "a torque-biasing differential",
        Style::TorsenB => "a Torsen type B differential",
        Style::TorsenA => "a Torsen type A differential",
    };
    let notes = vec![format!(
        "{name} carries {what}: module {}, {} {}, case Ø{}, half-shafts Ø{} reaching {} mm each side{}; with both wheels free the side gears and half-shafts turn with the case (driving straight)",
        num(module),
        carried.len(),
        if want.style().bevel() { "pinions" } else { "elements" },
        num(2.0 * r_o),
        num(axle),
        num(reach),
        if shaft.is_some() { " (the Shaft item on its axis is the half-shafts)" } else { "" }
    )];
    let unit = Unit { label: format!("differential {name}"), plan, refs: vec![Some(ring.gear)], shifts: vec![None], carrier: None, tags: Vec::new(), contact: Vec::new(), notes };
    Ok((unit, RingCut { r: r_o.max(r_bore), z: (z0_ring, z1_ring), more: ring_holes }))
}

fn set_op(st: &mut Step, to: Op) {
    match st {
        Step::Prism { op, .. } | Step::Taper { op, .. } | Step::RingTaper { op, .. } | Step::Loft { op, .. } | Step::Revolve { op, .. } | Step::Coil { op, .. } => *op = to,
        Step::Placed { step, .. } => set_op(step, to),
    }
}

/// What a differential came to, for a front end's differential tab: its item and ring gear, the
/// facts its planner gives (the module and teeth chosen, case, bearings, pins, hubs, ring bolts,
/// half-shafts, speeds and torque split, Torsen bias; labels without the unit's prefix), its
/// warnings and the note that sums it up.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffReply {
    pub item: usize,
    pub ring: GearRef,
    pub facts: Vec<crate::plan::Fact>,
    pub warnings: Vec<String>,
    pub note: Option<String>,
}

/// One reply per item with a differential, from a planned drivetrain (its facts and warnings,
/// which [`super::SystemPlan`] keeps under "differential Gear X: ").
pub fn differential_replies(items: &[GearItem], sp: &super::SystemPlan) -> Vec<DiffReply> {
    let mut out = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if it.differential.is_none() {
            continue;
        }
        let ring = match it.kind {
            ItemKind::Bevel => {
                let d = |g: usize| sp.gears.iter().find(|r| r.item == i && r.gear == g).map_or(0.0, |r| r.pitch_d);
                GearRef::new(i, usize::from(d(1) > d(0) + 1e-9))
            }
            _ => GearRef::new(i, 0),
        };
        let pre = format!("differential {}: ", it.who());
        let facts = sp.plan.facts.iter().filter_map(|f| f.label.strip_prefix(&pre).map(|l| crate::plan::Fact { label: l.to_string(), ..f.clone() })).collect();
        let warnings = sp.plan.warnings.iter().filter_map(|w| w.strip_prefix(&pre).map(str::to_string)).collect();
        let note = sp.notes.iter().find(|n| n.starts_with(&format!("{} carries ", it.who()))).cloned();
        out.push(DiffReply { item: i, ring, facts, warnings, note });
    }
    out
}
