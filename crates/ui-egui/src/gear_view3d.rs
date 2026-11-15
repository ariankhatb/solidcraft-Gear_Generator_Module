//! The Gear Generator's instant 3D view: the set drawn from the math, never from the kernel.
//!
//! What it shows, cheapest first: each member's shaft as a centre line with an arrowhead, the
//! angle between two shafts (with its value) and the offset or centre distance between them, the
//! pitch surfaces (cylinders, cones, a hyperboloid-like surface through a loft's sections) as
//! shaded solids in each member's colour, the blanks' tip circles, and with Teeth on, the teeth
//! themselves: every body's build steps (prisms, twisted prisms, tapers, lofts, revolves, coils)
//! tessellated as they are, so helical teeth twist and bevel teeth shrink to the apex. The set
//! turns as the 2D preview does.
//!
//! Drawing: a small software rasteriser with a depth buffer (correct where teeth interleave,
//! which painter's-order polygons are not) into a texture, flat Lambert shading per triangle,
//! the shafts and labels painted over it. Meshes are cached per plan; a still view is not drawn
//! again. Drag to orbit, right- or middle-drag to pan, wheel to zoom, double-click to fit.
//!
//! The geometry comes from [`schematic_of`], one function over the plan's bodies and the spec:
//! when `solidcraft_gear::simple::schematic` lands, that function is the one to point at it.
//! Translucent parts are supported (drawn after the solids, depth-tested, not hiding what is
//! behind) for the clearance envelope and bearing seats still to come.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use solidcraft_gear::clearance::{Clearance, Seat};
use solidcraft_gear::axes::{self, Axis, Relation};
use solidcraft_gear::geom::{Outline, Region, V2};
use solidcraft_gear::plan::{Body, Frame, Kind, Op, Plan, Spec, Step, taper_turn};

type P3 = [f64; 3];

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: P3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: P3) -> P3 {
    let l = len(a);
    if l > 1e-12 { mul(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}

/// How a member moves per driver radian.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Motion {
    /// Turn about the body's own z.
    pub spin: f64,
    /// Slide along the set's x (a rack), wrapped by `period`.
    pub slide: f64,
    pub period: f64,
    /// Turns of the body's centre about the set's origin (a cycloidal drive's disc), from
    /// `orbit_center` (the set's plane).
    pub orbit: f64,
    pub orbit_center: V2,
}

/// One body of the set, as the schematic sees it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SchemBody {
    pub body: usize,
    pub member: usize,
    pub name: String,
    /// The body's frame (world): its z is the shaft.
    pub frame: Frame,
    /// The pitch surface as (z, radius) along the shaft, in the body's frame; empty when none
    /// is known (a rack).
    pub pitch: Vec<[f64; 2]>,
    /// The blank's tip outline the same way.
    pub tip: Vec<[f64; 2]>,
    /// Teeth inside (a ring gear): the pitch surface has no end caps.
    pub internal: bool,
    /// The teeth's lead angle on the pitch surface (radians; 0 straight).
    pub helix: f64,
    /// How far the body reaches along its shaft (body frame z).
    pub z: (f64, f64),
    pub motion: Motion,
}

/// The set as the schematic views draw it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Schematic {
    pub bodies: Vec<SchemBody>,
    /// The two meshing members' first bodies, when the set has two shafts that differ.
    pub pair: Option<(usize, usize)>,
    pub relation: Option<Relation>,
    /// The set's own frame (its placement, or the world).
    pub set_frame: Frame,
    pub kind: Kind,
    /// For tessellating: the outline tolerance (mm).
    pub tol: f64,
}

impl Schematic {
    /// The two shafts of `pair` as axes.
    pub fn axes(&self) -> Option<(Axis, Axis)> {
        let (a, b) = self.pair?;
        let ax = |i: usize| Axis::new(self.bodies[i].frame.origin, self.bodies[i].frame.z);
        Some((ax(a)?, ax(b)?))
    }
}

/// A polyline of an outline, without repeats or the closing point, at most `cap` points.
fn ring(o: &Outline, tol: f64, cap: usize) -> Vec<V2> {
    let mut p = o.polyline(tol);
    p.dedup_by(|a, b| a.dist(*b) < 1e-9);
    if p.len() > 1 && p[0].dist(p[p.len() - 1]) < 1e-9 {
        p.pop();
    }
    if p.len() > cap {
        let step = p.len().div_ceil(cap);
        p = p.into_iter().step_by(step).collect();
    }
    p
}

fn radii(p: &[V2]) -> (f64, f64) {
    p.iter().map(|q| q.len()).fold((f64::MAX, 0.0f64), |(lo, hi), r| (lo.min(r), hi.max(r)))
}

/// Whether the origin is inside the polygon (even-odd).
fn holds_origin(p: &[V2]) -> bool {
    let mut inside = false;
    let n = p.len();
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        if (a.y > 0.0) != (b.y > 0.0) && 0.0 < a.x + (0.0 - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
    }
    inside
}

/// The teeth of a build step: (z, mid radius) sections, the tip radius at each, and whether the
/// teeth face in.
struct Toothed {
    sections: Vec<(f64, f64, f64)>,
    internal: bool,
    helix: f64,
}

fn toothed(step: &Step, tol: f64) -> Option<(usize, Toothed)> {
    let biggest = |r: &Region| -> Option<(usize, Vec<V2>, bool)> {
        let outer = r.outer.iter().map(|o| ring(o, tol, 4000)).max_by_key(|p| p.len())?;
        let hole = r.holes.iter().map(|o| ring(o, tol, 4000)).max_by_key(|p| p.len());
        match hole {
            Some(h) if h.len() > outer.len() && holds_origin(&h) => Some((h.len(), h, true)),
            _ => Some((outer.len(), outer, false)),
        }
    };
    let span = |p: &[V2]| {
        let (lo, hi) = radii(p);
        ((lo + hi) / 2.0, hi)
    };
    match step {
        Step::Prism { region, z0, height, twist, op: Op::New | Op::Join } => {
            let (n, p, internal) = biggest(region)?;
            if !holds_origin(&p) {
                return None;
            }
            let (mid, tip) = span(&p);
            let helix = if height.abs() > 1e-9 { (twist * mid / height).atan() } else { 0.0 };
            Some((n, Toothed { sections: vec![(*z0, mid, tip), (z0 + height, mid, tip)], internal, helix }))
        }
        Step::Taper { outline, z0, height, scale, op: Op::New | Op::Join, .. } => {
            let p = ring(outline, tol, 4000);
            if !holds_origin(&p) {
                return None;
            }
            let (mid, tip) = span(&p);
            Some((p.len(), Toothed { sections: vec![(*z0, mid, tip), (z0 + height, mid * scale, tip * scale)], internal: false, helix: 0.0 }))
        }
        Step::Loft { sections, op: Op::New | Op::Join } => {
            let mut out = Vec::new();
            let mut n = 0;
            for (o, h) in sections {
                let p = ring(o, tol, 4000);
                n = n.max(p.len());
                let (mid, tip) = span(&p);
                out.push((*h, mid, tip));
            }
            (out.len() >= 2).then_some((n, Toothed { sections: out, internal: false, helix: 0.0 }))
        }
        Step::Coil { section, z0, height, .. } => {
            let (lo, hi) = section.iter().map(|q| q.x).fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
            let mid = (lo + hi) / 2.0;
            Some((section.len() * 64, Toothed { sections: vec![(*z0, mid, hi), (z0 + height, mid, hi)], internal: false, helix: 0.0 }))
        }
        _ => None,
    }
}

