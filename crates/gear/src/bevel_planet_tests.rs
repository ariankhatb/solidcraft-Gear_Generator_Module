//! Bevel planetary sets mesh, proved with the 3D checker on every planet's two meshes (with the
//! sun and with the second sun or ring): through two tooth pitches no surface point of one is
//! inside the other and the flanks stay in contact (or the backlash apart); half a pitch out of
//! phase they collide. Run with `--nocapture` for the numbers.

use std::f64::consts::PI;

use crate::mesh2d::{Verdict, check_mesh};
use crate::mesh3d::{MeshPair3, Pair};
use crate::plan::bevel_planet::{geometry, placement_from_shafts};
use crate::plan::{BevelPlanetMode, Kind, Plan, Spec, plan};

const STEPS: usize = 16;
/// As in `mesh3d_tests`: flanks in contact may read as crossing by this much, or as this much
/// further apart than the backlash asks (× m).
const DEPTH: f64 = 2.0e-3;
const GAP: f64 = 2.5e-3;

fn d() -> Spec {
    Spec::defaults_for(Kind::BevelPlanetary)
}

fn set(mode: BevelPlanetMode, sigma: f64, zs: u32, zp: u32, n: u32) -> Spec {
    Spec { bevel_planet_mode: mode, shaft_angle: sigma, teeth: zs, teeth2: zp, planets: n, ..d() }
}

/// The worst of every mesh of a plan, checked as `mesh3d_tests` does.
fn prove(label: &str, s: &Spec) -> Plan {
    let p = plan(s).unwrap_or_else(|e| panic!("{label}: {e}"));
    let m = s.module;
    let t = std::time::Instant::now();
    let (mut pen, mut gmin, mut gmax, mut off_min) = (0.0f64, f64::MAX, 0.0f64, f64::MAX);
    for mp in &p.meshes3d {
        let MeshPair3 { bodies: Some([a, b]), ratio, period, gap, labels, .. } = mp else { panic!("{label}: mesh without bodies") };
        let mut pair = Pair::for_run(a, b, 0.03 * m, 0.1 * m, *ratio, -period, 2.0 * period);
        let r = pair.run(*ratio, 0.0, 0.0, 2.0 * period, STEPS);
        // Member b turned half its own pitch: its pitch is the driver's times the ratio.
        let half = 0.5 * period * ratio.abs();
        let off = pair.at(0.0, half);
        let names = labels.clone().unwrap_or_default().join(" / ");
        let line = format!(
            "{label:<40} {names:<20} pen {:>8.5} m  gap {:>8.5}..{:>8.5} m (wants {:.4}..{:.4} mm)  off: pen {:.3} mm vol {:.2} mm3",
            r.penetration / m,
            r.gap_min / m,
            r.gap_max / m,
            gap.0,
            gap.1,
            off.penetration,
            off.volume
        );
        eprintln!("{line}");
        if std::env::var("BP_WHERE").is_ok() && r.penetration > 0.0 {
            pair.a.spin = r.at;
            pair.b.spin = ratio * r.at;
            let (la, lb) = (pair.a.local(r.place), pair.b.local(r.place));
            eprintln!("    deepest at driver {:.4}: a r {:.3} z {:.3} (z {:?}), b r {:.3} z {:.3} (z {:?})", r.at, la[0].hypot(la[1]), la[2], pair.a.z, lb[0].hypot(lb[1]), lb[2], pair.b.z);
        }
        if std::env::var("BP_NOASSERT").is_ok() {
            continue;
        }
        assert!(r.penetration <= DEPTH * m, "{line}: flanks cross by {} mm at {}", r.penetration, r.at);
        assert!(r.gap_max <= gap.1 + GAP * m, "{line}: lose contact");
        assert!(r.gap_min >= gap.0 - GAP * m, "{line}: too close");
        assert!(off.penetration > 0.1 * m && off.volume > 0.02 * m * m * m, "{line}: half a pitch out should collide");
        pen = pen.max(r.penetration);
        gmin = gmin.min(r.gap_min);
        gmax = gmax.max(r.gap_max);
        off_min = off_min.min(off.penetration);
    }
    eprintln!(
        "== {label}: {} meshes, worst pen {:.5} m, gap {:.5}..{:.5} m, least off-phase pen {:.3} mm, {:.1} s{}",
        p.meshes3d.len(),
        pen / m,
        gmin / m,
        gmax / m,
        off_min,
        t.elapsed().as_secs_f64(),
        if p.warnings.is_empty() { String::new() } else { format!("  ! {}", p.warnings.join(" | ")) }
    );
    p
}

