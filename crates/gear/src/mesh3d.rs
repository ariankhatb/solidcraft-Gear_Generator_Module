//! A pure-math check that the members of a 3D gear set mesh.
//!
//! A planned body's solid is known exactly from its steps: at a height in the body's frame a
//! prism, taper or loft is its outline turned, scaled or blended between sections; a coil is its
//! section carried up a helix; a revolve is a polygon turned about the axis. So a [`Solid`] can
//! say whether a point is inside it ([`Solid::contains`]) and give points on its surface with
//! their normals ([`Surface`]). Two bodies are compared by measuring every surface point of one
//! against the other's surface ([`mesh_stat`]): how deep the deepest one is inside (penetration),
//! how near the nearest one is outside (gap), and about how much volume they share. Each body
//! can be turned about its own axis ([`Solid::spin`]), so a pair can be stepped through its
//! motion ([`Pair::run`]): no penetration and a gap of about the backlash at every step is a set
//! that meshes; a gap that opens means the teeth are apart, not in mesh.
//!
//! The sweeps are modelled as the steps define them (a prism's twist linear in height, a
//! taper's by the logarithm of its scale, a loft straight between corresponding points of its
//! sections), so this proves the plan's geometry; what the kernel makes of it is checked
//! separately with `inspect.interference`.

use std::f64::consts::{PI, TAU};
use std::ops::Range;

use crate::geom::{Outline, Seg, V2, natural_spline};
use crate::plan::{Body, Frame, Op, Step, taper_turn};

type P3 = [f64; 3];

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn arc_sweep(c: V2, a: V2, b: V2, ccw: bool) -> (f64, f64, f64) {
    let r = c.dist(a);
    let a0 = (a - c).angle();
    let mut sweep = (b - c).angle() - a0;
    if ccw {
        while sweep <= 0.0 {
            sweep += TAU;
        }
    } else {
        while sweep >= 0.0 {
            sweep -= TAU;
        }
    }
    (r, a0, sweep)
}

/// How many pieces each segment of `o` is cut into for points about `h` apart (a spline: each
/// of its spans).
pub fn seg_counts(o: &Outline, h: f64) -> Vec<usize> {
    let n = |len: f64| ((len / h).ceil() as usize).clamp(1, 4000);
    o.segs
        .iter()
        .map(|s| match s {
            Seg::Line(a, b) => n(a.dist(*b)),
            Seg::Arc { c, a, b, ccw } => {
                let (r, _, sweep) = arc_sweep(*c, *a, *b, *ccw);
                n(r * sweep.abs()).max((sweep.abs() / 0.05).ceil() as usize)
            }
            Seg::Spline(p) => n(p.windows(2).map(|w| w[0].dist(w[1])).fold(0.0, f64::max) * 1.05).max(4),
            Seg::Circle(_, r) => n(TAU * r).max(48),
        })
        .collect()
}

/// The outline as a closed polygon (the last point is not the first again), each segment cut
/// into its count of pieces: outlines of the same make sampled with the same counts give
/// points that correspond.
pub fn polygon(o: &Outline, counts: &[usize]) -> Vec<V2> {
    let mut out: Vec<V2> = Vec::new();
    for (s, n) in o.segs.iter().zip(counts) {
        let mut pts = Vec::new();
        match s {
            Seg::Line(a, b) => pts.extend((0..=*n).map(|i| a.lerp(*b, i as f64 / *n as f64))),
            Seg::Arc { c, a, b, ccw } => {
                let (r, a0, sweep) = arc_sweep(*c, *a, *b, *ccw);
                pts.extend((0..=*n).map(|i| *c + V2::polar(r, a0 + sweep * i as f64 / *n as f64)));
            }
            Seg::Spline(p) => natural_spline(p, *n, &mut pts),
            Seg::Circle(c, r) => pts.extend((0..=*n).map(|i| *c + V2::polar(*r, TAU * i as f64 / *n as f64))),
        }
        if !out.is_empty() && !pts.is_empty() {
            pts.remove(0);
        }
        out.extend(pts);
    }
    if out.len() > 1 && out[0].dist(out[out.len() - 1]) < 1e-9 {
        out.pop();
    }
    out
}

const BUCKETS: usize = 1440;

fn bucket_of(u: V2) -> usize {
    let mut f = u.angle();
    if f < 0.0 {
        f += TAU;
    }
    ((f / (TAU / BUCKETS as f64)).floor() as usize).min(BUCKETS - 1)
}

