//! Hypoid gears: an offset pinion with curved (face-milled, circular-arc) teeth driving a conical
//! ring gear, as in a car's final drive.
//!
//! # Method
//!
//! The pitch cones are worked out from first principles, in the same terms as ISO 23509 and
//! Gleason's hypoid calculation (Litvin & Fuentes, *Gear Geometry and Applied Theory*, ch. 21;
//! Stadtfeld, *Handbook of Bevel and Hypoid Gears*):
//!
//! - The ring gear's axis is z; the pinion's axis is square to the common perpendicular (y) at
//!   the shaft angle Σ, `offset` (E) away. The ring gear's outer pitch diameter and face width
//!   give its mean pitch radius r_m2 = (d_e2 − b2·sin δ2) / 2.
//! - The two pitch cones touch at the mean point P with a common tangent plane: P is on the
//!   gear's mean pitch circle, the gear cone's normal there is the pinion cone's (reversed), and
//!   that normal lies in the pinion's axial plane through P. That fixes the pinion axis' height
//!   (the crossing point) for a given angular position of P.
//! - The two cone generatrices through P make the offset angle ε′ in the pitch plane, so the
//!   tooth line through P has spiral angle β1 against the pinion's generatrix and β2 = β1 − ε′
//!   against the gear's: the pinion's spiral angle is the larger one.
//! - The normal pitches agree at P: r_m1·cos β1 / z1 = r_m2·cos β2 / z2. Solved for P's angular
//!   position this makes the pinion larger than a bevel pinion of the same ratio by
//!   cos β2 / cos β1 (its "enlargement").
//! - One more condition is needed to settle the cones. ISO 23509 iterates the wheel pitch angle
//!   until the limit curvature matches the cutter; here it is held at its first estimate, the
//!   bevel value tan δ2 = u·sin Σ / (1 + u·cos Σ) (simplified, documented choice: the pitch
//!   angles then differ from a full ISO calculation by a fraction of a degree to a degree or two).
//! - Depths are Gleason's: mean working depth 2·m_mn, gear addendum c·h with
//!   c = 0.21 + 0.29 / u², the pinion's the rest (long addendum pinion), clearance from the basic
//!   rack. Teeth taper to each member's own pitch apex.
//! - Tooth lines are circular arcs (a face-milling cutter of radius r_c) through P, centred in the
//!   pitch plane; each member's tooth trace is that circle developed onto its own pitch cone.
//!
//! Tooth sections are Tredgold back-cone profiles centrally projected onto planes square to each
//! member's axis between the heel and toe pitch circles, turned along the tooth trace and lofted.

use std::f64::consts::{PI, TAU};

use crate::geom::{Outline, Region, Seg, V2};
use crate::involute::{Involute, ToothFlank, external_flank, teeth_outline};
use crate::plan::{Body, Frame, Op, Plan, PreviewGear, Spec, Step, Unit};

