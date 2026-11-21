//! Blocks (crate::blocks): a block's settings reach every member gear that gives none of its own,
//! and a block's shaft Ø makes one shaft per axis, one body with the gears on it.

use crate::blocks::{block_settings, block_shafts, block_spec, chamfer_groups, expand_blocks, only_items, with_blocks};
use crate::items::{BlockSettings, Bore, FacePreset, FaceSides, GearItem, GearRef, GearSettings, ItemKind, ItemLink, ModuleBlock, Thing, gear_geoms};
use crate::plan::Spec;
use crate::plan::system::{SystemPlan, plan_items};
use crate::test_util::{close, g, mesh};

const Z: [f64; 3] = [0.0, 0.0, 1.0];

fn spur(id: &str, centre: [f64; 3], r: f64) -> GearItem {
    let mut it = GearItem::new(id, ItemKind::Spur).with(vec![Thing::circle(centre, Z, r)]);
    it.gears = vec![GearSettings::default()];
    it
}

/// A (Ø20 at the origin) meshes B (Ø30 at x 25); C (Ø16) sits on A's axis 12 mm up.
fn drawing() -> Vec<GearItem> {
    vec![spur("A", [0.0; 3], 10.0), spur("B", [25.0, 0.0, 0.0], 15.0), spur("C", [0.0, 0.0, 12.0], 8.0)]
}

fn block(members: Vec<usize>, settings: BlockSettings) -> ModuleBlock {
    ModuleBlock { id: "M1".into(), module: Some(1.0), members, settings }
}

fn planned(items: &[GearItem], links: &[ItemLink], blocks: &[ModuleBlock]) -> (SystemPlan, Vec<usize>) {
    let e = expand_blocks(items, links, blocks);
    let sp = plan_items(&e.items, &e.links, blocks, None, &Spec::default()).unwrap_or_else(|e| panic!("{e}"));
    assert!(sp.error.is_none(), "{:?}\nnotes {:?}", sp.error, sp.notes);
    (sp, e.added)
}

/// The bodies of the rigid member gear `r`'s body is in.
fn rigid_of(sp: &SystemPlan, r: GearRef) -> Vec<usize> {
    let b = sp.body(r).unwrap_or_else(|| panic!("{r:?} has no body"));
    crate::plan::solid::members(&sp.plan).into_iter().find(|m| m.bodies.contains(&b)).map(|m| m.bodies).unwrap_or_default()
}

#[test]
fn a_blocks_settings_fill_what_a_member_leaves_on_auto() {
    let b = BlockSettings { helix: Some(15.0), face_width: Some(12.0), shaft_diameter: Some(6.0), ..BlockSettings::default() };
    // All Auto: the block's helix, its face half each side, its shaft Ø as the bore.
    let s = block_settings(&b, ItemKind::Spur, &GearSettings::default());
    assert_eq!(s.helix, Some(15.0));
    assert_eq!((s.face.preset, s.face.a, s.face.b), (FacePreset::Symmetric, Some(6.0), None));
    assert_eq!(s.bore, Bore::Diameter(6.0));
    // A member's own values stay: its helix, its face, its bore.
    let own = GearSettings {
        helix: Some(0.0),
        face: FaceSides { preset: FacePreset::AOnly, a: Some(4.0), b: None },
        bore: Bore::Solid,
        ..GearSettings::default()
    };
    let s = block_settings(&b, ItemKind::Spur, &own);
    assert_eq!((s.helix, s.face, s.bore), (Some(0.0), own.face, Bore::Solid));
    // An A only face on Auto takes the whole width on side A.
    let a_only = GearSettings { face: FaceSides { preset: FacePreset::AOnly, a: None, b: None }, ..GearSettings::default() };
    assert_eq!(block_settings(&b, ItemKind::Spur, &a_only).face.a, Some(12.0));
    // A bevel takes no helix angle.
    assert_eq!(block_settings(&b, ItemKind::Bevel, &GearSettings::default()).helix, None);
    // Only the members get them, every gear of each; a member Shaft item its Ø.
    let mut items = drawing();
    items.push(GearItem::new("S", ItemKind::Shaft).with(vec![Thing::line([60.0, 0.0, 0.0], [60.0, 0.0, 30.0])]));
    let w = with_blocks(&items, &[block(vec![0, 3], b)]);
    assert_eq!(w[0].settings(0).helix, Some(15.0));
    assert_eq!(w[1], items[1], "not a member: as it was");
    assert_eq!(w[3].shaft_diameter, Some(6.0));
}