/// The angles an edge covers seen from the origin: (from, to), `to` ≥ `from`.
fn edge_span(a: V2, b: V2) -> (f64, f64) {
    let fa = a.angle();
    let d = crate::geom::wrap(b.angle() - fa);
    if d >= 0.0 { (fa, fa + d) } else { (fa + d, fa) }
}

fn fill_buckets(buckets: &mut [Vec<u32>], i: usize, lo: f64, hi: f64) {
    let w = TAU / BUCKETS as f64;
    let (i0, i1) = ((lo / w).floor() as i64 - 1, (hi / w).floor() as i64 + 1);
    if i1 - i0 >= BUCKETS as i64 {
        for b in buckets.iter_mut() {
            b.push(i as u32);
        }
        return;
    }
    for k in i0..=i1 {
        let idx = k.rem_euclid(BUCKETS as i64) as usize;
        if buckets[idx].last() != Some(&(i as u32)) {
            buckets[idx].push(i as u32);
        }
    }
}

/// Does the ray from `p` straight away from the origin cross the edge a→b?
fn crosses(a: V2, b: V2, u: V2, rho: f64) -> bool {
    let (sa, sb) = (u.cross(a), u.cross(b));
    if (sa > 0.0) != (sb > 0.0) {
        let t = sa / (sa - sb);
        (a + (b - a) * t).dot(u) > rho
    } else {
        false
    }
}

/// A closed polygon with its edges bucketed by angle about the origin, for fast inside tests.
pub struct Poly {
    pub pts: Vec<V2>,
    buckets: Vec<Vec<u32>>,
    pub rmax: f64,
}

impl Poly {
    pub fn new(o: &Outline, h: f64) -> Poly {
        Poly::from_points(polygon(o, &seg_counts(o, h)))
    }
    pub fn from_points(mut pts: Vec<V2>) -> Poly {
        if pts.len() > 1 && pts[0].dist(pts[pts.len() - 1]) < 1e-12 {
            pts.pop();
        }
        let n = pts.len();
        let mut buckets = vec![Vec::new(); BUCKETS];
        let mut rmax = 0.0f64;
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            rmax = rmax.max(a.len());
            let (lo, hi) = edge_span(a, b);
            fill_buckets(&mut buckets, i, lo, hi);
        }
        Poly { pts, buckets, rmax }
    }
    /// Inside, by the crossings of the ray from `p` straight away from the origin.
    pub fn contains(&self, p: V2) -> bool {
        let rho = p.len();
        if rho > self.rmax {
            return false;
        }
        let u = if rho > 1e-12 { p * (1.0 / rho) } else { V2::new(1.0, 0.0) };
        let n = self.pts.len();
        let mut inside = false;
        for &i in &self.buckets[bucket_of(u)] {
            if crosses(self.pts[i as usize], self.pts[(i as usize + 1) % n], u, rho) {
                inside = !inside;
            }
        }
        inside
    }
}

/// Two corresponding polygons on planes z = `z0` and `z1`, joined straight point to point.
struct Slab {
    a: Vec<V2>,
    b: Vec<V2>,
    z0: f64,
    z1: f64,
    buckets: Vec<Vec<u32>>,
    rmax: f64,
}

impl Slab {
    fn new(a: Vec<V2>, b: Vec<V2>, z0: f64, z1: f64) -> Slab {
        let n = a.len();
        let mut buckets = vec![Vec::new(); BUCKETS];
        let mut rmax = 0.0f64;
        for i in 0..n {
            let j = (i + 1) % n;
            rmax = rmax.max(a[i].len()).max(b[i].len());
            let (lo, hi) = edge_span(a[i], a[j]);
            let (lo2, hi2) = edge_span(b[i], b[j]);
            // The second edge's span, on the same turn as the first's.
            let shift = crate::geom::wrap(lo2 - lo) - (lo2 - lo);
            fill_buckets(&mut buckets, i, lo.min(lo2 + shift), hi.max(hi2 + shift));
        }
        Slab { a, b, z0, z1, buckets, rmax }
    }
    fn contains(&self, p: V2, z: f64) -> bool {
        let rho = p.len();
        if rho > self.rmax {
            return false;
        }
        let t = if self.z1 > self.z0 { (z - self.z0) / (self.z1 - self.z0) } else { 0.0 };
        let u = if rho > 1e-12 { p * (1.0 / rho) } else { V2::new(1.0, 0.0) };
        let n = self.a.len();
        let at = |i: usize| self.a[i].lerp(self.b[i], t);
        let mut inside = false;
        for &i in &self.buckets[bucket_of(u)] {
            if crosses(at(i as usize), at((i as usize + 1) % n), u, rho) {
                inside = !inside;
            }
        }
        inside
    }
}

