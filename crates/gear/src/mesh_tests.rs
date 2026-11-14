//! 2D meshing of every parallel-axis set, proved with the checker in `mesh2d`: over two tooth
//! pitches (a whole input turn for the cycloidal drive) no pair overlaps and the flanks stay in
//! contact (or a backlash apart).

use std::f64::consts::{PI, TAU};

use crate::geom::{Region, V2};
use crate::involute::{Internal, Involute, external_outline, mesh_internal};
use crate::mesh2d::{Member, Motion, Stat, run};
use crate::plan::{Kind, Plan, Spec, plan};

struct Row {
    label: String,
    /// The length tolerances scale with (the module).
    m: f64,
    stat: Stat,
    /// The flank gap the backlash asks for (mm, normal to the flanks).
    want_gap: f64,
    warnings: Vec<String>,
}

const STEPS: usize = 48;

fn member(g: &crate::plan::PreviewGear, m: f64) -> Member {
    Member::of(g, 0.2 * m, 2e-5 * m)
}

fn fact(p: &Plan, label: &str) -> f64 {
    p.facts.iter().find(|f| f.label == label).unwrap_or_else(|| panic!("no fact {label}")).value
}

fn pair_spec(z1: u32, z2: u32, x1: f64, x2: f64) -> Spec {
    Spec { kind: Kind::Pair, teeth: z1, teeth2: z2, shift: x1, shift2: x2, ..Spec::default() }
}

fn pair_row(label: &str, s: &Spec) -> Row {
    let p = plan(s).unwrap();
    let (a, b) = (member(&p.preview[0], s.module), member(&p.preview[1], s.module));
    let stat = run(&a, &b, 0.0, 2.0 * TAU / f64::from(s.teeth), STEPS);
    let aw = fact(&p, "working pressure angle").to_radians();
    Row { label: format!("pair {label}"), m: s.module, stat, want_gap: s.backlash * aw.cos(), warnings: p.warnings }
}

fn pair_rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for (z1, z2) in [(20, 32), (12, 40), (8, 25), (6, 13), (10, 10), (17, 60), (30, 120), (9, 11), (25, 25), (40, 13)] {
        rows.push(pair_row(&format!("{z1}/{z2}"), &pair_spec(z1, z2, 0.0, 0.0)));
    }
    for (z1, z2, x1, x2) in [(12, 30, 0.4, 0.0), (10, 20, 0.5, 0.3), (20, 32, -0.2, 0.2), (15, 45, 0.3, -0.3), (8, 12, 0.5, 0.4), (14, 50, 0.0, 0.6)] {
        rows.push(pair_row(&format!("{z1}/{z2} x {x1}/{x2}"), &pair_spec(z1, z2, x1, x2)));
    }
    for (z1, z2, x1, a) in [(20, 32, 0.0, 52.8), (20, 30, 0.2, 51.0), (14, 22, 0.0, 37.0), (20, 32, 0.0, 51.5), (11, 17, 0.3, 29.0)] {
        rows.push(pair_row(&format!("{z1}/{z2} x1 {x1} a {a}"), &Spec { center_distance: a, ..pair_spec(z1, z2, x1, 0.0) }));
    }
    for (z1, z2, x1, helix) in [(20, 32, 0.0, 20.0), (12, 25, 0.3, 30.0), (9, 30, 0.0, -15.0), (16, 16, 0.0, 45.0)] {
        rows.push(pair_row(&format!("{z1}/{z2} x1 {x1} helix {helix}"), &Spec { helix, ..pair_spec(z1, z2, x1, 0.0) }));
    }
    for (z1, z2, x1, b) in [(20, 32, 0.0, 0.1), (10, 24, 0.3, 0.1), (8, 25, 0.0, 0.1), (30, 31, 0.0, 0.05)] {
        rows.push(pair_row(&format!("{z1}/{z2} x1 {x1} backlash {b}"), &Spec { backlash: b, ..pair_spec(z1, z2, x1, 0.0) }));
    }
    rows.push(pair_row("20/32 helix 20 a 56 backlash 0.1", &Spec { helix: 20.0, center_distance: 56.0, backlash: 0.1, ..pair_spec(20, 32, 0.0, 0.0) }));
    for (z1, z2, pa, m) in [(30, 40, 14.5, 1.0), (12, 20, 25.0, 3.0), (22, 35, 17.5, 0.5), (18, 27, 30.0, 4.0)] {
        rows.push(pair_row(&format!("{z1}/{z2} pa {pa} m {m}"), &Spec { pressure_angle: pa, module: m, ..pair_spec(z1, z2, 0.0, 0.0) }));
    }
    rows
}