/// The z range a step fills (New and Join only).
fn reach(step: &Step) -> Option<(f64, f64)> {
    match step {
        Step::Prism { z0, height, op: Op::New | Op::Join, .. }
        | Step::Taper { z0, height, op: Op::New | Op::Join, .. }
        | Step::RingTaper { z0, height, op: Op::New | Op::Join, .. }
        | Step::Coil { z0, height, op: Op::New | Op::Join, .. } => {
            Some((z0.min(z0 + height), z0.max(z0 + height)))
        }
        Step::Loft { sections, op: Op::New | Op::Join } => sections.iter().map(|(_, h)| *h).fold(None, |r: Option<(f64, f64)>, h| Some(r.map_or((h, h), |(a, b)| (a.min(h), b.max(h))))),
        Step::Revolve { profile, op: Op::New | Op::Join } => profile.iter().map(|q| q.y).fold(None, |r: Option<(f64, f64)>, h| Some(r.map_or((h, h), |(a, b)| (a.min(h), b.max(h))))),
        _ => None,
    }
}

/// The member a 2D preview outline is, and a body is, as the window counts them.
fn members_of(kind: Kind, only: u8, plan: &Plan) -> (Vec<Option<usize>>, Vec<usize>) {
    let outlines = plan.preview.iter().enumerate().map(|(i, g)| crate::gear_window::member_of_outline(kind, i, &g.name)).collect();
    let bodies = plan.bodies.iter().enumerate().map(|(i, b)| crate::gear_window::member_of_body(kind, only, i, &b.name)).collect();
    (outlines, bodies)
}

/// The set's schematic from its plan: shafts, pitch surfaces, blanks and motion per body.
///
/// SWITCH POINT: derived here from the plan's body frames and build steps and the spec until
/// `solidcraft_gear::simple::schematic(spec)` exists; then build [`Schematic`] from that
/// (axes, frames, cylinders/cones/hyperboloids per member) and keep the rest of this file.
pub(crate) fn schematic_of(spec: &Spec, plan: &Plan) -> Schematic {
    let kind = spec.kind;
    let tol = (spec.module * 0.04).clamp(0.004, 0.5);
    let set_frame = spec.placement.unwrap_or(Frame::at([0.0; 3]));
    let (outline_member, body_member) = members_of(kind, spec.only, plan);
    let corr = (spec.rack.dedendum - spec.rack.addendum) / 2.0 * spec.module;
    // The solver's own schematic (µs): its pitch and tip surfaces where it has them.
    let simple = solidcraft_gear::simple::schematic(spec);
    let profile_of = |i: usize, role: solidcraft_gear::simple::Role| -> Option<Vec<[f64; 2]>> {
        use solidcraft_gear::simple::Shape;
        let m = simple.members.iter().find(|m| m.body == Some(i))?;
        m.parts.iter().filter(|p| p.role == role).find_map(|p| match &p.shape {
            Shape::Cylinder { r, z0, z1 } => Some(vec![[*z0, *r], [*z1, *r]]),
            Shape::Tube { r_out, z0, z1, .. } => Some(vec![[*z0, *r_out], [*z1, *r_out]]),
            Shape::Cone { r0, z0, r1, z1, .. } => Some(vec![[*z0, *r0], [*z1, *r1]]),
            Shape::Revolved { profile } if profile.len() >= 2 => Some(profile.iter().map(|q| [q[1], q[0]]).collect()),
            _ => None,
        })
    };
    let mut bodies = Vec::new();
    for (i, b) in plan.bodies.iter().enumerate() {
        let member = body_member[i];
        let teeth = b.steps.iter().filter_map(|s| toothed(s, tol)).max_by_key(|(n, _)| *n).map(|(_, t)| t);
        let z = b.steps.iter().filter_map(reach).fold(None, |r: Option<(f64, f64)>, (lo, hi)| Some(r.map_or((lo, hi), |(a, c)| (a.min(lo), c.max(hi))))).unwrap_or((0.0, spec.face_width));
        let preview = outline_member.iter().position(|m| *m == Some(member)).and_then(|j| plan.preview.get(j));
        let (pitch, tip, internal, helix) = match &teeth {
            Some(t) => {
                let exact = preview.map(|g| g.pitch_radius).filter(|r| *r > 0.0 && t.sections.len() == 2 && (t.sections[0].1 - t.sections[1].1).abs() < 1e-9);
                let r_of = |mid: f64, scale: f64| if t.internal { mid - corr * scale } else { mid + corr * scale };
                let s0 = t.sections[0].1.max(1e-9);
                let pitch: Vec<[f64; 2]> = t.sections.iter().map(|(z, mid, _)| [*z, exact.unwrap_or_else(|| r_of(*mid, mid / s0))]).collect();
                let tip: Vec<[f64; 2]> = t.sections.iter().map(|(z, _, tip)| [*z, *tip]).collect();
                (pitch, tip, t.internal, t.helix)
            }
            None => (Vec::new(), Vec::new(), false, 0.0),
        };
        let pitch = profile_of(i, solidcraft_gear::simple::Role::Pitch).unwrap_or(pitch);
        let tip = profile_of(i, solidcraft_gear::simple::Role::Tip).unwrap_or(tip);
        bodies.push(SchemBody { body: i, member, name: b.name.clone(), frame: b.frame, pitch, tip, internal, helix, z, motion: Motion::default() });
    }
    // Motion: non-parallel pairs as the 3D mesh check turns them; the rest as the 2D preview.
    let set_z = set_frame.z;
    if let Some(m3) = plan.meshes3d.first() {
        for sb in &mut bodies {
            sb.motion.spin = if sb.member == 0 { 1.0 } else { m3.ratio };
        }
    } else {
        for sb in &mut bodies {
            let Some(j) = outline_member.iter().position(|m| *m == Some(sb.member)) else { continue };
            let g = &plan.preview[j];
            let sign = if dot(sb.frame.z, set_z) < 0.0 { -1.0 } else { 1.0 };
            sb.motion = Motion { spin: if g.slide != 0.0 { 0.0 } else { sign * g.speed }, slide: g.slide, period: g.period, orbit: g.orbit, orbit_center: g.center };
        }
    }
    let first = |m: usize| bodies.iter().position(|b| b.member == m);
    let pair = match (first(0), first(1)) {
        (Some(a), Some(b)) if kind.is_set() && !matches!(kind, Kind::Planetary | Kind::CycloidalDrive | Kind::Spline) => Some((a, b)),
        _ => None,
    };
    let relation = pair.and_then(|(a, b)| {
        let ax = |i: usize| Axis::new(bodies[i].frame.origin, bodies[i].frame.z);
        Some(axes::relate(&ax(a)?, &ax(b)?))
    });
    Schematic { bodies, pair, relation, set_frame, kind, tol }
}

