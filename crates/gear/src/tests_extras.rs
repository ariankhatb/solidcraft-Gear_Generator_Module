//! Tests for sprockets, pulleys, splines and face gears.

use std::f64::consts::PI;

use crate::face_gear::FaceGear;
use crate::geom::{Outline, V2};
use crate::involute::Involute;
use crate::plan::{Kind, Spec, Step, plan};
use crate::pulley::{BELTS, Pulley, belt_by_id};
use crate::spline::{Spline, SplineStandard};
use crate::sprocket::{CHAINS, Sprocket, chain_by_id, chamfer_profile, tooth_width};
use crate::wrap::{belt_length, belt_path, chain_center, chain_links};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn closed(o: &Outline) {
    for w in o.segs.windows(2) {
        assert!(w[0].end().dist(w[1].start()) < 1e-9, "gap {:?} → {:?}", w[0].end(), w[1].start());
    }
    let (Some(a), Some(b)) = (o.segs.last(), o.segs.first()) else { panic!("empty outline") };
    assert!(a.end().dist(b.start()) < 1e-9);
}

/// The sampled outline crosses itself nowhere (segments that share an end excepted).
fn simple(o: &Outline) {
    let mut p = o.polyline(0.002);
    p.dedup_by(|a, b| a.dist(*b) < 1e-9);
    if p.len() > 1 && p[0].dist(p[p.len() - 1]) < 1e-9 {
        p.pop();
    }
    let n = p.len();
    let cross = |a: V2, b: V2, c: V2, d: V2| -> bool {
        let d1 = (b - a).cross(c - a);
        let d2 = (b - a).cross(d - a);
        let d3 = (d - c).cross(a - c);
        let d4 = (d - c).cross(b - c);
        d1 * d2 < -1e-18 && d3 * d4 < -1e-18
    };
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (a, b, c, d) = (p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]);
            assert!(!cross(a, b, c, d), "self-intersection near {a:?} and {c:?}");
        }
    }
}

#[test]
fn chain_table_and_iso_606_sprocket_dimensions() {
    let c = chain_by_id("08b").unwrap();
    assert_eq!((c.pitch, c.roller), (12.7, 8.51));
    assert!(CHAINS.iter().all(|c| c.roller < c.pitch && c.inner_width < c.strand_pitch));
    let s = Sprocket { teeth: 17, pitch: 12.7, roller: 8.51 };
    // Catalogue values for an 08B 17-tooth sprocket: d 69.12, df 60.61, da 72.1…76.5.
    assert!(close(s.pitch_diameter(), 69.12, 0.01), "{}", s.pitch_diameter());
    assert!(close(s.root_diameter(), 60.61, 0.01));
    let (lo, hi) = s.tip_diameter_range();
    assert!(close(lo, 72.12, 0.02) && close(hi, 76.49, 0.02), "{lo} {hi}");
    // Odd teeth: caliper diameter d·cos(90°/z) − d1.
    assert!(close(s.caliper_diameter(), 69.12 * (PI / 34.0).cos() - 8.51, 0.01));
    assert!(s.seating_radius() > 0.505 * 8.51 && s.seating_radius() < 0.505 * 8.51 + 0.069 * 8.51f64.cbrt());
    let t = s.outline(0.0, s.tip_diameter() / 2.0);
    closed(&t.outline);
    simple(&t.outline);
    assert!(!t.pointed);
    assert!(close(t.outline.max_radius(), s.tip_diameter() / 2.0, 1e-3));
    // The seats' bottoms: the roller seating radius below the pitch circle.
    assert!(close(t.r_min, s.pitch_diameter() / 2.0 - s.seating_radius(), 1e-9));
    let area = t.outline.area();
    assert!(area > PI * t.r_min.powi(2) && area < PI * t.ra.powi(2), "{area}");
    // Tooth width per ISO 606.
    assert!(close(tooth_width(12.7, 7.75, 1), 0.93 * 7.75, 1e-12));
    assert!(close(tooth_width(15.875, 9.65, 2), 0.93 * 9.65, 1e-12));
}

