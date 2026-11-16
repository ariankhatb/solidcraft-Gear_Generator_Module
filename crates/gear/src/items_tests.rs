//! Gear sets as items and links: meshes and shafts as linked, suggestions, module blocks,
//! per-slot errors, pairs, and each gear's ratio, speed and torque.

use crate::drivetrain::{Circle3, Line3, Thing};
use crate::items::{Block, BevelStyle, Drive, GearAsk, ItemIn, ItemType, LinkIn, LinkType, Port, report, resolve};
use crate::plan::{Kind, Spec};
use crate::simple::Simple;

const Z: [f64; 3] = [0.0, 0.0, 1.0];

fn circle(c: [f64; 3], r: f64) -> Thing {
    Thing::Circle(Circle3 { c, n: Z, r })
}

fn item(id: &str, kind: ItemType, components: Vec<(&str, Thing)>) -> ItemIn {
    ItemIn { id: id.into(), kind, components: components.into_iter().map(|(s, t)| (s.to_string(), t)).collect(), gears: Vec::new(), module_block: None }
}

fn port(item: &str, gear: usize) -> Port {
    Port { item: item.into(), gear }
}

fn mesh(a: &str, b: &str) -> LinkIn {
    LinkIn { a: port(a, 0), b: port(b, 0), kind: LinkType::Mesh }
}

fn go(items: &[ItemIn], links: &[LinkIn], blocks: &[Block]) -> crate::items::Resolved {
    resolve(items, links, blocks, None, &Simple::default(), &Spec::default(), false)
}

#[test]
fn two_linked_spurs_mesh_with_their_ratio_speed_and_torque() {
    let items = [item("A", ItemType::Spur, vec![("pitch", circle([0.0; 3], 20.0))]), item("B", ItemType::Spur, vec![("pitch", circle([50.0, 0.0, 0.0], 30.0))])];
    let mut r = go(&items, &[mesh("A", "B")], &[]);
    assert!(r.error.is_none(), "{:?}", r.error);
    let p = crate::plan(&r.solved.as_ref().unwrap().spec).unwrap();
    let l = &r.links[0];
    assert!(l.error.is_none() && (l.ratio.unwrap() - 1.5).abs() < 1e-12, "{l:?}");
    report(&mut r, &p, Some(&Drive { item: "A".into(), gear: 0, rpm: Some(1200.0), torque: Some(2.0) }));
    let b = &r.gears[1];
    assert!((b.ratio.unwrap() + 2.0 / 3.0).abs() < 1e-12 && (b.rpm.unwrap() + 800.0).abs() < 1e-9 && (b.torque.unwrap() - 3.0).abs() < 1e-9, "{b:?}");
    assert!(b.contact_ratio.unwrap() >= 1.2, "{b:?}");
    assert!(r.gears.iter().all(|g| g.teeth.unwrap() >= 17));
}

#[test]
fn tangent_but_unlinked_is_a_suggestion_and_linked_but_apart_an_error_with_a_hint() {
    let items = [item("A", ItemType::Spur, vec![("pitch", circle([0.0; 3], 20.0))]), item("B", ItemType::Spur, vec![("pitch", circle([50.0, 0.0, 0.0], 30.0))])];
    let r = go(&items, &[], &[]);
    assert!(r.suggestions.iter().any(|s| s.contains("item A gear 1 and item B gear 1 are tangent: link them?")), "{:?}", r.suggestions);
    let apart = [items[0].clone(), item("B", ItemType::Spur, vec![("pitch", circle([53.0, 0.0, 0.0], 30.0))])];
    let r = go(&apart, &[mesh("A", "B")], &[]);
    let l = &r.links[0];
    assert!(l.error.as_deref().unwrap_or_default().contains("don't touch: centres 53 mm apart"), "{l:?}");
    assert!(l.hint.as_deref().unwrap_or_default().contains("tangent"), "{l:?}");
    assert!(r.solved.is_none());
}

