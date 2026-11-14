//! A 2D mesh checker for the tests: members are polygonised regions with a rigid motion, and a
//! pair is probed over driver angles for overlap (area and penetration depth) and for the gap
//! between the flanks. Conjugate teeth overlap nowhere and stay in (near) contact throughout.

use crate::geom::{Region, V2};
use crate::plan::PreviewGear;

/// A rigid motion against the driver angle t: turned `speed`·t about a centre that goes round
/// the origin `orbit`·t and slides `slide`·t along +x.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub center: V2,
    pub speed: f64,
    pub slide: f64,
    pub orbit: f64,
}

impl Motion {
    pub fn turning(center: V2, speed: f64) -> Motion {
        Motion { center, speed, slide: 0.0, orbit: 0.0 }
    }
    /// The preview's own motion (a sliding member is not wrapped by its period here).
    pub fn of(g: &PreviewGear) -> Motion {
        Motion { center: g.center, speed: g.speed, slide: g.slide, orbit: g.orbit }
    }
    /// (angle, translation): a local point q is at q.rot(angle) + translation.
    fn pose(&self, t: f64) -> (f64, V2) {
        (self.speed * t, self.center.rot(self.orbit * t) + V2::new(self.slide * t, 0.0))
    }
}

struct Edge {
    a: V2,
    b: V2,
    hole: bool,
}

/// A region as closed polylines with the material on their left, and a grid over its edges for
/// fast inside and distance queries in its own frame.
pub struct Member {
    loops: Vec<Vec<V2>>,
    edges: Vec<Edge>,
    lo: V2,
    c: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
    solid: Vec<bool>,
    rows: Vec<Vec<u32>>,
    pub motion: Motion,
}

fn signed_area(p: &[V2]) -> f64 {
    p.windows(2).map(|w| w[0].cross(w[1])).sum::<f64>() * 0.5
}

fn seg_dist(p: V2, a: V2, b: V2) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    let t = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + d * t)
}

impl Member {
    /// `cell`: the grid's cell size; distances are exact up to it, and edges are no longer than
    /// half of it. `tol`: chord tolerance of arcs.
    pub fn new(region: &Region, motion: Motion, cell: f64, tol: f64) -> Member {
        let mut loops = Vec::new();
        let mut edges = Vec::new();
        for (outlines, hole) in [(&region.outer, false), (&region.holes, true)] {
            for o in outlines {
                let mut p = o.polyline(tol);
                if p.len() < 3 {
                    continue;
                }
                if p[0].dist(p[p.len() - 1]) > 1e-12 {
                    p.push(p[0]);
                }
                // Material on the left: outer loops counter-clockwise, holes clockwise.
                if (signed_area(&p) < 0.0) != hole {
                    p.reverse();
                }
                // No edge longer than half a cell.
                let mut q = vec![p[0]];
                for w in p.windows(2) {
                    let n = (w[0].dist(w[1]) / (0.5 * cell)).ceil().max(1.0) as usize;
                    for i in 1..=n {
                        q.push(w[0].lerp(w[1], i as f64 / n as f64));
                    }
                }
                for w in q.windows(2) {
                    if w[0].dist(w[1]) > 0.0 {
                        edges.push(Edge { a: w[0], b: w[1], hole });
                    }
                }
                loops.push(q);
            }
        }
        let (mut lo, mut hi) = (V2::new(f64::MAX, f64::MAX), V2::new(f64::MIN, f64::MIN));
        for q in loops.iter().flatten() {
            lo = V2::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = V2::new(hi.x.max(q.x), hi.y.max(q.y));
        }
        let c = cell.max((hi.x - lo.x).max(hi.y - lo.y) / 400.0);
        lo = lo - V2::new(2.0 * c, 2.0 * c);
        let nx = ((hi.x - lo.x) / c).ceil() as usize + 3;
        let ny = ((hi.y - lo.y) / c).ceil() as usize + 3;
        let mut m = Member { loops, edges, lo, c, nx, ny, cells: vec![Vec::new(); nx * ny], solid: vec![false; nx * ny], rows: vec![Vec::new(); ny], motion };
        for (k, e) in m.edges.iter().enumerate() {
            let (i0, j0) = m.cell(V2::new(e.a.x.min(e.b.x), e.a.y.min(e.b.y))).unwrap();
            let (i1, j1) = m.cell(V2::new(e.a.x.max(e.b.x), e.a.y.max(e.b.y))).unwrap();
            for j in j0..=j1 {
                m.rows[j].push(k as u32);
                for i in i0..=i1 {
                    m.cells[j * nx + i].push(k as u32);
                }
            }
        }
        for j in 0..ny {
            for i in 0..nx {
                if m.cells[j * nx + i].is_empty() {
                    m.solid[j * nx + i] = m.raycast(m.lo + V2::new((i as f64 + 0.5) * c, (j as f64 + 0.5) * c));
                }
            }
        }
        m
    }

    pub fn of(g: &PreviewGear, cell: f64, tol: f64) -> Member {
        Member::new(&g.region, Motion::of(g), cell, tol)
    }

    fn cell(&self, p: V2) -> Option<(usize, usize)> {
        let (x, y) = ((p.x - self.lo.x) / self.c, (p.y - self.lo.y) / self.c);
        if x < 0.0 || y < 0.0 || x >= self.nx as f64 || y >= self.ny as f64 {
            return None;
        }
        Some((x as usize, y as usize))
    }

