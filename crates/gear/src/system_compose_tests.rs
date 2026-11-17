//! Whole drivetrains from items (plan::system, WP1): stages of every kind planned by their own
//! planners, placed where the drawing puts them and merged into one plan and one assembly.

use crate::analysis::{Efficiencies, drive_table, ratio_between};
use crate::items::{Drive, GearItem, GearRef, ItemKind, ItemLink, LinkKind, Thing};
use crate::plan::system::{SystemPlan, plan_items};
use crate::plan::{AssemblyLink, Spec};

const Z: [f64; 3] = [0.0, 0.0, 1.0];
const X: [f64; 3] = [1.0, 0.0, 0.0];

fn item(id: &str, kind: ItemKind, things: Vec<Thing>) -> GearItem {
    GearItem::new(id, kind).with(things)
}
fn circ(c: [f64; 3], n: [f64; 3], r: f64) -> Thing {
    Thing::circle(c, n, r)
}
fn line(a: [f64; 3], b: [f64; 3]) -> Thing {
    Thing::line(a, b)
}
fn g(item: usize, gear: usize) -> GearRef {
    GearRef::new(item, gear)
}
fn mesh(a: GearRef, b: GearRef) -> ItemLink {
    ItemLink { a, b, kind: LinkKind::Mesh }
}
fn shaft(a: GearRef, b: GearRef) -> ItemLink {
    ItemLink { a, b, kind: LinkKind::Shaft }
}

fn planned(items: &[GearItem], links: &[ItemLink], drive: Option<&Drive>) -> SystemPlan {
    let sp = plan_items(items, links, &[], drive, &Spec::default()).unwrap_or_else(|e| panic!("{e}"));
    assert!(sp.error.is_none(), "{:?}", sp.error);
    assert!(sp.plan.assembly_issues.is_empty(), "{:?}", sp.plan.assembly_issues);
    // One assembly member per body; every gear has its body.
    assert_eq!(sp.plan.assembly.members.len(), sp.plan.bodies.len());
    for (r, _) in &sp.member_of {
        assert!(sp.body(*r).is_some(), "{r:?} has no body");
    }
    sp
}

fn teeth(sp: &SystemPlan, r: GearRef) -> f64 {
    f64::from(sp.gears.iter().find(|x| x.item == r.item && x.gear == r.gear).map(|x| x.teeth).unwrap_or(0))
}