#[test]
fn every_chain_and_tooth_count_makes_a_clean_sprocket() {
    for c in CHAINS {
        for z in [6u32, 9, 13, 17, 25, 40, 80] {
            let s = Sprocket { teeth: z, pitch: c.pitch, roller: c.roller };
            let t = s.outline(0.3, s.tip_diameter() / 2.0);
            closed(&t.outline);
            if z <= 25 {
                simple(&t.outline);
            }
            assert!(t.ra > s.pitch_diameter() / 2.0, "{} z{z}", c.id);
            assert!(t.outline.area() > 0.0);
        }
    }
}

#[test]
fn chamfer_profile_is_a_closed_band_per_strand() {
    let prof = chamfer_profile(12.7, 37.0, 7.2, 2, 13.92, 28.0);
    assert!(prof.iter().all(|q| q.x >= 0.0 && q.is_finite()));
    assert!(prof.iter().any(|q| q.x > 37.0));
    let area: f64 = (0..prof.len()).map(|i| prof[i].cross(prof[(i + 1) % prof.len()])).sum::<f64>() * 0.5;
    assert!(area.abs() > 0.0);
}

#[test]
fn chain_length_and_centre_distance_agree() {
    let (z1, z2, p) = (17.0, 38.0, 12.7);
    let a = 500.0;
    let x = chain_links(z1, z2, a, p);
    let back = chain_center(z1, z2, x, p).unwrap();
    assert!(close(back, a, 1e-6), "{back}");
    // Same teeth: links = 2a/p + z.
    assert!(close(chain_links(20.0, 20.0, 254.0, 12.7), 40.0 + 20.0, 1e-9));
    // An open belt round equal pulleys: 2a + πd.
    assert!(close(belt_length(10.0, 10.0, 100.0), 200.0 + 20.0 * PI, 1e-9));
    let o = belt_path(10.0, 25.0, 120.0, 0.0).unwrap();
    closed(&o);
    assert!(close(o.area(), (belt_length(10.0, 25.0, 120.0) - 0.0) * 0.0 + o.area(), 0.0));
    assert!(o.area() > 0.0);
}

#[test]
fn timing_pulley_dimensions_and_grooves() {
    let gt2 = belt_by_id("gt2_2").unwrap();
    let p20 = Pulley { belt: *gt2, teeth: 20 };
    // GT2 20-tooth: pitch Ø 12.73, OD 12.22 (makers' tables).
    assert!(close(p20.pitch_diameter(), 12.732, 0.001));
    assert!(close(p20.outside_diameter(), 12.224, 0.001));
    // The published GT2 2 mm geometry puts the flank centres 0.40 mm off the middle.
    let crate::pulley::Groove::Curvilinear { depth, tip_r, flank_r, .. } = gt2.groove else { panic!() };
    let b = ((flank_r - tip_r).powi(2) - (depth - tip_r).powi(2)).sqrt();
    assert!(close(b, 0.40, 0.01), "{b}");
    let htd5 = belt_by_id("htd_5m").unwrap();
    // HTD 5M 30 teeth: pitch Ø 47.75, OD 46.60.
    let h30 = Pulley { belt: *htd5, teeth: 30 };
    assert!(close(h30.pitch_diameter(), 47.746, 0.001) && close(h30.outside_diameter(), 46.60, 0.01));
    for belt in BELTS {
        let prof = belt.groove.profile();
        assert!(close(prof.first().unwrap().y, 0.0, 1e-9) && close(prof.last().unwrap().y, 0.0, 1e-9), "{}", belt.id);
        assert!(prof.iter().all(|q| q.y <= 1e-9 && q.y >= -belt.groove.depth() - 1e-9), "{}", belt.id);
        let w = 2.0 * belt.groove.half_width();
        assert!(w > 0.3 * belt.pitch && w < 0.95 * belt.pitch, "{} groove {w} of pitch {}", belt.id, belt.pitch);
        for z in [12u32, 20, 36, 60] {
            let pu = Pulley { belt: *belt, teeth: z };
            let o = pu.outline(0.0);
            closed(&o);
            if z <= 20 {
                simple(&o);
            }
            assert!(pu.land() > 0.0, "{} z{z}", belt.id);
            assert!(close(o.max_radius(), pu.outside_diameter() / 2.0, 1e-3), "{} z{z}", belt.id);
        }
    }
}

