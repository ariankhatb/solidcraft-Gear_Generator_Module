//! How the pairs of a drivetrain of items turn, checked against their teeth (not only the ratio's
//! size): the assembly's turn of the mate per turn of the driver, put on the planned bodies,
//! keeps the teeth in mesh (no penetration over a stretch of the turn, the teeth in the gaps at
//! rest), while the opposite sign drives them into each other. Drawn as the owner's 4×4: a prop
//! line bending 20° into a bevel pair (160° between the axes pointed at the apex), and a hypoid
//! whose pinion lies along the bent line, the rear one the front's mirror image.

use crate::items::{GearItem, GearRef, GearSettings, ItemKind, ItemLink, LinkKind, SizeBy, Thing};
use crate::plan::Spec;
use crate::plan::system::{SystemPlan, plan_items};

fn g(item: usize, gear: usize) -> GearRef {
    GearRef::new(item, gear)
}

/// The bend: from the prop line's end at `x` (±80) on, 20° up in the xz plane, 24.36 mm long.
fn bend(x: f64) -> [f64; 3] {
    let a = 20f64.to_radians();
    [x + x.signum() * 24.36 * a.cos(), 0.0, 24.36 * a.sin()]
}

fn teeth(z1: u32, z2: u32, module: Option<f64>) -> Vec<GearSettings> {
    let s = |z| GearSettings { size_by: SizeBy::Teeth, teeth: Some(z), module, ..GearSettings::default() };
    vec![s(z1), s(z2)]
}

fn bevel(x: f64) -> GearItem {
    let mut it = GearItem::new("K", ItemKind::Bevel).with(vec![Thing::line([0.0; 3], [x, 0.0, 0.0]), Thing::line([x, 0.0, 0.0], bend(x))]);
    it.gears = teeth(30, 30, Some(0.6));
    it
}

/// The hypoid at the end of the bent line: the ring (Ø50, axis y) at (±125, 0, 20), the pinion
/// along the bent line.
fn hypoid(x: f64) -> GearItem {
    let mut it = GearItem::new("H", ItemKind::Hypoid).with(vec![Thing::circle([x.signum() * 125.0, 0.0, 20.0], [0.0, 1.0, 0.0], 25.0), Thing::line([x, 0.0, 0.0], bend(x))]);
    it.gears = teeth(37, 9, None);
    it
}

fn planned(items: &[GearItem], links: &[ItemLink]) -> SystemPlan {
    let sp = plan_items(items, links, &[], None, &Spec::default()).unwrap_or_else(|e| panic!("{e}"));
    assert!(sp.error.is_none(), "{:?}", sp.error);
    sp
}

/// Turn of body `b` about its frame's z per turn of body `a` about its frame's z, as the
/// assembly has it (the assembly turns each member about its own axis, which may point against
/// its frame's z).
fn spin_ratio(sp: &SystemPlan, a: usize, b: usize) -> f64 {
    let asm = &sp.plan.assembly;
    let r = asm.ratio(a, b).unwrap_or_else(|| panic!("no ratio {a} → {b}"));
    let sign = |k: usize| {
        let d = asm.members[k].axis.dir;
        let z = sp.plan.bodies[k].frame.z;
        if d[0] * z[0] + d[1] * z[1] + d[2] * z[2] < 0.0 { -1.0 } else { 1.0 }
    };
    r * sign(a) * sign(b)
}

/// The deepest penetration of the pair turned `ratio` (frame spins) over `turn` radians of `a`
/// from rest, in `steps` steps, and at rest.
fn run(sp: &SystemPlan, a: usize, b: usize, ratio: f64, turn: f64, steps: usize) -> (f64, f64) {
    let (ba, bb) = (&sp.plan.bodies[a], &sp.plan.bodies[b]);
    let mut pair = crate::mesh3d::Pair::for_run(ba, bb, 0.05, 0.2, ratio, -turn, turn);
    let rest = pair.at(0.0, 0.0).penetration;
    let r = pair.run(ratio, 0.0, -turn, turn, steps);
    (r.penetration, rest)
}