// ---------------------------------------------------------------------------------------------
// Meshes.

type Tri = [[f32; 3]; 3];

/// Triangles of one body, in its own frame, in one colour.
struct Part {
    body: usize,
    member: usize,
    tris: Vec<Tri>,
    normals: Vec<[f32; 3]>,
    /// 1 solid; under 1 translucent (drawn after the solids, hiding nothing).
    alpha: f32,
    /// In world coordinates and never moved (the clearance body), not in a body's frame.
    fixed: bool,
}

/// A line segment of one body, in its own frame, depth tested.
struct Seg3 {
    body: usize,
    member: usize,
    a: [f32; 3],
    b: [f32; 3],
}

#[derive(Default)]
struct Builder {
    tris: Vec<Tri>,
}

impl Builder {
    fn tri(&mut self, a: P3, b: P3, c: P3) {
        let f = |p: P3| [p[0] as f32, p[1] as f32, p[2] as f32];
        self.tris.push([f(a), f(b), f(c)]);
    }
    fn quad(&mut self, a: P3, b: P3, c: P3, d: P3) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }
}

/// Fills a cap: a ring with one hole round the origin is zipped by angle, a ring round the
/// origin is fanned from it, anything else is ear clipped (or fanned from its centroid when
/// large). Lightening holes are not cut out of caps.
fn cap(outer: &[V2], hole: Option<&[V2]>) -> Vec<[V2; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }
    if let Some(h) = hole.filter(|h| h.len() >= 3 && holds_origin(h)) {
        return zip(outer, h);
    }
    if holds_origin(outer) {
        return (0..outer.len()).map(|i| [V2::ZERO, outer[i], outer[(i + 1) % outer.len()]]).collect();
    }
    if outer.len() <= 600 {
        return crate::gear_window::triangulate(outer);
    }
    let c = outer.iter().fold(V2::ZERO, |a, q| a + *q) * (1.0 / outer.len() as f64);
    (0..outer.len()).map(|i| [c, outer[i], outer[(i + 1) % outer.len()]]).collect()
}

/// Triangles between two rings round the origin, walking both by angle.
fn zip(a: &[V2], b: &[V2]) -> Vec<[V2; 3]> {
    let sorted = |p: &[V2]| -> (Vec<V2>, Vec<f64>) {
        let area: f64 = (0..p.len()).map(|i| p[i].cross(p[(i + 1) % p.len()])).sum();
        let mut q: Vec<V2> = if area >= 0.0 { p.to_vec() } else { p.iter().rev().copied().collect() };
        let ang = |v: V2| v.angle().rem_euclid(std::f64::consts::TAU);
        let start = (0..q.len()).min_by(|i, j| ang(q[*i]).total_cmp(&ang(q[*j]))).unwrap_or(0);
        q.rotate_left(start);
        let mut angles = Vec::with_capacity(q.len() + 1);
        let mut last = ang(q[0]);
        angles.push(last);
        for v in q.iter().skip(1) {
            let mut x = ang(*v);
            while x < last - 0.5 {
                x += std::f64::consts::TAU;
            }
            angles.push(x);
            last = x;
        }
        angles.push(angles[0] + std::f64::consts::TAU);
        q.push(q[0]);
        (q, angles)
    };
    let ((pa, aa), (pb, ab)) = (sorted(a), sorted(b));
    let (na, nb) = (pa.len() - 1, pb.len() - 1);
    let mut out = Vec::with_capacity(na + nb);
    let (mut i, mut j) = (0, 0);
    while i < na || j < nb {
        let take_a = j >= nb || (i < na && aa[i + 1] <= ab[j + 1]);
        if take_a {
            out.push([pa[i], pa[i + 1], pb[j]]);
            i += 1;
        } else {
            out.push([pa[i], pb[j + 1], pb[j]]);
            j += 1;
        }
    }
    out
}

/// A swept ring: section `t` in 0..=1 is the ring scaled `s(t)`, turned `turn(t)` at `z(t)`.
fn sweep(b: &mut Builder, ring: &[V2], k: usize, place: &dyn Fn(V2, f64) -> P3) {
    let n = ring.len();
    if n < 2 {
        return;
    }
    for j in 0..k {
        let (t0, t1) = (j as f64 / k as f64, (j + 1) as f64 / k as f64);
        for i in 0..n {
            let (p, q) = (ring[i], ring[(i + 1) % n]);
            b.quad(place(p, t0), place(q, t0), place(q, t1), place(p, t1));
        }
    }
}

/// The through cuts of a body that are holes in its caps (bores): outline and z range.
fn cuts(body: &Body, tol: f64) -> Vec<(Vec<V2>, f64, f64)> {
    body.steps
        .iter()
        .filter_map(|s| match s {
            Step::Prism { region, z0, height, twist, op: Op::Cut } if twist.abs() < 1e-12 => {
                let o = region.outer.first()?;
                let p = ring(o, tol, 800);
                holds_origin(&p).then(|| (p, z0.min(z0 + height), z0.max(z0 + height)))
            }
            _ => None,
        })
        .collect()
}

/// The hole a cap at `z` has: the region's own (a ring gear's teeth), else a bore cut through
/// it.
fn cap_hole<'a>(own: Option<&'a [V2]>, bores: &'a [(Vec<V2>, f64, f64)], z: f64) -> Option<&'a [V2]> {
    own.or_else(|| bores.iter().find(|(_, lo, hi)| *lo <= z + 1e-9 && *hi >= z - 1e-9).map(|(p, _, _)| p.as_slice()))
}