#[test]
fn a_module_only_block_changes_nothing_but_the_module() {
    let items = drawing();
    let blocks = [ModuleBlock { id: "M1".into(), module: Some(2.0), members: vec![0, 1], ..ModuleBlock::default() }];
    let e = expand_blocks(&items, &[mesh(g(0, 0), g(1, 0))], &blocks);
    assert_eq!(e.items, items);
    assert_eq!(e.links, vec![mesh(g(0, 0), g(1, 0))]);
    assert!(e.shafts.is_empty() && e.added.is_empty());
    assert!(chamfer_groups(3, &blocks, None).is_empty());
}

#[test]
fn a_blocks_pressure_angle_and_backlash_are_its_gears_spec() {
    let blocks = [block(vec![1], BlockSettings { pressure_angle: Some(25.0), backlash: Some(0.1), ..BlockSettings::default() })];
    let base = Spec::default();
    let s = block_spec(&blocks, &[g(0, 0), g(1, 0)], &base);
    assert!(close(s.pressure_angle, 25.0, 1e-12) && close(s.backlash, 0.1, 1e-12));
    let s = block_spec(&blocks, &[g(0, 0), g(2, 0)], &base);
    assert_eq!((s.pressure_angle, s.backlash), (base.pressure_angle, base.backlash), "no gear in the block: the set's");
    // Planned: the train of A and B takes the block's pressure angle.
    let (sp, _) = planned(&drawing()[..2], &[mesh(g(0, 0), g(1, 0))], &blocks);
    assert!(sp.error.is_none());
}

#[test]
fn a_blocks_tooth_chamfer_goes_on_its_members_and_the_sets_on_the_rest() {
    let blocks = [block(vec![0, 2], BlockSettings { tooth_chamfer: Some(0.3), ..BlockSettings::default() })];
    assert_eq!(chamfer_groups(4, &blocks, Some(0.5)), vec![(0.5, vec![1, 3]), (0.3, vec![0, 2])]);
    assert_eq!(chamfer_groups(4, &blocks, None), vec![(0.3, vec![0, 2])]);
    // The others are emptied (no gears) in their places.
    let only = only_items(&drawing(), &[0, 2]);
    assert_eq!(only.len(), 3);
    assert!(gear_geoms(&only[1]).is_empty() && gear_geoms(&only[2]).len() == 1);
}

#[test]
fn a_blocks_shaft_runs_along_each_axis_over_its_gears() {
    let blocks = [block(vec![0, 1, 2], BlockSettings { shaft_diameter: Some(6.0), ..BlockSettings::default() })];
    let shafts = block_shafts(&drawing(), &blocks);
    assert_eq!(shafts.len(), 2, "A and C on one axis, B on its own: {shafts:?}");
    let ac = shafts.iter().find(|s| s.gears.contains(&g(0, 0))).unwrap();
    assert_eq!(ac.gears, vec![g(0, 0), g(2, 0)]);
    // Module 1, Auto faces 5 mm each side: A from −5 to 5, C from 7 to 17; 6 mm past each end.
    assert!(close(ac.a[2], -11.0, 1e-9) && close(ac.b[2], 23.0, 1e-9), "{ac:?}");
    assert!(close(ac.length(), 34.0, 1e-9) && ac.diameter == 6.0 && ac.existing.is_none());
    let b = shafts.iter().find(|s| s.gears == vec![g(1, 0)]).unwrap();
    assert!(close(b.length(), 22.0, 1e-9) && close(b.a[0], 25.0, 1e-9), "{b:?}");
    // A bearing point on B's axis: its shaft reaches it.
    let mut items = drawing();
    items[1].things.push(Thing::point([25.0, 0.0, 30.0]));
    let b = block_shafts(&items, &blocks).into_iter().find(|s| s.gears == vec![g(1, 0)]).unwrap();
    assert!(close(b.b[2], 30.0, 1e-9) && close(b.a[2], -5.0, 1e-9), "from the gear's face to the bearing: {b:?}");
    // A Shaft item already on A's axis takes the Ø: no second shaft there.
    let mut items = drawing();
    items.push(GearItem::new("S", ItemKind::Shaft).with(vec![Thing::line([0.0, 0.0, -20.0], [0.0, 0.0, 30.0])]));
    let e = expand_blocks(&items, &[], &blocks);
    assert_eq!(e.added.len(), 1, "only B's axis gets a new shaft");
    assert_eq!(e.items[3].shaft_diameter, Some(6.0));
}