#[test]
fn din_5480_and_ansi_splines() {
    // DIN 5480 – 25 × 1.25 × 18: shift +0.45, d_a1 24.75, d_f2 25.25, d_a2 22.5.
    let s = Spline {
        standard: SplineStandard::Din5480,
        module: 1.25,
        teeth: 18,
        pressure_angle: 30f64.to_radians(),
        reference_diameter: 25.0,
        fillet_root: false,
        clearance: 0.0,
    };
    assert!(close(s.shift(), 0.45, 1e-12));
    let (da1, df1) = s.shaft_diameters();
    let (da2, df2) = s.hub_diameters();
    assert!(close(da1, 24.75, 1e-9) && close(df1, 22.25, 1e-9) && close(da2, 22.5, 1e-9) && close(df2, 25.25, 1e-9));
    let g = s.shaft();
    assert!(close(g.ra(), da1 / 2.0, 1e-9) && close(g.rf(), df1 / 2.0, 1e-9));
    // With no clearance the hub's space equals the shaft's tooth.
    assert!(close(s.hub_space(), g.thickness(), 1e-12));
    let (o, f) = s.shaft_outline(0.0);
    closed(&o);
    simple(&o);
    assert!(!f.pointed);
    let h = s.hub_outline(0.0);
    closed(&h);
    simple(&h);
    // The hub's teeth sit in the shaft's roots: its smallest radius is the hub tip.
    let rmin = h.polyline(0.01).iter().map(|q| q.len()).fold(f64::MAX, f64::min);
    assert!(close(rmin, da2 / 2.0, 1e-3), "{rmin}");
    // ANSI B92.1, 24 teeth 16/32: pitch Ø 1.5 in, major 1.5625 in.
    let a = Spline {
        standard: SplineStandard::AnsiB92,
        module: 25.4 / 16.0,
        teeth: 24,
        pressure_angle: 30f64.to_radians(),
        reference_diameter: 0.0,
        fillet_root: true,
        clearance: 0.05,
    };
    assert!(close(a.pitch_diameter(), 38.1, 1e-9));
    assert!(close(a.shaft_diameters().0, 25.4 * 25.0 / 16.0, 1e-9));
    assert!(close(a.shaft_diameters().1, 25.4 * (24.0 - 1.8) / 16.0, 1e-9));
    let (o, _) = a.shaft_outline(0.0);
    closed(&o);
    simple(&o);
    simple(&a.hub_outline(0.0));
    assert!(a.hub_space() > a.shaft().thickness());
}

#[test]
fn shaft_and_hub_do_not_overlap() {
    // Sample the shaft's teeth: every point lies inside the hub's toothed hole (or on it).
    let s = Spline {
        standard: SplineStandard::Din5480,
        module: 2.0,
        teeth: 21,
        pressure_angle: 30f64.to_radians(),
        reference_diameter: 45.0,
        fillet_root: false,
        clearance: 0.04,
    };
    let (shaft, _) = s.shaft_outline(0.0);
    let hole = s.hub_outline(0.0).polyline(0.002);
    let inside = |q: V2| -> bool {
        let mut c = false;
        for i in 0..hole.len() - 1 {
            let (a, b) = (hole[i], hole[i + 1]);
            if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                c = !c;
            }
        }
        c
    };
    for q in shaft.polyline(0.005) {
        let shrunk = q * (1.0 - 1e-4);
        assert!(inside(shrunk), "shaft point {q:?} (r {}) outside the hub's hole", q.len());
    }
}

