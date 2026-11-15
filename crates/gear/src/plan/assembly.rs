//! A planned set as an assembly ([`super::Assembly`]): which body turns about which shaft, which
//! is held, and how their motions are tied, worked out per type from the placed bodies.
//!
//! Every member is one of the plan's bodies, in their order (`members[i].body == i`). Its axis is
//! its body's frame z (where the teeth's turn is measured, as in [`crate::mesh3d::motion`]),
//! except where turning it round makes the link a plain gear: a left-hand worm, and a face gear's
//! pinion (pointed at the shaft crossing, as a bevel's axes are at the apex).
//!
//! The links form a tree: each member is driven by one link at most (a planetary ring by planet 1
//! only, a differential's second sun by planet 1 only), as SolidCraft's motion links take them.
//!
//! What the bodies don't hold stays out: a planetary set has no carrier body, so its planets turn
//! on fixed shafts (the carrier held) and the ring turns; a cycloidal drive has no input shaft,
//! so its disc only turns on its own axis and its pin ring is held. An elliptical pair is linked
//! at its mean ratio (1:1): the true ratio changes through the turn.

use super::{Assembly, AssemblyLink, AssemblyMember, Frame, Kind, Plan, Spec};
use crate::axes::Axis;

fn neg(d: [f64; 3]) -> [f64; 3] {
    [-d[0], -d[1], -d[2]]
}

fn turning(f: &Frame) -> Axis {
    Axis { origin: f.origin, dir: f.z }
}

/// The pitch radius of a parallel-axis involute gear of `z` teeth (transverse).
fn pitch_radius(s: &Spec, z: u32) -> f64 {
    s.module * f64::from(z) / (2.0 * s.helix.to_radians().cos())
}

