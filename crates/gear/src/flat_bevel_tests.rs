//! Bevel teeth end on their back and front cones (heel and toe), each member along its own
//! face: also on a nearly flat pitch cone (the bigger member of a set at a 100° to 150° shaft
//! angle, close to a crown gear), where a plane section square to the axis runs far past the
//! cones. Run with `--nocapture` for the numbers.

use crate::mesh3d::Solid;
use crate::mesh3d_tests::measure;
use crate::plan::{Kind, Op, Spec, Step, faces_of, plan};

fn d(k: Kind) -> Spec {
    Spec::defaults_for(k)
}

/// Sets with one member (or both) flatter than 70°.
pub(crate) fn flat_sets() -> Vec<(&'static str, Spec)> {
    let m25 = |k: Kind, sigma: f64| Spec { module: 2.5, teeth: 17, teeth2: 34, shaft_angle: sigma, ..d(k) };
    vec![
        ("spiral 110deg 17:34 m2.5", m25(Kind::SpiralBevel, 110.0)),
        ("straight 110deg 17:34 m2.5", m25(Kind::Bevel, 110.0)),
        ("spiral 100deg 17:34 m2.5", m25(Kind::SpiralBevel, 100.0)),
        ("straight 100deg 17:34 m2.5", m25(Kind::Bevel, 100.0)),
        ("spiral 120deg 20:30", Spec { shaft_angle: 120.0, teeth: 20, teeth2: 30, ..d(Kind::SpiralBevel) }),
        ("straight 120deg 20:30 backlash", Spec { shaft_angle: 120.0, teeth: 20, teeth2: 30, backlash: 0.1, ..d(Kind::Bevel) }),
        ("spiral left 105deg 15:40 backlash", Spec { shaft_angle: 105.0, teeth: 15, teeth2: 40, spiral_angle: -25.0, backlash: 0.1, ..d(Kind::SpiralBevel) }),
        ("straight 150deg 20:20 both flat", Spec { shaft_angle: 150.0, teeth: 20, teeth2: 20, ..d(Kind::Bevel) }),
        ("spiral 150deg 20:20 both flat", Spec { shaft_angle: 150.0, teeth: 20, teeth2: 20, spiral_angle: 30.0, ..d(Kind::SpiralBevel) }),
    ]
}

/// Each member's pitch cone angle (radians).
fn cones(s: &Spec) -> [f64; 2] {
    let sigma = s.shaft_angle.to_radians();
    let d1 = sigma.sin().atan2(f64::from(s.teeth2) / f64::from(s.teeth) + sigma.cos());
    [d1, sigma - d1]
}

#[test]
fn flat_bevels_mesh() {
    let rows: Vec<_> = flat_sets().iter().map(|(n, s)| measure(n, s)).collect();
    for r in &rows {
        r.show();
    }
    for r in &rows {
        r.check();
        assert!(!r.warnings.iter().any(|w| w.contains("80°")), "{}: {:?}", r.label, r.warnings);
    }
}

#[test]
fn bevels_with_their_own_faces_mesh() {
    let rows: Vec<_> = own_face_sets().iter().map(|(n, s)| measure(n, s)).collect();
    for r in &rows {
        r.show();
    }
    for r in &rows {
        r.check();
        assert!(!r.warnings.iter().any(|w| w.contains("80°")), "{}: {:?}", r.label, r.warnings);
    }
}

/// Sets whose members have faces of their own: another width, a heel in from the cone
/// distance (or out past it).
pub(crate) fn own_face_sets() -> Vec<(&'static str, Spec)> {
    vec![
        ("straight 20:30 gear 2 face 8", Spec { face_width2: 8.0, ..d(Kind::Bevel) }),
        ("straight 20:30 gear 2 face 16 heel in 2", Spec { face_width2: 16.0, face_offset2: 2.0, backlash: 0.1, ..d(Kind::Bevel) }),
        ("spiral 17:34 gear 2 face 9 heel in 1.5", Spec { face_width2: 9.0, face_offset2: 1.5, ..d(Kind::SpiralBevel) }),
        ("spiral 110deg 17:34 m2.5 gear 2 heel out 1", Spec { module: 2.5, teeth: 17, teeth2: 34, shaft_angle: 110.0, face_offset2: -1.0, backlash: 0.1, ..d(Kind::SpiralBevel) }),
    ]
}