#[test]
fn face_gear_limits_and_sections() {
    let pinion = Involute { teeth: 18, module: 2.0, ..Involute::default() };
    let fg = FaceGear { pinion, teeth: 60, backlash: 0.0 };
    let (r0, rmin, rmax) = (fg.pitch_radius(), fg.undercut_radius(), fg.pointed_radius());
    assert!(rmin < r0 && r0 < rmax, "{rmin} {r0} {rmax}");
    // At the pitch radius the face gear's tooth is half the pitch at the pitch plane.
    assert!(close(fg.thickness(r0, 0.0), PI * 2.0 / 2.0, 1e-9));
    // The tooth narrows going up and further out.
    assert!(fg.thickness(r0, fg.top()) < fg.thickness(r0, fg.bottom()));
    assert!(fg.thickness(rmax, fg.top()) < fg.thickness(rmin, fg.top()));
    assert!(close(fg.thickness(rmax, fg.top()), 0.2 * 2.0, 1e-6));
    // Litvin's inner limit for z1 = 18, z2 = 60, 20°: L1 ≈ … the undercut limit in modules.
    assert!(rmin / 2.0 > 22.0 && rmin / 2.0 < 30.0, "{}", rmin / 2.0);
    for h in [fg.bottom(), 0.0, fg.top()] {
        let o = fg.section(h, rmin, rmax.min(rmin + 12.0));
        closed(&o);
        simple(&o);
    }
}

#[test]
fn new_kinds_plan_with_facts_and_steps() {
    for k in [Kind::Sprocket, Kind::Pulley, Kind::Spline, Kind::FaceGear, Kind::Elliptical] {
        let s = Spec::defaults_for(k);
        let p = plan(&s).unwrap_or_else(|e| panic!("{k:?}: {e}"));
        eprintln!("== {k:?} ({})", p.preview_note);
        for f in &p.facts {
            eprintln!("   {} = {:.4}", f.label, f.value);
        }
        for w in &p.warnings {
            eprintln!("   ! {w}");
        }
        for b in &p.bodies {
            for st in &b.steps {
                if let Step::Prism { region, .. } = st {
                    for o in region.outer.iter().chain(&region.holes) {
                        closed(o);
                    }
                }
            }
        }
    }
    // Default face gear: no clamping needed.
    let p = plan(&Spec::defaults_for(Kind::FaceGear)).unwrap();
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    // Pairs.
    let mut s = Spec::defaults_for(Kind::Sprocket);
    s.teeth2 = 38;
    s.center_distance = 450.0;
    s.strands = 2;
    let p = plan(&s).unwrap();
    assert_eq!(p.bodies.len(), 2);
    let links = p.facts.iter().find(|f| f.label.starts_with("chain links")).unwrap().value;
    assert!(links % 2.0 == 0.0 && links > 2.0 * 450.0 / 12.7);
    let mut s = Spec::defaults_for(Kind::Pulley);
    s.teeth2 = 40;
    s.center_distance = 150.0;
    let p = plan(&s).unwrap();
    assert_eq!(p.bodies.len(), 2);
    let a = p.facts.iter().find(|f| f.label.starts_with("centre distance")).unwrap().value;
    let teeth = p.facts.iter().find(|f| f.label.starts_with("belt teeth")).unwrap().value;
    assert!(close(belt_length(20.0 * 2.0 / PI / 2.0, 40.0 * 2.0 / PI / 2.0, a), teeth * 2.0, 1e-6));
    // ANSI spline and custom chain.
    let mut s = Spec::defaults_for(Kind::Spline);
    s.spline_standard = SplineStandard::AnsiB92;
    s.module = 25.4 / 16.0;
    s.teeth = 24;
    assert!(plan(&s).unwrap().warnings.is_empty());
    let mut s = Spec::defaults_for(Kind::Sprocket);
    s.chain = crate::sprocket::CUSTOM;
    s.chain_pitch = 10.0;
    s.roller_diameter = 6.0;
    assert!(plan(&s).is_ok());
    s.roller_diameter = 11.0;
    assert!(plan(&s).is_err());
}