type V3 = [f64; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: V3) -> V3 {
    let l = len(a);
    if l > 1e-300 { mul(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}

/// A hypoid set's pitch cones, spiral angles and proportions, in a frame with the ring gear's
/// axis along z through the origin and its mean pitch circle on z = 0 (left-hand pinion; a
/// right-hand set is this one mirrored in y).
#[derive(Clone, Debug)]
pub struct Design {
    pub z1: f64,
    pub z2: f64,
    /// Shaft angle Σ and offset E (as used, after any clamping).
    pub sigma: f64,
    pub offset: f64,
    /// Pitch cone angles δ1, δ2.
    pub delta1: f64,
    pub delta2: f64,
    /// Mean spiral angles (magnitudes) β1 > β2 and the offset angle in the pitch plane ε′.
    pub beta1: f64,
    pub beta2: f64,
    pub eps_pitch: f64,
    /// Where the mean point lies round each axis: the angle from the plane through the axis
    /// parallel to the other axis to the axial plane through P (gear, pinion).
    pub eps_gear: f64,
    pub eta_pinion: f64,
    /// Mean pitch radii, mean and outer cone distances, outer pitch radii, face widths.
    pub rm1: f64,
    pub rm2: f64,
    pub cone_m1: f64,
    pub cone_m2: f64,
    pub cone_e1: f64,
    pub cone_e2: f64,
    pub re1: f64,
    pub re2: f64,
    pub b1: f64,
    pub b2: f64,
    /// Mean normal module, mean addenda and dedenda.
    pub mmn: f64,
    pub ha1: f64,
    pub hf1: f64,
    pub ha2: f64,
    pub hf2: f64,
    /// Cutter radius.
    pub rc: f64,
    /// Which side of the gear's centre the pinion's axis is (y = −side·E).
    pub side: f64,
    /// The crossing point's height on the gear axis.
    pub h: f64,
    /// The mean point, the gear cone's outward normal there, the pinion's axis direction (towards
    /// its apex), the crossing point on the pinion's axis, the two pitch apexes and the cutter
    /// centre.
    pub p: V3,
    pub n2: V3,
    pub a1: V3,
    pub o1: V3,
    pub apex1: V3,
    pub apex2: V3,
    pub cutter: V3,
    pub warnings: Vec<String>,
}

/// One trial position of the mean point.
#[derive(Clone, Copy, Debug)]
struct Trial {
    phi: f64,
    h: f64,
    p: V3,
    n2: V3,
    g1: V3,
    g2: V3,
    a1: V3,
    o1: V3,
    sd1: f64,
    rm1: f64,
    eps: f64,
    res: f64,
}

struct Setup {
    z1: f64,
    z2: f64,
    sigma: f64,
    rm2: f64,
    d2: f64,
    beta1: f64,
}

impl Setup {
    fn a1(&self) -> V3 {
        [self.sigma.sin(), 0.0, self.sigma.cos()]
    }
    /// The crossing point's height that puts the common normal in the pinion's axial plane.
    fn height(&self, phi: f64, e: f64) -> Option<f64> {
        let (sd, cd) = self.d2.sin_cos();
        let (sp, cp) = phi.sin_cos();
        let (ss, cs) = self.sigma.sin_cos();
        let den = ss * cd * sp;
        if den.abs() < 1e-12 {
            return None;
        }
        let h = (cd * cs * e * cp - sd * ss * (self.rm2 * sp + e)) / den;
        h.is_finite().then_some(h)
    }
    /// The pitch cones with the mean point at angle `phi` and the pinion's axis through
    /// (0, −e, h): `e` is signed.
    fn trial(&self, phi: f64, h: f64, e: f64) -> Option<Trial> {
        let (sd, cd) = self.d2.sin_cos();
        let (sp, cp) = phi.sin_cos();
        let p = [self.rm2 * cp, self.rm2 * sp, 0.0];
        let n2 = [cd * cp, cd * sp, sd];
        let g2 = [-sd * cp, -sd * sp, cd];
        let a1 = self.a1();
        let o1 = [0.0, -e, h];
        let sd1 = -dot(n2, a1);
        if !(1e-3..0.999).contains(&sd1) {
            return None;
        }
        let cd1 = (1.0 - sd1 * sd1).sqrt();
        let n1 = mul(n2, -1.0);
        let g1 = mul(sub(a1, mul(n1, sd1)), 1.0 / cd1);
        let rel = sub(p, o1);
        let radial = sub(rel, mul(a1, dot(rel, a1)));
        let rm1 = len(radial);
        // The pinion sits on the gear cone's outer side.
        if rm1 <= 1e-9 || dot(radial, n2) >= 0.0 {
            return None;
        }
        let eps = dot(cross(g2, g1), n2).atan2(dot(g2, g1));
        let res = rm1 * self.beta1.cos() * self.z2 - self.rm2 * (self.beta1 - eps).cos() * self.z1;
        Some(Trial { phi, h, p, n2, g1, g2, a1, o1, sd1, rm1, eps, res })
    }
    fn at(&self, phi: f64, e: f64) -> Option<Trial> {
        self.trial(phi, self.height(phi, e)?, e)
    }
    /// The mean point for offset `e` (≥ 0) with a left-hand pinion: the root nearest the
    /// zero-offset position with ε′ > 0.
    fn solve(&self, e: f64) -> Option<(Trial, f64)> {
        if e < 1e-3 {
            let h = self.rm2 / self.d2.tan();
            return self.trial(PI, h, 0.0).map(|t| (t, 1.0));
        }
        let mut best: Option<(Trial, f64)> = None;
        let scale = self.rm2 * self.z1.max(self.z2);
        for side in [1.0, -1.0] {
            let es = side * e;
            for dir in [1.0, -1.0] {
                // Offsets from φ = π, spaced evenly in their logarithm.
                let n = 1500;
                let mut prev: Option<Trial> = None;
                for i in 0..=n {
                    let d = 10f64.powf(-7.0 + (7.0 + (PI / 2.0 - 1e-3).log10()) * i as f64 / n as f64);
                    let phi = PI + dir * d;
                    let cur = self.at(phi, es);
                    if let (Some(a), Some(b)) = (prev, cur)
                        && a.res.signum() != b.res.signum()
                    {
                        let (mut lo, mut hi) = (a, b);
                        let mut ok = true;
                        for _ in 0..80 {
                            let mid = 0.5 * (lo.phi + hi.phi);
                            match self.at(mid, es) {
                                Some(m) if m.res.signum() == lo.res.signum() => lo = m,
                                Some(m) => hi = m,
                                None => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        let r = if lo.res.abs() < hi.res.abs() { lo } else { hi };
                        if ok && r.res.abs() < 1e-7 * scale && r.eps > 0.0 && r.eps < self.beta1 + 1.0 {
                            let better = best.as_ref().is_none_or(|(b, _)| (r.phi - PI).abs() < (b.phi - PI).abs());
                            if better {
                                best = Some((r, side));
                            }
                        }
                    }
                    prev = cur;
                }
            }
        }
        best
    }
}

/// Gleason's standard face-milling cutter radii (3½" to 12" diameter), mm.
const CUTTERS: [f64; 6] = [44.45, 63.5, 76.2, 95.25, 114.3, 152.4];

/// The hypoid set a spec describes.
pub fn design(s: &Spec) -> Result<Design, String> {
    let (z1, z2) = (f64::from(s.teeth), f64::from(s.teeth2));
    if s.teeth > s.teeth2 {
        return Err("the pinion needs fewer teeth than the ring gear".into());
    }
    if !(0.0..=70.0).contains(&s.spiral_angle) {
        return Err("the pinion's spiral angle must be between 0° and 70°".into());
    }
    if !(s.offset.is_finite() && s.offset >= 0.0) {
        return Err("the offset must not be negative".into());
    }
    let mut warnings = Vec::new();
    let sigma = s.shaft_angle.to_radians();
    let u = z2 / z1;
    let d2 = (u * sigma.sin()).atan2(1.0 + u * sigma.cos());
    let de2 = s.module * z2;
    let re2 = de2 / 2.0;
    let cone_e2 = re2 / d2.sin();
    let b2 = s.face_width.min(0.45 * cone_e2);
    if s.face_width > cone_e2 * 0.3 {
        warnings.push(format!("the face width is over 0.3 × the ring gear's cone distance ({:.1} mm), which makes weak toes", 0.3 * cone_e2));
    }
    let rm2 = (de2 - b2 * d2.sin()) / 2.0;
    let setup = Setup { z1, z2, sigma, rm2, d2, beta1: s.spiral_angle.to_radians() };
    let mut e = s.offset;
    let (t, side) = match setup.solve(e) {
        Some(x) => x,
        None => {
            // The largest offset that still works.
            let (mut lo, mut hi) = (0.0, e);
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                if setup.solve(mid).is_some() { lo = mid } else { hi = mid }
            }
            e = lo;
            warnings.push(format!("an offset of {:.2} mm is too large for these teeth and spiral angle; using {:.2} mm", s.offset, e));
            setup.solve(e).ok_or("no pitch cones fit these numbers")?
        }
    };
    let beta1 = setup.beta1;
    let beta2 = beta1 - t.eps;
    let delta1 = t.sd1.asin();
    let cone_m1 = t.rm1 / t.sd1;
    let cone_m2 = rm2 / d2.sin();
    let b1 = b2 * (beta1.cos() / beta2.cos()).max(0.8) * 1.1;
    let cone_e1 = cone_m1 + b1 / 2.0;
    let mmn = 2.0 * rm2 * beta2.cos() / z2;
    let hmw = 2.0 * s.rack.addendum * mmn;
    let c_ham = 0.21 + 0.29 / (u * u);
    let clearance = (s.rack.dedendum - s.rack.addendum).max(0.0) * mmn;
    let ha2 = c_ham * hmw;
    let ha1 = hmw - ha2;
    let (hf1, hf2) = (ha2 + clearance, ha1 + clearance);
    let rc = if s.cutter_radius > 0.0 {
        s.cutter_radius
    } else {
        let want = 0.9 * cone_m2;
        CUTTERS.iter().copied().min_by(|a, b| (a - want).abs().total_cmp(&(b - want).abs())).filter(|c| (c - want).abs() < 0.3 * want).unwrap_or(want)
    };
    // The tooth line through P and the cutter's centre on its concave side (towards the apexes).
    let tdir = add(mul(t.g2, (-beta2).cos()), mul(cross(t.n2, t.g2), (-beta2).sin()));
    let mut nt = cross(t.n2, tdir);
    if dot(nt, t.g2) + dot(nt, t.g1) < 0.0 {
        nt = mul(nt, -1.0);
    }
    let cutter = add(t.p, mul(nt, rc));
    let apex1 = add(t.p, mul(t.g1, cone_m1));
    let apex2 = add(t.p, mul(t.g2, cone_m2));
    // Offset angles in each member's plane of rotation: from the plane through its axis parallel
    // to the other's axis, to its axial plane through P.
    let eps_gear = (t.phi - PI).abs().min(PI);
    let foot = add(t.o1, mul(t.a1, dot(sub(t.p, t.o1), t.a1)));
    let radial = unit(sub(t.p, foot));
    let down = unit(sub([0.0, 0.0, -1.0], mul(t.a1, -t.a1[2])));
    let eta_pinion = dot(radial, down).clamp(-1.0, 1.0).acos();
    if beta2 < 10f64.to_radians() {
        warnings.push(format!("the ring gear's spiral angle is only {:.1}°: raise the pinion's spiral angle or lower the offset", beta2.to_degrees()));
    }
    if e > 0.25 * de2 {
        warnings.push("an offset over a quarter of the ring gear's diameter is more than hypoid sets usually use".into());
    }
    if beta1.to_degrees() > 60.0 {
        warnings.push("pinion spiral angles over 60° make high axial thrust".into());
    }
    Ok(Design {
        z1,
        z2,
        sigma,
        offset: e,
        delta1,
        delta2: d2,
        beta1,
        beta2,
        eps_pitch: t.eps,
        eps_gear,
        eta_pinion,
        rm1: t.rm1,
        rm2,
        cone_m1,
        cone_m2,
        cone_e1,
        cone_e2: cone_m2 + b2 / 2.0,
        re1: cone_e1 * t.sd1,
        re2,
        b1,
        b2,
        mmn,
        ha1,
        hf1,
        ha2,
        hf2,
        rc,
        side,
        h: t.h,
        p: t.p,
        n2: t.n2,
        a1: t.a1,
        o1: t.o1,
        apex1,
        apex2,
        cutter,
        warnings,
    })
}

impl Design {
    /// The pinion's cone generatrix at P, towards its apex.
    pub fn g1(&self) -> V3 {
        unit(sub(self.apex1, self.p))
    }
    pub fn g2(&self) -> V3 {
        unit(sub(self.apex2, self.p))
    }
    /// How far a member turns (about its axis, towards its apex) for its tooth line to reach cone
    /// distance `r` from the mean point; `None` where the cutter's circle doesn't reach.
    pub fn turn(&self, pinion: bool, r: f64) -> Option<f64> {
        let (apex, n, sd, rm) = if pinion { (self.apex1, mul(self.n2, -1.0), self.delta1.sin(), self.cone_m1) } else { (self.apex2, self.n2, self.delta2.sin(), self.cone_m2) };
        let e1 = unit(sub(self.p, apex));
        let e2 = cross(n, e1);
        let c = sub(self.cutter, apex);
        let (cx, cy) = (dot(c, e1), dot(c, e2));
        let d = cx.hypot(cy);
        let ac = cy.atan2(cx);
        let gamma = |rr: f64| ((rr * rr + d * d - self.rc * self.rc) / (2.0 * rr * d)).clamp(-1.0, 1.0).acos();
        let g0 = gamma(rm);
        let sign = if wrap(ac + g0).abs() < wrap(ac - g0).abs() { 1.0 } else { -1.0 };
        let cosg = (r * r + d * d - self.rc * self.rc) / (2.0 * r * d);
        if !(-1.0..=1.0).contains(&cosg) {
            return None;
        }
        Some(wrap(ac + sign * cosg.acos()) / sd)
    }
}

fn wrap(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

fn mm(v: f64) -> String {
    format!("{v:.3}").trim_end_matches('0').trim_end_matches('.').to_string()
}

fn wanted(s: &Spec, member: u8) -> bool {
    s.only == 0 || s.only == member
}

fn name(s: &Spec, base: &str) -> String {
    if s.name.trim().is_empty() { base.to_string() } else { format!("{} {base}", s.name.trim()) }
}

/// One member's teeth: the heel's virtual (back-cone) profile and how it maps onto planes.
struct Member {
    z: u32,
    delta: f64,
    /// Outer pitch radius and the apex's height above the heel pitch plane.
    re: f64,
    h: f64,
    rv: f64,
    flank: ToothFlank,
    pinion: bool,
    /// Sections' teeth end at this radius (the ring gear's heel).
    clip: f64,
}

impl Member {
    fn new(s: &Spec, d: &Design, pinion: bool) -> Member {
        let (z, delta, cone_e, cone_m, beta, ha) = if pinion { (s.teeth, d.delta1, d.cone_e1, d.cone_m1, d.beta1, d.ha1) } else { (s.teeth2, d.delta2, d.cone_e2, d.cone_m2, d.beta2, d.ha2) };
        let mn_e = d.mmn * cone_e / cone_m;
        let v = Involute {
            teeth: z,
            module: mn_e,
            pressure_angle: s.pressure_angle.to_radians(),
            helix: beta.abs().min(1.3),
            shift: ha / d.mmn - s.rack.addendum,
            rack: s.rack,
            backlash: s.backlash,
            tip_shortening: 0.0,
            virtual_teeth: f64::from(z) / delta.cos(),
        };
        let flank = external_flank(&v);
        Member { z, delta, re: cone_e * delta.sin(), h: cone_e * delta.cos(), rv: v.r(), flank, pinion, clip: f64::INFINITY }
    }
    /// A back-cone point (virtual radius `rr`, half angle `half`) on the plane `zs`.
    fn to_plane(&self, rr: f64, half: f64, zs: f64) -> (f64, f64) {
        let t = rr - self.rv;
        let (sd, cd) = self.delta.sin_cos();
        let rho = self.re + t * cd;
        let zz = t * sd;
        let lambda = (self.h - zs) / (self.h - zz);
        (rho * lambda, half * self.rv / self.re)
    }
    fn root_at(&self, zs: f64) -> f64 {
        self.to_plane(self.flank.rf, 0.0, zs).0
    }
    fn section(&self, zs: f64, phase: f64) -> Outline {
        let f = &self.flank;
        let map = |pts: &[(f64, f64)]| -> Vec<(f64, f64)> { pts.iter().map(|(rr, hh)| self.to_plane(*rr, *hh, zs)).collect() };
        let mut ff = ToothFlank {
            rf: self.to_plane(f.rf, 0.0, zs).0,
            ra: self.to_plane(f.ra, 0.0, zs).0,
            root_half: f.root_half * self.rv / self.re,
            tip_half: f.tip_half * self.rv / self.re,
            fillet: map(&f.fillet),
            flank: map(&f.flank),
            undercut: f.undercut,
            pointed: f.pointed,
        };
        if self.clip < ff.ra && self.clip > ff.rf * 1.001 {
            clip_flank(&mut ff, self.clip);
        }
        teeth_outline(&ff, self.z, phase)
    }
}

/// The tooth cut off at radius `r`: its flank ends there and the tip land is the arc at `r`.
fn clip_flank(f: &mut ToothFlank, r: f64) {
    let cut = |pts: &mut Vec<(f64, f64)>| -> Option<f64> {
        let k = pts.iter().position(|p| p.0 >= r)?;
        let half = if k == 0 {
            pts[0].1
        } else {
            let (a, b) = (pts[k - 1], pts[k]);
            let t = ((r - a.0) / (b.0 - a.0).max(1e-12)).clamp(0.0, 1.0);
            a.1 + (b.1 - a.1) * t
        };
        pts.truncate(k);
        // A last point too close to the cut makes a kinked spline end: drop it.
        if pts.last().is_some_and(|p| r - p.0 < 0.02 * (r - f.rf).max(1e-9)) && pts.len() > 1 {
            pts.pop();
        }
        pts.push((r, half));
        Some(half)
    };
    let half = match cut(&mut f.fillet) {
        Some(h) => {
            f.flank.clear();
            h
        }
        None => match cut(&mut f.flank) {
            Some(h) => h,
            None => return,
        },
    };
    f.ra = r;
    f.tip_half = half.max(0.0);
}

/// Section count along each member's face (more on the pinion: its teeth turn further).
const PINION_SECTIONS: usize = 6;
const GEAR_SECTIONS: usize = 5;

/// The pinion's frame x axis: square to its axis, towards the gear's +z where it can.
fn pinion_x(a1: V3) -> V3 {
    let z = [0.0, 0.0, 1.0];
    let x = sub(z, mul(a1, dot(z, a1)));
    if len(x) > 1e-6 { unit(x) } else { [1.0, 0.0, 0.0] }
}

/// Builds the hypoid set into `p`.
pub fn build(s: &Spec, p: &mut Plan) -> Result<(), String> {
    let d = design(s)?;
    for w in &d.warnings {
        p.warn(w.clone());
    }
    let hand = if s.left_hand { "left" } else { "right" };
    let other = if s.left_hand { "right" } else { "left" };
    p.fact("ratio", d.z2 / d.z1, Unit::None);
    p.fact("offset", d.offset, Unit::Mm);
    p.fact("shaft angle", d.sigma.to_degrees(), Unit::Deg);
    p.fact(format!("pinion mean spiral angle ({hand} hand)"), d.beta1.to_degrees(), Unit::Deg);
    p.fact(format!("gear mean spiral angle ({other} hand)"), d.beta2.to_degrees(), Unit::Deg);
    p.fact("offset angle in the pitch plane ε′", d.eps_pitch.to_degrees(), Unit::Deg);
    p.fact("offset angle round the gear axis", d.eps_gear.to_degrees(), Unit::Deg);
    p.fact("offset angle round the pinion axis", d.eta_pinion.to_degrees(), Unit::Deg);
    p.fact("pinion pitch cone angle", d.delta1.to_degrees(), Unit::Deg);
    p.fact("gear pitch cone angle", d.delta2.to_degrees(), Unit::Deg);
    p.fact("pinion face angle", (d.delta1 + (d.ha1 / d.cone_m1).atan()).to_degrees(), Unit::Deg);
    p.fact("gear face angle", (d.delta2 + (d.ha2 / d.cone_m2).atan()).to_degrees(), Unit::Deg);
    p.fact("pinion root angle", (d.delta1 - (d.hf1 / d.cone_m1).atan()).to_degrees(), Unit::Deg);
    p.fact("gear root angle", (d.delta2 - (d.hf2 / d.cone_m2).atan()).to_degrees(), Unit::Deg);
    p.fact("pinion mean pitch diameter", 2.0 * d.rm1, Unit::Mm);
    p.fact("gear mean pitch diameter", 2.0 * d.rm2, Unit::Mm);
    p.fact("pinion outer pitch diameter", 2.0 * d.re1, Unit::Mm);
    p.fact("gear outer pitch diameter", 2.0 * d.re2, Unit::Mm);
    p.fact("pinion mean cone distance", d.cone_m1, Unit::Mm);
    p.fact("gear mean cone distance", d.cone_m2, Unit::Mm);
    p.fact("pinion face width", d.b1, Unit::Mm);
    p.fact("gear face width", d.b2, Unit::Mm);
    p.fact("mean normal module", d.mmn, Unit::None);
    p.fact("pinion mean addendum", d.ha1, Unit::Mm);
    p.fact("pinion mean dedendum", d.hf1, Unit::Mm);
    p.fact("gear mean addendum", d.ha2, Unit::Mm);
    p.fact("gear mean dedendum", d.hf2, Unit::Mm);
    p.fact("pinion enlargement over a bevel pinion", d.rm1 * d.z2 / (d.rm2 * d.z1), Unit::None);
    p.fact("pinion apex beyond the crossing point", dot(sub(d.apex1, d.o1), d.a1), Unit::Mm);
    p.fact("gear apex beyond the crossing point", d.apex2[2] - d.h, Unit::Mm);
    p.fact("cutter radius", d.rc, Unit::Mm);

    let pin = Member::new(s, &d, true);
    let mut gear = Member::new(s, &d, false);
    for (who, m) in [("pinion", &pin), ("gear", &gear)] {
        if m.flank.pointed {
            p.warn(format!("{who}: the teeth come to a point at the heel; the tips are lowered"));
        }
        if m.flank.undercut {
            p.warn(format!("{who}: undercut teeth"));
        }
    }
    // World placement: the crossing point on the gear's axis goes to `origin`.
    let world = |q: V3| -> V3 { [q[0] + s.origin[0], q[1] + s.origin[1], q[2] - d.h + s.origin[2]] };
    let mut tooth_line_missed = false;
    // Sections on planes from `z_lo` to `z_hi` (above the heel pitch plane); the tooth line's turn
    // stays at its end values beyond the face.
    let mut sections = |m: &Member, count: usize, phase0: f64, cone_e: f64, z_lo: f64, z_hi: f64| -> Vec<(Outline, f64)> {
        let cone_i = if m.pinion { d.cone_m1 - d.b1 / 2.0 } else { d.cone_m2 - d.b2 / 2.0 };
        (0..count)
            .map(|i| {
                let zs = z_lo + (z_hi - z_lo) * i as f64 / (count - 1) as f64;
                let r = (cone_e - zs / m.delta.cos()).clamp(cone_i, cone_e);
                let turn = d.turn(m.pinion, r).unwrap_or_else(|| {
                    tooth_line_missed = true;
                    0.0
                });
                (m.section(zs, phase0 + turn), zs)
            })
            .collect()
    };
    let mut bodies = Vec::new();
    // The ring gear: heel pitch circle centre on its axis, apex towards +z; a tooth centred on P.
    let gear_frame = Frame { origin: world([0.0, 0.0, -(d.b2 / 2.0) * d.delta2.cos()]), x: [1.0, 0.0, 0.0], z: [0.0, 0.0, 1.0] };
    let psi2 = d.p[1].atan2(d.p[0]);
    // The ring gear is nearly a crown gear: planes square to its axis cut its teeth at a long
    // slant, so its sections run from below the heel's roots (else the tooth spaces would be open
    // underneath) to the toe's pitch plane, each section's teeth cut off at the heel's tip radius
    // (a cylinder for a back cone: a boolean trim of the lofted teeth is unreliable in the
    // kernel). Towards the toe the teeth run on, shrinking along the cone, to the top face.
    let (sd2, cd2) = d.delta2.sin_cos();
    let heel_root_z = (gear.flank.rf - gear.rv) * sd2;
    let tip_t = gear.flank.ra - gear.rv;
    let gear_lo = heel_root_z;
    let gear_hi = d.b2 * cd2;
    let heel_tip_r = gear.re + tip_t * cd2;
    gear.clip = heel_tip_r;
    let gear_secs = sections(&gear, GEAR_SECTIONS, psi2, d.cone_e2, gear_lo, gear_hi);
    // The pinion: heel pitch circle centre on its axis, its axis towards its apex; a space on P.
    let a1 = d.a1;
    let x1 = pinion_x(a1);
    let y1 = cross(a1, x1);
    let heel1 = sub(d.apex1, mul(a1, d.cone_e1 * d.delta1.cos()));
    let pin_frame = Frame { origin: world(heel1), x: x1, z: a1 };
    let rel = sub(d.p, heel1);
    let psi1 = dot(rel, y1).atan2(dot(rel, x1));
    let pin_secs = sections(&pin, PINION_SECTIONS, psi1 + PI / d.z1, d.cone_e1, 0.0, d.b1 * d.delta1.cos());
    let pin_top = pin_secs.last().map(|x| x.1).unwrap_or(0.0);
    if tooth_line_missed {
        p.warn("the cutter radius is too small to reach across the face; part of the tooth line is left straight");
    }
    let circle = |r: f64| Region { outer: vec![Outline::circle(V2::ZERO, r)], holes: vec![] };
    if wanted(s, 1) {
        let mut steps = vec![Step::Loft { sections: pin_secs, op: Op::New }];
        // A shank out of the heel, a little into the teeth so the two overlap.
        let lap = 0.12 * pin_top;
        let root = pin.root_at(lap) * 0.97;
        let shank_r = if s.hub_diameter > 0.0 { (s.hub_diameter / 2.0).min(root) } else { 0.6 * root };
        let shank_l = if s.hub_length > 0.0 { s.hub_length } else { d.b1 };
        steps.push(Step::Prism { region: circle(shank_r), z0: -shank_l, height: shank_l + lap, twist: 0.0, op: Op::Join });
        if let Some(b) = s.bore.outline() {
            if s.bore.reach() > shank_r {
                p.warn("pinion: the bore is larger than the shank");
            }
            steps.push(Step::Prism { region: Region { outer: vec![b], holes: vec![] }, z0: -shank_l - 1.0, height: shank_l + pin_top + 2.0, twist: 0.0, op: Op::Cut });
        }
        bodies.push(Body { name: name(s, &format!("Hypoid pinion z{}", s.teeth)), frame: pin_frame, steps });
    }
    if wanted(s, 2) {
        let mut steps = vec![Step::Loft { sections: gear_secs, op: Op::New }];
        let lap = 0.1 * (gear_hi - gear_lo);
        let web_r = gear.root_at(gear_lo + lap) * 0.97;
        let web = if s.rim > 0.0 { s.rim } else { 0.06 * 2.0 * d.re2 };
        let bottom = gear_lo - web;
        steps.push(Step::Prism { region: circle(web_r), z0: bottom, height: web + lap, twist: 0.0, op: Op::Join });
        if let Some(b) = s.bore2.outline() {
            // Through the flat faces only (inside the roots on the top face).
            if s.bore2.reach() > gear.root_at(gear_hi) * 0.95 {
                p.warn("gear: the bore is large for this gear");
            }
            steps.push(Step::Prism { region: Region { outer: vec![b], holes: vec![] }, z0: bottom - 1.0, height: gear_hi - bottom + 2.0, twist: 0.0, op: Op::Cut });
        }
        bodies.push(Body { name: name(s, &format!("Hypoid gear z{}", s.teeth2)), frame: gear_frame, steps });
    }
    // The 2D preview: a top view down the ring gear's axis, with its curved tooth lines and the
    // offset pinion over them.
    let mut strips = Vec::new();
    let half = 0.42 * PI / d.z2;
    let (ri, ro) = (d.cone_m2 - d.b2 / 2.0, d.cone_e2);
    for k in 0..s.teeth2 {
        let a0 = psi2 + TAU * f64::from(k) / d.z2;
        let n = 8;
        let line: Vec<(f64, f64)> = (0..=n)
            .map(|i| {
                let r = ri + (ro - ri) * i as f64 / n as f64;
                (r * d.delta2.sin(), a0 + d.turn(false, r).unwrap_or(0.0))
            })
            .collect();
        let mut pts: Vec<V2> = line.iter().map(|(r, a)| V2::polar(*r, a - half * r / d.rm2)).collect();
        pts.extend(line.iter().rev().map(|(r, a)| V2::polar(*r, a + half * r / d.rm2)));
        strips.push(polygon(&pts));
    }
    let gear_region = Region { outer: strips, holes: vec![Outline::circle(V2::ZERO, (ri * d.delta2.sin() - 1.5 * d.mmn).max(0.1))] };
    // The pinion's outline seen from above: its face cone between heel and toe, and a shank.
    let flat = |q: V3| V2::new(q[0], q[1]);
    let wide = flat_unit([y1[0], y1[1]]).unwrap_or(V2::new(0.0, 1.0));
    let along = flat_unit([a1[0], a1[1]]).unwrap_or(V2::new(1.0, 0.0));
    let heel_c = flat(heel1);
    let toe_c = flat(add(heel1, mul(a1, d.b1 * d.delta1.cos())));
    let tip_heel = d.re1 + d.ha1 * d.cone_e1 / d.cone_m1 * d.delta1.cos();
    let tip_toe = tip_heel * (d.cone_e1 - d.b1) / d.cone_e1;
    let shank_r = 0.6 * pin.root_at(0.0);
    let back = heel_c - along * d.b1;
    let silhouette = polygon(&[heel_c - wide * tip_heel, toe_c - wide * tip_toe, toe_c + wide * tip_toe, heel_c + wide * tip_heel]);
    let shank = polygon(&[back - wide * shank_r, heel_c - wide * shank_r, heel_c + wide * shank_r, back + wide * shank_r]);
    let mut gear_prev = PreviewGear { name: "gear".into(), region: gear_region, center: V2::ZERO, speed: d.z1 / d.z2, slide: 0.0, pitch_radius: d.rm2, period: 0.0 };
    let mut pin_prev = PreviewGear { name: "pinion".into(), region: Region { outer: vec![silhouette, shank], holes: vec![] }, center: V2::ZERO, speed: 0.0, slide: 0.0, pitch_radius: 0.0, period: 0.0 };
    if !s.left_hand {
        // A right-hand pinion: the whole set mirrored in y (offset to the other side).
        for b in &mut bodies {
            mirror_body(b, s.origin[1]);
        }
        let my = |q: V2| V2::new(q.x, -q.y);
        gear_prev.region = mirror_region(&gear_prev.region, &my);
        pin_prev.region = mirror_region(&pin_prev.region, &my);
        gear_prev.speed = -gear_prev.speed;
    }
    p.bodies.extend(bodies);
    p.preview.push(gear_prev);
    p.preview.push(pin_prev);
    p.preview_note = format!("top view down the ring gear's axis: {hand}-hand pinion {} mm off centre", mm(d.offset));
    Ok(())
}

fn flat_unit(v: [f64; 2]) -> Option<V2> {
    let l = v[0].hypot(v[1]);
    (l > 1e-9).then(|| V2::new(v[0] / l, v[1] / l))
}

fn polygon(pts: &[V2]) -> Outline {
    let n = pts.len();
    Outline { segs: (0..n).map(|i| Seg::Line(pts[i], pts[(i + 1) % n])).collect() }
}

fn mirror_region(r: &Region, f: &dyn Fn(V2) -> V2) -> Region {
    Region { outer: r.outer.iter().map(|o| o.map(f, true)).collect(), holes: r.holes.iter().map(|o| o.map(f, true)).collect() }
}

/// A body mirrored in the plane y = `y0` (world): the frame mirrored, its local y negated.
fn mirror_body(b: &mut Body, y0: f64) {
    let m = |v: V3| [v[0], -v[1], v[2]];
    let f = b.frame;
    b.frame = Frame { origin: [f.origin[0], 2.0 * y0 - f.origin[1], f.origin[2]], x: m(f.x), z: m(f.z) };
    let my = |q: V2| V2::new(q.x, -q.y);
    for st in &mut b.steps {
        match st {
            Step::Prism { region, twist, .. } => {
                *region = mirror_region(region, &my);
                *twist = -*twist;
            }
            Step::Loft { sections, .. } => {
                for (o, _) in sections.iter_mut() {
                    *o = o.map(&my, true);
                }
            }
            Step::Revolve { .. } => {}
            Step::Coil { left, .. } => *left = !*left,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Kind;

    fn spec() -> Spec {
        Spec::defaults_for(Kind::Hypoid)
    }

    /// The pitch cones touch at P and agree on the normal pitch; the pinion's apex is on its axis.
    fn consistent(d: &Design) {
        let g1 = d.g1();
        let g2 = d.g2();
        let n1 = mul(d.n2, -1.0);
        assert!(dot(g1, d.n2).abs() < 1e-9 && dot(g2, d.n2).abs() < 1e-9);
        assert!((dot(n1, d.a1) - d.delta1.sin()).abs() < 1e-9);
        // The pinion apex is on its axis and the gear apex on z.
        let rel = sub(d.apex1, d.o1);
        assert!(len(sub(rel, mul(d.a1, dot(rel, d.a1)))) < 1e-7, "pinion apex off its axis");
        assert!(d.apex2[0].abs() < 1e-9 && d.apex2[1].abs() < 1e-9);
        // Pitch radii from the cones.
        assert!((d.cone_m1 * d.delta1.sin() - d.rm1).abs() < 1e-9);
        assert!((d.cone_m2 * d.delta2.sin() - d.rm2).abs() < 1e-9);
        // Equal normal pitch: m_mn = 2 r cos β / z for both.
        let mn1 = 2.0 * d.rm1 * d.beta1.cos() / d.z1;
        let mn2 = 2.0 * d.rm2 * d.beta2.cos() / d.z2;
        assert!((mn1 - mn2).abs() < 1e-6 * mn2, "{mn1} {mn2}");
        // The angle between the generatrices is ε′ and β1 − β2 = ε′.
        let between = dot(g1, g2).clamp(-1.0, 1.0).acos();
        assert!((between - d.eps_pitch.abs()).abs() < 1e-9);
        assert!((d.beta1 - d.beta2 - d.eps_pitch).abs() < 1e-12);
        // The pinion's axis is E from the gear's.
        assert!((d.o1[1].abs() - d.offset).abs() < 1e-9);
    }

    #[test]
    fn zero_offset_is_a_spiral_bevel_pair() {
        let mut s = spec();
        s.offset = 0.0;
        let d = design(&s).unwrap();
        consistent(&d);
        assert!(d.eps_pitch.abs() < 1e-9);
        assert!((d.beta1 - d.beta2).abs() < 1e-9);
        // Cone angles of a 90° bevel pair, a common apex.
        assert!((d.delta1 + d.delta2 - PI / 2.0).abs() < 1e-9);
        assert!((d.delta2.tan() - 41.0 / 10.0).abs() < 1e-9);
        assert!(len(sub(d.apex1, d.apex2)) < 1e-6);
        assert!((d.rm1 / d.rm2 - 10.0 / 41.0).abs() < 1e-9);
    }

    #[test]
    fn offset_raises_the_pinion_spiral_angle_and_enlarges_it() {
        let s = spec();
        let d = design(&s).unwrap();
        consistent(&d);
        assert!((d.offset - 25.0).abs() < 1e-9);
        assert!(d.beta1 > d.beta2 && d.eps_pitch > 0.0);
        // Offset hypoid pinion cones are flatter than the bevel's: δ1 + δ2 < Σ.
        assert!(d.delta1 + d.delta2 < PI / 2.0);
        // Typical car axle numbers: ε′ several degrees, pinion enlarged 20-40%.
        let enlarge = d.rm1 * d.z2 / (d.rm2 * d.z1);
        assert!((1.1..1.6).contains(&enlarge), "{enlarge}");
        assert!((enlarge - d.beta2.cos() / d.beta1.cos()).abs() < 1e-6);
        assert!((3f64.to_radians()..25f64.to_radians()).contains(&d.eps_pitch), "{}", d.eps_pitch.to_degrees());
        // The pinion's axis sits below the gear's pitch apex by less than the pinion's size.
        assert!(d.side.abs() == 1.0);
    }

    #[test]
    fn larger_offsets_grow_the_spiral_difference() {
        let mut last = 0.0;
        for e in [5.0, 15.0, 25.0, 35.0] {
            let mut s = spec();
            s.offset = e;
            let d = design(&s).unwrap();
            consistent(&d);
            assert!(d.eps_pitch > last);
            last = d.eps_pitch;
        }
    }

    #[test]
    fn other_shaft_angles_solve() {
        for sigma in [70.0, 100.0] {
            let mut s = spec();
            s.shaft_angle = sigma;
            let d = design(&s).unwrap();
            consistent(&d);
        }
    }

    #[test]
    fn an_impossible_offset_is_clamped_with_a_warning() {
        let mut s = spec();
        s.offset = 400.0;
        let d = design(&s).unwrap();
        consistent(&d);
        assert!(d.offset < 400.0);
        assert!(d.warnings.iter().any(|w| w.contains("too large")), "{:?}", d.warnings);
    }

    #[test]
    fn tooth_lines_pass_through_the_mean_point_and_curve() {
        let d = design(&spec()).unwrap();
        for pinion in [true, false] {
            let rm = if pinion { d.cone_m1 } else { d.cone_m2 };
            let b = if pinion { d.b1 } else { d.b2 };
            assert!(d.turn(pinion, rm).unwrap().abs() < 1e-9);
            let (a, c) = (d.turn(pinion, rm - b / 2.0).unwrap(), d.turn(pinion, rm + b / 2.0).unwrap());
            assert!(a * c < 0.0, "the tooth line crosses P");
        }
        // Gear: developed angle over the face ≈ b·tan β / R_m (curvature aside).
        let a = d.turn(false, d.cone_m2 + 1e-3).unwrap() * d.delta2.sin() / 1e-3 * d.cone_m2;
        assert!((a.abs() - d.beta2.tan()).abs() < 1e-3, "{a} {}", d.beta2.tan());
        let a = d.turn(true, d.cone_m1 + 1e-3).unwrap() * d.delta1.sin() / 1e-3 * d.cone_m1;
        assert!((a.abs() - d.beta1.tan()).abs() < 1e-3, "{a} {}", d.beta1.tan());
    }

    #[test]
    fn the_set_plans_two_lofted_bodies_and_mirrors_for_a_right_hand_pinion() {
        let mut s = spec();
        let p = crate::plan(&s).unwrap();
        assert_eq!(p.bodies.len(), 2);
        assert!(matches!(&p.bodies[0].steps[0], Step::Loft { sections, .. } if sections.len() == PINION_SECTIONS));
        // The pinion's axis is E from the gear's (z) axis, square to it.
        let f = p.bodies[0].frame;
        assert!(f.z[2].abs() < 1e-9);
        assert!((f.origin[1].abs() - 25.0).abs() < 1e-6, "{:?}", f.origin);
        assert!(p.facts.iter().any(|f| f.label.starts_with("pinion mean spiral angle (left")));
        s.left_hand = false;
        let q = crate::plan(&s).unwrap();
        assert!((q.bodies[0].frame.origin[1] + f.origin[1]).abs() < 1e-6);
        // The ring gear's sections end at its heel's tip radius, from below its roots up.
        if let Step::Loft { sections, .. } = &p.bodies[1].steps[0] {
            let d = design(&spec()).unwrap();
            let gear = Member::new(&spec(), &d, false);
            let heel_tip = gear.re + (gear.flank.ra - gear.rv) * d.delta2.cos();
            assert!(sections[0].1 < 0.0 && sections.last().unwrap().1 > 0.0);
            for (o, _) in sections {
                assert!(o.max_radius() < heel_tip + 1e-6, "{} {heel_tip}", o.max_radius());
                assert!(o.area() > 0.0);
            }
        }
        // Sections stay closed outlines with the area they had.
        if let (Step::Loft { sections: a, .. }, Step::Loft { sections: b, .. }) = (&p.bodies[1].steps[0], &q.bodies[1].steps[0]) {
            assert!((a[0].0.area() - b[0].0.area()).abs() < 1e-6 * a[0].0.area().abs());
            assert!(a[0].0.area() > 0.0 && b[0].0.area() > 0.0);
        }
    }
}