/// Every member ends on its back cone (nothing past its heel along the pitch element) and its
/// front cone (nothing of the teeth nearer the apex than its toe), and its outer tip diameter
/// is (r + ha·cos δ) at its heel.
#[test]
fn bevels_end_on_their_back_and_front_cones() {
    let mut sets = flat_sets();
    sets.extend(own_face_sets());
    sets.push(("straight default", d(Kind::Bevel)));
    sets.push(("spiral default", d(Kind::SpiralBevel)));
    sets.push(("straight 60deg 16:40", Spec { teeth: 16, teeth2: 40, shaft_angle: 60.0, ..d(Kind::Bevel) }));
    for (label, s) in sets {
        let p = plan(&s).unwrap_or_else(|e| panic!("{label}: {e}"));
        let m = s.module;
        let ha = s.rack.addendum * m * if s.kind == Kind::SpiralBevel { s.spiral_angle.to_radians().cos() } else { 1.0 };
        for (i, (body, delta)) in p.bodies.iter().zip(cones(&s)).enumerate() {
            let z = if i == 0 { s.teeth } else { s.teeth2 };
            let r = m * f64::from(z) / 2.0;
            let re = r / delta.sin();
            let face = faces_of(&s, re)[i];
            let (heel, toe) = (face.heel, face.toe());
            let h = re * delta.cos();
            let trims = body.steps.iter().filter(|st| matches!(st, Step::Revolve { op: Op::Intersect, .. })).count();
            assert_eq!(trims, 1, "{label} {}: {:?}", body.name, body.steps.len());
            let pts = Solid::new(body, 0.03 * m).material_points(0.1 * m);
            assert!(pts.len() > 1000, "{label} {}: {} points", body.name, pts.len());
            let (sd, cd) = delta.sin_cos();
            // Along the pitch element at each point's own azimuth.
            let along = |q: &[f64; 3]| q[0].hypot(q[1]) * sd - (q[2] - h) * cd;
            let most = pts.iter().map(along).fold(f64::MIN, f64::max);
            let tip = pts.iter().map(|q| q[0].hypot(q[1])).fold(0.0, f64::max);
            let want = heel / re * (r + ha * delta.cos());
            eprintln!(
                "{label:<36} {:<12} δ {:5.1}°  tip Ø {:8.3} (r + ha·cos δ: {:8.3}, {:+.3}%)  most along the element {:8.3} of {:8.3}",
                body.name,
                delta.to_degrees(),
                2.0 * tip,
                2.0 * want,
                100.0 * (tip / want - 1.0),
                most,
                heel
            );
            assert!((tip / want - 1.0).abs() < 0.01, "{label} {}: tip Ø {} against {}", body.name, 2.0 * tip, 2.0 * want);
            assert!(most <= heel + 1e-3 * m, "{label} {}: a tooth reaches {} along the pitch element, past the heel at {heel}", body.name, most);
            // Nothing of the teeth is nearer the apex than the front cone: what is (the body's
            // front face and its hub) is under the roots, no higher than the front face.
            let Some(Step::Revolve { profile, .. }) = body.steps.iter().find(|st| matches!(st, Step::Revolve { op: Op::Intersect, .. })) else { unreachable!() };
            let face = profile.last().map_or(0.0, |q| q.y);
            let inside = pts.iter().filter(|q| along(q) < toe - 1e-3 * m).map(|q| q[2]).fold(f64::MIN, f64::max);
            assert!(inside <= face + 1e-6, "{label} {}: material inside the front cone at z {inside}, above the front face at {face}", body.name);
        }
    }
}

/// The over 80° warning is gone, and a cone at 85° and under still builds.
#[test]
fn nearly_crown_bevels_plan() {
    for sigma in [100.0, 110.0, 115.0, 120.0] {
        for k in [Kind::Bevel, Kind::SpiralBevel] {
            let s = Spec { module: 2.5, teeth: 17, teeth2: 34, shaft_angle: sigma, ..d(k) };
            let delta2 = cones(&s)[1].to_degrees();
            let p = plan(&s);
            if delta2 > 85.0 {
                assert!(p.is_err(), "{k:?} {sigma}°: δ {delta2} should be refused");
                continue;
            }
            let p = p.unwrap_or_else(|e| panic!("{k:?} {sigma}°: {e}"));
            assert_eq!(p.bodies.len(), 2);
            assert!(!p.warnings.iter().any(|w| w.contains("crown")), "{k:?} {sigma}°: {:?}", p.warnings);
        }
    }
}

/// Gear 2's face may not miss gear 1's, nor reach too near the apex.
#[test]
fn faces_that_miss_are_refused() {
    for (label, s) in [
        ("heel past gear 1's toe", Spec { face_offset2: 13.0, ..d(Kind::Bevel) }),
        ("toe past gear 1's heel", Spec { face_offset2: -13.0, ..d(Kind::Bevel) }),
        ("toe near the apex", Spec { face_width2: 12.0, face_offset2: 22.0, ..d(Kind::SpiralBevel) }),
    ] {
        assert!(plan(&s).is_err(), "{label}");
    }
}

#[test]
#[ignore]
fn probe_flat() {
    let name = std::env::var("FLAT_CASE").unwrap_or_default();
    let extra = vec![("mitre 24:24", Spec { teeth: 24, teeth2: 24, ..d(Kind::Bevel) })];
    for (label, s) in flat_sets().into_iter().chain(own_face_sets()).chain(extra) {
        if label.contains(name.as_str()) {
            measure(label, &s).show();
        }
    }
}

/// Bevel planetary sets with flat planets (near a crown) and planets with
/// faces of their own.
pub(crate) fn planetary_sets() -> Vec<(&'static str, Spec)> {
    use crate::plan::BevelPlanetMode::{Differential, Single};
    let bp = d(Kind::BevelPlanetary);
    let set = |mode, angle: f64, zs: u32, zp: u32, n: u32| Spec { bevel_planet_mode: mode, shaft_angle: angle, teeth: zs, teeth2: zp, planets: n, ..bp.clone() };
    vec![
        ("bp single 100 12:30 x1 flat planet", set(Single, 100.0, 12, 30, 1)),
        ("bp diff 90 10:30 x2 flat planets spiral 30", Spec { spiral_angle: 30.0, ..set(Differential, 90.0, 10, 30, 2) }),
        ("bp single 90 16:10 x3 planets face 8 heel in 1", Spec { face_width2: 8.0, face_offset2: 1.0, ..set(Single, 90.0, 16, 10, 3) }),
    ]
}

#[test]
fn bevel_planetary_flat_and_own_faces_mesh() {
    for (label, s) in planetary_sets() {
        crate::bevel_planet_tests::prove(label, &s);
    }
}

