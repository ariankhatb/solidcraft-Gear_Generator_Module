//! Face (crown) gears meshing with an involute spur pinion at 90°, axes intersecting.
//!
//! The face gear is cut by the pinion (a shaper of the same size). In the plane square to a
//! radius R of the face gear, the face gear moves past the pinion like a rack at speed ω₂·R while
//! the pinion turns about its own axis, so that section is the rack conjugate to the pinion
//! rolling on the working pitch radius r_w = R·z₁/z₂: an involute meshes with a straight-sided
//! rack at any pitch radius, with the pressure angle α_w = acos(r_b/r_w). The tooth section at R
//! is therefore that trapezoid (this ignores the face gear's turning across one tooth, a small
//! effect for z₂ ≫ 1). Its flanks are straight in height at each radius, so the teeth are the
//! loft of two sections at constant height.
//!
//! - Undercut (inner limit): above the height where the line of action touches the pinion's base
//!   circle, r₁ − r_b²/r_w, the face tooth's flank would be cut away by the pinion's root.
//! - Pointing (outer limit): α_w grows with R, so the tooth narrows to a point at its top.

use std::f64::consts::{PI, TAU};

use crate::geom::{Outline, inv};
use crate::involute::{Involute, ToothFlank, teeth_outline};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceGear {
    /// The pinion (spur, the module, pressure angle and basic rack of the set).
    pub pinion: Involute,
    /// Face gear teeth.
    pub teeth: u32,
    /// Circular backlash taken off the face gear's teeth at the working pitch line.
    pub backlash: f64,
}

impl FaceGear {
    fn m(&self) -> f64 {
        self.pinion.module
    }
    pub fn z2(&self) -> f64 {
        f64::from(self.teeth)
    }
    pub fn z1(&self) -> f64 {
        self.pinion.z()
    }
    /// The radius where the face gear's pitch matches the pinion's (m·z₂/2).
    pub fn pitch_radius(&self) -> f64 {
        self.m() * self.z2() / 2.0
    }
    /// The face teeth's top and the tooth spaces' bottom, as heights above the face gear's pitch
    /// plane (the pinion's axis is r₁ above it): clear of the pinion's root and tip by the rack's
    /// clearance.
    pub fn top(&self) -> f64 {
        self.pinion.rack.addendum * self.m()
    }
    pub fn bottom(&self) -> f64 {
        -self.pinion.rack.dedendum * self.m()
    }
    /// Working pitch radius on the pinion and the pressure angle at face gear radius `r`.
    fn working(&self, r: f64) -> (f64, f64) {
        let rw = r * self.z1() / self.z2();
        let rb = self.pinion.rb();
        (rw, (rb / rw.max(rb)).clamp(-1.0, 1.0).acos())
    }
    /// The face tooth's thickness (across the radius, in the tangential direction) at radius `r`
    /// and height `h`.
    pub fn thickness(&self, r: f64, h: f64) -> f64 {
        let g = &self.pinion;
        let (rw, aw) = self.working(r);
        let r1 = g.r();
        let sw = 2.0 * rw * (g.thickness() / (2.0 * r1) + inv(g.alpha_t()) - inv(aw));
        let tw = TAU * rw / self.z1() - sw - self.backlash;
        let hw = r1 - rw;
        tw - 2.0 * (h - hw) * aw.tan()
    }
    /// The smallest radius without undercut at the teeth's top.
    pub fn undercut_radius(&self) -> f64 {
        let g = &self.pinion;
        let rw = g.rb().powi(2) / (g.r() - self.top()).max(1e-9);
        rw * self.z2() / self.z1()
    }
    /// The largest radius at which the top keeps a land of 0.2 modules.
    pub fn pointed_radius(&self) -> f64 {
        let land = 0.2 * self.m();
        let ok = |r: f64| self.thickness(r, self.top()) >= land;
        let (mut lo, mut hi) = (self.undercut_radius().max(self.pitch_radius() * 0.5), self.pitch_radius() * 4.0);
        if !ok(lo) {
            return lo;
        }
        if ok(hi) {
            return hi;
        }
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if ok(mid) { lo = mid } else { hi = mid }
        }
        lo
    }
    /// The face gear's teeth at height `h`, between radii `ri` and `ro`, tooth 0 centred on +x:
    /// joined at `ri` by an arc (the inside stays solid, a raised hub inside the teeth).
    pub fn section(&self, h: f64, ri: f64, ro: f64) -> Outline {
        // Three points a side: the side bows gently (about 0.05 modules), and fewer points keep the loft quick.
        let n = 2;
        let max_half = PI / self.z2() * 0.97;
        let pts: Vec<(f64, f64)> = (0..=n)
            .map(|i| {
                let r = ri + (ro - ri) * f64::from(i) / f64::from(n);
                (r, (self.thickness(r, h) / (2.0 * r)).clamp(1e-4, max_half))
            })
            .collect();
        let (first, last) = (pts.first().map(|p| p.1).unwrap_or(0.0), pts.last().map(|p| p.1).unwrap_or(0.0));
        let f = ToothFlank { rf: ri, ra: ro, root_half: first, tip_half: last, fillet: Vec::new(), flank: pts, undercut: false, pointed: false };
        teeth_outline(&f, self.teeth, 0.0)
    }
}
