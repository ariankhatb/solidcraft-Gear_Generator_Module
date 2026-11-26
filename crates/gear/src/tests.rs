use std::f64::consts::{PI, TAU};

use crate::geom::{Seg, V2, inv, inv_inv};
use crate::involute::{Involute, external_flank, external_outline, mesh_external, shift_sum_for_center};
use crate::plan::{Kind, Spec, Step, plan};

use crate::test_util::close;

#[test]
fn involute_function_round_trips() {
    for deg in [5.0f64, 14.5, 20.0, 25.0, 35.0, 50.0] {
        let a = deg.to_radians();
        assert!(close(inv_inv(inv(a)), a, 1e-12), "{deg}");
    }
}

#[test]
fn generated_flank_is_the_involute_above_the_form_circle() {
    let g = Involute { teeth: 24, module: 2.0, ..Involute::default() };
    let f = external_flank(&g);
    assert!(close(f.rf, g.rf(), 1e-9));
    assert!(close(f.ra, g.ra(), 1e-9));
    // On the involute part the generated half angle matches the exact formula.
    for (rr, h) in &f.flank {
        if *rr > g.rb() * 1.02 {
            assert!(close(*h, g.half_angle_at(*rr), 2e-4), "r {rr}: {h} vs {}", g.half_angle_at(*rr));
        }
    }
    // Pitch circle tooth thickness is half the pitch.
    let r = g.r();
    assert!(close(g.thickness_at(r), PI * 2.0 / 2.0, 1e-9));
    assert!(!f.undercut && !f.pointed);
}

#[test]
fn shift_thickens_the_tooth_and_small_counts_undercut() {
    let g = Involute { teeth: 8, module: 1.0, ..Involute::default() };
    assert!(external_flank(&g).undercut);
    let shifted = Involute { shift: 0.6, ..g };
    assert!(!external_flank(&shifted).undercut);
    assert!(shifted.thickness() > g.thickness());
    assert!(close(Involute::default().min_teeth_no_undercut(), 17.097, 1e-2));
}

#[test]
fn outline_is_closed_and_has_the_right_area() {
    let g = Involute { teeth: 30, module: 1.5, ..Involute::default() };
    let (o, f) = external_outline(&g, 0.0);
    for w in o.segs.windows(2) {
        assert!(w[0].end().dist(w[1].start()) < 1e-9);
    }
    assert!(o.segs.last().unwrap().end().dist(o.segs[0].start()) < 1e-9);
    let a = o.area();
    let (ri, ro) = (f.rf, f.ra);
    assert!(a > PI * ri * ri && a < PI * ro * ro, "{a}");
    // Roughly the pitch circle's area.
    assert!(close(a / (PI * g.r() * g.r()), 1.0, 0.05), "{}", a / (PI * g.r() * g.r()));
    assert!(o.segs.iter().all(|s| !matches!(s, Seg::Spline(p) if p.iter().any(|q| !q.is_finite()))));
}

#[test]
fn mesh_of_standard_gears_and_shift_for_a_centre_distance() {
    let g1 = Involute { teeth: 20, module: 2.0, ..Involute::default() };
    let g2 = Involute { teeth: 40, module: 2.0, ..Involute::default() };
    let m = mesh_external(&g1, &g2, 10.0);
    assert!(close(m.center_distance, 60.0, 1e-9));
    assert!(close(m.working_pressure_angle, 20f64.to_radians(), 1e-9));
    assert!(m.contact_ratio > 1.55 && m.contact_ratio < 1.75, "{}", m.contact_ratio);
    let sum = shift_sum_for_center(&g1, &g2, 61.0).unwrap();
    let g2s = Involute { shift: sum, ..g2 };
    assert!(close(mesh_external(&g1, &g2s, 10.0).center_distance, 61.0, 1e-6));
}