/// A pair's check: at rest the teeth sit in the gaps, the assembly's turn keeps them in mesh,
/// and the other sign runs them into each other (so the check can tell).
fn meshes(sp: &SystemPlan, r0: GearRef, r1: GearRef, module: f64) {
    let (a, b) = (sp.body(r0).unwrap(), sp.body(r1).unwrap());
    let ratio = spin_ratio(sp, a, b);
    let turn = 1.5 * std::f64::consts::TAU / 30.0;
    let (good, rest) = run(sp, a, b, ratio, turn, 5);
    let (bad, _) = run(sp, a, b, -ratio, turn, 5);
    let tol = 0.05 * module;
    assert!(rest <= tol, "at rest the teeth overlap {rest} mm: their phase is wrong");
    assert!(good <= tol, "turned as the assembly turns them (ratio {ratio}) the teeth run {good} mm into each other");
    assert!(bad > 4.0 * tol, "turned the other way the teeth run only {bad} mm in: the check can't tell the sign");
}

#[test]
fn a_bevel_pair_at_a_20_degree_bend_meshes_and_turns_the_right_way() {
    for x in [80.0, -80.0] {
        let sp = planned(&[bevel(x)], &[]);
        meshes(&sp, g(0, 0), g(0, 1), 0.6);
        // 30 : 30, so 1 : 1 in size.
        let r = sp.plan.assembly.ratio(sp.body(g(0, 0)).unwrap(), sp.body(g(0, 1)).unwrap()).unwrap();
        assert!((r.abs() - 1.0).abs() < 1e-9, "{r}");
    }
}

#[test]
fn a_hypoid_on_the_bent_line_turns_its_ring_the_way_its_pinion_drives_it() {
    for x in [80.0, -80.0] {
        let sp = planned(&[hypoid(x)], &[]);
        let m = sp.gears.iter().find(|r| r.item == 0 && r.gear == 0).map(|r| r.module).unwrap();
        meshes(&sp, g(0, 1), g(0, 0), m);
    }
}

#[test]
fn front_and_rear_axles_turn_the_same_way_about_y() {
    // Prop shaft along x (both bevels' first gears on it), each bend's mate carries its hypoid's
    // pinion: the rings must turn the same way about +y, so the car goes one way.
    let mut prop = GearItem::new("P", ItemKind::Shaft).with(vec![Thing::line([-80.0, 0.0, 0.0], [80.0, 0.0, 0.0])]);
    prop.shaft_diameter = Some(6.0);
    let items = vec![bevel(80.0), bevel(-80.0), hypoid(80.0), hypoid(-80.0), prop];
    let sh = |a: GearRef, b: GearRef| ItemLink { a, b, kind: LinkKind::Shaft };
    let links = vec![sh(g(4, 0), g(0, 0)), sh(g(4, 0), g(1, 0)), sh(g(0, 1), g(2, 1)), sh(g(1, 1), g(3, 1))];
    let sp = planned(&items, &links);
    let p = sp.body(g(4, 0)).unwrap();
    let about_y = |r: GearRef| {
        let b = sp.body(r).unwrap();
        let d = sp.plan.assembly.members[b].axis.dir;
        sp.plan.assembly.ratio(p, b).unwrap() * d[1].signum()
    };
    let (front, rear) = (about_y(g(2, 0)), about_y(g(3, 0)));
    assert!((front.abs() - rear.abs()).abs() < 1e-9, "{front} {rear}");
    assert!(front * rear > 0.0, "the front axle turns {front} about +y per turn of the prop shaft, the rear {rear}");
}

// ---------------------------------------------------------------------------------------------
// Differentials on the hypoid rings (the 4×4's axles)
// ---------------------------------------------------------------------------------------------

fn with_diff(mut it: GearItem) -> GearItem {
    it.differential = Some(crate::items::DiffItem::default());
    it
}

fn body_named(sp: &SystemPlan, name: &str) -> usize {
    sp.plan.bodies.iter().position(|b| b.name == name).unwrap_or_else(|| panic!("no body {name}: {:?}", sp.plan.bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>()))
}