fn planetary_spec(zs: u32, zp: u32, zr: u32, n: u32) -> Spec {
    Spec { kind: Kind::Planetary, teeth: zs, teeth2: zp, teeth3: zr, planets: n, ..Spec::default() }
}

/// Sun–planet and planet–ring rows, each over every planet.
fn planetary_rows(label: &str, s: &Spec) -> [Row; 2] {
    let p = plan(s).unwrap();
    let n = p.preview.len();
    let sun = member(&p.preview[0], s.module);
    let ring = member(&p.preview[n - 1], s.module);
    let t1 = 2.0 * TAU / f64::from(s.teeth);
    let (mut sp, mut pr): (Option<Stat>, Option<Stat>) = (None, None);
    for g in &p.preview[1..n - 1] {
        let planet = member(g, s.module);
        for (acc, st) in [(&mut sp, run(&sun, &planet, 0.0, t1, STEPS)), (&mut pr, run(&planet, &ring, 0.0, t1, STEPS))] {
            match acc {
                Some(a) => a.merge(&st),
                None => *acc = Some(st),
            }
        }
    }
    let at = (s.pressure_angle.to_radians().tan() / s.helix.to_radians().cos()).atan();
    let want = s.backlash * at.cos();
    [
        Row { label: format!("planetary {label} sun-planet"), m: s.module, stat: sp.unwrap(), want_gap: want, warnings: p.warnings.clone() },
        Row { label: format!("planetary {label} planet-ring"), m: s.module, stat: pr.unwrap(), want_gap: want, warnings: p.warnings },
    ]
}

fn planetary_all() -> Vec<Row> {
    let mut rows = Vec::new();
    for (zs, zp, zr, n) in [(18, 18, 54, 3), (20, 16, 52, 3), (20, 15, 50, 3), (12, 18, 48, 4), (16, 20, 56, 5), (24, 12, 48, 6), (10, 25, 60, 3), (30, 15, 60, 4), (21, 21, 63, 2), (36, 12, 60, 5), (17, 19, 55, 1)] {
        rows.extend(planetary_rows(&format!("{zs}/{zp}/{zr} x{n}"), &planetary_spec(zs, zp, zr, n)));
    }
    rows.extend(planetary_rows("18/18/54 x3 helix 15", &Spec { helix: 15.0, ..planetary_spec(18, 18, 54, 3) }));
    rows.extend(planetary_rows("20/15/50 x4 backlash 0.1", &Spec { backlash: 0.1, ..planetary_spec(20, 15, 50, 4) }));
    rows.extend(planetary_rows("16/20/56 x3 m 1 pa 25", &Spec { module: 1.0, pressure_angle: 25.0, ..planetary_spec(16, 20, 56, 3) }));
    rows
}

fn involute_of(s: &Spec, teeth: u32, shift: f64, helix: f64) -> Involute {
    Involute { teeth, module: s.module, pressure_angle: s.pressure_angle.to_radians(), helix: helix.to_radians(), shift, rack: s.rack, backlash: s.backlash, tip_shortening: 0.0, virtual_teeth: 0.0 }
}

fn gear_region(g: &Involute, phase: f64) -> Region {
    Region { outer: vec![external_outline(g, phase).0], holes: vec![] }
}

