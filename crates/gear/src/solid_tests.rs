//! How a plan becomes solids ([`crate::plan::solid`]): rigid members, their assembly, the tries a
//! step gets.

use crate::V2;
use crate::plan::solid::{NUDGES, assembly, attempts, members, mirrored_half};
use crate::plan::{AssemblyLink, Kind, Op, Spec, Step};
use crate::simple::Simple;
use crate::sketch_rules::{Circle, Drawn, Reading, read, train_spec};

fn circle(x: f64, y: f64, r: f64) -> Circle {
    Circle { c: V2::new(x, y), r }
}

/// A compound train: 1 drives 2; 2 and 3 share a shaft; 3 drives 4.
fn compound() -> crate::plan::Plan {
    let d = Drawn { points: vec![], circles: vec![circle(0.0, 0.0, 20.0), circle(50.0, 0.0, 30.0), circle(50.0, 0.0, 15.0), circle(50.0, 35.0, 20.0)], lines: vec![] };
    let Reading::Train { circles: picked } = read(Some(Kind::Spur), &d).unwrap_or_else(|e| panic!("{e}")) else { panic!("not read as a train") };
    let cs: Vec<Circle> = picked.iter().map(|i| d.circles[*i]).collect();
    let settings = Simple { module: Some(2.0), ..Simple::default() };
    let (spec, _) = train_spec(&cs, Kind::Spur, &settings, &Spec::defaults_for(Kind::Spur)).unwrap_or_else(|e| panic!("{e}"));
    crate::plan(&spec).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_cluster_is_one_rigid_member_and_one_assembly_member() {
    let p = compound();
    let rigid = members(&p);
    // Three parts turn: gear 1, the cluster 2+3, gear 4.
    assert_eq!(rigid.len(), 3, "{rigid:?}");
    let every: Vec<usize> = rigid.iter().flat_map(|r| r.bodies.clone()).collect();
    let mut sorted = every.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), every.len(), "a body in two members");
    assert_eq!(sorted, (0..p.bodies.len()).collect::<Vec<_>>(), "every body in a member");
    let a = assembly(&p, &rigid);
    assert_eq!(a.members.len(), 3);
    assert!(a.members.iter().enumerate().all(|(k, m)| m.body == k), "members are the rigid members in order");
    assert!(!a.links.iter().any(|l| matches!(l, AssemblyLink::Rigid { .. })), "no rigid link inside one body: {:?}", a.links);
    // Gear 4 turns by the teeth: (-20/30) * (-15/20) of gear 1.
    let mut speed = [0.0; 3];
    speed[0] = 1.0;
    for l in &a.links {
        let (x, y) = l.members();
        speed[y] = speed[x] * l.ratio();
    }
    assert!((speed[2] - (20.0 / 30.0) * (15.0 / 20.0)).abs() < 1e-9, "{speed:?}");
}

#[test]
fn a_set_without_clusters_is_a_member_per_body() {
    let p = crate::plan(&Spec::defaults_for(Kind::Pair)).unwrap_or_else(|e| panic!("{e}"));
    let rigid = members(&p);
    assert_eq!(rigid.iter().map(|r| r.bodies.clone()).collect::<Vec<_>>(), (0..p.bodies.len()).map(|b| vec![b]).collect::<Vec<_>>());
    assert_eq!(assembly(&p, &rigid), p.assembly, "nothing to collapse");
}

#[test]
fn trims_are_nudged_and_short_lofts_thinned() {
    let blank = Step::Revolve { profile: vec![V2::new(0.0, 0.0), V2::new(1.0, 0.0), V2::new(1.0, 1.0)], op: Op::Intersect };
    assert_eq!(attempts(&blank).iter().map(|(_, n)| *n).collect::<Vec<_>>(), NUDGES.to_vec());
    let o = crate::geom::Outline::circle(V2::ZERO, 5.0);
    let loft = Step::Loft { sections: (0..5).map(|k| (o.clone(), f64::from(k))).collect(), op: Op::New };
    let tries: Vec<usize> = attempts(&loft).iter().map(|(s, _)| if let Step::Loft { sections, .. } = s { sections.len() } else { 0 }).collect();
    assert_eq!(tries, vec![5, 3, 2]);
}

#[test]
fn only_a_herringbones_second_half_is_a_mirror() {
    let p = crate::plan(&Spec::defaults_for(Kind::Herringbone)).unwrap_or_else(|e| panic!("{e}"));
    let b = &p.bodies[0];
    assert!(mirrored_half(b, 1, &b.steps[1]).is_some(), "{:?}", b.steps.iter().map(std::mem::discriminant).collect::<Vec<_>>());
    assert!(mirrored_half(b, 0, &b.steps[0]).is_none());
}

#[test]
fn holes_go_with_the_outline_round_them() {
    use crate::geom::{Outline, Region};
    use crate::plan::solid::region_parts;
    let r = Region {
        outer: vec![Outline::circle(V2::ZERO, 10.0), Outline::circle(V2::new(50.0, 0.0), 10.0)],
        holes: vec![Outline::circle(V2::new(50.0, 0.0), 3.0), Outline::circle(V2::ZERO, 3.0)],
    };
    assert_eq!(region_parts(&r), vec![(0, vec![1]), (1, vec![0])]);
}
