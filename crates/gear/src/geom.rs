//! Plane geometry the gear outlines are made of: points, segments (lines, arcs, fit point
//! splines) and closed outlines, with sampling for previews.

use std::f64::consts::{PI, TAU};
use std::ops::{Add, Mul, Neg, Sub};

/// A point or vector in the plane, millimetres.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    /// The point at radius `r` and polar angle `a` (radians, counter-clockwise from +x).
    pub fn polar(r: f64, a: f64) -> Self {
        let (s, c) = a.sin_cos();
        Self { x: r * c, y: r * s }
    }
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }
    pub fn dot(self, o: V2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: V2) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn dist(self, o: V2) -> f64 {
        (self - o).len()
    }
    /// Turned counter-clockwise by `a` radians about the origin.
    pub fn rot(self, a: f64) -> Self {
        let (s, c) = a.sin_cos();
        Self { x: self.x * c - self.y * s, y: self.x * s + self.y * c }
    }
    pub fn norm(self) -> Self {
        let l = self.len();
        if l > 0.0 { self * (1.0 / l) } else { self }
    }
    /// Mirrored across the x axis.
    pub fn flip_y(self) -> Self {
        Self { x: self.x, y: -self.y }
    }
    pub fn lerp(self, o: V2, t: f64) -> Self {
        self + (o - self) * t
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 {
        V2::new(self.x * k, self.y * k)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        V2::new(-self.x, -self.y)
    }
}

/// One piece of an outline, in the direction the outline runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Line(V2, V2),
    /// An arc about `c` from `a` to `b`: counter-clockwise when `ccw`, else clockwise.
    Arc { c: V2, a: V2, b: V2, ccw: bool },
    /// A fit point spline through the points, first to last.
    Spline(Vec<V2>),
    /// A whole circle (only as an outline of its own).
    Circle(V2, f64),
}

impl Seg {
    pub fn start(&self) -> V2 {
        match self {
            Seg::Line(a, _) => *a,
            Seg::Arc { a, .. } => *a,
            Seg::Spline(p) => p[0],
            Seg::Circle(c, r) => *c + V2::new(*r, 0.0),
        }
    }
    pub fn end(&self) -> V2 {
        match self {
            Seg::Line(_, b) => *b,
            Seg::Arc { b, .. } => *b,
            Seg::Spline(p) => p[p.len() - 1],
            Seg::Circle(c, r) => *c + V2::new(*r, 0.0),
        }
    }
    /// Points along the segment, from its start to its end, `tol` the most a chord may stray from
    /// an arc. Splines give their fit points with a Catmull-Rom curve between them.
    pub fn sample(&self, tol: f64, out: &mut Vec<V2>) {
        match self {
            Seg::Line(a, b) => {
                out.push(*a);
                out.push(*b);
            }
            Seg::Arc { c, a, b, ccw } => {
                let r = c.dist(*a);
                let a0 = (*a - *c).angle();
                let mut sweep = (*b - *c).angle() - a0;
                if *ccw {
                    while sweep <= 0.0 {
                        sweep += TAU;
                    }
                } else {
                    while sweep >= 0.0 {
                        sweep -= TAU;
                    }
                }
                let n = arc_steps(r, sweep.abs(), tol);
                for i in 0..=n {
                    let t = a0 + sweep * i as f64 / n as f64;
                    out.push(*c + V2::polar(r, t));
                }
            }
            Seg::Spline(p) => catmull_rom(p, 6, out),
            Seg::Circle(c, r) => {
                let n = arc_steps(*r, TAU, tol);
                for i in 0..=n {
                    out.push(*c + V2::polar(*r, TAU * i as f64 / n as f64));
                }
            }
        }
    }
    /// The segment moved by `f`, which must be a rigid motion or a uniform scale (arcs stay arcs);
    /// `mirror` says whether `f` reverses orientation.
    pub fn map(&self, f: &dyn Fn(V2) -> V2, mirror: bool) -> Seg {
        match self {
            Seg::Line(a, b) => Seg::Line(f(*a), f(*b)),
            Seg::Arc { c, a, b, ccw } => Seg::Arc { c: f(*c), a: f(*a), b: f(*b), ccw: *ccw != mirror },
            Seg::Spline(p) => Seg::Spline(p.iter().map(|q| f(*q)).collect()),
            Seg::Circle(c, r) => {
                let k = f(*c + V2::new(*r, 0.0)).dist(f(*c));
                Seg::Circle(f(*c), k)
            }
        }
    }
    /// The same piece run the other way.
    pub fn reversed(&self) -> Seg {
        match self {
            Seg::Line(a, b) => Seg::Line(*b, *a),
            Seg::Arc { c, a, b, ccw } => Seg::Arc { c: *c, a: *b, b: *a, ccw: !*ccw },
            Seg::Spline(p) => Seg::Spline(p.iter().rev().copied().collect()),
            Seg::Circle(c, r) => Seg::Circle(*c, *r),
        }
    }
}