/// Inside a small polygon (any shape), by ray crossings along +x.
fn in_polygon(pts: &[V2], p: V2) -> bool {
    let n = pts.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

/// A polygon with points added along its edges so none is longer than `h`.
fn refined(pts: &[V2], h: f64) -> Vec<V2> {
    let n = pts.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let k = ((a.dist(b) / h).ceil() as usize).max(1);
        out.extend((0..k).map(|j| a.lerp(b, j as f64 / k as f64)));
    }
    out
}

enum Part {
    Prism { outer: Vec<Poly>, holes: Vec<Poly>, z0: f64, height: f64, twist: f64 },
    Taper { poly: Poly, z0: f64, height: f64, twist: f64, scale: f64 },
    Loft { slabs: Vec<Slab> },
    Revolve { profile: Vec<V2> },
    Coil { section: Vec<V2>, pitch: f64, turns: f64, left: bool, zlo: f64, zhi: f64 },
}

impl Part {
    fn contains(&self, p: P3) -> bool {
        let q = V2::new(p[0], p[1]);
        match self {
            Part::Prism { outer, holes, z0, height, twist } => {
                let t = (p[2] - z0) / height;
                if !(0.0..=1.0).contains(&t) {
                    return false;
                }
                let q = q.rot(-twist * t);
                outer.iter().any(|o| o.contains(q)) && !holes.iter().any(|o| o.contains(q))
            }
            Part::Taper { poly, z0, height, twist, scale } => {
                let t = (p[2] - z0) / height;
                if !(0.0..=1.0).contains(&t) {
                    return false;
                }
                let s = 1.0 + (scale - 1.0) * t;
                poly.contains(q.rot(-taper_turn(*twist, *scale, s)) * (1.0 / s))
            }
            Part::Loft { slabs } => slabs.iter().find(|s| p[2] >= s.z0 && p[2] <= s.z1).is_some_and(|s| s.contains(q, p[2])),
            Part::Revolve { profile } => in_polygon(profile, V2::new(q.len(), p[2])),
            Part::Coil { section, pitch, turns, left, zlo, zhi } => {
                let r = q.len();
                let mut th = q.angle();
                if *left {
                    th = -th;
                }
                if th < 0.0 {
                    th += TAU;
                }
                // The section at angle th + 2πn sits pitch·(th/2π + n) up the axis.
                let base = p[2] - pitch * th / TAU;
                let (n0, n1) = (((base - zhi) / pitch).floor() as i64, ((base - zlo) / pitch).ceil() as i64);
                for n in n0.max(0)..=n1 {
                    if th / TAU + n as f64 > *turns {
                        break;
                    }
                    if in_polygon(section, V2::new(r, base - pitch * n as f64)) {
                        return true;
                    }
                }
                false
            }
        }
    }
    /// The part's side surface as rows of corresponding points (each row a closed loop), the
    /// rows about `hz` apart: one list of rows per sheet.
    fn sheets(&self, h: f64, hz: f64) -> Vec<Vec<Vec<P3>>> {
        let layers = |len: f64| ((len.abs() / hz).ceil() as usize).clamp(1, 20000);
        let mut out = Vec::new();
        match self {
            Part::Prism { outer, holes, z0, height, twist } => {
                for poly in outer.iter().chain(holes) {
                    let turned = poly.rmax * twist.abs();
                    let n = layers(height.hypot(turned));
                    out.push(
                        (0..=n)
                            .map(|j| {
                                let t = j as f64 / n as f64;
                                poly.pts.iter().map(|q| q.rot(twist * t)).map(|q| [q.x, q.y, z0 + height * t]).collect()
                            })
                            .collect(),
                    );
                }
            }
            Part::Taper { poly, z0, height, twist, scale } => {
                let n = layers(height.hypot(poly.rmax * twist.abs()));
                out.push(
                    (0..=n)
                        .map(|j| {
                            let t = j as f64 / n as f64;
                            let s = 1.0 + (scale - 1.0) * t;
                            let turn = taper_turn(*twist, *scale, s);
                            poly.pts.iter().map(|q| q.rot(turn) * s).map(|q| [q.x, q.y, z0 + height * t]).collect()
                        })
                        .collect(),
                );
            }
            Part::Loft { slabs } => {
                for s in slabs {
                    let n = layers(s.z1 - s.z0);
                    out.push(
                        (0..=n)
                            .map(|j| {
                                let t = j as f64 / n as f64;
                                s.a.iter().zip(&s.b).map(|(a, b)| a.lerp(*b, t)).map(|q| [q.x, q.y, s.z0 + (s.z1 - s.z0) * t]).collect()
                            })
                            .collect(),
                    );
                }
            }
            Part::Revolve { profile } => {
                let pts = refined(profile, h);
                let rmax = pts.iter().map(|q| q.x).fold(0.0, f64::max);
                let n = layers(TAU * rmax).max(24);
                out.push((0..=n).map(|j| pts.iter().map(|q| V2::polar(q.x, TAU * j as f64 / n as f64)).zip(&pts).map(|(c, q)| [c.x, c.y, q.y]).collect()).collect());
            }
            Part::Coil { section, pitch, turns, left, .. } => {
                let pts = refined(section, h);
                let rmax = pts.iter().map(|q| q.x).fold(0.0, f64::max);
                let n = layers(turns * (TAU * rmax).hypot(*pitch)).max(24);
                let hand = if *left { -1.0 } else { 1.0 };
                out.push(
                    (0..=n)
                        .map(|j| {
                            let th = TAU * turns * j as f64 / n as f64;
                            pts.iter().map(|q| (V2::polar(q.x, hand * th), q.y + pitch * th / TAU)).map(|(c, z)| [c.x, c.y, z]).collect()
                        })
                        .collect(),
                );
            }
        }
        out
    }
}