    fn raycast(&self, p: V2) -> bool {
        let Some((_, j)) = self.cell(p) else { return false };
        let (mut outer, mut hole) = (false, false);
        for &k in &self.rows[j] {
            let e = &self.edges[k as usize];
            if (e.a.y > p.y) != (e.b.y > p.y) {
                let x = e.a.x + (p.y - e.a.y) / (e.b.y - e.a.y) * (e.b.x - e.a.x);
                if x > p.x {
                    if e.hole {
                        hole = !hole;
                    } else {
                        outer = !outer;
                    }
                }
            }
        }
        outer && !hole
    }

    /// Is the local point in the material?
    pub fn inside(&self, p: V2) -> bool {
        match self.cell(p) {
            None => false,
            Some((i, j)) => {
                if self.cells[j * self.nx + i].is_empty() {
                    self.solid[j * self.nx + i]
                } else {
                    self.raycast(p)
                }
            }
        }
    }

    /// The distance from the local point to the boundary when it is under a cell away (else
    /// infinity or some larger distance).
    pub fn dist(&self, p: V2) -> f64 {
        let Some((i, j)) = self.cell(p) else { return f64::INFINITY };
        let mut d = f64::INFINITY;
        for jj in j.saturating_sub(1)..=(j + 1).min(self.ny - 1) {
            for ii in i.saturating_sub(1)..=(i + 1).min(self.nx - 1) {
                for &k in &self.cells[jj * self.nx + ii] {
                    let e = &self.edges[k as usize];
                    d = d.min(seg_dist(p, e.a, e.b));
                }
            }
        }
        d
    }
}

/// What a pair does over a range of driver angles.
#[derive(Clone, Copy, Debug)]
pub struct Stat {
    /// Largest overlap area (mm²) and the driver angle it happens at.
    pub area: f64,
    pub area_at: f64,
    /// Deepest penetration of one boundary into the other's material (mm).
    pub depth: f64,
    /// Smallest and largest (over the angles) of the least distance between the boundaries
    /// where they don't overlap (mm).
    pub gap_min: f64,
    pub gap_max: f64,
}

impl Stat {
    pub fn merge(&mut self, o: &Stat) {
        if o.area > self.area {
            self.area = o.area;
            self.area_at = o.area_at;
        }
        self.depth = self.depth.max(o.depth);
        self.gap_min = self.gap_min.min(o.gap_min);
        self.gap_max = self.gap_max.max(o.gap_max);
    }
}

/// `a`'s boundary against `b`'s material at driver angle t: (area term, depth, gap). The area
/// term is the part of Green's integral over the pieces of `a`'s boundary inside `b`, about `o`.
fn pass(a: &Member, b: &Member, t: f64, o: V2) -> (f64, f64, f64) {
    let (aa, ta) = a.motion.pose(t);
    let (ab, tb) = b.motion.pose(t);
    let (s, c) = aa.sin_cos();
    let (sr, cr) = (aa - ab).sin_cos();
    let d = (ta - tb).rot(-ab);
    let world = |q: V2| V2::new(q.x * c - q.y * s, q.x * s + q.y * c) + ta;
    let local = |q: V2| V2::new(q.x * cr - q.y * sr, q.x * sr + q.y * cr) + d;
    let (mut area, mut depth, mut gap) = (0.0, 0.0f64, f64::INFINITY);
    for lp in &a.loops {
        // The loop's last point repeats its first.
        let n = lp.len() - 1;
        let pts: Vec<(V2, bool)> = lp[..n].iter().map(|q| (*q, b.inside(local(*q)))).collect();
        for (q, inside) in &pts {
            let dist = b.dist(local(*q));
            if *inside {
                depth = depth.max(dist.min(b.c));
            } else {
                gap = gap.min(dist);
            }
        }
        let Some(start) = pts.iter().position(|(_, inside)| !inside) else {
            // Wholly inside: the loop's own area.
            area += (0..n).map(|k| 0.5 * (world(pts[k].0) - o).cross(world(pts[(k + 1) % n].0) - o)).sum::<f64>();
            continue;
        };
        // Each run of the boundary inside `b`, from where it enters to where it leaves, closed by
        // the chord back: its area does not depend on `o`, so runs that the other boundary
        // answers only in part (a corner through one edge) still add up.
        let mut entry = V2::ZERO;
        for k in 0..n {
            let ((q0, in0), (q1, in1)) = (pts[(start + k) % n], pts[(start + k + 1) % n]);
            if !in0 && !in1 {
                continue;
            }
            let (w0, w1) = (world(q0) - o, world(q1) - o);
            if in0 && in1 {
                area += 0.5 * w0.cross(w1);
                continue;
            }
            // The crossing, by bisection on the edge.
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                if b.inside(local(q0.lerp(q1, mid))) == in0 { lo = mid } else { hi = mid }
            }
            let x = w0.lerp(w1, 0.5 * (lo + hi));
            if in1 {
                entry = x;
                area += 0.5 * x.cross(w1);
            } else {
                area += 0.5 * w0.cross(x) + 0.5 * x.cross(entry);
            }
        }
    }
    (area, depth, gap)
}

/// The pair at one driver angle.
pub fn probe(a: &Member, b: &Member, t: f64) -> Stat {
    let o = (a.motion.pose(t).1 + b.motion.pose(t).1) * 0.5;
    let (a1, d1, g1) = pass(a, b, t, o);
    let (a2, d2, g2) = pass(b, a, t, o);
    let gap = g1.min(g2);
    Stat { area: (a1 + a2).abs(), area_at: t, depth: d1.max(d2), gap_min: gap, gap_max: gap }
}

/// The pair over `steps` + 1 driver angles from `t0` to `t1`.
pub fn run(a: &Member, b: &Member, t0: f64, t1: f64, steps: usize) -> Stat {
    let mut s = probe(a, b, t0);
    for i in 1..=steps {
        s.merge(&probe(a, b, t0 + (t1 - t0) * i as f64 / steps as f64));
    }
    s
}