#[test]
fn a_compound_item_on_its_shaft_meshes_two_spurs() {
    let compound = item(
        "C",
        ItemType::Compound,
        vec![
            ("axis", Thing::Line(Line3 { a: [50.0, 0.0, -2.0], b: [50.0, 0.0, 30.0] })),
            ("pitch", circle([50.0, 0.0, 0.0], 30.0)),
            ("pitch2", circle([50.0, 0.0, 20.0], 15.0)),
        ],
    );
    let items = [item("A", ItemType::Spur, vec![("pitch", circle([0.0; 3], 20.0))]), compound, item("D", ItemType::Spur, vec![("pitch", circle([85.0, 0.0, 20.0], 20.0))])];
    let links = [mesh("A", "C"), LinkIn { a: port("C", 1), b: port("D", 0), kind: LinkType::Mesh }];
    let r = resolve(&items, &links, &[], None, &Simple { face_width: Some(8.0), ..Simple::default() }, &Spec::default(), true);
    assert!(r.error.is_none(), "{:?}", r.error);
    let p = crate::plan(&r.solved.as_ref().unwrap().spec).unwrap();
    assert_eq!(p.clusters.len(), 1);
    assert!(r.links.iter().all(|l| l.error.is_none() && l.ratio.is_some()), "{:?}", r.links);
    // The compound is one body with its shaft.
    assert_eq!(p.members[1].body, p.members[2].body);
}

#[test]
fn a_module_block_keeps_one_module_and_names_a_conflict() {
    let mut items = vec![item("A", ItemType::Spur, vec![("pitch", circle([0.0; 3], 20.0))]), item("B", ItemType::Spur, vec![("pitch", circle([50.0, 0.0, 0.0], 30.0))])];
    for it in &mut items {
        it.module_block = Some("M".into());
    }
    let r = go(&items, &[mesh("A", "B")], &[Block { id: "M".into(), module: Some(1.25) }]);
    assert!(r.error.is_none(), "{:?}", r.error);
    assert!(r.gears.iter().all(|g| g.module == Some(1.25)), "{:?}", r.gears);
    items[1].gears = vec![GearAsk { module: Some(2.0), ..GearAsk::default() }];
    let r = go(&items, &[mesh("A", "B")], &[Block { id: "M".into(), module: Some(1.25) }]);
    let e = r.block_errors[0].1.clone().unwrap();
    assert!(e.contains("module block M: item B gear 1 asks module 2"), "{e}");
}

#[test]
fn missing_components_are_named_per_slot() {
    let r = go(&[item("C", ItemType::Compound, vec![("pitch", circle([0.0; 3], 20.0)), ("pitch2", circle([0.0, 0.0, 15.0], 10.0))])], &[], &[]);
    assert!(r.item_errors[0].1.as_deref().unwrap_or_default().contains("item C (compound): pick a line for slot `axis`"), "{:?}", r.item_errors);
    let r = go(&[item("S", ItemType::Spur, vec![("pitch", Thing::Line(Line3 { a: [0.0; 3], b: [1.0, 0.0, 0.0] }))])], &[], &[]);
    assert!(r.item_errors[0].1.as_deref().unwrap_or_default().contains("slot `pitch` takes a circle"), "{:?}", r.item_errors);
}

#[test]
fn a_bevel_pair_item_makes_both_gears_and_refuses_links() {
    let bevel = ItemIn {
        gears: vec![GearAsk { bevel_style: Some(BevelStyle::Chevron), spiral_angle: Some(30.0), ..GearAsk::default() }],
        ..item("P", ItemType::Bevel, vec![("pitch", circle([0.0, 0.0, -30.0], 20.0)), ("pitch2", Thing::Circle(Circle3 { c: [20.0, 0.0, 0.0], n: [1.0, 0.0, 0.0], r: 30.0 }))])
    };
    let r = go(std::slice::from_ref(&bevel), &[], &[]);
    assert!(r.error.is_none(), "{:?}", r.error);
    let spec = &r.solved.as_ref().unwrap().spec;
    assert_eq!(spec.kind, Kind::SpiralBevel);
    assert!(spec.herringbone && (spec.spiral_angle - 30.0).abs() < 1e-12);
    let other = item("A", ItemType::Spur, vec![("pitch", circle([100.0, 0.0, 0.0], 20.0))]);
    let r = go(&[bevel, other], &[], &[]);
    assert!(r.item_errors[0].1.as_deref().unwrap_or_default().contains("a bevel pair is a set of its own"), "{:?}", r.item_errors);
}

#[test]
fn a_shaft_link_must_be_coaxial() {
    let items = [item("A", ItemType::Spur, vec![("pitch", circle([0.0; 3], 20.0))]), item("B", ItemType::Spur, vec![("pitch", circle([4.1, 0.0, 20.0], 10.0))])];
    let r = go(&items, &[LinkIn { a: port("A", 0), b: port("B", 0), kind: LinkType::Shaft }], &[]);
    assert!(r.links[0].error.as_deref().unwrap_or_default().contains("not on one axis (4.1 mm apart)"), "{:?}", r.links);
}