fn arc_steps(r: f64, sweep: f64, tol: f64) -> usize {
    let tol = tol.max(1e-6);
    let step = if r > tol { 2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos() } else { PI };
    ((sweep / step.max(1e-3)).ceil() as usize).clamp(1, 720)
}

/// A centripetal Catmull-Rom curve through `p`, `n` points per span: close to the sketch's
/// fit point spline, for previews.
fn catmull_rom(p: &[V2], n: usize, out: &mut Vec<V2>) {
    if p.len() < 3 {
        out.extend_from_slice(p);
        return;
    }
    let get = |i: isize| -> V2 {
        if i < 0 {
            p[0] * 2.0 - p[1]
        } else if i as usize >= p.len() {
            p[p.len() - 1] * 2.0 - p[p.len() - 2]
        } else {
            p[i as usize]
        }
    };
    out.push(p[0]);
    for i in 0..p.len() - 1 {
        let (p0, p1, p2, p3) = (get(i as isize - 1), get(i as isize), get(i as isize + 1), get(i as isize + 2));
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let t2 = t * t;
            let t3 = t2 * t;
            let q = (p1 * 2.0 + (p2 - p0) * t + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2 + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3) * 0.5;
            out.push(q);
        }
    }
}

/// A closed outline: segments end to end, the last ending where the first starts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    pub segs: Vec<Seg>,
}

impl Outline {
    pub fn circle(c: V2, r: f64) -> Self {
        Self { segs: vec![Seg::Circle(c, r)] }
    }
    /// Points round the outline (closed: the last equals the first).
    pub fn polyline(&self, tol: f64) -> Vec<V2> {
        let mut out = Vec::new();
        for s in &self.segs {
            let mut pts = Vec::new();
            s.sample(tol, &mut pts);
            if !out.is_empty() && !pts.is_empty() {
                pts.remove(0);
            }
            out.extend(pts);
        }
        out
    }
    pub fn map(&self, f: &dyn Fn(V2) -> V2, mirror: bool) -> Outline {
        let mut segs: Vec<Seg> = self.segs.iter().map(|s| s.map(f, mirror)).collect();
        if mirror {
            segs = segs.iter().rev().map(Seg::reversed).collect();
        }
        Outline { segs }
    }
    pub fn rotated(&self, a: f64) -> Outline {
        self.map(&|p| p.rot(a), false)
    }
    pub fn translated(&self, d: V2) -> Outline {
        self.map(&|p| p + d, false)
    }
    pub fn scaled(&self, k: f64) -> Outline {
        self.map(&|p| p * k, false)
    }
    /// The largest distance from the origin among the outline's sample points.
    pub fn max_radius(&self) -> f64 {
        self.polyline(0.05).iter().map(|p| p.len()).fold(0.0, f64::max)
    }
    /// Signed area (positive counter-clockwise) of the sampled outline.
    pub fn area(&self) -> f64 {
        let p = self.polyline(0.01);
        let mut a = 0.0;
        for w in p.windows(2) {
            a += w[0].cross(w[1]);
        }
        a * 0.5
    }
}

/// A planar region: an outer outline and the holes in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    pub outer: Vec<Outline>,
    pub holes: Vec<Outline>,
}

/// The involute function: tan a − a.
pub fn inv(a: f64) -> f64 {
    a.tan() - a
}

/// The angle whose involute function is `v` (0 ≤ v < ~2), by Newton's method.
pub fn inv_inv(v: f64) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    // Cheng's starting guess, then Newton on tan a − a − v (derivative tan² a).
    let mut a = (3.0 * v).cbrt() - 0.2 * v.powf(1.0 / 3.0) * v.powf(2.0 / 3.0).min(1.0);
    a = a.clamp(1e-6, 1.5);
    for _ in 0..50 {
        let t = a.tan();
        let f = t - a - v;
        let d = t * t;
        if d <= 0.0 {
            break;
        }
        let next = (a - f / d).clamp(1e-9, 1.5707);
        if (next - a).abs() < 1e-15 {
            a = next;
            break;
        }
        a = next;
    }
    a
}

/// Wraps an angle into (−π, π].
pub fn wrap(a: f64) -> f64 {
    let mut a = a % TAU;
    if a > PI {
        a -= TAU;
    } else if a <= -PI {
        a += TAU;
    }
    a
}

/// Wraps `a` into (−period/2, period/2].
pub fn wrap_period(a: f64, period: f64) -> f64 {
    let mut a = a % period;
    if a > period / 2.0 {
        a -= period;
    } else if a <= -period / 2.0 {
        a += period;
    }
    a
}