#[test]
fn every_kind_plans() {
    for k in Kind::ALL.iter().copied() {
        let s = Spec::defaults_for(k);
        let p = plan(&s).unwrap_or_else(|e| panic!("{k:?}: {e}"));
        assert!(!p.bodies.is_empty(), "{k:?}");
        assert!(!p.preview.is_empty(), "{k:?}");
        assert!(!p.facts.is_empty(), "{k:?}");
        for b in &p.bodies {
            assert!(!b.steps.is_empty());
            for st in &b.steps {
                match st {
                    Step::Prism { region, height, .. } => {
                        assert!(*height > 0.0);
                        assert!(!region.outer.is_empty());
                    }
                    Step::Loft { sections, .. } => assert!(sections.len() >= 2),
                    Step::Taper { height, scale, .. } => assert!(*height > 0.0 && *scale > 0.0),
                    Step::RingTaper { height, scale, rim, outline, .. } => assert!(*height > 0.0 && *scale > 0.0 && *rim > outline.max_radius()),
                    Step::Revolve { profile, .. } => assert!(profile.len() >= 3),
                    Step::Coil { section, pitch, .. } => assert!(section.len() >= 3 && *pitch > 0.0),
                    // One hand of V bevel teeth, trimmed to its half of the face.
                    Step::Group { steps, .. } => assert!(steps.first().map(Step::op) == Some(crate::plan::Op::New), "{k:?}"),
                    // A differential's case has its pin holes and windows in frames of their own.
                    Step::Placed { .. } if k == Kind::Differential => {}
                    // A bevel planetary set's carrier has a pin placed along every planet's axis.
                    Step::Placed { .. } if b.name.contains(crate::plan::bevel_planet::CARRIER) => {}
                    Step::Placed { .. } => panic!("{k:?}: nothing is placed by default"),
                }
            }
        }
        for w in &p.warnings {
            eprintln!("{k:?}: {w}");
        }
    }
}

#[test]
fn planetary_planets_mesh_with_the_ring() {
    let s = Spec::defaults_for(Kind::Planetary);
    let p = plan(&s).unwrap();
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    assert_eq!(p.bodies.len(), 1 + 3 + 1);
}

#[test]
fn bevel_teeth_shrink_towards_the_apex() {
    let s = Spec::defaults_for(Kind::Bevel);
    let p = plan(&s).unwrap();
    let Step::Taper { height, scale, twist, .. } = &p.bodies[0].steps[0] else { panic!() };
    assert!(*height > 0.0 && *scale < 1.0 && *scale > 0.3 && twist.abs() < 1e-12);
    let s = Spec::defaults_for(Kind::SpiralBevel);
    let p = plan(&s).unwrap();
    let Step::Taper { twist, .. } = &p.bodies[0].steps[0] else { panic!() };
    assert!(twist.abs() > 0.01);
}

#[test]
fn polyline_samples_arcs_the_right_way() {
    let seg = Seg::Arc { c: V2::ZERO, a: V2::new(1.0, 0.0), b: V2::new(0.0, 1.0), ccw: true };
    let mut pts = Vec::new();
    seg.sample(0.001, &mut pts);
    assert!(pts.iter().all(|p| p.x >= -1e-9 && p.y >= -1e-9));
    let back = seg.reversed();
    let mut pts = Vec::new();
    back.sample(0.001, &mut pts);
    assert!(pts.iter().all(|p| p.x >= -1e-9 && p.y >= -1e-9));
    assert!(close(TAU, TAU, 0.0));
}

#[test]
fn tooth_sides_with_an_empty_fit_piece_still_make_an_outline() {
    // An empty piece used to reach the corner joining as a spline with no first point and panic.
    use crate::involute::{ToothSides, sided_outline};
    let side = |s: f64| vec![vec![], vec![(10.0, 0.1 * s), (12.0, 0.05 * s)], vec![]];
    let t = ToothSides { rf: 10.0, ra: 12.0, lower: side(-1.0), upper: side(1.0) };
    let o = sided_outline(&t, 12, 0.0);
    assert!(o.segs.iter().all(|s| !matches!(s, Seg::Spline(p) if p.is_empty())));
    let n = o.segs.len();
    for i in 0..n {
        let (end, start) = (o.segs[(i + n - 1) % n].end(), o.segs[i].start());
        assert!((end.x - start.x).abs() < 1e-12 && (end.y - start.y).abs() < 1e-12, "gap at segment {i}");
    }
    assert_eq!(Seg::Spline(vec![]).start(), V2::ZERO);
}

