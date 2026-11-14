//! The 3D sets mesh: no shared volume in several poses of their motion, and little free play.

use std::f64::consts::TAU;

use crate::mesh3d::{Solid, overlap, play};
use crate::plan::{Kind, Spec, plan};

/// What a set measures: the worst shared volume over `poses` steps of one tooth pitch, and the
/// free play of member 2 as an arc at its pitch radius (back, forward), worst (largest) over the poses.
pub struct Measured {
    pub overlap: f64,
    pub play: Option<(f64, f64)>,
}

/// `r2`: member 2's pitch radius for turning its play into an arc.
pub fn measure(s: &Spec, poses: usize, r2: f64, cell: f64) -> Measured {
    let p = plan(s).unwrap();
    assert_eq!(p.bodies.len(), 2);
    let mut a = Solid::new(&p.bodies[0]);
    let mut b = Solid::new(&p.bodies[1]);
    let (z1, z2) = match s.kind {
        Kind::Worm => (f64::from(s.teeth), f64::from(s.starts)),
        _ => (f64::from(s.teeth), f64::from(s.teeth2)),
    };
    let sign = if s.kind == Kind::Worm && s.helix < 0.0 { 1.0 } else { -1.0 };
    let mut worst = 0.0f64;
    let mut pl: Option<(f64, f64)> = Some((0.0, 0.0));
    for i in 0..poses {
        let turn = TAU / z1 * i as f64 / poses as f64;
        a.spin = turn;
        b.spin = sign * turn * z1 / z2;
        worst = worst.max(overlap(&a, &b, cell));
        let reach = TAU / z2 * 0.5;
        match (play(&a, &mut b, cell * 1.5, 0.02, reach), pl) {
            (Some((x, y)), Some((px, py))) => pl = Some((px.max(x * r2), py.max(y * r2))),
            _ => pl = None,
        }
    }
    Measured { overlap: worst, play: pl }
}

fn sets() -> Vec<(String, Spec)> {
    let mut v = Vec::new();
    let d = Spec::defaults_for;
    let mut add = |n: &str, s: Spec| v.push((n.to_string(), s));
    add("worm default", d(Kind::Worm));
    add("worm 2-start z41", Spec { starts: 2, teeth: 41, ..d(Kind::Worm) });
    add("worm 4-start q8 left", Spec { starts: 4, teeth: 36, worm_diameter: 16.0, helix: -1.0, ..d(Kind::Worm) });
    add("worm left backlash", Spec { helix: -1.0, backlash: 0.1, teeth: 25, module: 3.0, ..d(Kind::Worm) });
    add("worm 3-start backlash shift", Spec { starts: 3, backlash: 0.1, shift: 0.3, pressure_angle: 25.0, ..d(Kind::Worm) });
    add("bevel default", d(Kind::Bevel));
    add("bevel mitre", Spec { teeth: 24, teeth2: 24, ..d(Kind::Bevel) });
    add("bevel 60deg", Spec { teeth: 16, teeth2: 40, shaft_angle: 60.0, ..d(Kind::Bevel) });
    add("bevel 120deg backlash", Spec { teeth: 18, teeth2: 27, shaft_angle: 120.0, backlash: 0.1, ..d(Kind::Bevel) });
    add("bevel 13:31 m3 backlash", Spec { teeth: 13, teeth2: 31, module: 3.0, backlash: 0.1, face_width: 15.0, ..d(Kind::Bevel) });
    add("spiral default", d(Kind::SpiralBevel));
    add("spiral left 20deg", Spec { spiral_angle: -20.0, teeth: 15, teeth2: 31, ..d(Kind::SpiralBevel) });
    add("spiral 60deg shaft", Spec { shaft_angle: 60.0, teeth: 19, teeth2: 28, ..d(Kind::SpiralBevel) });
    add("spiral 120deg backlash", Spec { shaft_angle: 120.0, backlash: 0.1, spiral_angle: 30.0, teeth: 20, teeth2: 29, ..d(Kind::SpiralBevel) });
    add("spiral mitre 45deg backlash", Spec { teeth: 21, teeth2: 21, spiral_angle: 45.0, backlash: 0.1, ..d(Kind::SpiralBevel) });
    add("crossed default", d(Kind::CrossedHelical));
    add("crossed 30/60", Spec { helix: 30.0, teeth: 18, teeth2: 23, ..d(Kind::CrossedHelical) });
    add("crossed 60deg shaft", Spec { shaft_angle: 60.0, helix: 20.0, teeth: 17, teeth2: 29, ..d(Kind::CrossedHelical) });
    add("crossed 120deg backlash", Spec { shaft_angle: 120.0, helix: 50.0, backlash: 0.1, teeth: 16, teeth2: 24, ..d(Kind::CrossedHelical) });
    add("crossed opposite hands", Spec { shaft_angle: 30.0, helix: 50.0, teeth: 20, teeth2: 20, backlash: 0.1, ..d(Kind::CrossedHelical) });
    v
}