fn no_collision_warning(label: &str, p: &Plan) {
    assert!(!p.warnings.iter().any(|w| w.contains("collide")), "{label}: {:?}", p.warnings);
}

#[test]
fn single_sun_sets_mesh() {
    use BevelPlanetMode::Single;
    for (label, s) in [
        ("single 90 16:10 x2", set(Single, 90.0, 16, 10, 2)),
        ("single 90 16:10 x3", set(Single, 90.0, 16, 10, 3)),
        ("single 90 16:10 x4", set(Single, 90.0, 16, 10, 4)),
        ("single 90 24:10 x6", set(Single, 90.0, 24, 10, 6)),
        ("single 80 16:10 x3", set(Single, 80.0, 16, 10, 3)),
        ("single 80 24:10 x6 az 17", Spec { first_azimuth: 17.0, ..set(Single, 80.0, 24, 10, 6) }),
        ("single 60 24:12 x4", set(Single, 60.0, 24, 12, 4)),
        ("single 60 24:12 x6", set(Single, 60.0, 24, 12, 6)),
    ] {
        let p = prove(label, &s);
        no_collision_warning(label, &p);
        assert_eq!(p.meshes3d.len(), s.planets as usize, "{label}");
    }
}

#[test]
fn differentials_mesh() {
    use BevelPlanetMode::Differential;
    for (label, s) in [
        ("diff 90 16:10:16 x2", set(Differential, 90.0, 16, 10, 2)),
        ("diff 90 16:10:16 x4 (default)", d()),
        ("diff 90 18:10:18 x3", set(Differential, 90.0, 18, 10, 3)),
        ("diff 90 24:10:24 x6", set(Differential, 90.0, 24, 10, 6)),
        ("diff 80 16:23:24 x2 (exact 80)", set(Differential, 80.0, 16, 23, 2)),
        ("diff 80 16:10 x3 (angle moves)", set(Differential, 80.0, 16, 10, 3)),
        ("diff 60 10:20:30 x2", set(Differential, 60.0, 10, 20, 2)),
        ("diff 60 10:20:30 x3 az 40", Spec { first_azimuth: 40.0, ..set(Differential, 60.0, 10, 20, 3) }),
    ] {
        let p = prove(label, &s);
        no_collision_warning(label, &p);
        assert_eq!(p.meshes3d.len(), 2 * s.planets as usize, "{label}");
    }
}

#[test]
fn bevel_rings_mesh() {
    use BevelPlanetMode::Ring;
    for (label, s) in [
        ("ring 60 24:12:36 x2", set(Ring, 60.0, 24, 12, 2)),
        ("ring 60 24:12:36 x3", set(Ring, 60.0, 24, 12, 3)),
        ("ring 60 24:12:36 x4", set(Ring, 60.0, 24, 12, 4)),
        ("ring 60 24:12:36 x6", set(Ring, 60.0, 24, 12, 6)),
        ("ring 45 20:10:34 x3", set(Ring, 45.0, 20, 10, 3)),
    ] {
        let p = prove(label, &s);
        no_collision_warning(label, &p);
        assert_eq!(p.meshes3d.len(), 2 * s.planets as usize, "{label}");
    }
}

#[test]
fn spiral_and_backlash_sets_mesh() {
    use BevelPlanetMode::*;
    for (label, s) in [
        ("single 90 16:10 x3 spiral 30", Spec { spiral_angle: 30.0, ..set(Single, 90.0, 16, 10, 3) }),
        ("diff 90 16:10 x4 spiral 30", Spec { spiral_angle: 30.0, ..d() }),
        ("diff 80 16:23 x2 spiral -25 backlash", Spec { spiral_angle: -25.0, backlash: 0.1, ..set(Differential, 80.0, 16, 23, 2) }),
        ("ring 60 24:12 x3 spiral 25", Spec { spiral_angle: 25.0, ..set(Ring, 60.0, 24, 12, 3) }),
        ("ring 60 24:12 x3 backlash", Spec { backlash: 0.1, ..set(Ring, 60.0, 24, 12, 3) }),
    ] {
        prove(label, &s);
    }
}

