//! The tooth formulas against the exact numeric teeth they stand for.

use crate::equations::{Edge, End, Form, Repeat, TeethSource, ToothCell, cell_for, cell_of, eval, render};
use crate::geom::{Outline, V2};
use crate::involute::{ToothFlank, rack_outline};
use crate::plan::{Kind, Spec, Step, plan};

fn source(s: &Spec, who: &str) -> TeethSource {
    let p = plan(s).unwrap();
    p.teeth.iter().find(|t| t.who == who).cloned().unwrap_or_else(|| panic!("no {who} in {:?}", p.teeth.iter().map(|t| &t.who).collect::<Vec<_>>()))
}

fn spec(kind: Kind, f: impl FnOnce(&mut Spec)) -> Spec {
    let mut s = Spec::defaults_for(kind);
    f(&mut s);
    s
}

/// The parameter where curve `i`'s radius is `r` (it grows along the curve).
fn at_radius(cell: &ToothCell, i: usize, r: f64) -> f64 {
    let (a, b) = cell.range(i).unwrap();
    let (mut lo, mut hi) = (a, b);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if cell.curve_at(i, mid).unwrap().len() < r {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Every fit point of the numeric flank and fillet lies on the formulas' curves, both sides,
/// within 1e-6 modules; and the curves' ends are the numeric tooth's.
fn check_external(src: &TeethSource) -> ToothCell {
    let Form::External { g, f } = &src.form else { panic!("not external") };
    let cell = cell_of(src, 0.0).unwrap();
    let tol = 1e-6 * g.module;
    let (fillet_lower, flank_lower, flank_upper, fillet_upper) = (0, 1, 2, 3);
    for (side, sign, flank, fillet) in [("upper", 1.0, flank_upper, fillet_upper), ("lower", -1.0, flank_lower, fillet_lower)] {
        for (r, h) in &f.flank {
            let q = cell.curve_at(flank, at_radius(&cell, flank, *r)).unwrap();
            let want = V2::polar(*r, sign * h);
            assert!(q.dist(want) < tol, "{side} flank at r {r}: {q:?} vs {want:?} ({})", q.dist(want));
        }
        for (r, h) in f.fillet.iter().take(f.fillet.len() - 1) {
            let q = cell.curve_at(fillet, at_radius(&cell, fillet, *r)).unwrap();
            let want = V2::polar(*r, sign * h);
            assert!(q.dist(want) < tol, "{side} fillet at r {r}: {q:?} vs {want:?} ({})", q.dist(want));
        }
        // Ends: the root, the handover and the tip.
        let root = cell.end(&End::Start(fillet)).unwrap();
        assert!(root.dist(V2::polar(f.rf, sign * f.root_half)) < tol, "{side} root {root:?}");
        let tip = cell.end(&End::End(flank)).unwrap();
        assert!(tip.dist(V2::polar(f.ra, sign * f.tip_half)) < tol, "{side} tip {tip:?}");
        let joint = cell.end(&End::End(fillet)).unwrap().dist(cell.end(&End::Start(flank)).unwrap());
        assert!(joint < 1e-9 * g.module.max(1.0), "{side}: the fillet and the flank meet ({joint})");
        let start = cell.end(&End::Start(flank)).unwrap();
        assert!((start.len() - f.flank[0].0).abs() < tol, "{side}: the flank starts at r {} not {}", start.len(), f.flank[0].0);
    }
    cell
}

#[test]
fn the_template_evaluator_reads_the_expression_language() {
    let vars = |k: &str| match k {
        "m" => Some(2.0),
        "t" => Some(0.5),
        _ => None,
    };
    let near = |a: f64, b: f64| assert!((a - b).abs() < 1e-12, "{a} vs {b}");
    near(eval("{m} * 3 + 1", &vars).unwrap(), 7.0);
    near(eval("-({m}) ^ 2", &vars).unwrap(), -4.0);
    near(eval("2 ^ 3 ^ 2", &vars).unwrap(), 512.0);
    near(eval("180 deg", &vars).unwrap(), std::f64::consts::PI);
    near(eval("t * 1 rad + atan(t)", &vars).unwrap(), 0.5 + 0.5f64.atan());
    near(eval("hypot(3 mm, 4 mm) + min(1, 2, -3) + max(0 mm, 2)", &vars).unwrap(), 4.0);
    near(eval("1e-3 + 2.5e2", &vars).unwrap(), 250.001);
    near(eval("pi / {m}", &vars).unwrap(), std::f64::consts::FRAC_PI_2);
    assert!(eval("{nope}", &vars).is_err());
    assert!(eval("2 +", &vars).is_err());
    assert_eq!(render("{a} + {b_c} * {a}", &|k| format!("P_{k}")), "P_a + P_b_c * P_a");
}

#[test]
fn a_spur_gears_formulas_are_its_generated_flank_and_fillet() {
    let src = source(&Spec::default(), "gear");
    let cell = check_external(&src);
    assert!(cell.exact, "a 20-tooth gear has no undercut: every range is a formula");
    assert_eq!(cell.count().unwrap(), 20);
    assert_eq!(cell.curves.len(), 4);
    assert_eq!(cell.joins.len(), 2);
    assert!(matches!(cell.repeat, Repeat::Circular { .. }));
    // The root half-arcs end half a pitch either side of the tooth: copies meet there.
    let Form::External { f, .. } = &src.form else { unreachable!() };
    let half = std::f64::consts::PI / 20.0;
    assert!(cell.point(1).unwrap().dist(V2::polar(f.rf, half)) < 1e-12);
    assert!(cell.point(0).unwrap().dist(V2::polar(f.rf, -half)) < 1e-12);
}

#[test]
fn helical_shifted_and_backlashed_gears_follow_their_flanks() {
    let cases: Vec<Spec> = vec![
        spec(Kind::Helical, |s| s.helix = 25.0),
        spec(Kind::Spur, |s| {
            s.shift = 0.4;
            s.backlash = 0.1;
        }),
        spec(Kind::Spur, |s| {
            s.teeth = 41;
            s.module = 1.5;
            s.pressure_angle = 25.0;
            s.shift = -0.2;
        }),
        spec(Kind::Spur, |s| s.rack = crate::involute::BasicRack::standard("print_fdm").unwrap()),
        spec(Kind::Herringbone, |s| s.teeth = 30),
    ];
    for s in cases {
        let src = source(&s, "gear");
        let cell = check_external(&src);
        assert!(cell.exact, "{:?} z{} x{}: exact", s.kind, s.teeth, s.shift);
        assert_eq!(cell.helix_key.is_some(), s.kind != Kind::Spur);
    }
}

#[test]
fn an_undercut_gear_gets_numeric_ranges_where_the_fillet_crosses_the_flank() {
    for z in [7, 9, 12] {
        let src = source(&spec(Kind::Spur, |s| s.teeth = z), "gear");
        let Form::External { f, .. } = &src.form else { unreachable!() };
        assert!(f.undercut, "z{z} is undercut");
        let cell = check_external(&src);
        assert!(!cell.exact, "z{z}: the crossing is numeric");
        assert!(cell.joins.is_empty());
    }
}

#[test]
fn a_ring_gears_formulas_are_its_tooth_spaces() {
    for s in [
        spec(Kind::Internal, |_| {}),
        spec(Kind::Internal, |s| {
            s.teeth = 45;
            s.helix = 15.0;
            s.shift = 0.3;
        }),
    ] {
        let src = source(&s, "ring");
        let Form::Internal { ring, f } = &src.form else { panic!("not a ring") };
        let cell = cell_of(&src, 0.0).unwrap();
        let tol = 1e-6 * ring.gear.module;
        for (sign, i) in [(-1.0, 0), (1.0, 1)] {
            for (r, h) in &f.flank {
                let q = cell.curve_at(i, at_radius(&cell, i, *r)).unwrap();
                assert!(q.dist(V2::polar(*r, sign * h)) < tol, "flank at r {r}: {q:?}");
            }
            let tip = cell.end(&End::End(i)).unwrap();
            assert!(tip.dist(V2::polar(f.ra, sign * f.tip_half)) < tol, "root end {tip:?}");
        }
        for (sign, q) in [(-1.0, cell.point(0).unwrap()), (1.0, cell.point(1).unwrap())] {
            assert!(q.dist(V2::polar(f.rf, sign * std::f64::consts::PI / f64::from(ring.gear.teeth))) < tol);
        }
        assert_eq!(cell.count().unwrap() as u32, ring.gear.teeth);
    }
}

#[test]
fn a_racks_points_are_its_outline() {
    for s in [spec(Kind::Rack, |_| {}), spec(Kind::Rack, |s| {
        s.shift = 0.3;
        s.backlash = 0.05;
        s.rack_teeth = 7;
    })] {
        let src = source(&s, "rack");
        let Form::Rack { g, count, below } = &src.form else { panic!("not a rack") };
        let o = rack_outline(g, *count, *below);
        let cell = cell_of(&src, 0.0).unwrap();
        let Repeat::Linear { spacing, .. } = &cell.repeat else { panic!("linear") };
        let pitch = cell.vars.eval(spacing, None).unwrap();
        // Every outline corner is a cell corner, shifted by whole pitches (or on the back).
        let mut corners: Vec<V2> = Vec::new();
        for k in 0..*count {
            for e in &cell.edges {
                let (Edge::Line(a, b) | Edge::Arc(a, b)) = e;
                for end in [a, b] {
                    corners.push(cell.end(end).unwrap() + V2::new(pitch * f64::from(k), 0.0));
                }
            }
        }
        for e in &cell.closing {
            let (Edge::Line(a, b) | Edge::Arc(a, b)) = e;
            corners.push(cell.end(a).unwrap());
            corners.push(cell.end(b).unwrap());
        }
        for seg in &o.segs {
            let q = seg.start();
            assert!(corners.iter().any(|c| c.dist(q) < 1e-9), "corner {q:?} is not drawn");
        }
        assert_eq!(cell.count().unwrap() as u32, *count);
    }
}

/// The tooth outlines of every prism of a plan.
fn prism_outlines(s: &Spec) -> Vec<Outline> {
    let p = plan(s).unwrap();
    let mut out = Vec::new();
    for b in &p.bodies {
        for st in &b.steps {
            if let Step::Prism { region, .. } = st {
                out.extend(region.outer.iter().chain(&region.holes).filter(|o| o.segs.len() > 12).cloned());
            }
        }
    }
    out
}

#[test]
fn every_tooth_outline_of_the_planar_sets_finds_its_formulas() {
    for (kind, members) in [
        (Kind::Spur, 1),
        (Kind::Helical, 1),
        (Kind::Herringbone, 2),
        (Kind::Internal, 1),
        (Kind::Pair, 2),
        (Kind::Planetary, 5),
        (Kind::Rack, 1),
    ] {
        let s = Spec::defaults_for(kind);
        let p = plan(&s).unwrap();
        let outlines = prism_outlines(&s);
        let found: Vec<(&Outline, ToothCell)> = outlines.iter().filter_map(|o| cell_for(&p.teeth, o).map(|c| (o, c))).collect();
        assert!(found.len() >= members, "{kind:?}: {} of {} outlines found formulas", found.len(), outlines.len());
        for (o, c) in &found {
            // The cell's tooth 0 is where the outline's is: its curves' ends are outline points.
            let pts: Vec<V2> = o.segs.iter().map(|s| s.start()).collect();
            for i in 0..c.curves.len() {
                let q = c.end(&End::Start(i)).unwrap();
                assert!(pts.iter().any(|p| p.dist(q) < 1e-6 * c.module), "{kind:?} {}: curve {i} starts off the outline", c.who);
            }
        }
    }
}

#[test]
fn tuned_tips_have_no_formulas_and_are_left_to_the_fitted_splines() {
    let s = spec(Kind::Spur, |s| s.tip_round = 0.2);
    let p = plan(&s).unwrap();
    let outlines = prism_outlines(&s);
    assert!(!outlines.is_empty());
    assert!(outlines.iter().all(|o| cell_for(&p.teeth, o).is_none()), "tip rounding stays fitted");
}

#[test]
fn rendered_formulas_fit_an_equation_curve_and_evaluate_alike() {
    for s in [Spec::default(), spec(Kind::Helical, |s| s.helix = 20.0), spec(Kind::Spur, |s| s.teeth = 9), spec(Kind::Internal, |_| {})] {
        let p = plan(&s).unwrap();
        let src = &p.teeth[0];
        let cell = cell_of(src, 0.3).unwrap();
        for tpl in cell.templates() {
            // As design parameters with a long prefix, and as numbers: both short enough for an
            // equation curve (1024 characters), and the numbers give the same values.
            let named = render(tpl, &|k| format!("Planetary_set_{k}"));
            assert!(named.len() < 1024, "{} characters: {named}", named.len());
            let numeric = cell.vars.numeric(tpl);
            assert!(numeric.len() < 1024, "{} characters: {numeric}", numeric.len());
            assert!(!numeric.contains('{'), "{numeric}");
            for t in [0.0, 0.3, 1.1] {
                let a = cell.vars.eval(tpl, Some(t)).unwrap();
                let b = eval(&numeric, &|k| (k == "t").then_some(t)).unwrap();
                assert!((a - b).abs() <= 1e-9 * a.abs().max(1.0), "{tpl}: {a} vs {b}");
            }
        }
        // Derived values agree with their formulas over the inputs.
        for v in &cell.vars.list {
            if !v.input {
                let again = cell.vars.eval(&v.expr, None).unwrap();
                assert!((again - v.value).abs() <= 1e-12 * v.value.abs().max(1.0), "{}", v.key);
            }
        }
    }
}

#[test]
fn a_flank_without_a_land_is_refused() {
    let src = source(&spec(Kind::Spur, |s| {
        s.teeth = 10;
        s.shift = 1.2;
    }), "gear");
    let Form::External { f, .. } = &src.form else { unreachable!() };
    let f: &ToothFlank = f;
    if f.pointed {
        assert!(cell_of(&src, 0.0).is_err());
    }
}