/// A planned body as a solid to test points against, with a spin about its own axis.
pub struct Solid {
    pub frame: Frame,
    parts: Vec<(Part, Op)>,
    /// How far the body is turned about its frame's z axis (radians, counter-clockwise).
    pub spin: f64,
    /// A cylinder about the axis that holds the body: radius and z range (frame coordinates).
    pub radius: f64,
    pub z: (f64, f64),
    h: f64,
}

impl Solid {
    /// The body's solid, its curved outlines as polygons with points about `h` apart.
    pub fn new(body: &Body, h: f64) -> Solid {
        let mut parts = Vec::new();
        let (mut radius, mut zr) = (0.0f64, (f64::MAX, f64::MIN));
        for st in &body.steps {
            let adds = |op: &Op| matches!(op, Op::New | Op::Join);
            match st {
                Step::Prism { region, z0, height, twist, op } => {
                    let polys = |v: &Vec<Outline>| v.iter().map(|o| Poly::new(o, h)).collect::<Vec<_>>();
                    let (outer, holes) = (polys(&region.outer), polys(&region.holes));
                    if adds(op) {
                        radius = outer.iter().fold(radius, |a, o| a.max(o.rmax));
                        zr = (zr.0.min(*z0), zr.1.max(z0 + height));
                    }
                    parts.push((Part::Prism { outer, holes, z0: *z0, height: *height, twist: *twist }, *op));
                }
                Step::Taper { outline, z0, height, twist, scale, op } => {
                    let poly = Poly::new(outline, h);
                    if adds(op) {
                        radius = radius.max(poly.rmax * scale.max(1.0));
                        zr = (zr.0.min(*z0), zr.1.max(z0 + height));
                    }
                    parts.push((Part::Taper { poly, z0: *z0, height: *height, twist: *twist, scale: *scale }, *op));
                }
                Step::Loft { sections, op } => {
                    // The same cut of every section, fine enough for the largest.
                    let mut counts = seg_counts(&sections[0].0, h);
                    for (o, _) in sections {
                        assert_eq!(o.segs.len(), counts.len(), "loft sections must be made of the same segments");
                        for (c, k) in counts.iter_mut().zip(seg_counts(o, h)) {
                            *c = (*c).max(k);
                        }
                    }
                    let polys: Vec<Vec<V2>> = sections.iter().map(|(o, _)| polygon(o, &counts)).collect();
                    assert!(polys.iter().all(|p| p.len() == polys[0].len()), "loft sections must correspond point for point");
                    let slabs: Vec<Slab> = (0..sections.len() - 1).map(|j| Slab::new(polys[j].clone(), polys[j + 1].clone(), sections[j].1, sections[j + 1].1)).collect();
                    if adds(op) {
                        radius = slabs.iter().fold(radius, |a, s| a.max(s.rmax));
                        zr = (zr.0.min(sections[0].1), zr.1.max(sections[sections.len() - 1].1));
                    }
                    parts.push((Part::Loft { slabs }, *op));
                }
                Step::Revolve { profile, op } => {
                    if adds(op) {
                        radius = profile.iter().fold(radius, |a, q| a.max(q.x));
                        zr = profile.iter().fold(zr, |a, q| (a.0.min(q.y), a.1.max(q.y)));
                    }
                    parts.push((Part::Revolve { profile: profile.clone() }, *op));
                }
                Step::Coil { section, pitch, height, left, op, .. } => {
                    let zlo = section.iter().map(|q| q.y).fold(f64::MAX, f64::min);
                    let zhi = section.iter().map(|q| q.y).fold(f64::MIN, f64::max);
                    if adds(op) {
                        radius = section.iter().fold(radius, |a, q| a.max(q.x));
                        zr = (zr.0.min(zlo), zr.1.max(zhi + height));
                    }
                    parts.push((Part::Coil { section: section.clone(), pitch: *pitch, turns: height / pitch, left: *left, zlo, zhi }, *op));
                }
            }
        }
        Solid { frame: body.frame, parts, spin: 0.0, radius, z: zr, h }
    }
    /// A world point in the body's own coordinates (its spin undone).
    pub fn local(&self, p: P3) -> P3 {
        let f = &self.frame;
        let d = sub(p, f.origin);
        let q = V2::new(dot(d, f.x), dot(d, f.y())).rot(-self.spin);
        [q.x, q.y, dot(d, f.z)]
    }
    /// A point of the body (its own coordinates) in the world, the body turned by its spin.
    pub fn world(&self, p: P3) -> P3 {
        let q = V2::new(p[0], p[1]).rot(self.spin);
        self.frame.world([q.x, q.y, p[2]])
    }
    fn world_dir(&self, n: P3) -> P3 {
        let q = V2::new(n[0], n[1]).rot(self.spin);
        let (x, y, z) = (self.frame.x, self.frame.y(), self.frame.z);
        [0, 1, 2].map(|i| q.x * x[i] + q.y * y[i] + n[2] * z[i])
    }
    fn contains_local(&self, l: P3) -> bool {
        if l[2] < self.z.0 || l[2] > self.z.1 || l[0] * l[0] + l[1] * l[1] > self.radius * self.radius {
            return false;
        }
        let mut inside = false;
        for (part, op) in &self.parts {
            match op {
                Op::New | Op::Join => inside = inside || part.contains(l),
                Op::Cut => inside = inside && !part.contains(l),
                Op::Intersect => inside = inside && part.contains(l),
            }
        }
        inside
    }
    /// Is the world point inside the body?
    pub fn contains(&self, p: P3) -> bool {
        self.contains_local(self.local(p))
    }
    /// Is the world point within `margin` of the cylinder that holds the body?
    fn near(&self, p: P3, margin: f64) -> bool {
        let l = self.local(p);
        l[2] >= self.z.0 - margin && l[2] <= self.z.1 + margin && l[0] * l[0] + l[1] * l[1] <= (self.radius + margin).powi(2)
    }
    /// The world box that holds the body's bounding cylinder.
    pub fn bounds(&self) -> (P3, P3) {
        let f = &self.frame;
        let (mut lo, mut hi) = ([0.0; 3], [0.0; 3]);
        for i in 0..3 {
            let side = self.radius * (1.0 - f.z[i] * f.z[i]).max(0.0).sqrt();
            let (a, b) = (f.origin[i] + f.z[i] * self.z.0, f.origin[i] + f.z[i] * self.z.1);
            lo[i] = a.min(b) - side;
            hi[i] = a.max(b) + side;
        }
        (lo, hi)
    }
    /// The band of this body (least radius, z range, its own coordinates) that can come within
    /// `margin` of `other`'s bounding cylinder however either is turned.
    fn reach_of(&self, other: &Solid, margin: f64) -> (f64, f64, f64) {
        let (mut rmin, mut zlo, mut zhi) = (f64::MAX, f64::MAX, f64::MIN);
        let rr = other.radius + margin;
        for k in 0..=24 {
            let z = other.z.0 - margin + (other.z.1 - other.z.0 + 2.0 * margin) * f64::from(k) / 24.0;
            for i in 0..96 {
                for f in [1.0, 0.66, 0.33, 0.0] {
                    let c = V2::polar(rr * f, TAU * f64::from(i) / 96.0);
                    let l = self.local(other.frame.world([c.x, c.y, z]));
                    rmin = rmin.min(l[0].hypot(l[1]));
                    zlo = zlo.min(l[2]);
                    zhi = zhi.max(l[2]);
                }
            }
        }
        // The probe points are a grid: allow for what lies between them.
        let slack = 0.05 * rr + (other.z.1 - other.z.0) / 24.0;
        ((rmin - slack).max(0.0), zlo - slack, zhi + slack)
    }
}