/// Member `m`'s turn about the world's y per turn of `root`, as the joints have it: through the
/// links where they reach it, else (a differential's side gears and pinions, whose links start
/// at the free side gear A) with the member it turns on: its carrier, or a mate's carrier on its
/// own axis (the engine's joint base), its own turn on it 0 while nothing drives it.
fn world_y(sp: &SystemPlan, root: usize, m: usize) -> f64 {
    let asm = &sp.plan.assembly;
    if let Some(r) = asm.ratio(root, m) {
        return r * asm.members[m].axis.dir[1];
    }
    let coaxial = |a: usize, b: usize| {
        let (x, y) = (&asm.members[a].axis, &asm.members[b].axis);
        crate::axes::len(crate::axes::cross(x.dir, y.dir)) < 1e-9 && x.distance(y.origin) < 1e-6
    };
    let base = asm.members[m].carrier.or_else(|| {
        asm.links.iter().find_map(|l| {
            let (a, b) = l.members();
            let other = if a == m {
                b
            } else if b == m {
                a
            } else {
                return None;
            };
            asm.members[other].carrier.filter(|c| *c != m && coaxial(*c, m))
        })
    });
    world_y(sp, root, base.unwrap_or_else(|| panic!("member {m} turns on nothing the drive reaches")))
}

fn rigid_pair(sp: &SystemPlan, p: usize, q: usize) -> bool {
    sp.plan.assembly.links.iter().any(|l| {
        let (x, y) = l.members();
        matches!(l, crate::plan::AssemblyLink::Rigid { .. }) && ((x, y) == (p, q) || (x, y) == (q, p))
    })
}

#[test]
fn a_hypoid_ring_carries_a_differential_that_turns_with_it_and_meshes() {
    for x in [80.0, -80.0] {
        let sp = planned(&[with_diff(hypoid(x))], &[]);
        let ring = sp.body(g(0, 0)).unwrap();
        let pinion = sp.body(g(0, 1)).unwrap();
        let case = body_named(&sp, "Diff H case");
        let (a, b) = (body_named(&sp, "Diff H side A + axle"), body_named(&sp, "Diff H side B + axle"));
        let p1 = body_named(&sp, "Diff H pinion 1");
        let asm = &sp.plan.assembly;
        // The case is bolted to the ring; the pinions turn on the case; the sides on the ring's axis.
        assert!(rigid_pair(&sp, ring, case));
        assert_eq!(asm.members[p1].carrier, Some(case));
        let ring_axis = &asm.members[ring].axis;
        for s in [case, a, b] {
            assert!(ring_axis.distance(asm.members[s].axis.origin) < 1e-6 && crate::axes::len(crate::axes::cross(ring_axis.dir, asm.members[s].axis.dir)) < 1e-9);
        }
        // Driven straight: the half-shafts turn with the case, which turns with the ring.
        let (wr, wc, wa, wb) = (world_y(&sp, pinion, ring), world_y(&sp, pinion, case), world_y(&sp, pinion, a), world_y(&sp, pinion, b));
        assert!(wr.abs() > 0.1 && (wc - wr).abs() < 1e-9 && (wa - wr).abs() < 1e-9 && (wb - wr).abs() < 1e-9, "ring {wr} case {wc} sides {wa} {wb}");
        // Open: with the case held, side B turns back as fast as side A turns on.
        let same = |k: usize| if crate::axes::dot(asm.members[k].axis.dir, ring_axis.dir) < 0.0 { -1.0 } else { 1.0 };
        let rel = asm.ratio(a, b).unwrap() * same(a) * same(b);
        assert!((rel + 1.0).abs() < 1e-9, "side B per side A, the case held: {rel}");
        // The side gear and the pinion mesh as the assembly turns them (the case held).
        let m = sp.plan.facts.iter().find(|f| f.label.ends_with("differential module")).map_or(0.5, |f| f.value);
        let ratio = spin_ratio(&sp, a, p1);
        let turn = 1.5 * std::f64::consts::TAU / 16.0;
        let mut pair = crate::mesh3d::Pair::for_run(&sp.plan.bodies[a], &sp.plan.bodies[p1], 0.05, 0.2, ratio, -turn, turn);
        let rest = pair.at(0.0, 0.0).penetration;
        let good = pair.run(ratio, 0.0, -turn, turn, 5).penetration;
        let bad = pair.run(-ratio, 0.0, -turn, turn, 5).penetration;
        assert!(rest <= 0.05 * m && good <= 0.05 * m && bad > 0.2 * m, "side A and pinion 1: at rest {rest}, turned {good}, the other way {bad}");
        // Each half-shaft reaches 1.5 ring diameters (75 mm) from the middle, along the axis.
        for s in [a, b] {
            let (lo, hi) = crate::mesh3d::Solid::new(&sp.plan.bodies[s], 1.0).bounds();
            assert!((lo[1].abs().max(hi[1].abs()) - 75.0).abs() < 1.0, "{lo:?} {hi:?}");
        }
        assert!(sp.notes.iter().any(|n| n.contains("Gear H carries an open differential")), "{:?}", sp.notes);
    }
}