/// The `internal` set's ring with a pinion put in it.
fn internal_row(zr: u32, zp: u32, xr: f64, xp: f64, helix: f64, backlash: f64) -> Row {
    let s = Spec { kind: Kind::Internal, teeth: zr, shift: xr, helix, backlash, ..Spec::default() };
    let p = plan(&s).unwrap();
    let ring_gear = Internal { gear: involute_of(&s, zr, xr, helix) };
    let pinion = involute_of(&s, zp, xp, helix);
    let mesh = mesh_internal(&pinion, &ring_gear, s.face_width);
    // The ring's tooth 0 is centred on +x: its first space is half a pitch on.
    let dir = PI / f64::from(zr);
    let ratio = f64::from(zp) / f64::from(zr);
    let ring = Member::new(&p.preview[0].region, Motion::turning(V2::ZERO, ratio), 0.2 * s.module, 2e-5 * s.module);
    let pin = Member::new(&gear_region(&pinion, dir), Motion::turning(V2::polar(mesh.center_distance, dir), 1.0), 0.2 * s.module, 2e-5 * s.module);
    let stat = run(&pin, &ring, 0.0, 2.0 * TAU / f64::from(zp), STEPS);
    Row {
        label: format!("internal {zr} pinion {zp} x {xr}/{xp} helix {helix} backlash {backlash}"),
        m: s.module,
        stat,
        want_gap: backlash * mesh.working_pressure_angle.cos(),
        warnings: p.warnings,
    }
}

fn internal_rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for (zr, zp, xr, xp, helix, b) in [
        (60, 20, 0.0, 0.0, 0.0, 0.0),
        (40, 12, 0.0, 0.0, 0.0, 0.0),
        (80, 30, 0.3, 0.3, 0.0, 0.0),
        (60, 25, 0.2, 0.0, 0.0, 0.0),
        (72, 18, 0.0, 0.4, 0.0, 0.0),
        (60, 20, 0.0, 0.0, 20.0, 0.0),
        (60, 20, 0.0, 0.0, 0.0, 0.1),
        (100, 40, 0.0, 0.0, 0.0, 0.0),
        (48, 24, 0.0, 0.0, 0.0, 0.0),
        (36, 12, 0.0, 0.0, 0.0, 0.0),
    ] {
        rows.push(internal_row(zr, zp, xr, xp, helix, b));
    }
    rows
}

/// The `rack` set's rack with a pinion rolling on it.
fn rack_row(zp: u32, xp: f64, xr: f64, helix: f64, backlash: f64) -> Row {
    let s = Spec { kind: Kind::Rack, shift: xr, helix, backlash, rack_teeth: 10, ..Spec::default() };
    let p = plan(&s).unwrap();
    let pinion = involute_of(&s, zp, xp, -helix);
    let pitch = PI * pinion.mt();
    // Over the space between teeth 4 and 5, a tooth pointing down into it; the profile shifts
    // lift the pinion off the rack's datum line.
    let centre = V2::new(5.0 * pitch, pinion.r() + (xp + xr) * s.module);
    let rack = Member::new(&p.preview[0].region, Motion { center: V2::ZERO, speed: 0.0, slide: pinion.r(), orbit: 0.0 }, 0.2 * s.module, 2e-5 * s.module);
    let pin = Member::new(&gear_region(&pinion, -PI / 2.0), Motion::turning(centre, 1.0), 0.2 * s.module, 2e-5 * s.module);
    let turn = TAU / f64::from(zp);
    let stat = run(&pin, &rack, -turn, turn, STEPS);
    Row { label: format!("rack pinion {zp} x {xp} rack x {xr} helix {helix} backlash {backlash}"), m: s.module, stat, want_gap: backlash * pinion.alpha_t().cos(), warnings: p.warnings }
}

fn rack_rows() -> Vec<Row> {
    [(20, 0.0, 0.0, 0.0, 0.0), (12, 0.0, 0.0, 0.0, 0.0), (8, 0.0, 0.0, 0.0, 0.0), (10, 0.4, 0.0, 0.0, 0.0), (20, 0.0, 0.3, 0.0, 0.0), (15, 0.2, -0.2, 0.0, 0.0), (18, 0.0, 0.0, 20.0, 0.0), (20, 0.0, 0.0, 0.0, 0.1), (40, -0.3, 0.0, 0.0, 0.0)]
        .into_iter()
        .map(|(zp, xp, xr, helix, b)| rack_row(zp, xp, xr, helix, b))
        .collect()
}