#[test]
fn geometry_follows_the_derivation() {
    // Σ = 90°, equal side gears: the classic differential.
    let g = geometry(&d()).unwrap();
    assert_eq!(g.z_far, 16);
    assert!((g.sigma - PI / 2.0).abs() < 1e-12);
    assert!((g.delta_sun.tan() - 1.6).abs() < 1e-12);
    assert!((g.delta_far - (PI / 2.0 + g.delta_planet)).abs() < 1e-12);
    // zf = zs + 2·zp·cos Σ and the far cone's pitch is the planets'.
    let s = set(BevelPlanetMode::Ring, 60.0, 24, 12, 3);
    let g = geometry(&s).unwrap();
    assert_eq!(g.z_far, 36);
    assert!((2.0 * g.re * g.delta_far.sin() / s.module - 36.0).abs() < 1e-9);
    assert!((2.0 * g.re * g.delta_planet.sin() / s.module - 12.0).abs() < 1e-9);
    // Differential at 80°: 16 + 2·10·cos 80° = 19.47 → 19 teeth, built at 81.37°, with a warning.
    let s = set(BevelPlanetMode::Differential, 80.0, 16, 10, 3);
    let g = geometry(&s).unwrap();
    assert_eq!(g.z_far, 19);
    assert!((g.sigma.to_degrees() - (3.0f64 / 20.0).acos().to_degrees()).abs() < 1e-9);
    assert!(g.warnings.iter().any(|w| w.contains("cone angle is")), "{:?}", g.warnings);
    // (16 + 19) / 3 is not whole: the nearest places, with a warning.
    assert!(g.warnings.iter().any(|w| w.contains("not whole")), "{:?}", g.warnings);
    let step = 2.0 * PI / 35.0;
    for a in &g.azimuths {
        let k = a / step;
        assert!((k - k.round()).abs() < 1e-9);
    }
    // Single suns space evenly whatever the counts.
    let g = geometry(&set(BevelPlanetMode::Single, 80.0, 17, 11, 3)).unwrap();
    assert!(g.warnings.is_empty(), "{:?}", g.warnings);
    assert!((g.azimuths[1] - 2.0 * PI / 3.0).abs() < 1e-12);
}

#[test]
fn bad_sets_are_refused_or_warned() {
    use BevelPlanetMode::*;
    // A ring at 90° faces away from the sun's side: that's a second sun.
    let e = plan(&set(Ring, 90.0, 16, 10, 3)).unwrap_err();
    assert!(e.contains("second sun"), "{e}");
    // A second sun where the far side opens downwards: that's a ring.
    let e = plan(&set(Differential, 60.0, 24, 12, 3)).unwrap_err();
    assert!(e.contains("ring"), "{e}");
    // Six big planets at 90° collide.
    let p = plan(&set(Single, 90.0, 16, 10, 6)).unwrap();
    assert!(p.warnings.iter().any(|w| w.contains("collide")), "{:?}", p.warnings);
    // Too many planets for the range.
    assert!(plan(&set(Single, 90.0, 16, 10, 30)).is_err());
    // A second-sun count out of reach of any cone angle.
    let e = plan(&Spec { teeth3: 60, ..d() }).unwrap_err();
    assert!(e.contains("out of reach"), "{e}");
}