#[test]
fn a_blocks_shaft_and_the_gears_on_it_are_one_body_per_axis() {
    let blocks = [block(vec![0, 1, 2], BlockSettings { shaft_diameter: Some(6.0), ..BlockSettings::default() })];
    // With links of its own (A meshes B) and with none (inferred from the drawing).
    for links in [vec![mesh(g(0, 0), g(1, 0))], Vec::new()] {
        let (sp, added) = planned(&drawing(), &links, &blocks);
        assert_eq!(added, vec![3, 4]);
        assert_eq!(sp.plan.shafts.len(), 2, "{:?}", sp.plan.shafts);
        assert!(sp.plan.shafts.iter().all(|s| close(s.diameter, 6.0, 1e-9)), "{:?}", sp.plan.shafts);
        let (a, b) = (rigid_of(&sp, g(0, 0)), rigid_of(&sp, g(1, 0)));
        for r in [g(2, 0), g(3, 0)] {
            assert!(a.contains(&sp.body(r).unwrap()), "{r:?} with A: {a:?}");
        }
        assert!(b.contains(&sp.body(g(4, 0)).unwrap()), "B's shaft with B: {b:?}");
        assert!(!a.contains(&sp.body(g(1, 0)).unwrap()), "B turns apart from A");
        assert!(sp.plan.bodies[sp.body(g(3, 0)).unwrap()].name.starts_with("Shaft"), "{}", sp.plan.bodies[sp.body(g(3, 0)).unwrap()].name);
    }
}

/// A (Ø20) meshes B (Ø30, x 25); D sits on B's shaft 12 mm up and meshes C, on A's axis: C is
/// driven back round to A's axis at (10/15)·(rd/rc) of A's speed.
fn back_round(rc: f64) -> (Vec<GearItem>, Vec<ItemLink>) {
    let items = vec![spur("A", [0.0; 3], 10.0), spur("B", [25.0, 0.0, 0.0], 15.0), spur("C", [0.0, 0.0, 12.0], rc), spur("D", [25.0, 0.0, 12.0], 25.0 - rc)];
    (items, vec![mesh(g(0, 0), g(1, 0)), crate::test_util::shaft(g(1, 0), g(3, 0)), mesh(g(3, 0), g(2, 0))])
}

#[test]
fn a_gear_the_links_turn_at_another_speed_turns_free_on_its_blocks_shaft() {
    let blocks = [block(vec![0, 2], BlockSettings { shaft_diameter: Some(6.0), ..BlockSettings::default() })];
    // C at 4/9 of A's speed: A is on the shaft, C turns free on it in a clearance bore.
    let (items, links) = back_round(15.0);
    let e = expand_blocks(&items, &links, &blocks);
    assert_eq!(e.shafts.len(), 1, "{:?}", e.shafts);
    let s = &e.shafts[0];
    assert_eq!((s.gears.clone(), s.free.clone()), (vec![g(0, 0)], vec![g(2, 0)]), "{s:?}");
    assert!(close(s.length(), 34.0, 1e-9), "the shaft still runs under C: {s:?}");
    assert_eq!(e.items[2].settings(0).bore, Bore::Diameter(6.0 + crate::blocks::FREE_FIT));
    assert_eq!(e.items[0].settings(0).bore, Bore::Diameter(6.0));
    assert!(!e.links.iter().any(|l| l.a.item == 4 && l.b == g(2, 0)), "no shaft link to C: {:?}", e.links);
    let (sp, added) = planned(&items, &links, &blocks);
    let shaft = sp.body(g(added[0], 0)).unwrap();
    assert!(rigid_of(&sp, g(0, 0)).contains(&shaft), "A is on the shaft");
    assert!(!rigid_of(&sp, g(2, 0)).contains(&shaft), "C is not locked onto it");
    // C at A's speed (rd/rc = 15/10): one shaft for both, as before.
    let (items, links) = back_round(10.0);
    let e = expand_blocks(&items, &links, &blocks);
    assert_eq!((e.shafts[0].gears.clone(), e.shafts[0].free.clone()), (vec![g(0, 0), g(2, 0)], vec![]));
    assert_eq!(e.items[2].settings(0).bore, Bore::Diameter(6.0));
    let (sp, _) = planned(&items, &links, &blocks);
    assert!(rigid_of(&sp, g(0, 0)).contains(&sp.body(g(2, 0)).unwrap()), "A and C one body on the shaft");
    // Gears the links don't relate stay on the shaft (A and C with no link between them).
    let e = expand_blocks(&items, &[mesh(g(0, 0), g(1, 0))], &blocks);
    assert_eq!(e.shafts[0].gears, vec![g(0, 0), g(2, 0)]);
}