#[test]
fn front_and_rear_differentials_turn_their_axles_the_same_way_about_y() {
    let mut prop = GearItem::new("P", ItemKind::Shaft).with(vec![Thing::line([-80.0, 0.0, 0.0], [80.0, 0.0, 0.0])]);
    prop.shaft_diameter = Some(6.0);
    let (mut front, mut rear) = (with_diff(hypoid(80.0)), with_diff(hypoid(-80.0)));
    (front.id, rear.id) = ("F".into(), "B".into());
    let items = vec![bevel(80.0), bevel(-80.0), front, rear, prop];
    let sh = |a: GearRef, b: GearRef| ItemLink { a, b, kind: LinkKind::Shaft };
    let links = vec![sh(g(4, 0), g(0, 0)), sh(g(4, 0), g(1, 0)), sh(g(0, 1), g(2, 1)), sh(g(1, 1), g(3, 1))];
    let sp = planned(&items, &links);
    let p = sp.body(g(4, 0)).unwrap();
    let mut turns = Vec::new();
    for id in ["F", "B"] {
        let ring = world_y(&sp, p, body_named(&sp, &format!("Diff {id} case")));
        let a = world_y(&sp, p, body_named(&sp, &format!("Diff {id} side A + axle")));
        let b = world_y(&sp, p, body_named(&sp, &format!("Diff {id} side B + axle")));
        assert!((a - ring).abs() < 1e-9 && (b - ring).abs() < 1e-9, "{id}: case {ring}, half-shafts {a} {b}");
        turns.push(a);
    }
    assert!(turns[0] * turns[1] > 0.0 && (turns[0].abs() - turns[1].abs()).abs() < 1e-9, "front axle {} and rear {} about +y per turn of the prop shaft", turns[0], turns[1]);
}

#[test]
fn a_shaft_drawn_on_the_ring_axis_is_the_half_shafts() {
    let mut axle = GearItem::new("S", ItemKind::Shaft).with(vec![Thing::line([125.0, -60.0, 20.0], [125.0, 50.0, 20.0])]);
    axle.shaft_diameter = Some(5.0);
    let sp = planned(&[with_diff(hypoid(80.0)), axle], &[]);
    let names: Vec<String> = sp.plan.bodies.iter().map(|b| b.name.clone()).collect();
    assert_eq!(names.iter().filter(|n| n.starts_with("Diff H")).count(), 5, "{names:?}");
    assert_eq!(names.len(), 7, "the ring, its pinion and the differential's five: {names:?}");
    let (a, b) = (body_named(&sp, "Diff H side A + axle"), body_named(&sp, "Diff H side B + axle"));
    let ys: Vec<(f64, f64)> = [a, b].iter().map(|k| crate::mesh3d::Solid::new(&sp.plan.bodies[*k], 1.0).bounds()).map(|(lo, hi)| (lo[1], hi[1])).collect();
    let (lo, hi) = (ys.iter().map(|y| y.0).fold(f64::MAX, f64::min), ys.iter().map(|y| y.1).fold(f64::MIN, f64::max));
    assert!((lo + 60.0).abs() < 0.5 && (hi - 50.0).abs() < 0.5, "the half-shafts run {lo} … {hi}");
    let d = sp.plan.facts.iter().find(|f| f.label.ends_with("half-shaft diameter")).map(|f| f.value);
    assert_eq!(d, Some(5.0));
    assert!(sp.notes.iter().any(|n| n.contains("the Shaft item on its axis is the half-shafts")), "{:?}", sp.notes);
}

#[test]
fn a_differential_needs_a_ring_gear() {
    let mut it = GearItem::new("A", ItemKind::Spur).with(vec![Thing::circle([0.0; 3], [0.0, 0.0, 1.0], 20.0)]);
    it.differential = Some(crate::items::DiffItem::default());
    let sp = plan_items(&[it], &[], &[], None, &Spec::default()).unwrap();
    assert!(sp.error.as_deref().is_some_and(|e| e.contains("rides on a ring gear")), "{:?}", sp.error);
}