#[test]
fn facts_and_ratios() {
    let p = plan(&d()).unwrap();
    let fact = |l: &str| p.facts.iter().find(|f| f.label == l).unwrap_or_else(|| panic!("no fact {l}: {:?}", p.facts.iter().map(|f| &f.label).collect::<Vec<_>>())).value;
    assert!((fact("ratio (carrier fixed, sun in, sun 2 out)") + 1.0).abs() < 1e-12);
    assert!((fact("ratio (sun 2 fixed, sun in, carrier out)") - 2.0).abs() < 1e-12);
    assert!((fact("cone angle (sun axis to planet axes)") - 90.0).abs() < 1e-9);
    assert!((fact("planet 2 azimuth") - 90.0).abs() < 1e-9);
    assert_eq!(p.bodies.len(), 6);
    let r = plan(&set(BevelPlanetMode::Ring, 60.0, 24, 12, 3)).unwrap();
    let f = |l: &str| r.facts.iter().find(|f| f.label == l).unwrap().value;
    assert!((f("ratio (ring fixed, sun in, carrier out)") - 2.5).abs() < 1e-12);
    // Building one member keeps the whole set's meshes.
    let only = plan(&Spec { only: 2, ..d() }).unwrap();
    assert_eq!(only.bodies.len(), 4);
    assert_eq!(only.meshes3d.len(), 8);
    // The preview: sun, one planet, the far member.
    assert_eq!(p.preview.len(), 3);
    assert_eq!(r.preview.len(), 3);
    assert!(!r.preview[2].region.holes.is_empty());
    let sheet = crate::sheet::design_sheet(&d(), &p);
    assert!(sheet.contains("Second sun teeth") && sheet.contains("Planets"), "{sheet}");
}

/// The window's badge: Ok for the default and every planet named; a planet turned half a pitch
/// is reported.
#[test]
fn window_check_names_every_planet() {
    let mut p = plan(&d()).unwrap();
    let t = std::time::Instant::now();
    let r = check_mesh(&p);
    eprintln!("{:.0} ms: {}", t.elapsed().as_secs_f64() * 1e3, r.iter().map(|x| x.message.clone()).collect::<Vec<_>>().join(" | "));
    assert_eq!(r.len(), 8);
    assert!(r.iter().all(|x| x.verdict == Verdict::Ok), "{r:?}");
    assert!(r.iter().any(|x| x.message.contains("planet 3")));
    // Planet 2 (in its sun mesh) turned half its pitch.
    let half = PI / 10.0;
    if let Some(b) = &mut p.meshes3d[2].bodies {
        let f = &mut b[1].frame;
        let y = f.y();
        f.x = [0, 1, 2].map(|i| f.x[i] * half.cos() + y[i] * half.sin());
    }
    let r = check_mesh(&p);
    assert_eq!(r[2].verdict, Verdict::Interferes, "{}", r[2].message);
}

#[test]
fn placement_from_a_side_view_sketch() {
    // Sun shaft down the sketch's -y, planet shaft at 80° from it, apex at (5, 5, 0), sketch on
    // the world xy plane.
    let a = 80f64.to_radians();
    let (f, sigma) = placement_from_shafts([5.0, 5.0, 0.0], [0.0, -1.0, 0.0], [a.sin(), -a.cos(), 0.0]).unwrap();
    assert!((sigma - 80.0).abs() < 1e-9);
    let s = Spec { origin: [0.0; 3], placement: Some(f), shaft_angle: sigma, bevel_planet_mode: BevelPlanetMode::Single, ..d() };
    let p = plan(&s).unwrap();
    // Planet 1's axis (towards the apex) lies in the sketch plane and runs back to the apex.
    let planet = p.bodies.iter().find(|b| b.name.contains("Planet 1")).unwrap();
    assert!(planet.frame.z[2].abs() < 1e-9);
    let o = planet.frame.origin;
    let t = ((5.0 - o[0]) * planet.frame.z[0] + (5.0 - o[1]) * planet.frame.z[1]).abs();
    let back = [o[0] + t * planet.frame.z[0], o[1] + t * planet.frame.z[1]];
    assert!((back[0] - 5.0).abs() < 1e-9 && (back[1] - 5.0).abs() < 1e-9, "{back:?}");
    // The sun's axis is the sketch's sun line.
    let sun = &p.bodies[0];
    assert!((sun.frame.z[1] - 1.0).abs() < 1e-12);
    assert!(placement_from_shafts([0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, 2.0]).is_err());
}