/// A point on a body's surface (the body's own coordinates), the outward normal there, and
/// the area it stands for.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub p: P3,
    pub n: P3,
    /// Half the patch's two sides: the sample stands for the parallelogram p ± hu ± hv.
    pub hu: P3,
    pub hv: P3,
    pub area: f64,
}

const SECTORS: usize = 90;

/// The part of a body's surface that can meet another body, as samples grouped by their angle
/// about the axis.
pub struct Surface {
    pub samples: Vec<Sample>,
    sectors: Vec<Range<usize>>,
    /// About how far a sample is from the corners of the patch it stands for.
    reach: f64,
}

impl Surface {
    /// Samples of `solid`'s surface where it can meet `other`: about `h` apart round the
    /// outlines and `hz` apart along the sweeps.
    pub fn of(solid: &Solid, other: &Solid, hz: f64) -> Surface {
        let h = solid.h;
        let (rmin, zlo, zhi) = solid.reach_of(other, 1.0);
        let eps = 0.02 * h;
        let mut samples = Vec::new();
        for (part, _) in &solid.parts {
            for rows in part.sheets(h, hz) {
                for w in rows.windows(2) {
                    let (r0, r1) = (&w[0], &w[1]);
                    let n = r0.len();
                    for i in 0..n {
                        let j = (i + 1) % n;
                        let (a, b, c, d) = (r0[i], r0[j], r1[i], r1[j]);
                        let p = [0, 1, 2].map(|k| 0.25 * (a[k] + b[k] + c[k] + d[k]));
                        if p[2] < zlo || p[2] > zhi || p[0].hypot(p[1]) < rmin {
                            continue;
                        }
                        let du = [0, 1, 2].map(|k| 0.5 * (b[k] + d[k] - a[k] - c[k]));
                        let dv = [0, 1, 2].map(|k| 0.5 * (c[k] + d[k] - a[k] - b[k]));
                        let nn = cross(du, dv);
                        let area = dot(nn, nn).sqrt();
                        if area < 1e-14 {
                            continue;
                        }
                        let mut nn = nn.map(|v| v / area);
                        // On the finished body's boundary, and which way is out.
                        let inner = solid.contains_local([0, 1, 2].map(|k| p[k] - eps * nn[k]));
                        let outer = solid.contains_local([0, 1, 2].map(|k| p[k] + eps * nn[k]));
                        if inner == outer {
                            continue;
                        }
                        if outer {
                            nn = nn.map(|v| -v);
                        }
                        samples.push(Sample { p, n: nn, hu: du.map(|v| 0.5 * v), hv: dv.map(|v| 0.5 * v), area });
                    }
                }
            }
        }
        let sector = |s: &Sample| (((s.p[1].atan2(s.p[0]) + PI) / TAU * SECTORS as f64).floor() as usize).min(SECTORS - 1);
        samples.sort_by_key(sector);
        let mut sectors = Vec::with_capacity(SECTORS);
        let mut at = 0;
        for k in 0..SECTORS {
            let start = at;
            while at < samples.len() && sector(&samples[at]) == k {
                at += 1;
            }
            sectors.push(start..at);
        }
        Surface { samples, sectors, reach: 0.5 * h.hypot(hz) }
    }
    /// The samples near `other` as the two bodies are turned now, in world coordinates.
    fn facing(&self, solid: &Solid, other: &Solid, margin: f64) -> Vec<Sample> {
        let mut out = Vec::new();
        for r in &self.sectors {
            let s = &self.samples[r.clone()];
            // A sector none of whose scattered samples is anywhere near is skipped whole.
            let stride = (s.len() / 150).max(1);
            if !s.iter().step_by(stride).any(|q| other.near(solid.world(q.p), margin + 1.5)) {
                continue;
            }
            for q in s {
                let p = solid.world(q.p);
                if other.near(p, margin) {
                    out.push(Sample { p, n: solid.world_dir(q.n), hu: solid.world_dir(q.hu), hv: solid.world_dir(q.hv), area: q.area });
                }
            }
        }
        out
    }
}