/// The arc (mm) member 2 moves at its pitch radius per radian of its spin (a worm: its thread's
/// travel along the axis).
fn arc(s: &Spec) -> f64 {
    let m = s.module;
    match s.kind {
        Kind::Worm => m * f64::from(s.starts) / 2.0,
        Kind::CrossedHelical => m * f64::from(s.teeth2) / 2.0 / (s.shaft_angle - s.helix).to_radians().cos(),
        _ => m * f64::from(s.teeth2) / 2.0,
    }
}

/// The default sets share no volume as planned (quick: one pose, a coarser grid).
#[test]
fn default_sets_do_not_interfere() {
    for k in [Kind::Worm, Kind::Bevel, Kind::SpiralBevel, Kind::CrossedHelical] {
        let p = plan(&Spec::defaults_for(k)).unwrap();
        let (a, b) = (Solid::new(&p.bodies[0]), Solid::new(&p.bodies[1]));
        let v = overlap(&a, &b, 0.12);
        assert!(v < 0.05, "{k:?}: {v} mm³ shared");
    }
}

/// Every set, through four steps of a tooth pitch: no shared volume, and member 2 can turn only
/// by about the backlash either way before it does (so the teeth are in mesh, not merely apart).
/// Slow without optimisation: `cargo test --release -p solidcraft-gear -- --ignored`.
#[test]
#[ignore]
fn sets_mesh_through_their_motion() {
    let mut bad = Vec::new();
    for (n, s) in sets() {
        let m = measure(&s, 4, arc(&s), 0.08);
        let line = format!("{n}: shared {:.4} mm³, play {:?} mm", m.overlap, m.play);
        eprintln!("{line}");
        // Each member's teeth are thinned by the backlash, so up to twice that shows as play
        // (a worm's thread by half), plus the grid's resolution.
        // Crossed helicals touch at a point (the grid needs a deeper bite to see it) and their
        // backlash is across the teeth, more along the pitch circle of a steep helix.
        let most = if s.kind == Kind::CrossedHelical { 0.05 + 2.0 * s.backlash } else { 0.03 + 1.2 * s.backlash };
        let ok = m.overlap < 0.02 && m.play.is_some_and(|(x, y)| x <= most && y <= most && (s.backlash == 0.0 || x + y > 0.5 * s.backlash));
        if !ok {
            bad.push(line);
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Half a tooth out of phase, every set collides: the check does see teeth that are misplaced.
#[test]
#[ignore]
fn half_a_tooth_out_of_phase_collides() {
    for (n, s) in sets() {
        let p = plan(&s).unwrap();
        let (a, mut b) = (Solid::new(&p.bodies[0]), Solid::new(&p.bodies[1]));
        let z2 = if s.kind == Kind::Worm { f64::from(s.starts) } else { f64::from(s.teeth2) };
        b.spin = TAU / z2 / 2.0;
        let v = overlap(&a, &b, 0.1);
        eprintln!("{n}: {v:.2} mm³ out of phase");
        assert!(v > 1.0, "{n}: {v}");
    }
}