#[test]
#[ignore]
fn probe_ring() {
    let s = Spec { backlash: 0.1, ..set(BevelPlanetMode::Ring, 45.0, 20, 10, 1) };
    for fw in [1.5, 3.0, 7.0] {
        let s = Spec { face_width: fw, ..s.clone() };
        let p = plan(&s).unwrap();
        let mp = &p.meshes3d[1];
        let [a, b] = mp.bodies.clone().unwrap();
        let m = s.module;
        let mut pair = Pair::for_run(&a, &b, 0.03 * m, 0.1 * m, mp.ratio, -mp.period, 2.0 * mp.period);
        for off in [-0.01, -0.005, 0.0, 0.005, 0.01] {
            let r = pair.run(mp.ratio, off, 0.0, 2.0 * mp.period, 16);
            eprintln!("face {fw} ring offset {off:+.3} rad: pen {:.4} mm gap {:.4}..{:.4} at driver {:.3}", r.penetration, r.gap_min, r.gap_max, r.at);
            if off == 0.0 {
                let q = r.place;
                let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
                let polar = (-q[2] / l).acos().to_degrees();
                let ax = a.frame.z;
                let from_planet = (-(q[0] * ax[0] + q[1] * ax[1] + q[2] * ax[2]) / l).acos().to_degrees();
                let g = geometry(&s).unwrap();
                eprintln!("   at {q:?}: dist {l:.3} polar {polar:.3} (ring pitch {:.3}), from planet axis {from_planet:.3} (planet pitch {:.3}), azimuth {:.3} deg", g.delta_far.to_degrees(), g.delta_planet.to_degrees(), q[1].atan2(q[0]).to_degrees());
                pair.a.spin = r.at; pair.b.spin = mp.ratio * r.at;
                eprintln!("   in planet {} in ring {}", pair.a.contains(q), pair.b.contains(q));
                let st = pair.at(r.at, mp.ratio * r.at);
                eprintln!("   stat at worst: {st:?}");
                eprintln!("   overlap volume {:.5} mm3", crate::mesh3d::overlap(&pair.a, &pair.b, 0.02));
                let st = pair.at(r.at, mp.ratio * r.at + 0.01);
                eprintln!("   +0.01: {st:?} overlap {:.5}", crate::mesh3d::overlap(&pair.a, &pair.b, 0.02));
            }
        }
    }
}

#[test]
#[ignore]
fn probe_sets() {
    use BevelPlanetMode::*;
    for (label, s) in [
        ("ring 45 20:10:34 x1", set(Ring, 45.0, 20, 10, 1)),
        ("ring 45 20:10:34 x1 j.1", Spec { backlash: 0.1, ..set(Ring, 45.0, 20, 10, 1) }),
        ("ring 45 20:16 x1", set(Ring, 45.0, 20, 16, 1)),
        ("ring 30 20:10 x3", set(Ring, 30.0, 20, 10, 3)),
        ("ring 60 30:10 x2", set(Ring, 60.0, 30, 10, 2)),
        ("diff 60 8:24:32 x2", set(Differential, 60.0, 8, 24, 2)),
        ("diff 60 12:24:24 x2", set(Differential, 60.0, 12, 24, 2)),
    ] {
        match plan(&s) {
            Ok(_) => {
                prove(label, &s);
            }
            Err(e) => eprintln!("{label}: {e}"),
        }
    }
}

#[test]
#[ignore]
fn probe_pair() {
    let b = Spec::defaults_for(Kind::Bevel);
    for (label, s) in [
        ("18:27 120 j.1", Spec { teeth: 18, teeth2: 27, shaft_angle: 120.0, backlash: 0.1, ..b.clone() }),
        ("18:27 120 j0", Spec { teeth: 18, teeth2: 27, shaft_angle: 120.0, ..b.clone() }),
        ("20:30 120 j0", Spec { teeth: 20, teeth2: 30, shaft_angle: 120.0, ..b.clone() }),
        ("20:30 120 j0 f8", Spec { teeth: 20, teeth2: 30, shaft_angle: 120.0, face_width: 8.0, ..b.clone() }),
        ("30:20 120 j0", Spec { teeth: 30, teeth2: 20, shaft_angle: 120.0, ..b.clone() }),
        ("20:30 110 j0", Spec { teeth: 20, teeth2: 30, shaft_angle: 110.0, ..b.clone() }),
        ("20:30 100 j0", Spec { teeth: 20, teeth2: 30, shaft_angle: 100.0, ..b.clone() }),
    ] {
        crate::mesh3d_tests::measure(label, &s).show();
    }
}