/// The teeth (and hubs, shafts) of one body as triangles in its own frame.
fn body_mesh(body: &Body, tol: f64, cap_pts: usize) -> Vec<Tri> {
    let mut b = Builder::default();
    let bores = cuts(body, tol);
    let (zlo, zhi) = body.steps.iter().filter_map(reach).fold((f64::MAX, f64::MIN), |(a, c), (lo, hi)| (a.min(lo), c.max(hi)));
    if body.steps.iter().any(|st| matches!(st, Step::Revolve { op: Op::Intersect, .. })) {
        // Trimmed by a revolved blank (a bevel's teeth ending on its back and front cones): the
        // steps' surfaces and the blank's, each triangle kept only where it is on the finished
        // body's surface.
        let solid = solidcraft_gear::mesh3d::Solid::new(body, tol.max(1e-3));
        let eps = (0.25 * tol).max(1e-4);
        let mut raw = Builder::default();
        for st in &body.steps {
            match st {
                Step::Revolve { profile, op: Op::Intersect } => revolve_profile(&mut raw, profile, 360),
                _ => step_mesh(&mut raw, st, tol, cap_pts, &bores, None),
            }
        }
        for t in raw.tris {
            let p = |k: usize| [f64::from(t[k][0]), f64::from(t[k][1]), f64::from(t[k][2])];
            let (a, bb, c) = (p(0), p(1), p(2));
            let (u, v) = ([bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len < 1e-18 {
                continue;
            }
            let mid = [0, 1, 2].map(|i| (a[i] + bb[i] + c[i]) / 3.0);
            let at = |sign: f64| solid.contains(body.frame.world([0, 1, 2].map(|i| mid[i] + sign * eps * n[i] / len)));
            if at(1.0) != at(-1.0) {
                b.tris.push(t);
            }
        }
    } else {
        for st in &body.steps {
            step_mesh(&mut b, st, tol, cap_pts, &bores, None);
        }
    }
    // The bores' walls, where the body is.
    for (p, lo, hi) in &bores {
        let (lo, hi) = (lo.max(zlo), hi.min(zhi));
        if hi > lo {
            sweep(&mut b, p, 1, &|q, t| [q.x, q.y, lo + (hi - lo) * t]);
        }
    }
    b.tris
}

fn step_mesh(b: &mut Builder, st: &Step, tol: f64, cap_pts: usize, bores: &[(Vec<V2>, f64, f64)], frame: Option<&Frame>) {
    let mut local = Builder::default();
    match st {
        Step::Prism { region, z0, height, twist, op: Op::New | Op::Join } => {
            let k = if twist.abs() > 1e-9 { ((twist.abs() / 0.035).ceil() as usize).clamp(2, 24) } else { 1 };
            let rings: Vec<Vec<V2>> = region.outer.iter().chain(&region.holes).map(|o| ring(o, tol, cap_pts)).collect();
            let place = |q: V2, t: f64| {
                let r = q.rot(twist * t);
                [r.x, r.y, z0 + height * t]
            };
            for r in &rings {
                sweep(&mut local, r, k, &place);
            }
            let own_hole = region.holes.first().map(|_| rings[region.outer.len()].as_slice());
            for (outer, i) in rings.iter().take(region.outer.len()).zip(0..) {
                let hole = if i == 0 { own_hole } else { None };
                for (t, z) in [(0.0, *z0), (1.0, z0 + height)] {
                    for tri in cap(outer, cap_hole(hole, bores, z)) {
                        local.tri(place(tri[0], t), place(tri[1], t), place(tri[2], t));
                    }
                }
            }
        }
        Step::Taper { outline, z0, height, twist, scale, op: Op::New | Op::Join } => {
            // (Fine along the taper: a bevel's is trimmed by its back and front cones, triangle
            // by triangle.)
            let k = ((twist.abs() / 0.035).ceil().max(((scale - 1.0).abs() / 0.015).ceil()) as usize).clamp(2, 64);
            let r = ring(outline, tol, cap_pts);
            let place = |q: V2, t: f64| {
                let s = 1.0 + (scale - 1.0) * t;
                let p = (q * s).rot(taper_turn(*twist, *scale, s));
                [p.x, p.y, z0 + height * t]
            };
            sweep(&mut local, &r, k, &place);
            for (t, z) in [(0.0, *z0), (1.0, z0 + height)] {
                for tri in cap(&r, cap_hole(None, bores, z)) {
                    local.tri(place(tri[0], t), place(tri[1], t), place(tri[2], t));
                }
            }
        }
        Step::Loft { sections, op: Op::New | Op::Join } => {
            let same = sections.windows(2).all(|w| w[0].0.segs.len() == w[1].0.segs.len());
            let rings: Vec<(Vec<V2>, f64)> = match sections.first() {
                Some((first, _)) if same => {
                    let counts = solidcraft_gear::mesh3d::seg_counts(first, tol * 2.0);
                    sections
                        .iter()
                        .map(|(o, h)| {
                            let mut p = solidcraft_gear::mesh3d::polygon(o, &counts);
                            if p.len() > 1 && p[0].dist(p[p.len() - 1]) < 1e-9 {
                                p.pop();
                            }
                            (p, *h)
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            if rings.len() >= 2 && rings.windows(2).all(|w| w[0].0.len() == w[1].0.len()) {
                for w in rings.windows(2) {
                    let ((p0, h0), (p1, h1)) = (&w[0], &w[1]);
                    let n = p0.len();
                    for i in 0..n {
                        let j = (i + 1) % n;
                        local.quad([p0[i].x, p0[i].y, *h0], [p0[j].x, p0[j].y, *h0], [p1[j].x, p1[j].y, *h1], [p1[i].x, p1[i].y, *h1]);
                    }
                }
                for (p, h) in [rings.first(), rings.last()].into_iter().flatten() {
                    for tri in cap(p, cap_hole(None, bores, *h)) {
                        local.tri([tri[0].x, tri[0].y, *h], [tri[1].x, tri[1].y, *h], [tri[2].x, tri[2].y, *h]);
                    }
                }
            } else {
                // Sections that don't match up: a surface of revolution through their tips.
                let prof: Vec<[f64; 2]> = sections.iter().map(|(o, h)| [*h, o.max_radius()]).collect();
                revolve(&mut local, &prof, true);
            }
        }
        Step::Revolve { profile, op: Op::New | Op::Join } => {
            let n = 48;
            let m = profile.len();
            for i in 0..m {
                let (p, q) = (profile[i], profile[(i + 1) % m]);
                for j in 0..n {
                    let (a0, a1) = (j as f64 / n as f64 * std::f64::consts::TAU, (j + 1) as f64 / n as f64 * std::f64::consts::TAU);
                    let at = |v: V2, a: f64| [v.x * a.cos(), v.x * a.sin(), v.y];
                    local.quad(at(p, a0), at(q, a0), at(q, a1), at(p, a1));
                }
            }
        }
        Step::Coil { section, pitch, z0, height, left, op: Op::New | Op::Join } => {
            let turns = (height / pitch.abs().max(1e-9)).abs();
            let k = ((turns * 36.0).ceil() as usize).clamp(8, 2400);
            let sign = if *left { -1.0 } else { 1.0 };
            let place = |q: V2, t: f64| {
                let a = sign * std::f64::consts::TAU * turns * t;
                [q.x * a.cos(), q.x * a.sin(), q.y + z0 + height * t]
            };
            let sec: Vec<V2> = section.clone();
            sweep(&mut local, &sec, k, &place);
            let tris = crate::gear_window::triangulate(&sec);
            for t in [0.0, 1.0] {
                for tri in &tris {
                    local.tri(place(tri[0], t), place(tri[1], t), place(tri[2], t));
                }
            }
        }
        // An internal bevel (a bevel ring): drawn as its rim, from the teeth's tips out.
        Step::RingTaper { outline, rim, z0, height, op: Op::New | Op::Join, .. } => {
            let r = outline.max_radius();
            let blank = Step::Revolve { profile: vec![V2::new(r, *z0), V2::new(*rim, *z0), V2::new(*rim, z0 + height), V2::new(r, z0 + height)], op: Op::New };
            step_mesh(&mut local, &blank, tol, cap_pts, bores, None);
        }
        Step::Placed { frame: f, step } => {
            step_mesh(&mut local, step, tol, cap_pts, &[], Some(f));
        }
        _ => {}
    }
    match frame {
        None => b.tris.extend(local.tris),
        Some(f) => {
            for t in local.tris {
                let w = |p: [f32; 3]| {
                    let q = f.world([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
                    [q[0] as f32, q[1] as f32, q[2] as f32]
                };
                b.tris.push([w(t[0]), w(t[1]), w(t[2])]);
            }
        }
    }
}

/// A closed (radius, z) polygon turned about the axis in `n` steps.
fn revolve_profile(b: &mut Builder, profile: &[V2], n: usize) {
    let m = profile.len();
    let at = |v: V2, a: f64| [v.x * a.cos(), v.x * a.sin(), v.y];
    for i in 0..m {
        let (p, q) = (profile[i], profile[(i + 1) % m]);
        for j in 0..n {
            let (a0, a1) = (j as f64 / n as f64 * std::f64::consts::TAU, (j + 1) as f64 / n as f64 * std::f64::consts::TAU);
            b.quad(at(p, a0), at(q, a0), at(q, a1), at(p, a1));
        }
    }
}

/// A surface of revolution through (z, r) points, with flat end caps unless `open`.
fn revolve(b: &mut Builder, prof: &[[f64; 2]], open: bool) {
    let n = 64;
    let at = |z: f64, r: f64, j: usize| {
        let a = j as f64 / n as f64 * std::f64::consts::TAU;
        [r * a.cos(), r * a.sin(), z]
    };
    for w in prof.windows(2) {
        let ([z0, r0], [z1, r1]) = (w[0], w[1]);
        for j in 0..n {
            b.quad(at(z0, r0, j), at(z0, r0, j + 1), at(z1, r1, j + 1), at(z1, r1, j));
        }
    }
    if !open {
        for &[z, r] in [prof.first(), prof.last()].into_iter().flatten() {
            for j in 0..n {
                b.tri([0.0, 0.0, z], at(z, r, j), at(z, r, j + 1));
            }
        }
    }
}

/// Triangle budget for the teeth over the whole set; members past it show their pitch bodies.
const TEETH_BUDGET: usize = 360_000;

/// What the view draws, built once per plan (and teeth choice).
pub(crate) struct Scene {
    parts: Vec<Part>,
    segs: Vec<Seg3>,
    /// Members whose teeth were left out for the budget.
    pub dropped: usize,
    pub triangles: usize,
    /// How long building took (ms).
    pub built_ms: f64,
}

fn normals(tris: &[Tri]) -> Vec<[f32; 3]> {
    tris.iter()
        .map(|t| {
            let (a, b, c) = (t[0], t[1], t[2]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 1e-20 { [n[0] / l, n[1] / l, n[2] / l] } else { [0.0, 0.0, 0.0] }
        })
        .collect()
}

/// The clearance body (world coordinates, translucent) and its bearing seats: a ring at each
/// end of every seat, on its member's shaft, in the member's colour.
fn clearance_parts(sch: &Schematic, plan: &Plan, clear: Option<&Clearance>, parts: &mut Vec<Part>, segs: &mut Vec<Seg3>) {
    let Some(body) = &plan.clearance else { return };
    let f = &body.frame;
    let (o, ex, ey, ez) = (f.origin, f.x, f.y(), f.z);
    let tris: Vec<Tri> = body_mesh(body, sch.tol.max(0.05), 400)
        .into_iter()
        .map(|t| {
            t.map(|v| {
                let w = add(o, add(add(mul(ex, f64::from(v[0])), mul(ey, f64::from(v[1]))), mul(ez, f64::from(v[2]))));
                [w[0] as f32, w[1] as f32, w[2] as f32]
            })
        })
        .collect();
    let normals = normals(&tris);
    parts.push(Part { body: 0, member: usize::MAX, tris, normals, alpha: 0.22, fixed: true });
    let Some(c) = clear else { return };
    let size = c.bearing.as_deref().and_then(solidcraft_gear::hub::bearing).map(|(_, od, w)| (od, w)).or(c.bearing_size.map(|b| (b.od, b.width)));
    let Some((od, width)) = size else { return };
    let r = (od + c.bearing_fit) / 2.0;
    for seat in &c.seats {
        let Seat::Along { member, along } = *seat else { continue };
        let Some(sb) = sch.bodies.iter().find(|b| b.body + 1 == member as usize) else { continue };
        let mid = match (sb.pitch.first(), sb.pitch.last()) {
            (Some(a), Some(b)) => (a[0] + b[0]) / 2.0,
            _ => (sb.z.0 + sb.z.1) / 2.0,
        };
        let n = 48;
        for z in [mid + along - width / 2.0, mid + along + width / 2.0] {
            for j in 0..n {
                let (a0, a1) = (j as f64 / n as f64 * std::f64::consts::TAU, (j + 1) as f64 / n as f64 * std::f64::consts::TAU);
                let p = |a: f64| [(r * a.cos()) as f32, (r * a.sin()) as f32, z as f32];
                segs.push(Seg3 { body: sb.body, member: sb.member, a: p(a0), b: p(a1) });
            }
        }
    }
}

fn build_scene(sch: &Schematic, plan: &Plan, teeth: bool, clear: Option<&Clearance>) -> Scene {
    let started = crate::now_ms();
    let mut parts = Vec::new();
    let mut segs = Vec::new();
    let mut used = 0usize;
    let mut dropped = 0;
    for sb in &sch.bodies {
        let body = &plan.bodies[sb.body];
        let mut tris = Vec::new();
        let rack = sb.pitch.is_empty();
        if teeth || rack {
            let t = body_mesh(body, sch.tol, 3000);
            if used + t.len() <= TEETH_BUDGET || rack {
                used += t.len();
                tris = t;
            } else {
                dropped += 1;
            }
        }
        if tris.is_empty() && !sb.pitch.is_empty() {
            let mut b = Builder::default();
            revolve(&mut b, &sb.pitch, sb.internal);
            tris = b.tris;
            // The blank's tip circles at its ends, and the teeth's slant on the pitch surface.
            let f = |p: P3| [p[0] as f32, p[1] as f32, p[2] as f32];
            let circle = |z: f64, r: f64, segs: &mut Vec<Seg3>| {
                let n = 72;
                for j in 0..n {
                    let (a0, a1) = (j as f64 / n as f64 * std::f64::consts::TAU, (j + 1) as f64 / n as f64 * std::f64::consts::TAU);
                    segs.push(Seg3 { body: sb.body, member: sb.member, a: f([r * a0.cos(), r * a0.sin(), z]), b: f([r * a1.cos(), r * a1.sin(), z]) });
                }
            };
            for &[z, r] in [sb.tip.first(), sb.tip.last()].into_iter().flatten() {
                circle(z, r, &mut segs);
            }
            if sb.helix.abs() > 0.5f64.to_radians() && sb.pitch.len() == 2 {
                let ([z0, r], [z1, _]) = (sb.pitch[0], sb.pitch[1]);
                let turn = (z1 - z0) * sb.helix.tan() / r.max(1e-9);
                for k in 0..12 {
                    let a = k as f64 / 12.0 * std::f64::consts::TAU;
                    let steps = 8;
                    for s in 0..steps {
                        let (t0, t1) = (s as f64 / steps as f64, (s + 1) as f64 / steps as f64);
                        let p = |t: f64| {
                            let aa = a + turn * t;
                            f([r * 1.002 * aa.cos(), r * 1.002 * aa.sin(), z0 + (z1 - z0) * t])
                        };
                        segs.push(Seg3 { body: sb.body, member: sb.member, a: p(t0), b: p(t1) });
                    }
                }
            }
        }
        let n = normals(&tris);
        parts.push(Part { body: sb.body, member: sb.member, tris, normals: n, alpha: 1.0, fixed: false });
    }
    clearance_parts(sch, plan, clear, &mut parts, &mut segs);
    let triangles = parts.iter().map(|p| p.tris.len()).sum();
    Scene { parts, segs, dropped, triangles, built_ms: crate::now_ms() - started }
}

// ---------------------------------------------------------------------------------------------
// Drawing.

/// Each body's placement at a driver angle: origin and its turned x, y, z.
fn poses(sch: &Schematic, t: f64) -> Vec<(P3, P3, P3, P3)> {
    let sx = sch.set_frame.x;
    let sy = sch.set_frame.y();
    sch.bodies
        .iter()
        .map(|sb| {
            let f = &sb.frame;
            let m = sb.motion;
            let a = m.spin * t;
            let (c, s) = (a.cos(), a.sin());
            let (fx, fy) = (f.x, f.y());
            let ex = add(mul(fx, c), mul(fy, s));
            let ey = add(mul(fx, -s), mul(fy, c));
            let mut o = f.origin;
            if m.slide != 0.0 && m.period > 0.0 {
                let d = (m.slide * t + m.period / 2.0).rem_euclid(m.period) - m.period / 2.0;
                o = add(o, mul(sx, d));
            }
            if m.orbit != 0.0 {
                let moved = m.orbit_center.rot(m.orbit * t) - m.orbit_center;
                o = add(o, add(mul(sx, moved.x), mul(sy, moved.y)));
            }
            (o, ex, ey, f.z)
        })
        .collect()
}

struct Raster {
    w: usize,
    h: usize,
    color: Vec<Color32>,
    depth: Vec<f32>,
    id: Vec<u8>,
}

impl Raster {
    fn new(w: usize, h: usize, bg: Color32) -> Raster {
        Raster { w, h, color: vec![bg; w * h], depth: vec![f32::NEG_INFINITY; w * h], id: vec![0; w * h] }
    }

    /// A triangle in pixel coordinates (x, y, depth: larger nearer).
    fn tri(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], col: Color32, id: u8, alpha: f32) {
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if area.abs() < 1e-6 || !area.is_finite() {
            return;
        }
        let minx = a[0].min(b[0]).min(c[0]).floor().max(0.0) as i64;
        let maxx = a[0].max(b[0]).max(c[0]).ceil().min(self.w as f32 - 1.0) as i64;
        let miny = a[1].min(b[1]).min(c[1]).floor().max(0.0) as i64;
        let maxy = a[1].max(b[1]).max(c[1]).ceil().min(self.h as f32 - 1.0) as i64;
        if minx > maxx || miny > maxy {
            return;
        }
        let inv = 1.0 / area;
        for y in miny..=maxy {
            let py = y as f32 + 0.5;
            for x in minx..=maxx {
                let px = x as f32 + 0.5;
                let w0 = ((c[0] - b[0]) * (py - b[1]) - (c[1] - b[1]) * (px - b[0])) * inv;
                let w1 = ((a[0] - c[0]) * (py - c[1]) - (a[1] - c[1]) * (px - c[0])) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                let i = y as usize * self.w + x as usize;
                if z > self.depth[i] {
                    if alpha >= 1.0 {
                        self.depth[i] = z;
                        self.color[i] = col;
                        self.id[i] = id;
                    } else {
                        let d = self.color[i];
                        let mix = |p: u8, q: u8| (f32::from(p) * (1.0 - alpha) + f32::from(q) * alpha) as u8;
                        self.color[i] = Color32::from_rgb(mix(d.r(), col.r()), mix(d.g(), col.g()), mix(d.b(), col.b()));
                    }
                }
            }
        }
    }

    /// A depth-tested line, `bias` in front.
    fn line(&mut self, a: [f32; 3], b: [f32; 3], col: Color32, bias: f32) {
        let steps = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil() as usize).clamp(1, 4000);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let (x, y, z) = (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t + bias);
            if x < 0.0 || y < 0.0 {
                continue;
            }
            let (xi, yi) = (x as usize, y as usize);
            if xi >= self.w || yi >= self.h {
                continue;
            }
            let i = yi * self.w + xi;
            if z >= self.depth[i] {
                self.color[i] = col;
            }
        }
    }
}

/// The view's camera and cached drawing, kept by the window.
pub(crate) struct View3d {
    yaw: f64,
    pitch: f64,
    zoom: f64,
    pan: P3,
    /// Show the teeth (otherwise the pitch bodies).
    pub teeth: bool,
    scene: Option<(u64, Scene)>,
    texture: Option<egui::TextureHandle>,
    drawn: Option<u64>,
    ids: (usize, usize, Vec<u8>, f32),
    /// The fitted centre and radius of the set at the build pose.
    fit: Option<(u64, P3, f64)>,
}

impl Default for View3d {
    fn default() -> Self {
        View3d { yaw: -45f64.to_radians(), pitch: 35.264f64.to_radians(), zoom: 1.0, pan: [0.0; 3], teeth: true, scene: None, texture: None, drawn: None, ids: (0, 0, Vec::new(), 1.0), fit: None }
    }
}

/// What the view needs from the window each frame.
pub(crate) struct Inputs<'a> {
    pub key: u64,
    pub sch: &'a Schematic,
    pub plan: &'a Plan,
    pub turned: f64,
    pub lit: &'a [usize],
    pub size: egui::Vec2,
    /// The clearance body's settings, for its seats (the body itself is the plan's).
    pub clear: Option<&'a Clearance>,
}

fn hash_of(parts: &[u64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

impl View3d {
    /// Back to the iso view, fitted.
    pub fn reset(&mut self) {
        let teeth = self.teeth;
        *self = View3d { teeth, scene: self.scene.take(), texture: self.texture.take(), ..View3d::default() };
    }

    /// Triangles drawn and the scene's build time (ms), for the cost line.
    pub fn cost(&self) -> Option<(usize, f64, usize)> {
        self.scene.as_ref().map(|(_, s)| (s.triangles, s.built_ms, s.dropped))
    }

    /// Draws the view; returns the member under the pointer.
    pub fn show(&mut self, ui: &mut Ui, inp: Inputs<'_>) -> Option<usize> {
        let (rect, resp) = ui.allocate_exact_size(inp.size, Sense::click_and_drag());
        let key = hash_of(&[inp.key, u64::from(self.teeth)]);
        if self.scene.as_ref().is_none_or(|(k, _)| *k != key) {
            self.scene = Some((key, build_scene(inp.sch, inp.plan, self.teeth, inp.clear)));
            self.drawn = None;
        }
        // Orbit, pan, zoom, fit.
        if resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            self.yaw -= f64::from(d.x) * 0.01;
            self.pitch = (self.pitch + f64::from(d.y) * 0.01).clamp(-1.55, 1.55);
        }
        let scene = &self.scene.as_ref().expect("built above").1;
        let fit = match self.fit {
            Some((k, c, r)) if k == key => (c, r),
            _ => {
                let p0 = poses(inp.sch, 0.0);
                let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
                for part in scene.parts.iter().filter(|p| !p.fixed) {
                    let (o, ex, ey, ez) = p0[part.body];
                    for t in part.tris.iter().step_by(7) {
                        for v in t {
                            let w = add(o, add(add(mul(ex, f64::from(v[0])), mul(ey, f64::from(v[1]))), mul(ez, f64::from(v[2]))));
                            for i in 0..3 {
                                lo[i] = lo[i].min(w[i]);
                                hi[i] = hi[i].max(w[i]);
                            }
                        }
                    }
                }
                let (c, r) = if lo[0] <= hi[0] { (mul(add(lo, hi), 0.5), (len(sub(hi, lo)) / 2.0).max(1.0)) } else { ([0.0; 3], 10.0) };
                self.fit = Some((key, c, r));
                (c, r)
            }
        };
        let (cy, sy, cp, sp) = (self.yaw.cos(), self.yaw.sin(), self.pitch.cos(), self.pitch.sin());
        let view = [cp * cy, cp * sy, sp];
        let right = [-sy, cy, 0.0];
        let up = cross(view, right);
        let scale = 0.9 * f64::from(rect.width().min(rect.height())) / (2.0 * fit.1) * self.zoom;
        if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
            let d = resp.drag_delta();
            self.pan = sub(self.pan, add(mul(right, f64::from(d.x) / scale), mul(up, -f64::from(d.y) / scale)));
        }
        if resp.hovered() {
            let s = ui.input(|i| i.smooth_scroll_delta.y);
            if s != 0.0 {
                self.zoom = (self.zoom * (f64::from(s) * 0.0025).exp()).clamp(0.05, 50.0);
                ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
        if resp.double_clicked() {
            self.zoom = 1.0;
            self.pan = [0.0; 3];
            self.yaw = -45f64.to_radians();
            self.pitch = 35.264f64.to_radians();
        }
        let target = add(fit.0, self.pan);
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();
        let bg = visuals.extreme_bg_color;
        let ink = visuals.text_color();
        // The image, at up to two pixels per point and 0.9 megapixels.
        let ppp = ui.ctx().pixels_per_point().min(2.0);
        let mut res = ppp;
        while (rect.width() * res) * (rect.height() * res) > 900_000.0 && res > 0.5 {
            res *= 0.8;
        }
        let (w, h) = ((rect.width() * res).max(1.0) as usize, (rect.height() * res).max(1.0) as usize);
        let lit_bits = inp.lit.iter().fold(0u64, |a, m| a | (1 << (m % 60)));
        let still = hash_of(&[key, self.yaw.to_bits(), self.pitch.to_bits(), self.zoom.to_bits(), target[0].to_bits(), target[1].to_bits(), target[2].to_bits(), inp.turned.to_bits(), w as u64, h as u64, lit_bits, u64::from(visuals.dark_mode)]);
        let s = scale * f64::from(res);
        let (cxp, cyp) = (w as f64 / 2.0, h as f64 / 2.0);
        let project = |p: P3| -> [f32; 3] {
            let d = sub(p, target);
            [(cxp + dot(d, right) * s) as f32, (cyp - dot(d, up) * s) as f32, dot(d, view) as f32]
        };
        let pose = poses(inp.sch, inp.turned);
        let world = |b: usize, v: [f32; 3]| -> P3 {
            let (o, ex, ey, ez) = pose[b];
            add(o, add(add(mul(ex, f64::from(v[0])), mul(ey, f64::from(v[1]))), mul(ez, f64::from(v[2]))))
        };
        if self.drawn != Some(still) || self.texture.is_none() {
            let mut r = Raster::new(w, h, bg);
            let light = unit(add(add(mul(right, -0.35), mul(up, 0.55)), mul(view, 0.75)));
            let dim = !inp.lit.is_empty();
            let mut order: Vec<&Part> = scene.parts.iter().collect();
            order.sort_by(|a, b| b.alpha.total_cmp(&a.alpha));
            for part in order {
                let base = if part.fixed { ink.lerp_to_gamma(bg, 0.35) } else { crate::gear_window::member_color(part.member) };
                let lit = inp.lit.contains(&part.member);
                let base = if dim && !lit && !part.fixed { base.gamma_multiply(0.55) } else { base };
                let (_, ex, ey, ez) = if part.fixed { ([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]) } else { pose[part.body] };
                let id = if part.fixed { 0 } else { (part.member + 1).min(250) as u8 };
                let place = |v: [f32; 3]| if part.fixed { [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])] } else { world(part.body, v) };
                for (t, n) in part.tris.iter().zip(&part.normals) {
                    let nw = add(add(mul(ex, f64::from(n[0])), mul(ey, f64::from(n[1]))), mul(ez, f64::from(n[2])));
                    let shade = (0.3 + 0.7 * dot(nw, light).abs()) as f32;
                    let col = Color32::from_rgb((f32::from(base.r()) * shade) as u8, (f32::from(base.g()) * shade) as u8, (f32::from(base.b()) * shade) as u8);
                    r.tri(project(place(t[0])), project(place(t[1])), project(place(t[2])), col, id, part.alpha);
                }
            }
            let bias = (fit.1 * 0.004) as f32;
            for sg in &scene.segs {
                let col = crate::gear_window::member_color(sg.member).lerp_to_gamma(ink, 0.55);
                r.line(project(world(sg.body, sg.a)), project(world(sg.body, sg.b)), col, bias);
            }
            let image = egui::ColorImage::new([w, h], r.color);
            match &mut self.texture {
                Some(t) => t.set(image, egui::TextureOptions::LINEAR),
                None => self.texture = Some(ui.ctx().load_texture("gear_view3d", image, egui::TextureOptions::LINEAR)),
            }
            self.ids = (w, h, r.id, res);
            self.drawn = Some(still);
        }
        if let Some(t) = &self.texture {
            painter.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        // The overlay: shafts, the angle between them, the offset or centre distance, names.
        let to_screen = |p: P3| -> Pos2 {
            let d = sub(p, target);
            pos2(rect.center().x + (dot(d, right) * scale) as f32, rect.center().y - (dot(d, up) * scale) as f32)
        };
        let overlay = Overlay { painter: &painter, to_screen: &to_screen, ink, bg };
        overlay.draw(inp.sch, &pose, fit.1);
        // The member under the pointer.
        let hovered = resp.hover_pos().and_then(|p| {
            let (w, h, ids, res) = (&self.ids.0, &self.ids.1, &self.ids.2, self.ids.3);
            let (x, y) = (((p.x - rect.left()) * res) as usize, ((p.y - rect.top()) * res) as usize);
            (x < *w && y < *h).then(|| ids[y * w + x]).filter(|i| *i > 0).map(|i| usize::from(i) - 1)
        });
        hovered
    }
}

struct Overlay<'a> {
    painter: &'a egui::Painter,
    to_screen: &'a dyn Fn(P3) -> Pos2,
    ink: Color32,
    bg: Color32,
}

impl Overlay<'_> {
    fn label(&self, at: Pos2, text: &str, color: Color32) {
        let g = self.painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(11.5), color);
        let pos = at - g.size() / 2.0;
        self.painter.rect_filled(Rect::from_min_size(pos, g.size()).expand(2.0), 3.0, self.bg.gamma_multiply(0.85));
        self.painter.galley(pos, g, color);
    }

    fn arrow_line(&self, a: P3, b: P3, color: Color32) {
        let (pa, pb) = ((self.to_screen)(a), (self.to_screen)(b));
        self.painter.extend(egui::Shape::dashed_line(&[pa, pb], Stroke::new(1.3, color), 9.0, 3.0));
        let d = pb - pa;
        if d.length() > 4.0 {
            let u = d / d.length();
            let n = vec2(-u.y, u.x);
            let tip = pb;
            let pts = vec![tip, tip - u * 9.0 + n * 4.0, tip - u * 9.0 - n * 4.0];
            self.painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }
    }

    fn draw(&self, sch: &Schematic, pose: &[(P3, P3, P3, P3)], size: f64) {
        // Each shaft once (planets share none: each its own).
        let mut drawn: Vec<(P3, P3)> = Vec::new();
        for sb in &sch.bodies {
            let (o, _, _, z) = pose[sb.body];
            if drawn.iter().any(|(p, d)| len(cross(*d, z)) < 1e-6 && len(cross(sub(o, *p), *d)) < 1e-6) {
                continue;
            }
            drawn.push((o, z));
            let (lo, hi) = sb.z;
            let ext = (hi - lo).abs().max(size * 0.25) * 0.6;
            let color = crate::gear_window::member_color(sb.member).lerp_to_gamma(self.ink, 0.25);
            self.arrow_line(add(o, mul(z, lo - ext)), add(o, mul(z, hi + ext)), color);
        }
        for (i, sb) in sch.bodies.iter().enumerate() {
            // One name per member (its first body).
            if sch.bodies.iter().position(|x| x.member == sb.member) != Some(i) {
                continue;
            }
            let (o, _, _, z) = pose[sb.body];
            let mid = add(o, mul(z, (sb.z.0 + sb.z.1) / 2.0));
            let at = (self.to_screen)(mid);
            let color = crate::gear_window::member_color(sb.member).lerp_to_gamma(self.ink, 0.5);
            self.label(at, &sb.name, color);
        }
        let Some((a1, a2)) = sch.axes() else { return };
        let k = size * 0.35;
        match sch.relation {
            Some(Relation::Parallel { distance, foot1, foot2 }) => {
                let (p, q) = ((self.to_screen)(foot1), (self.to_screen)(foot2));
                self.painter.line_segment([p, q], Stroke::new(1.0, self.ink.gamma_multiply(0.8)));
                self.label(p.lerp(q, 0.5), &format!("a = {}", fmt(distance)), self.ink);
            }
            Some(Relation::Intersecting { apex, .. }) => {
                // The shaft angle between the rays the gears sit on.
                let ray = |a: &Axis| {
                    let t = dot(sub(a.origin, apex), a.dir);
                    if t < 0.0 { mul(a.dir, -1.0) } else { a.dir }
                };
                let (s1, s2) = (ray(&a1), ray(&a2));
                let (r1, r2) = (
                    sch.bodies.iter().find(|b| b.member == 0).map(|b| side(b, pose, apex)).unwrap_or(s1),
                    sch.bodies.iter().find(|b| b.member == 1).map(|b| side(b, pose, apex)).unwrap_or(s2),
                );
                self.arc(apex, r1, r2, k, &format!("Σ {}°", fmt(angle(r1, r2).to_degrees())));
                self.painter.circle_filled((self.to_screen)(apex), 3.0, self.ink);
            }
            Some(Relation::Skew { offset, foot1, foot2, .. }) => {
                let (p, q) = ((self.to_screen)(foot1), (self.to_screen)(foot2));
                self.painter.line_segment([p, q], Stroke::new(1.0, self.ink.gamma_multiply(0.8)));
                self.label(p.lerp(q, 0.5), &format!("e = {}", fmt(offset)), self.ink);
                // Axis 2's direction carried to axis 1's foot, and the angle between them.
                let ghost = (self.to_screen)(add(foot1, mul(a2.dir, k * 1.4)));
                self.painter.extend(egui::Shape::dashed_line(&[(self.to_screen)(foot1), ghost], Stroke::new(1.0, self.ink.gamma_multiply(0.5)), 4.0, 4.0));
                self.arc(foot1, a1.dir, a2.dir, k, &format!("Σ {}°", fmt(angle(a1.dir, a2.dir).to_degrees())));
            }
            _ => {}
        }
    }

    fn arc(&self, at: P3, d1: P3, d2: P3, r: f64, text: &str) {
        let (u, v) = (unit(d1), unit(d2));
        let ang = angle(u, v);
        let w = unit(sub(v, mul(u, dot(u, v))));
        let n = 24;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = ang * i as f64 / n as f64;
                (self.to_screen)(add(at, mul(add(mul(u, a.cos()), mul(w, a.sin())), r)))
            })
            .collect();
        let mid = pts[n / 2];
        self.painter.add(egui::Shape::line(pts, Stroke::new(1.4, self.ink)));
        self.label(mid, text, self.ink);
    }
}

/// The ray from `apex` a body sits on.
fn side(b: &SchemBody, pose: &[(P3, P3, P3, P3)], apex: P3) -> P3 {
    let (o, _, _, z) = pose[b.body];
    let mid = add(o, mul(z, (b.z.0 + b.z.1) / 2.0));
    let t = dot(sub(mid, apex), z);
    if t < 0.0 { mul(z, -1.0) } else { z }
}

fn angle(a: P3, b: P3) -> f64 {
    len(cross(a, b)).atan2(dot(a, b))
}

pub(crate) fn fmt(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_gear::plan::{Kind, Spec, plan};

    /// Every kind's default set makes its 3D preview quickly and in bounded size, with and
    /// without teeth (a bevel planetary set once ran away with memory in the window).
    #[test]
    fn every_default_set_previews_in_bounds() {
        for k in Kind::ALL {
            let spec = Spec::defaults_for(k);
            let Ok(p) = plan(&spec) else { continue };
            for teeth in [false, true] {
                let t = std::time::Instant::now();
                let sch = schematic_of(&spec, &p);
                let scene = build_scene(&sch, &p, teeth, None);
                let ms = t.elapsed().as_secs_f64() * 1e3;
                let segs = scene.segs.len();
                eprintln!("{k:?} teeth={teeth}: {} triangles, {segs} lines, {ms:.0} ms", scene.triangles);
                assert!(scene.triangles < 3_000_000 && segs < 1_000_000 && ms < 5000.0, "{k:?} teeth={teeth}: {} triangles, {segs} lines, {ms:.0} ms", scene.triangles);
            }
        }
    }
}
