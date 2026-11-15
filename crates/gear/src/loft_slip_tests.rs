//! Every loft the engine builds must survive the kernel's section alignment (it restarts each
//! section at the segment that best lines up with the one before, `loft_numbered_loops`): if a
//! section is matched a few segments off its partner, the loft's walls cut across the teeth.
//! [`crate::hypoid::slips`] copies that rule; this runs it on every kind's lofts, including the
//! spiral tapers the engine lofts through [`crate::plan::taper_sections`] sections.

use crate::geom::Outline;
use crate::plan::{Kind, Spec, Step, plan, taper_turn};


/// The sections the engine lofts for a step, if it lofts it.
fn lofted(step: &Step) -> Option<Vec<Outline>> {
    match step {
        Step::Loft { sections, .. } => Some(sections.iter().map(|(o, _)| o.clone()).collect()),
        Step::Taper { outline, twist, scale, .. } if twist.abs() > 1e-12 => Some({
            let n = crate::plan::taper_sections(outline, *twist, *scale);
            (0..n)
                .map(|i| {
                    let t = i as f64 / (n - 1) as f64;
                    let k = 1.0 + (scale - 1.0) * t;
                    outline.scaled(k).rotated(taper_turn(*twist, *scale, k))
                })
                .collect()
        }),
        Step::Taper { outline, scale, .. } => Some(vec![outline.clone(), outline.scaled(*scale)]),
        Step::Placed { step, .. } => lofted(step),
        _ => None,
    }
}

/// The tooth count a loft's outline repeats with: the first of the set's counts that divides
/// its corner count (whole-tooth restarts are harmless), else none (any restart must be 0).
fn teeth_of(s: &Spec, sections: &[Outline]) -> usize {
    let corners = sections[0].segs.len();
    [s.teeth, s.teeth2, s.teeth3].into_iter().map(|z| z as usize).find(|z| *z > 1 && corners % z == 0).unwrap_or(0)
}

fn check(s: &Spec, label: &str, bad: &mut Vec<String>) {
    let Ok(p) = plan(s) else { return };
    for b in p.bodies.iter().chain(p.clearance.iter()) {
        for (i, st) in b.steps.iter().enumerate() {
            if let Some(sections) = lofted(st)
                && sections.len() >= 2
                && sections.iter().all(|o| o.segs.len() == sections[0].segs.len())
                && crate::hypoid::slips(&sections, teeth_of(s, &sections))
            {
                bad.push(format!("{label}: {} step {} ({} sections)", b.name, i + 1, sections.len()));
            }
        }
    }
}

#[test]
fn no_kernel_loft_slips_in_any_kind() {
    let mut bad = Vec::new();
    for k in Kind::ALL {
        check(&Spec::defaults_for(k), &format!("{k:?} defaults"), &mut bad);
    }
    // Harder variants of the lofted kinds: steep spirals, other shaft angles, multi-start worms.
    let mut variants = Vec::new();
    for (angle, spiral) in [(90.0, 45.0), (60.0, 35.0), (120.0, 35.0), (90.0, 20.0)] {
        variants.push((format!("spiral bevel {angle}°/{spiral}°"), Spec { shaft_angle: angle, spiral_angle: spiral, ..Spec::defaults_for(Kind::SpiralBevel) }));
    }
    for (starts, teeth) in [(1, 30), (2, 41), (4, 40)] {
        variants.push((format!("worm {starts}-start z{teeth}"), Spec { starts, teeth, ..Spec::defaults_for(Kind::Worm) }));
    }
    for angle in [60.0, 75.0, 90.0] {
        variants.push((format!("hypoid {angle}°"), Spec { shaft_angle: angle, ..Spec::defaults_for(Kind::Hypoid) }));
    }
    for (label, s) in &variants {
        check(s, label, &mut bad);
    }
    assert!(bad.is_empty(), "lofts the kernel would mis-align:\n{}", bad.join("\n"));
}

#[test]
fn a_pair_can_be_herringbone() {
    let s = Spec { helix: 20.0, herringbone: true, ..Spec::defaults_for(Kind::Pair) };
    let p = plan(&s).unwrap();
    // Each gear's teeth are two opposite halves: the helical body turns one way, then back.
    for b in &p.bodies {
        let twisted = b.steps.iter().filter(|st| match st { Step::Loft { .. } => true, Step::Prism { twist, .. } => *twist != 0.0, _ => false }).count();
        assert!(twisted >= 1, "{}: {:?}", b.name, b.steps.len());
    }
    let simple = crate::simple::outputs_of(&s);
    assert!(simple.herringbone);
    let back = crate::simple::solve(&simple, &s);
    assert!(back.spec.herringbone, "{:?}", back.notes);
    assert!(!back.notes.iter().any(|n| n.contains("not built")), "{:?}", back.notes);
}