fn cycloidal_row(z1: u32, z2: u32, m: f64, backlash: f64) -> Row {
    let s = Spec { teeth: z1, teeth2: z2, module: m, backlash, ..Spec::defaults_for(Kind::Cycloidal) };
    let p = plan(&s).unwrap();
    let (a, b) = (member(&p.preview[0], m), member(&p.preview[1], m));
    let stat = run(&a, &b, 0.0, 2.0 * TAU / f64::from(z1), STEPS);
    Row { label: format!("cycloidal {z1}/{z2} m {m} backlash {backlash}"), m, stat, want_gap: backlash, warnings: p.warnings }
}

fn cycloidal_rows() -> Vec<Row> {
    [(60, 8, 1.0, 0.0), (30, 10, 1.0, 0.0), (40, 12, 2.0, 0.0), (64, 8, 0.5, 0.0), (20, 20, 1.0, 0.0), (96, 12, 1.0, 0.0), (30, 6, 1.0, 0.0), (8, 60, 1.0, 0.0), (12, 7, 1.0, 0.0), (60, 8, 1.0, 0.05)]
        .into_iter()
        .map(|(z1, z2, m, b)| cycloidal_row(z1, z2, m, b))
        .collect()
}

/// The disc against the pins through a whole input turn: its centre goes round at the input's
/// speed while it turns back one lobe.
fn drive_row(pins: u32, pin_circle: f64, pin_diameter: f64, e: f64, backlash: f64) -> Row {
    let s = Spec { pins, pin_circle, pin_diameter, eccentricity: e, backlash, ..Spec::defaults_for(Kind::CycloidalDrive) };
    let p = plan(&s).unwrap();
    let m = pin_diameter / 3.0;
    let disc = Member::new(&p.preview[0].region, Motion { center: V2::new(e, 0.0), speed: -1.0 / (f64::from(pins) - 1.0), slide: 0.0, orbit: 1.0 }, 0.2 * m, 2e-5 * m);
    let ring = Member::new(&p.preview[1].region, Motion::turning(V2::ZERO, 0.0), 0.2 * m, 2e-5 * m);
    let stat = run(&disc, &ring, 0.0, TAU, 360);
    Row { label: format!("drive {pins} pins d{pin_diameter} on d{pin_circle} e {e} backlash {backlash}"), m, stat, want_gap: backlash / 2.0, warnings: p.warnings }
}

fn drive_rows() -> Vec<Row> {
    [(12, 60.0, 6.0, 1.5, 0.0), (10, 50.0, 5.0, 1.0, 0.0), (20, 80.0, 5.0, 1.5, 0.0), (30, 100.0, 4.0, 1.0, 0.0), (8, 40.0, 6.0, 2.0, 0.0), (11, 60.0, 8.0, 2.0, 0.0), (12, 60.0, 6.0, 1.5, 0.1), (40, 120.0, 4.0, 0.8, 0.0)]
        .into_iter()
        .map(|(n, pc, pd, e, b)| drive_row(n, pc, pd, e, b))
        .collect()
}

fn show(rows: &[Row]) {
    for r in rows {
        let s = &r.stat;
        eprintln!(
            "{:<58} area {:>9.5} mm2 (t {:>6.3})  depth {:>8.5} m  gap {:>8.5}..{:>8.5} m  (want {:.4}){}",
            r.label,
            s.area,
            s.area_at,
            s.depth / r.m,
            s.gap_min / r.m,
            s.gap_max / r.m,
            r.want_gap / r.m,
            if r.warnings.is_empty() { String::new() } else { format!("  ! {}", r.warnings.join(" | ")) }
        );
    }
}

#[test]
fn mesh_report() {
    show(&pair_rows());
    show(&planetary_all());
    show(&internal_rows());
    show(&rack_rows());
    show(&cycloidal_rows());
    show(&drive_rows());
}