/// Whether two frames turn about the same line.
fn coaxial(a: &Frame, b: &Frame) -> bool {
    let d = [b.origin[0] - a.origin[0], b.origin[1] - a.origin[1], b.origin[2] - a.origin[2]];
    let along = d[0] * a.z[0] + d[1] * a.z[1] + d[2] * a.z[2];
    let off = [d[0] - along * a.z[0], d[1] - along * a.z[1], d[2] - along * a.z[2]];
    let par = a.z[0] * b.z[0] + a.z[1] * b.z[1] + a.z[2] * b.z[2];
    (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt() < 1e-6 && par.abs() > 1.0 - 1e-9
}

/// The assembly of the planned set `p` (its bodies placed where they go).
pub(crate) fn fill(s: &Spec, p: &mut Plan) {
    let bodies = &p.bodies;
    let n = bodies.len();
    let mut members: Vec<AssemblyMember> = bodies.iter().enumerate().map(|(i, b)| AssemblyMember { body: i, axis: turning(&b.frame), grounded: false, carrier: None }).collect();
    let mut links = Vec::new();
    let gear = |a: usize, b: usize, ra: f64, rb: f64, internal: bool| AssemblyLink::Gear { a, b, ra, rb, internal, reverse: false };
    // One member of a set built alone: it turns, linked to nothing.
    let whole = s.only == 0;
    match s.kind {
        Kind::Rack => {
            // It slides along its length (its frame's x).
            for (m, b) in members.iter_mut().zip(bodies) {
                m.axis = Axis { origin: b.frame.origin, dir: b.frame.x };
            }
        }
        Kind::Pair if whole && n == 2 => links.push(gear(0, 1, pitch_radius(s, s.teeth), pitch_radius(s, s.teeth2), false)),
        Kind::Cycloidal if whole && n == 2 => {
            links.push(gear(0, 1, s.module * f64::from(s.teeth) / 2.0, s.module * f64::from(s.teeth2) / 2.0, false));
        }
        Kind::Planetary if whole && n >= 2 => {
            // Sun first, the planets, then the ring on the sun's axis. The carrier is held.
            let sun = &bodies[0].frame;
            let ring = (1..n).find(|i| coaxial(sun, &bodies[*i].frame));
            let (rs, rp) = (pitch_radius(s, s.teeth), pitch_radius(s, s.teeth2));
            let zr = if s.teeth3 > 0 { s.teeth3 } else { s.teeth + 2 * s.teeth2 };
            let planets: Vec<usize> = (1..n).filter(|i| Some(*i) != ring).collect();
            for &k in &planets {
                links.push(gear(0, k, rs, rp, false));
            }
            if let (Some(r), Some(&first)) = (ring, planets.first()) {
                links.push(gear(first, r, rp, pitch_radius(s, zr), true));
            }
        }
        Kind::Bevel | Kind::SpiralBevel | Kind::Hypoid | Kind::CrossedHelical if whole && n == 2 => {
            // Turned about the frames' z (a bevel's both point at the apex), as mesh3d checks them.
            let (ratio, _, _) = crate::mesh3d::motion(s);
            let (ra, rb) = match s.kind {
                Kind::Bevel | Kind::SpiralBevel => (s.module * f64::from(s.teeth) / 2.0, s.module * f64::from(s.teeth2) / 2.0),
                _ => (f64::from(s.teeth), f64::from(s.teeth2)),
            };
            links.push(gear(0, 1, ra, rb, ratio > 0.0));
        }
        Kind::Worm if whole && n == 2 => {
            // The wheel first, then the worm. A right-hand worm turned forwards carries the wheel
            // back (mesh3d::motion): the worm's axis is its frame's z for a right hand, turned
            // round for a left hand, so the wheel turns -starts / teeth either way.
            if s.helix < 0.0 {
                members[1].axis.dir = neg(members[1].axis.dir);
            }
            links.push(AssemblyLink::Worm { worm: 1, wheel: 0, starts: s.starts.max(1), wheel_teeth: s.teeth.max(1) });
        }
        Kind::CycloidalDrive => {
            // The disc, then the pin ring: the ring is held, the disc turns on its own axis.
            for m in &mut members {
                m.grounded = if whole { n == 2 && m.body == 1 } else { s.only == 2 };
            }
        }
        Kind::Sprocket | Kind::Pulley if whole && n == 2 => {
            // A chain or belt: both wheels turn the same way, by their tooth counts.
            links.push(gear(0, 1, f64::from(s.teeth), f64::from(s.teeth2), true));
        }
        Kind::Spline if whole && n == 2 => links.push(AssemblyLink::Rigid { a: 0, b: 1 }),
        Kind::FaceGear if whole && n == 2 => {
            // The face gear, then its pinion, whose axis is turned to point at the shaft crossing
            // (as a bevel's): then the pinion drives the face gear as a plain external gear.
            members[1].axis.dir = neg(members[1].axis.dir);
            links.push(gear(1, 0, f64::from(s.teeth), f64::from(s.teeth2), false));
        }
        Kind::Elliptical if whole && n == 2 => {
            // Linked at the mean ratio: the true one changes through the turn.
            links.push(gear(0, 1, f64::from(s.teeth), f64::from(s.teeth), false));
        }
        Kind::BevelPlanetary if whole => {
            // Sun, planets, then a second sun or a ring: every planet driven by the sun, the far
            // member by planet 1, at mesh3d's ratios (every axis points at the apex; the carrier
            // is held).
            let index = |name: &str| bodies.iter().position(|b| b.name == name);
            let mut driven = vec![false; n];
            for m in &p.meshes3d {
                let Some([ba, bb]) = &m.bodies else { continue };
                let (Some(a), Some(b)) = (index(&ba.name), index(&bb.name)) else { continue };
                if driven[b] || m.ratio == 0.0 || !m.ratio.is_finite() {
                    continue;
                }
                driven[b] = true;
                let ra = f64::from(if a == 0 { s.teeth } else { s.teeth2 });
                links.push(gear(a, b, ra, ra / m.ratio.abs(), m.ratio > 0.0));
            }
        }
        _ => {}
    }
    p.assembly = Assembly { members, links };
}