/// How two bodies sit against each other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stat {
    /// How deep the deepest surface point of one is inside the other (mm; 0: none is).
    pub penetration: f64,
    /// How near the nearest surface point of one is to the other's surface (mm; 0 when they
    /// penetrate, [`Stat::FAR`] when nothing is within reach).
    pub gap: f64,
    /// About how much volume the two share (mm³).
    pub volume: f64,
    /// Where the deepest point is (or, with none inside, the nearest): world coordinates.
    pub at: [f64; 3],
}

impl Stat {
    /// The gap reported when the surfaces are further apart than the check looks.
    pub const FAR: f64 = 1.0;
}

/// Each query sample against the target body: (deepest inside, nearest outside, shared volume).
fn one_way(queries: &[Sample], target: &Solid, ts: &[Sample], cell: f64) -> (f64, f64, f64, P3) {
    if queries.is_empty() {
        return (0.0, Stat::FAR, 0.0, [0.0; 3]);
    }
    let (mut at_deep, mut at_near) = ([0.0; 3], [0.0; 3]);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for s in ts {
        for k in 0..3 {
            lo[k] = lo[k].min(s.p[k]);
            hi[k] = hi[k].max(s.p[k]);
        }
    }
    if ts.is_empty() {
        lo = [0.0; 3];
        hi = [0.0; 3];
    }
    let dims = if ts.is_empty() { [1usize; 3] } else { [0, 1, 2].map(|k| ((hi[k] - lo[k]) / cell).floor() as usize + 1) };
    let index = |p: P3| -> Option<[i64; 3]> {
        let c = [0, 1, 2].map(|k| ((p[k] - lo[k]) / cell).floor() as i64);
        Some(c)
    };
    let flat = |c: [i64; 3]| -> Option<usize> {
        if (0..3).all(|k| c[k] >= 0 && (c[k] as usize) < dims[k]) { Some((c[0] as usize * dims[1] + c[1] as usize) * dims[2] + c[2] as usize) } else { None }
    };
    // Counting sort of the target's samples into the grid's cells.
    let cells = dims[0] * dims[1] * dims[2];
    let mut start = vec![0u32; cells + 1];
    let keys: Vec<usize> = ts.iter().map(|s| flat(index(s.p).unwrap()).unwrap()).collect();
    for k in &keys {
        start[k + 1] += 1;
    }
    for i in 0..cells {
        start[i + 1] += start[i];
    }
    let mut order = vec![0u32; ts.len()];
    let mut fill = start.clone();
    for (i, k) in keys.iter().enumerate() {
        order[fill[*k] as usize] = i as u32;
        fill[*k] += 1;
    }
    let (mut deep, mut near, mut volume) = (0.0f64, Stat::FAR, 0.0f64);
    for q in queries {
        let c = index(q.p).unwrap();
        // The nearest patch (a sample with the parallelogram it stands for): how far, which
        // side of it, and whether the nearest point is within the patch or on its border.
        let mut best: Option<(f64, f64, bool)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(f) = flat([c[0] + dx, c[1] + dy, c[2] + dz]) else { continue };
                    for &i in &order[start[f] as usize..start[f + 1] as usize] {
                        let t = &ts[i as usize];
                        let d = sub(q.p, t.p);
                        if dot(d, d) > cell * cell {
                            continue;
                        }
                        let (uu, uv, vv) = (dot(t.hu, t.hu), dot(t.hu, t.hv), dot(t.hv, t.hv));
                        let (du, dv) = (dot(d, t.hu), dot(d, t.hv));
                        let det = (uu * vv - uv * uv).max(1e-300);
                        let (a, b) = ((du * vv - dv * uv) / det, (dv * uu - du * uv) / det);
                        let (ac, bc) = (a.clamp(-1.0, 1.0), b.clamp(-1.0, 1.0));
                        let r = [0, 1, 2].map(|k| d[k] - ac * t.hu[k] - bc * t.hv[k]);
                        let dist = dot(r, r).sqrt();
                        if best.is_none_or(|x| dist < x.0) {
                            best = Some((dist, dot(d, t.n), a == ac && b == bc));
                        }
                    }
                }
            }
        }
        let signed = match best {
            Some((dist, along, true)) => {
                if along >= 0.0 {
                    dist
                } else {
                    -dist
                }
            }
            // Nearest a patch's border (past an edge of the body, or of the sampled zone): the
            // body itself says which side, asked only when it could matter.
            Some((dist, _, false)) => {
                if dist >= near && dist < 0.5 * cell {
                    continue;
                } else if target.contains(q.p) {
                    -dist
                } else {
                    dist
                }
            }
            None => {
                if target.contains(q.p) {
                    -cell
                } else {
                    continue;
                }
            }
        };
        if signed < 0.0 {
            if -signed > deep {
                deep = -signed;
                at_deep = q.p;
            }
            volume += -signed * q.area;
        } else if signed < near {
            near = signed;
            at_near = q.p;
        }
    }
    (deep, near, volume, if deep > 0.0 { at_deep } else { at_near })
}