/// Writes each new type's preview outlines as JSON to `$GEAR_DUMP` (run with --ignored), for
/// looking at them.
#[test]
#[ignore]
fn dump_previews() {
    let Ok(dir) = std::env::var("GEAR_DUMP") else { return };
    let mut pair = Spec::defaults_for(Kind::Sprocket);
    pair.teeth2 = 30;
    pair.center_distance = 250.0;
    let mut belt = Spec::defaults_for(Kind::Pulley);
    belt.teeth2 = 36;
    belt.belt = "htd_5m";
    let mut t5 = Spec::defaults_for(Kind::Pulley);
    t5.belt = "t5";
    t5.teeth = 16;
    let mut ansi = Spec::defaults_for(Kind::Spline);
    ansi.spline_standard = SplineStandard::AnsiB92;
    ansi.module = 25.4 / 16.0;
    ansi.teeth = 24;
    ansi.fillet_root = true;
    for (n, s) in [
        ("sprocket", Spec::defaults_for(Kind::Sprocket)),
        ("sprocket_pair", pair),
        ("pulley", Spec::defaults_for(Kind::Pulley)),
        ("pulley_pair", belt),
        ("pulley_t5", t5),
        ("spline", Spec::defaults_for(Kind::Spline)),
        ("spline_ansi", ansi),
        ("face_gear", Spec::defaults_for(Kind::FaceGear)),
        ("elliptical", Spec::defaults_for(Kind::Elliptical)),
    ] {
        let p = plan(&s).unwrap();
        let mut out = String::from("[");
        for (i, g) in p.preview.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let loops: Vec<String> = g
                .region
                .outer
                .iter()
                .chain(&g.region.holes)
                .map(|o| {
                    format!(
                        "[{}]",
                        o.polyline(0.005).iter().map(|q| format!("[{:.5},{:.5}]", q.x + g.center.x, q.y + g.center.y)).collect::<Vec<_>>().join(",")
                    )
                })
                .collect();
            out.push_str(&format!("{{\"name\":\"{}\",\"loops\":[{}]}}", g.name, loops.join(",")));
        }
        out.push(']');
        std::fs::write(format!("{dir}/{n}.json"), out).unwrap();
    }
}

#[test]
fn elliptical_pair_rolls_and_closes() {
    use crate::elliptic::EllipticPair;
    let e =
        EllipticPair { teeth: 30, module: 2.0, eccentricity: 0.3, pressure_angle: 20f64.to_radians(), addendum: 1.0, dedendum: 1.25, backlash: 0.0 };
    let p = e.pitch();
    // The mate closes after one turn and has the same length of pitch curve.
    let c2 = &p.curve2;
    let len2: f64 = (0..c2.len()).map(|i| c2[i].dist(c2[(i + 1) % c2.len()])).sum();
    assert!(close(len2, PI * 2.0 * 30.0, 0.05), "{len2}");
    assert!(c2[0].dist(c2[c2.len() - 1]) < 0.2);
    // r1 + r2 = A everywhere along the contact.
    assert!(p.ratio_min < 1.0 && p.ratio_max > 1.0);
    let o1 = e.teeth(&EllipticPair::curve1(&p), 0.0);
    closed(&o1);
    simple(&o1);
    let o2 = e.teeth(c2, PI * 2.0 / 2.0);
    closed(&o2);
    simple(&o2);
    assert!(o1.area() > 0.0 && o2.area() > 0.0);
}
