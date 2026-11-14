//! Involute splines: the external spline (shaft) generated like a gear by its basic rack, and the
//! internal spline (hub) whose spaces take the shaft's teeth. DIN 5480 (30°, sized by the
//! reference diameter with the profile shift that sets) and ANSI B92.1 flat root side fit
//! (diametral pitch, no shift), with a flat or a fillet root.

use std::f64::consts::PI;

use crate::geom::Outline;
use crate::involute::{BasicRack, Involute, ToothFlank, external_flank, teeth_outline};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SplineStandard {
    #[default]
    Din5480,
    AnsiB92,
}

impl SplineStandard {
    pub const ALL: [(SplineStandard, &'static str, &'static str); 2] =
        [(SplineStandard::Din5480, "din5480", "DIN 5480"), (SplineStandard::AnsiB92, "ansi_b92", "ANSI B92.1 (flat root side fit)")];
    pub fn name(self) -> &'static str {
        Self::ALL.iter().find(|(k, _, _)| *k == self).map(|(_, n, _)| *n).unwrap_or("din5480")
    }
    pub fn label(self) -> &'static str {
        Self::ALL.iter().find(|(k, _, _)| *k == self).map(|(_, _, l)| *l).unwrap_or("DIN 5480")
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().find(|(_, n, _)| *n == s).map(|(k, _, _)| *k)
    }
}

/// A spline connection: shaft and hub.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spline {
    pub standard: SplineStandard,
    pub module: f64,
    pub teeth: u32,
    /// Pressure angle (radians; 30° for DIN 5480).
    pub pressure_angle: f64,
    /// DIN 5480 reference diameter d_B (0: module × (teeth + 1.1), no shift).
    pub reference_diameter: f64,
    pub fillet_root: bool,
    /// Circular clearance between the flanks at the pitch circle (half off the shaft's teeth,
    /// half added to the hub's spaces).
    pub clearance: f64,
}

impl Spline {
    fn m(&self) -> f64 {
        self.module
    }
    fn z(&self) -> f64 {
        f64::from(self.teeth)
    }
    /// Pitch diameter m·z.
    pub fn pitch_diameter(&self) -> f64 {
        self.m() * self.z()
    }
    /// DIN 5480's reference diameter d_B (ANSI: the major diameter).
    pub fn reference_diameter(&self) -> f64 {
        match self.standard {
            SplineStandard::Din5480 if self.reference_diameter > 0.0 => self.reference_diameter,
            SplineStandard::Din5480 => self.m() * (self.z() + 1.1),
            SplineStandard::AnsiB92 => self.m() * (self.z() + 1.0),
        }
    }
    /// Profile shift coefficient: DIN 5480's x = (d_B − m·z − 1.1·m) / 2m; ANSI none.
    pub fn shift(&self) -> f64 {
        match self.standard {
            SplineStandard::Din5480 => (self.reference_diameter() - self.pitch_diameter() - 1.1 * self.m()) / (2.0 * self.m()),
            SplineStandard::AnsiB92 => 0.0,
        }
    }
    /// The shaft's tip and root diameters.
    pub fn shaft_diameters(&self) -> (f64, f64) {
        let (m, d) = (self.m(), self.pitch_diameter());
        match self.standard {
            SplineStandard::Din5480 => {
                let db = self.reference_diameter();
                (db - 0.2 * m, db - 2.2 * m)
            }
            SplineStandard::AnsiB92 => (d + m, if self.fillet_root { d - 1.8 * m } else { d - 1.35 * m }),
        }
    }
    /// The hub's tip (minor) and root (major) diameters.
    pub fn hub_diameters(&self) -> (f64, f64) {
        let (m, d) = (self.m(), self.pitch_diameter());
        match self.standard {
            SplineStandard::Din5480 => {
                let db = self.reference_diameter();
                (db - 2.0 * m, db + 0.2 * m)
            }
            SplineStandard::AnsiB92 => (d - m, if self.fillet_root { d + 1.8 * m } else { d + 1.35 * m }),
        }
    }
    /// The shaft as a gear: the basic rack set so its cutter makes these tip and root diameters.
    pub fn shaft(&self) -> Involute {
        let (m, r, x) = (self.m(), self.pitch_diameter() / 2.0, self.shift());
        let (da, df) = self.shaft_diameters();
        let addendum = (da / 2.0 - r) / m - x;
        let dedendum = (r - df / 2.0) / m + x;
        Involute {
            teeth: self.teeth,
            module: m,
            pressure_angle: self.pressure_angle,
            helix: 0.0,
            shift: x,
            // A fillet root is the full round the cutter's tip allows (the generator caps it).
            rack: BasicRack { addendum, dedendum, root_radius: if self.fillet_root { 10.0 } else { 0.16 } },
            backlash: self.clearance / 2.0,
            tip_shortening: 0.0,
            virtual_teeth: 0.0,
        }
    }
    /// The shaft's generated flank.
    pub fn shaft_flank(&self) -> ToothFlank {
        external_flank(&self.shaft())
    }
    /// The shaft's teeth, tooth 0 centred at `phase`.
    pub fn shaft_outline(&self, phase: f64) -> (Outline, ToothFlank) {
        let f = self.shaft_flank();
        (teeth_outline(&f, self.teeth, phase), f)
    }
    /// Space width on the pitch circle of the hub (= the shaft's tooth thickness without clearance,
    /// plus the clearance).
    pub fn hub_space(&self) -> f64 {
        let g = Involute { backlash: -self.clearance / 2.0, ..self.shaft() };
        g.thickness()
    }
    /// The hub's toothed bore, counter-clockwise, a space centred at `phase` (where the shaft has a
    /// tooth). Its flanks are the shaft's involutes widened by the clearance, radial below the
    /// base circle; its root is flat.
    pub fn hub_outline(&self, phase: f64) -> Outline {
        let g = Involute { backlash: -self.clearance / 2.0, ..self.shaft() };
        let (dt, dr) = self.hub_diameters();
        let (r0, r1) = (dt / 2.0, dr / 2.0);
        let n = 14;
        let pts: Vec<(f64, f64)> = (0..=n)
            .map(|i| {
                let rr = r0 + (r1 - r0) * f64::from(i) / f64::from(n);
                (rr, g.half_angle_at(rr).clamp(0.0, PI / self.z() * 0.995))
            })
            .collect();
        let (first, last) = (pts.first().map(|p| p.1).unwrap_or(0.0), pts.last().map(|p| p.1).unwrap_or(0.0));
        let f =
            ToothFlank { rf: r0, ra: r1, root_half: first, tip_half: last, fillet: Vec::new(), flank: pts, undercut: false, pointed: last <= 0.0 };
        teeth_outline(&f, self.teeth, phase)
    }
}