/// How the two bodies sit against each other as they are turned now.
pub fn mesh_stat(a: &Solid, sa: &Surface, b: &Solid, sb: &Surface) -> Stat {
    let cell = 0.3f64.max(2.0 * sa.reach).max(2.0 * sb.reach);
    let (wa, wb) = (sa.facing(a, b, cell), sb.facing(b, a, cell));
    let (d1, g1, v1, p1) = one_way(&wa, b, &wb, cell);
    let (d2, g2, v2, p2) = one_way(&wb, a, &wa, cell);
    let penetration = d1.max(d2);
    let at = if penetration > 0.0 { if d1 >= d2 { p1 } else { p2 } } else if g1 <= g2 { p1 } else { p2 };
    Stat { penetration, gap: if penetration > 0.0 { 0.0 } else { g1.min(g2) }, volume: 0.5 * (v1 + v2), at }
}

/// What a pair does over a stretch of its motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    /// The deepest penetration at any step, the driver angle it was at and where (world).
    pub penetration: f64,
    pub at: f64,
    pub place: [f64; 3],
    /// Where the gap was smallest (world), and at which driver angle.
    pub gap_place: [f64; 3],
    pub gap_at: f64,
    /// The smallest and the largest gap over the steps: both about the backlash when the teeth
    /// stay in mesh.
    pub gap_min: f64,
    pub gap_max: f64,
    /// The most volume shared at any step.
    pub volume: f64,
}