#[test]
fn a_face_gear_with_tens_of_thousands_of_teeth_gives_sections_not_a_panic() {
    // π / 30 000 · 0.85 is below the 1e-4 rad floor on a tooth's half width: the clamp's bounds
    // crossed and it panicked.
    let g = crate::face_gear::FaceGear::new(Involute { teeth: 18, module: 0.01, ..Involute::default() }, 30_000, 0.0);
    let r = 0.005 * 30_000.0;
    let _ = g.sections(&[0.0, 0.01], r, r + 1.0);
}

#[test]
fn an_envelope_reports_no_crossing_where_its_curve_is_not_finite() {
    // r = 1 + t, angle t, but no value for 0.4 < t < 0.6. Sampled only at its ends, the bracket
    // for r = 1.5 refines straight into the gap: it used to report the bracket's start (angle
    // 0, at r = 1) as the crossing.
    use crate::worm::Envelope;
    let f = |_: usize, t: f64| if t > 0.4 && t < 0.6 { (f64::NAN, f64::NAN) } else { (1.0 + t, t) };
    assert_eq!(Envelope::new(f, &[1]).extreme(1.5), None);
    // Where the curve has values, the crossing is found: angle 0.25 at r = 1.25.
    let (a, _) = Envelope::new(f, &[10]).extreme(1.25).unwrap();
    assert!((a - 0.25).abs() < 1e-9, "{a}");
}

#[test]
fn a_worm_wheel_tooth_with_no_flank_spans_is_drawn_with_one() {
    // A module 1, single start ZA worm (reference radius 5, 20° axial pressure angle) on a
    // 30-tooth wheel (pitch radius 15, centre distance 20), the hob as the planner makes it.
    let a20 = 20f64.to_radians().tan();
    let hob = crate::worm::Hob {
        centre: 20.0,
        wheel_pitch: 15.0,
        reference: 5.0,
        tip: 6.25f64.min(5.0 + (PI / 4.0 - 0.02) / a20),
        corner: 0.38,
        low: 3.25,
        half: PI / 4.0,
        tan_axial: a20,
        screw: PI / TAU,
    };
    // 0 flank spans used to make every flank point 0 / 0 = NaN.
    let none = hob.tooth_with(0.0, 16.0, 30, 1.0, 6, 0);
    assert_eq!(none, hob.tooth_with(0.0, 16.0, 30, 1.0, 6, 1));
    if let Some(t) = none {
        assert!(t.lower.iter().chain(&t.upper).flatten().all(|p| p.0.is_finite() && p.1.is_finite()));
    }
}

#[test]
fn a_tiny_cycloidal_gears_dedendum_stays_radial() {
    // Rolling circles half the pitch radius make the hypocycloid a diameter: below the pitch
    // circle each side runs at the half pitch angle π / (2 z). At module 1e-8 (pitch radius
    // 1e-7) the rolling circle's 1e-6 floor used to exceed the pitch radius and bend it.
    use crate::cycloid::{Cycloidal, cycloidal_outline};
    for m in [1.0, 1e-8] {
        let c = Cycloidal::with_defaults(20, m);
        let (_, f) = cycloidal_outline(&c, 0.0);
        let want = PI / 40.0;
        let below: Vec<_> = f.fillet.iter().chain(&f.flank).filter(|q| q.0 < c.r() * (1.0 - 1e-6)).collect();
        assert!(!below.is_empty(), "module {m}");
        for q in below {
            assert!((q.1 - want).abs() < 1e-9, "module {m}: angle {} at r {}", q.1, q.0);
        }
    }
}
