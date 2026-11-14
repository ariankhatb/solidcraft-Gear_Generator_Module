//! A pure-math check that the members of a 3D gear set mesh: each planned body becomes a point
//! membership test (what its solid steps describe, in its frame), the volume two bodies share is
//! estimated on a jittered grid over the zone where both can be, and a member can be spun about
//! its own axis to find the free play and to step the pair through its motion.
//!
//! The sweeps are modelled as the steps define them (a prism's twist linear in height, a taper's
//! as the logarithmic spiral about its apex, a coil's section carried up a helix), so this
//! proves the plan's geometry; the kernel is checked separately with `inspect.interference`.

use std::f64::consts::{PI, TAU};

use crate::geom::{Outline, Region, V2};
use crate::plan::{Body, Frame, Op, Step};

/// A closed polygon with its edges bucketed by angle about the origin, for fast inside tests.
pub struct Poly {
    pts: Vec<V2>,
    buckets: Vec<Vec<u32>>,
    pub rmax: f64,
}

const BUCKETS: usize = 1440;

impl Poly {
    pub fn new(o: &Outline) -> Poly {
        Poly::from_points(o.polyline(0.0005))
    }
    pub fn from_points(mut pts: Vec<V2>) -> Poly {
        if pts.len() > 1 && pts[0].dist(pts[pts.len() - 1]) < 1e-12 {
            pts.pop();
        }
        let n = pts.len();
        let mut buckets = vec![Vec::new(); BUCKETS];
        let w = TAU / BUCKETS as f64;
        let mut rmax = 0.0f64;
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            rmax = rmax.max(a.len());
            let fa = a.angle();
            let mut d = b.angle() - fa;
            while d > PI {
                d -= TAU;
            }
            while d < -PI {
                d += TAU;
            }
            let (lo, hi) = if d >= 0.0 { (fa, fa + d) } else { (fa + d, fa) };
            let (i0, i1) = ((lo / w).floor() as i64 - 1, (hi / w).floor() as i64 + 1);
            for k in i0..=i1 {
                let idx = k.rem_euclid(BUCKETS as i64) as usize;
                if buckets[idx].last() != Some(&(i as u32)) {
                    buckets[idx].push(i as u32);
                }
            }
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
        let mut f = u.angle();
        if f < 0.0 {
            f += TAU;
        }
        let idx = ((f / (TAU / BUCKETS as f64)).floor() as usize).min(BUCKETS - 1);
        let n = self.pts.len();
        let mut inside = false;
        for &i in &self.buckets[idx] {
            let (a, b) = (self.pts[i as usize], self.pts[(i as usize + 1) % n]);
            let (sa, sb) = (u.cross(a), u.cross(b));
            if (sa > 0.0) != (sb > 0.0) {
                let t = sa / (sa - sb);
                let q = a + (b - a) * t;
                if q.dot(u) > rho {
                    inside = !inside;
                }
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

enum Part {
    Prism { outer: Vec<Poly>, holes: Vec<Poly>, z0: f64, height: f64, twist: f64 },
    Taper { poly: Poly, z0: f64, height: f64, twist: f64, scale: f64 },
    Revolve { profile: Vec<V2> },
    Coil { section: Vec<V2>, pitch: f64, turns: f64, left: bool, zlo: f64, zhi: f64 },
}

impl Part {
    fn contains(&self, p: [f64; 3]) -> bool {
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
}

/// How far a taper's section has turned where its scale is `s`: the plan's law (see
/// [`Step::Taper`]).
pub fn taper_turn(twist: f64, scale: f64, s: f64) -> f64 {
    crate::plan::taper_turn(twist, scale, s)
}

/// A planned body as a solid to test points against, with a spin about its own axis.
pub struct Solid {
    pub frame: Frame,
    parts: Vec<(Part, Op)>,
    pub spin: f64,
    /// A cylinder about the axis that holds the body: radius and z range (frame coordinates).
    pub radius: f64,
    pub z: (f64, f64),
}

impl Solid {
    pub fn new(body: &Body) -> Solid {
        let mut parts = Vec::new();
        let (mut radius, mut zr) = (0.0f64, (f64::MAX, f64::MIN));
        for st in &body.steps {
            let adds = |op: &Op| matches!(op, Op::New | Op::Join);
            match st {
                Step::Prism { region, z0, height, twist, op } => {
                    let polys = |v: &Vec<Outline>| v.iter().map(Poly::new).collect::<Vec<_>>();
                    let (outer, holes) = (polys(&region.outer), polys(&region.holes));
                    if adds(op) {
                        radius = outer.iter().fold(radius, |a, o| a.max(o.rmax));
                        zr = (zr.0.min(*z0), zr.1.max(z0 + height));
                    }
                    parts.push((Part::Prism { outer, holes, z0: *z0, height: *height, twist: *twist }, *op));
                }
                Step::Taper { outline, z0, height, twist, scale, op } => {
                    let poly = Poly::new(outline);
                    if adds(op) {
                        radius = radius.max(poly.rmax * scale.max(1.0));
                        zr = (zr.0.min(*z0), zr.1.max(z0 + height));
                    }
                    parts.push((Part::Taper { poly, z0: *z0, height: *height, twist: *twist, scale: *scale }, *op));
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
                Step::Loft { .. } => panic!("lofts are not modelled"),
            }
        }
        Solid { frame: body.frame, parts, spin: 0.0, radius, z: zr }
    }
    #[allow(dead_code)]
    pub fn region_only(region: &Region, frame: Frame, z0: f64, height: f64) -> Solid {
        Solid::new(&Body { name: String::new(), frame, steps: vec![Step::Prism { region: region.clone(), z0, height, twist: 0.0, op: Op::New }] })
    }
    fn local(&self, p: [f64; 3]) -> [f64; 3] {
        let f = &self.frame;
        let y = f.y();
        let d = [p[0] - f.origin[0], p[1] - f.origin[1], p[2] - f.origin[2]];
        let dot = |a: [f64; 3]| d[0] * a[0] + d[1] * a[1] + d[2] * a[2];
        let q = V2::new(dot(f.x), dot(y)).rot(-self.spin);
        [q.x, q.y, dot(f.z)]
    }
    pub fn contains(&self, p: [f64; 3]) -> bool {
        let l = self.local(p);
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
    /// The world box that holds the body's bounding cylinder.
    pub fn bounds(&self) -> ([f64; 3], [f64; 3]) {
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
}

/// The points of a jittered grid (cells of about `cell`, at most `max_points`) over the zone
/// both bodies can reach; returns them with the volume each stands for.
pub fn zone(a: &Solid, b: &Solid, cell: f64, max_points: f64) -> (Vec<[f64; 3]>, f64) {
    let ((la, ha), (lb, hb)) = (a.bounds(), b.bounds());
    let lo = [0, 1, 2].map(|i| la[i].max(lb[i]));
    let hi = [0, 1, 2].map(|i| ha[i].min(hb[i]));
    let size = [0, 1, 2].map(|i| (hi[i] - lo[i]).max(0.0));
    let vol = size[0] * size[1] * size[2];
    if vol <= 0.0 {
        return (Vec::new(), 0.0);
    }
    let cell = cell.max((vol / max_points).cbrt());
    let n = [0, 1, 2].map(|i| (size[i] / cell).ceil().max(1.0) as usize);
    let d = [0, 1, 2].map(|i| size[i] / n[i] as f64);
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut pts = Vec::new();
    for i in 0..n[0] {
        for j in 0..n[1] {
            for k in 0..n[2] {
                pts.push([lo[0] + (i as f64 + rnd()) * d[0], lo[1] + (j as f64 + rnd()) * d[1], lo[2] + (k as f64 + rnd()) * d[2]]);
            }
        }
    }
    (pts, d[0] * d[1] * d[2])
}

/// The volume (mm³) the two bodies share, to the grid's resolution.
pub fn overlap(a: &Solid, b: &Solid, cell: f64) -> f64 {
    let (pts, dv) = zone(a, b, cell, 6.0e6);
    pts.iter().filter(|p| a.contains(**p) && b.contains(**p)).count() as f64 * dv
}

/// How far `b` can turn either way from its spin (radians, `a` held) before the two share more
/// than `eps` mm³: (back, forward), both ≥ 0; None when they already do.
pub fn play(a: &Solid, b: &mut Solid, cell: f64, eps: f64, reach: f64) -> Option<(f64, f64)> {
    let (pts, dv) = zone(a, b, cell, 3.0e6);
    let in_a: Vec<[f64; 3]> = pts.into_iter().filter(|p| a.contains(*p)).collect();
    let spin0 = b.spin;
    let vol = |b: &mut Solid, turn: f64| {
        b.spin = spin0 + turn;
        let v = in_a.iter().filter(|p| b.contains(**p)).count() as f64 * dv;
        b.spin = spin0;
        v
    };
    if vol(b, 0.0) > eps {
        return None;
    }
    let mut side = |sign: f64| {
        let steps = 24;
        let mut free = 0.0;
        let mut hit = reach;
        for i in 1..=steps {
            let t = reach * i as f64 / steps as f64;
            if vol(b, sign * t) > eps {
                hit = t;
                break;
            }
            free = t;
        }
        if free >= reach {
            return reach;
        }
        for _ in 0..7 {
            let mid = 0.5 * (free + hit);
            if vol(b, sign * mid) > eps { hit = mid } else { free = mid }
        }
        free
    };
    let back = side(-1.0);
    let fwd = side(1.0);
    Some((back, fwd))
}