/// Two bodies with their surfaces sampled where they can meet.
pub struct Pair {
    pub a: Solid,
    pub b: Solid,
    sa: Surface,
    sb: Surface,
}

impl Pair {
    /// `h`: the spacing of the surface samples round the outlines (about 0.03 modules is good);
    /// along the sweeps they are `hz` apart.
    pub fn new(a: &Body, b: &Body, h: f64, hz: f64) -> Pair {
        let (a, b) = (Solid::new(a, h), Solid::new(b, h));
        let (sa, sb) = (Surface::of(&a, &b, hz), Surface::of(&b, &a, hz));
        Pair { a, b, sa, sb }
    }
    /// The pair with the first body turned `ta` and the second `tb` about their own axes.
    pub fn at(&mut self, ta: f64, tb: f64) -> Stat {
        self.a.spin = ta;
        self.b.spin = tb;
        mesh_stat(&self.a, &self.sa, &self.b, &self.sb)
    }
    /// The pair stepped through its motion: the first body from `t0` to `t1` in `steps` steps,
    /// the second turned `ratio` times as far (plus `offset`).
    pub fn run(&mut self, ratio: f64, offset: f64, t0: f64, t1: f64, steps: usize) -> Run {
        let mut r = Run { penetration: 0.0, at: t0, place: [0.0; 3], gap_place: [0.0; 3], gap_at: t0, gap_min: f64::MAX, gap_max: 0.0, volume: 0.0 };
        for i in 0..steps {
            let t = t0 + (t1 - t0) * i as f64 / steps as f64;
            let s = self.at(t, ratio * t + offset);
            if s.penetration > r.penetration {
                r.penetration = s.penetration;
                r.at = t;
                r.place = s.at;
            }
            if s.gap < r.gap_min && s.penetration == 0.0 {
                r.gap_place = s.at;
                r.gap_at = t;
            }
            r.gap_min = r.gap_min.min(s.gap);
            r.gap_max = r.gap_max.max(s.gap);
            r.volume = r.volume.max(s.volume);
        }
        r
    }
    pub fn samples(&self) -> (usize, usize) {
        (self.sa.samples.len(), self.sb.samples.len())
    }
}

/// The volume (mm³) two bodies share, counted on a jittered grid of cells about `cell` across
/// over the zone both can reach: slower than [`mesh_stat`]'s estimate and blind to anything
/// thinner than a cell, but independent of it.
pub fn overlap(a: &Solid, b: &Solid, cell: f64) -> f64 {
    let ((la, ha), (lb, hb)) = (a.bounds(), b.bounds());
    let lo = [0, 1, 2].map(|i| la[i].max(lb[i]));
    let hi = [0, 1, 2].map(|i| ha[i].min(hb[i]));
    let size = [0, 1, 2].map(|i| (hi[i] - lo[i]).max(0.0));
    let vol = size[0] * size[1] * size[2];
    if vol <= 0.0 {
        return 0.0;
    }
    let cell = cell.max((vol / 6.0e6).cbrt());
    let n = [0, 1, 2].map(|i| (size[i] / cell).ceil().max(1.0) as usize);
    let d = [0, 1, 2].map(|i| size[i] / n[i] as f64);
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut count = 0usize;
    for i in 0..n[0] {
        for j in 0..n[1] {
            for k in 0..n[2] {
                let p = [lo[0] + (i as f64 + rnd()) * d[0], lo[1] + (j as f64 + rnd()) * d[1], lo[2] + (k as f64 + rnd()) * d[2]];
                if a.contains(p) && b.contains(p) {
                    count += 1;
                }
            }
        }
    }
    count as f64 * d[0] * d[1] * d[2]
}