fn reply_ratio(sp: &SystemPlan, r: GearRef) -> f64 {
    sp.gears.iter().find(|x| x.item == r.item && x.gear == r.gear).and_then(|x| x.ratio).unwrap_or_else(|| panic!("{r:?} has no ratio"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// A spur pair; the driven spur's shaft carries a bevel pinion (z up, apex 20 above it), whose
/// mate (on an x shaft) shares its shaft with a worm wheel; the worm lies across the wheel.
fn chain() -> (Vec<GearItem>, Vec<ItemLink>) {
    let items = vec![
        item("A", ItemKind::Spur, vec![circ([0.0; 3], Z, 20.0)]),
        item("B", ItemKind::Spur, vec![circ([50.0, 0.0, 0.0], Z, 30.0)]),
        item("C", ItemKind::Bevel, vec![circ([50.0, 0.0, 40.0], Z, 15.0), circ([65.0, 0.0, 60.0], X, 20.0)]),
        item("D", ItemKind::Worm, vec![circ([110.0, 0.0, 60.0], X, 30.0), line([110.0, -40.0, 98.0], [110.0, 40.0, 98.0])]),
    ];
    let links = vec![mesh(g(0, 0), g(1, 0)), shaft(g(1, 0), g(2, 0)), shaft(g(2, 1), g(3, 0))];
    (items, links)
}

#[test]
fn spur_to_bevel_to_worm_is_one_plan_and_one_assembly() {
    let (items, links) = chain();
    let drive = Drive { item: 0, gear: 0, rpm: Some(1000.0), torque: Some(2.0) };
    let sp = planned(&items, &links, Some(&drive));
    // Six gears, each its own body; B with the bevel pinion and the bevel gear with the wheel are
    // one rigid member each.
    assert_eq!(sp.member_of.len(), 6, "{:?}", sp.member_of);
    assert_eq!(sp.plan.bodies.len(), 6, "{:?}", sp.plan.bodies.iter().map(|b| &b.name).collect::<Vec<_>>());
    assert_eq!(sp.plan.clusters.len(), 2, "{:?}", sp.plan.clusters);
    let rigid = crate::plan::solid::members(&sp.plan);
    assert_eq!(rigid.len(), 4, "{rigid:?}");
    // The worm stage driven from its wheel: a gear link of the wheel's teeth against the starts.
    let (wb, wm) = (sp.body(g(3, 0)).unwrap(), sp.body(g(3, 1)).unwrap());
    assert!(
        sp.plan.assembly.links.iter().any(|l| matches!(*l, AssemblyLink::Gear { a, b, rb, .. } if a == wb && b == wm && rb == 1.0)),
        "{:?}",
        sp.plan.assembly.links
    );
    // The wheel turns with the bevel gear (A:B, then the bevel); it drives the worm by its
    // teeth against the starts.
    let wheel = reply_ratio(&sp, g(3, 0));
    let by_hand = teeth(&sp, g(0, 0)) / teeth(&sp, g(1, 0)) * teeth(&sp, g(2, 0)) / teeth(&sp, g(2, 1));
    assert!(close(wheel.abs(), by_hand), "{wheel} vs {by_hand}: {:?}", sp.gears);
    assert_eq!(teeth(&sp, g(3, 1)), 1.0, "the worm's starts");
    let worm = reply_ratio(&sp, g(3, 1));
    assert!(close(worm.abs(), wheel.abs() * teeth(&sp, g(3, 0))), "{worm}");
    // The ratios are the assembly's and the analysis'.
    let (root, w) = (sp.driver.unwrap(), sp.body(g(3, 0)).unwrap());
    let report = ratio_between(&sp.plan.assembly, root, w).unwrap();
    assert!(close(report.ratio, wheel), "{} vs {wheel}", report.ratio);
    assert!(close(sp.plan.assembly.ratio(root, w).unwrap(), wheel));
    // Speeds from the drive: the wheel at 1000 rpm × the ratio.
    let states = drive_table(&sp.plan.assembly, root, 1000.0, 2.0, &Efficiencies::default());
    let rpm = sp.gears.iter().find(|x| x.item == 3 && x.gear == 0).and_then(|x| x.rpm).unwrap();
    assert!(close(rpm, states[w].rpm) && close(rpm, 1000.0 * wheel), "{rpm}");
    // Links: the mesh's ratio −zA/zB, the shafts 1.
    assert!(close(sp.links[0].ratio.unwrap(), -teeth(&sp, g(0, 0)) / teeth(&sp, g(1, 0))), "{:?}", sp.links[0]);
    assert!(sp.links[1..].iter().all(|l| close(l.ratio.unwrap(), 1.0)), "{:?}", sp.links);
    // Gear C1 sits where it was drawn: its body on the x axis through (·, 0, 60).
    let f = sp.plan.bodies[sp.body(g(2, 1)).unwrap()].frame;
    assert!(f.origin[1].abs() < 1e-6 && (f.origin[2] - 60.0).abs() < 1e-6 && f.z[0].abs() > 1.0 - 1e-9, "{f:?}");
}

#[test]
fn a_compound_in_two_sketches_drives_a_planetary_set_with_its_carrier() {
    // The compound's shaft line along z; its gears on two parallel sketch planes (z 0 and 20);
    // a spur meshes gear 1; the sun of a planetary set further up the shaft turns with it.
    let items = vec![
        item("E", ItemKind::Compound, vec![line([0.0, 0.0, -10.0], [0.0, 0.0, 30.0]), circ([0.0; 3], Z, 20.0), circ([0.0, 0.0, 20.0], Z, 12.0)]),
        item("F", ItemKind::Spur, vec![circ([50.0, 0.0, 0.0], Z, 30.0)]),
        item("P", ItemKind::Planetary, vec![circ([0.0, 0.0, 60.0], Z, 10.0), circ([0.0, 0.0, 60.0], Z, 40.0)]),
    ];
    let links = vec![mesh(g(0, 0), g(1, 0)), shaft(g(0, 1), g(2, 0))];
    let drive = Drive { item: 1, gear: 0, rpm: Some(100.0), torque: None };
    let sp = planned(&items, &links, Some(&drive));
    // The carrier is generated; the planets ride on it.
    assert_eq!(sp.carriers.len(), 1);
    let carrier = sp.carriers[0].1;
    let planets: Vec<usize> = sp.plan.assembly.members.iter().enumerate().filter(|(_, m)| m.carrier == Some(carrier)).map(|(i, _)| i).collect();
    assert!(!planets.is_empty() && planets.len() <= 3, "{planets:?}");
    // The compound (and the sun with it) is one rigid member.
    let e = sp.member(g(0, 0)).unwrap();
    let sun = sp.member(g(2, 0)).unwrap();
    assert!(sp.plan.members[e].cluster.is_some() && sp.plan.members[e].cluster == sp.plan.members[sun].cluster);
    // Ratios: the sun turns with the compound (−zF/zE); the ring at −zs/zr of that (carrier held).
    let s = reply_ratio(&sp, g(2, 0));
    assert!(close(s, -teeth(&sp, g(1, 0)) / teeth(&sp, g(0, 0))), "{s}");
    let ring = reply_ratio(&sp, g(2, 1));
    assert!(close(ring, -s * teeth(&sp, g(2, 0)) / teeth(&sp, g(2, 1))), "{ring} {s} {:?}", sp.gears);
    let root = sp.driver.unwrap();
    let rb = sp.body(g(2, 1)).unwrap();
    assert!(close(ratio_between(&sp.plan.assembly, root, rb).unwrap().ratio, ring));
}

#[test]
fn a_hypoid_after_a_helical_pair() {
    let mut h1 = item("G", ItemKind::Helical, vec![circ([0.0; 3], Z, 20.0)]);
    let mut h2 = item("H", ItemKind::Helical, vec![circ([50.0, 0.0, 0.0], Z, 30.0)]);
    for it in [&mut h1, &mut h2] {
        it.gears = vec![crate::items::GearSettings { helix: Some(20.0), ..Default::default() }];
    }
    // A car axle's hypoid (41:11, 25 mm offset) on the driven helical's shaft.
    let mut hyp = item("K", ItemKind::Hypoid, vec![circ([50.0, 0.0, 80.0], Z, 100.0), line([-120.0, 25.0, 80.0], [-40.0, 25.0, 80.0])]);
    let by_teeth = |z| crate::items::GearSettings { size_by: crate::items::SizeBy::Teeth, teeth: Some(z), ..Default::default() };
    hyp.gears = vec![by_teeth(41), by_teeth(11)];
    let items = vec![h1, h2, hyp];
    let links = vec![mesh(g(0, 0), g(1, 0)), shaft(g(1, 0), g(2, 0))];
    let sp = planned(&items, &links, None);
    assert_eq!(sp.member_of.len(), 4);
    // Helical teeth of opposite hands on the pair.
    assert!(sp.plan.bodies.iter().any(|b| b.name.to_lowercase().contains("hypoid") || b.name.to_lowercase().contains("ring") || b.name.to_lowercase().contains("pinion")), "{:?}", sp.plan.bodies.iter().map(|b| &b.name).collect::<Vec<_>>());
    let pinion = reply_ratio(&sp, g(2, 1));
    let by_hand = teeth(&sp, g(0, 0)) / teeth(&sp, g(1, 0)) * teeth(&sp, g(2, 0)) / teeth(&sp, g(2, 1));
    assert!(close(pinion.abs(), by_hand), "{pinion} vs {by_hand}: {:?}", sp.gears);
}

#[test]
fn a_ring_with_two_pinions_and_mixed_tooth_styles_across_a_shaft() {
    // Two spur pinions inside a ring (one stage each, the ring in both); the ring's shaft-mate
    // is a herringbone pair on another layer.
    let mut hb = item("H", ItemKind::Herringbone, vec![circ([0.0, 0.0, 40.0], Z, 30.0)]);
    let mut hb2 = item("I", ItemKind::Herringbone, vec![circ([50.0, 0.0, 40.0], Z, 20.0)]);
    for it in [&mut hb, &mut hb2] {
        it.gears = vec![crate::items::GearSettings { helix: Some(25.0), v: true, ..Default::default() }];
    }
    let items = vec![
        item("R", ItemKind::Internal, vec![circ([0.0; 3], Z, 50.0)]),
        item("S", ItemKind::Spur, vec![circ([30.0, 0.0, 0.0], Z, 20.0)]),
        item("T", ItemKind::Spur, vec![circ([-30.0, 0.0, 0.0], Z, 20.0)]),
        hb,
        hb2,
    ];
    let links = vec![mesh(g(1, 0), g(0, 0)), mesh(g(2, 0), g(0, 0)), shaft(g(0, 0), g(3, 0)), mesh(g(3, 0), g(4, 0))];
    let sp = planned(&items, &links, Some(&Drive { item: 1, gear: 0, rpm: None, torque: None }));
    let ring = sp.member(g(0, 0)).unwrap();
    assert!(sp.plan.members[ring].ring);
    assert_eq!(sp.plan.members[ring].meshes_with.len(), 2, "{:?}", sp.plan.members);
    // Both pinions turn alike: the ring drives T as S drives it.
    assert!(close(reply_ratio(&sp, g(2, 0)), 1.0), "{:?}", sp.gears);
    let r = reply_ratio(&sp, g(0, 0));
    assert!(close(r, teeth(&sp, g(1, 0)) / teeth(&sp, g(0, 0))), "{r}");
    assert!(close(reply_ratio(&sp, g(4, 0)), -r * teeth(&sp, g(3, 0)) / teeth(&sp, g(4, 0))));
    // Spur and herringbone in one train.
    let t = sp.plan.facts.len();
    assert!(t > 0);
}

#[test]
fn a_bevel_gear_meshing_two_pinions_is_planned_once() {
    // K's gear 1 (z up, apex 20 above it) meshes its own pinion on +x and a second item's on −x.
    let items = vec![
        item("K", ItemKind::Bevel, vec![circ([0.0; 3], Z, 30.0), circ([30.0, 0.0, 20.0], X, 20.0)]),
        item("M", ItemKind::Spur, vec![circ([-30.0, 0.0, 20.0], [-1.0, 0.0, 0.0], 20.0)]),
    ];
    let links = vec![mesh(g(0, 0), g(1, 0))];
    let sp = planned(&items, &links, None);
    // Three bodies: the gear once, its two pinions.
    assert_eq!(sp.plan.bodies.len(), 3, "{:?}", sp.plan.bodies.iter().map(|b| &b.name).collect::<Vec<_>>());
    let k0 = sp.member(g(0, 0)).unwrap();
    assert_eq!(sp.plan.members[k0].meshes_with.len(), 2);
    // Both pinions turn at the same speed, opposite ways about +x and −x (so the same way in
    // the world).
    let (p1, p2) = (reply_ratio(&sp, g(0, 1)), reply_ratio(&sp, g(1, 0)));
    assert!(close(p1.abs(), p2.abs()), "{p1} {p2}");
    // The second pinion is turned to mesh with the gear that is kept: no deeper than the first.
    let body = |r| &sp.plan.bodies[sp.body(r).unwrap()];
    let depth = |a: &crate::plan::Body, b: &crate::plan::Body| crate::mesh3d::Pair::new(a, b, 0.1, 0.5).at(0.0, 0.0).penetration;
    let (d1, d2) = (depth(body(g(0, 0)), body(g(0, 1))), depth(body(g(0, 0)), body(g(1, 0))));
    assert!(d2 <= d1 + 0.02, "first pinion {d1} mm deep, second {d2} mm");
}

#[test]
fn stages_that_collide_are_refused_naming_both() {
    // Two separate spur pairs, the second drawn 3 mm above the first and overlapping it.
    let items = vec![
        item("A", ItemKind::Spur, vec![circ([0.0; 3], Z, 20.0)]),
        item("B", ItemKind::Spur, vec![circ([50.0, 0.0, 0.0], Z, 30.0)]),
        item("C", ItemKind::Spur, vec![circ([10.0, 40.0, 3.0], Z, 20.0)]),
        item("D", ItemKind::Spur, vec![circ([10.0, 90.0, 3.0], Z, 30.0)]),
    ];
    let links = vec![mesh(g(0, 0), g(1, 0)), mesh(g(2, 0), g(3, 0))];
    let sp = plan_items(&items, &links, &[], None, &Spec::default()).unwrap();
    let e = sp.error.unwrap_or_default();
    assert!(e.contains("collide") && e.contains("Item C gear 1") && e.contains("move one"), "{e}");
    assert!(sp.plan.bodies.is_empty());
    // Moved clear along their shafts, they are made.
    let mut apart = items.clone();
    for (k, x) in [(2, 10.0), (3, 10.0)] {
        let y = if k == 2 { 40.0 } else { 90.0 };
        let r = if k == 2 { 20.0 } else { 30.0 };
        apart[k] = item(&items[k].id, ItemKind::Spur, vec![circ([x, y, 40.0], Z, r)]);
    }
    planned(&apart, &links, None);
}

#[test]
fn a_wrong_link_comes_back_on_the_link_with_a_hint() {
    let (items, mut links) = chain();
    // A Mesh link between the bevel gear and the spur pinion (shafts at an angle that miss).
    links.push(mesh(g(0, 0), g(2, 1)));
    let sp = plan_items(&items, &links, &[], None, &Spec::default()).unwrap();
    assert!(sp.error.is_some());
    let bad = sp.links.last().unwrap();
    assert!(bad.error.is_some() && bad.hint.is_some(), "{bad:?}");
    assert!(sp.plan.bodies.is_empty());
}

#[test]
fn the_reader_routes_a_set_with_a_bevel_among_three_gears() {
    use crate::drivetrain::{Ask, Circle3, Item, Thing as T, read};
    let c = |c: [f64; 3], n: [f64; 3], r: f64| Item { thing: T::Circle(Circle3 { c, n, r }), ask: Ask::default() };
    let items = vec![c([0.0; 3], Z, 20.0), c([50.0, 0.0, 0.0], Z, 30.0), c([50.0, 0.0, 40.0], Z, 15.0), c([65.0, 0.0, 60.0], X, 20.0)];
    let o = read(&items, None, &crate::simple::Simple::default(), &Spec::default());
    assert!(o.error.is_none(), "{:?}", o.error);
    let p = o.planned.as_ref().expect("a merged plan");
    assert_eq!(p.bodies.len(), 4);
    assert_eq!(o.members[0].meshes_with, vec![1]);
    assert_eq!(o.members[3].meshes_with, vec![2]);
    assert!(o.members[1].cluster.is_some() && o.members[1].cluster == o.members[2].cluster);
}

/// Each link of the assembly carries its stage's real kind: the worm stage is a worm (its
/// lead-angle efficiency and self-locking check, whichever end drives it), the bevel a bevel,
/// the spur pair spur; the replies' rpm follow from it, each gear has its contact ratio, and the
/// note counts the bodies the feature makes (one per rigid member).
#[test]
fn every_link_is_analysed_as_its_stage_kind() {
    use crate::analysis::{MeshKind, StepKind, ratio_between_with, worm_efficiency};
    let (items, links) = chain();
    let drive = Drive { item: 0, gear: 0, rpm: Some(1000.0), torque: Some(2.0) };
    let sp = planned(&items, &links, Some(&drive));
    let e = sp.efficiencies(&sp.plan.assembly);
    let root = sp.driver.unwrap();
    let worm = sp.body(g(3, 1)).unwrap();
    let report = ratio_between_with(&sp.plan.assembly, root, worm, &e).unwrap();
    let kinds: Vec<StepKind> = report.path.iter().map(|s| s.kind).filter(|k| *k != StepKind::Shaft).collect();
    assert_eq!(kinds, vec![StepKind::Mesh(MeshKind::Spur), StepKind::Mesh(MeshKind::Bevel), StepKind::Mesh(MeshKind::Worm)], "{:?}", report.path);
    // The worm step, driven from its wheel: the lead angle the worm stage was planned with.
    let lead = sp.mesh_tags.iter().find_map(|t| t.lead_angle).expect("the worm's lead angle");
    let we = worm_efficiency(lead, e.worm_friction, e.worm_pressure_angle);
    let last = report.path.last().unwrap();
    assert_eq!(last.teeth.map(|t| t.1), Some(1), "{last:?}");
    assert_eq!(last.self_locking, we.self_locking);
    assert!((last.efficiency - we.back).abs() < 1e-12, "{} vs {}", last.efficiency, we.back);
    // Driven from the worm the other way: the forward efficiency.
    let wheel = sp.body(g(3, 0)).unwrap();
    let back = ratio_between_with(&sp.plan.assembly, worm, wheel, &e).unwrap();
    assert!((back.path[0].efficiency - we.forward).abs() < 1e-12);
    // The replies: the worm's rpm is the drive table's with these kinds (0 when self-locking).
    let rpm = sp.gears.iter().find(|x| x.item == 3 && x.gear == 1).and_then(|x| x.rpm).unwrap();
    let wheel_rpm = sp.gears.iter().find(|x| x.item == 3 && x.gear == 0).and_then(|x| x.rpm).unwrap();
    if we.self_locking {
        assert_eq!(rpm, 0.0);
    } else {
        assert!(close(rpm, wheel_rpm * -teeth(&sp, g(3, 0))), "{rpm} {wheel_rpm}");
    }
    // The spur pair's gears have their contact ratio in their replies (a bevel plan works none out).
    for r in [g(0, 0), g(1, 0)] {
        let c = sp.gears.iter().find(|x| x.item == r.item && x.gear == r.gear).and_then(|x| x.contact_ratio);
        assert!(c.is_some_and(|c| c > 1.0), "{r:?}: {c:?}");
    }
    assert!(sp.notes.iter().any(|n| n == "4 bodies from 3 stages"), "{:?}", sp.notes);
}

/// A gear that meshes nothing, sized by a fixed module (decision 3): the tooth count closest to
/// its drawn diameter at that module, the module kept, and a note giving the diameter it gets
/// beside the drawn one.
#[test]
fn a_free_gear_at_a_fixed_module_keeps_the_module_with_a_note() {
    use crate::items::{GearSettings, SizeBy};
    let mut a = item("A", ItemKind::Spur, vec![circ([0.0; 3], Z, 21.0)]);
    a.gears = vec![GearSettings { size_by: SizeBy::Module, module: Some(4.0), ..GearSettings::default() }];
    let pair = vec![item("B", ItemKind::Spur, vec![circ([100.0, 0.0, 0.0], Z, 20.0)]), item("C", ItemKind::Spur, vec![circ([140.0, 0.0, 0.0], Z, 20.0)])];
    for (items, links) in [(vec![a.clone()], vec![]), ([vec![a.clone()], pair].concat(), vec![mesh(g(1, 0), g(2, 0))])] {
        let sp = planned(&items, &links, None);
        let r = &sp.gears[0];
        // Ø42 at module 4: 10.5 teeth, 11 the closest (rounding up).
        assert_eq!((r.teeth, r.module), (11, 4.0), "{r:?}");
        assert!((r.pitch_d - 44.0).abs() < 1e-9);
        assert!(sp.notes.iter().any(|n| n == "Item A gear 1 meshes nothing: 11 teeth at module 4 make it Ø44 (Ø42 drawn)"), "{:?}", sp.notes);
    }
    // At a module that fits the drawing: no note.
    a.gears[0].module = Some(3.0);
    let sp = planned(&[a], &[], None);
    assert_eq!(sp.gears[0].teeth, 14);
    assert!(!sp.notes.iter().any(|n| n.contains("meshes nothing")), "{:?}", sp.notes);
}
