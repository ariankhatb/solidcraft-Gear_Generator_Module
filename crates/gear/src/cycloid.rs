//! Cycloidal gears (clock and instrument gears: epicycloid addenda, hypocycloid dedenda) and the
//! cycloidal drive's lobed disc (an offset epitrochoid) with its pin ring.

use std::f64::consts::{PI, TAU};

use crate::geom::{Outline, Seg, V2};
use crate::involute::{ToothFlank, teeth_outline};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cycloidal {
    pub teeth: u32,
    pub module: f64,
    /// Radius of the circle rolled outside the pitch circle (addendum flank).
    pub rolling_radius_addendum: f64,
    /// Radius of the circle rolled inside (dedendum flank); half the pitch radius gives straight
    /// radial flanks.
    pub rolling_radius_dedendum: f64,
    /// Addendum and dedendum in multiples of the module.
    pub addendum: f64,
    pub dedendum: f64,
    /// Circular backlash at the pitch circle (mm).
    pub backlash: f64,
}

impl Cycloidal {
    pub fn r(&self) -> f64 {
        self.module * f64::from(self.teeth) / 2.0
    }
    /// Standard choice: rolling circles half the pitch radius (radial flanks).
    pub fn with_defaults(teeth: u32, module: f64) -> Self {
        let r = module * f64::from(teeth) / 2.0;
        Self { teeth, module, rolling_radius_addendum: r / 2.0, rolling_radius_dedendum: r / 2.0, addendum: 1.0, dedendum: 1.25, backlash: 0.0 }
    }
}

/// Polar angle reached by an epicycloid (outside) or hypocycloid (inside) traced from the
/// pitch circle, as a function of radius.
fn cycloid_polar(r: f64, a: f64, outside: bool, target: f64) -> f64 {
    // Bisection on the rolling parameter t for the wanted radius.
    let point = |t: f64| -> V2 {
        if outside {
            V2::new((r + a) * t.cos() - a * ((r + a) / a * t).cos(), (r + a) * t.sin() - a * ((r + a) / a * t).sin())
        } else {
            V2::new((r - a) * t.cos() + a * ((r - a) / a * t).cos(), (r - a) * t.sin() - a * ((r - a) / a * t).sin())
        }
    };
    // The radius is monotone over the first half turn of the rolling circle (arc π·a).
    let (mut lo, mut hi) = (0.0, PI * a / r.max(1e-9));
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let rr = point(mid).len();
        let past = if outside { rr > target } else { rr < target };
        if past {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    point(0.5 * (lo + hi)).angle()
}

pub fn cycloidal_outline(c: &Cycloidal, phase: f64) -> (Outline, ToothFlank) {
    let r = c.r();
    let half_pitch = (PI * c.module / 2.0 - c.backlash) / (2.0 * r);
    let (ra_full, rf) = (r + c.addendum * c.module, r - c.dedendum * c.module);
    let a_add = c.rolling_radius_addendum.max(1e-6);
    let a_ded = c.rolling_radius_dedendum.clamp(1e-6, r * 0.999);
    // Highest radius the epicycloid reaches: r + 2a.
    let mut ra = ra_full.min(r + 2.0 * a_add * 0.999);
    let half = |rr: f64| -> f64 {
        if rr >= r {
            half_pitch - cycloid_polar(r, a_add, true, rr)
        } else {
            half_pitch + cycloid_polar(r, a_ded, false, rr).abs().min(PI)
        }
    };
    // A pointed (ogival) tip ends where the flanks meet.
    let mut pointed = false;
    if half(ra) <= 1e-4 {
        pointed = true;
        let (mut lo, mut hi) = (r, ra);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if half(mid) > 1e-4 { lo = mid } else { hi = mid }
        }
        ra = lo;
    }
    let lowest = (r - 2.0 * a_ded * 0.999).max(rf);
    let n = 12;
    let ded: Vec<(f64, f64)> = (0..=n).map(|i| lowest + (r - lowest) * i as f64 / n as f64).map(|rr| (rr, half(rr))).collect();
    let add: Vec<(f64, f64)> = (0..=n).map(|i| r + (ra - r) * i as f64 / n as f64).map(|rr| (rr, half(rr))).collect();
    let pitch = TAU / f64::from(c.teeth);
    let root_half = ded[0].1.min(pitch / 2.0);
    // Straight radial drop from the hypocycloid's end to the root circle.
    let mut fillet = vec![(rf, root_half)];
    if lowest > rf + 1e-9 {
        fillet.push((lowest, root_half));
    }
    let mut flank = ded;
    flank.extend(add.into_iter().skip(1));
    if fillet.len() < 2 {
        fillet = vec![(rf, root_half), flank[0]];
        flank.remove(0);
        if flank.len() < 2 {
            flank.insert(0, fillet[1]);
        }
    }
    let f = ToothFlank { rf, ra, root_half, tip_half: half(ra).max(0.0), fillet, flank, undercut: false, pointed };
    (teeth_outline(&f, c.teeth, phase), f)
}

/// A cycloidal drive: N pins on a circle, a disc with N − 1 lobes on an eccentric.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CycloidalDrive {
    /// Number of ring pins N (the disc has N − 1 lobes; reduction N − 1 : 1).
    pub pins: u32,
    /// Radius of the circle the pin centres are on.
    pub pin_circle_radius: f64,
    pub pin_radius: f64,
    /// Eccentricity of the disc's bearing.
    pub eccentricity: f64,
    /// Clearance taken off the disc's profile (mm, normal offset).
    pub clearance: f64,
}

impl CycloidalDrive {
    pub fn lobes(&self) -> u32 {
        self.pins.saturating_sub(1)
    }
    /// The disc's profile (centred on its own bearing), as one spline per lobe.
    pub fn disc(&self) -> Outline {
        let n = f64::from(self.pins);
        let (rr, rp, e) = (self.pin_circle_radius, self.pin_radius + self.clearance, self.eccentricity);
        let lobes = self.lobes().max(1);
        let per_lobe = 40;
        let total = per_lobe * lobes as usize;
        let point = |t: f64| -> V2 {
            // Standard epitrochoid offset by the pin radius (t: the input crank angle).
            let psi = ((1.0 - n) * t).sin().atan2(rr / (e * n) - ((1.0 - n) * t).cos());
            V2::new(rr * t.cos() - rp * (t + psi).cos() - e * (n * t).cos(), -rr * t.sin() + rp * (t + psi).sin() + e * (n * t).sin())
        };
        let mut pts: Vec<V2> = (0..=total).map(|i| point(TAU * i as f64 / total as f64)).collect();
        // Counter-clockwise.
        let mut area = 0.0;
        for w in pts.windows(2) {
            area += w[0].cross(w[1]);
        }
        if area < 0.0 {
            pts.reverse();
        }
        let mut segs = Vec::new();
        for k in 0..lobes as usize {
            segs.push(Seg::Spline(pts[k * per_lobe..=(k + 1) * per_lobe].to_vec()));
        }
        Outline { segs }
    }
    /// The largest eccentricity before the profile loops (cusps): e < rr / N roughly, and the
    /// pin must fit its curvature.
    pub fn max_eccentricity(&self) -> f64 {
        self.pin_circle_radius / f64::from(self.pins)
    }
    pub fn pin_centres(&self) -> Vec<V2> {
        (0..self.pins).map(|k| V2::polar(self.pin_circle_radius, TAU * f64::from(k) / f64::from(self.pins))).collect()
    }
}
