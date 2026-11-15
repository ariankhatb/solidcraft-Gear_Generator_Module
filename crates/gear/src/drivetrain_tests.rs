//! Drivetrains read from 3D geometry: trains with clusters, long trains, bevels, bearings and
//! per-member errors; and the flat sketch of the user's build 6 report.

use crate::drivetrain::{Ask, Circle3, Item, Line3, Outcome, Role, Thing, read};
use crate::mesh2d::{Verdict, check_mesh};
use crate::plan::{AssemblyLink, Kind, Spec};
use crate::simple::Simple;
use crate::sketch_rules::{Circle, Drawn, Reading};
use crate::V2;

fn circle(c: [f64; 3], n: [f64; 3], r: f64) -> Item {
    Item { thing: Thing::Circle(Circle3 { c, n, r }), ask: Ask::default() }
}
fn line(a: [f64; 3], b: [f64; 3]) -> Item {
    Item { thing: Thing::Line(Line3 { a, b }), ask: Ask::default() }
}
fn point(p: [f64; 3]) -> Item {
    Item { thing: Thing::Point(p), ask: Ask::default() }
}
const Z: [f64; 3] = [0.0, 0.0, 1.0];

fn clean(p: &crate::Plan, count: usize) {
    let r = check_mesh(p);
    assert_eq!(r.len(), count);
    for m in &r {
        assert_eq!(m.verdict, Verdict::Ok, "{}", m.message);
    }
}

fn planned(o: &Outcome) -> crate::Plan {
    assert!(o.error.is_none(), "{:?}", o.error);
    crate::plan(&o.solved.as_ref().unwrap().spec).unwrap()
}

/// Shafts A, B, C along z; gear 1 on A meshes gear 2 on B; gear 3 on B (20 mm up) is joined to
/// gear 2 and meshes gear 4 on C; a bearing point on A, and a dot off every shaft.
fn compound() -> Vec<Item> {
    vec![
        line([0.0, 0.0, -10.0], [0.0, 0.0, 40.0]),
        line([50.0, 0.0, -10.0], [50.0, 0.0, 40.0]),
        line([85.0, 0.0, -10.0], [85.0, 0.0, 40.0]),
        circle([0.0, 0.0, 0.0], Z, 20.0),
        circle([50.0, 0.0, 0.0], Z, 30.0),
        circle([50.0, 0.0, 20.0], Z, 15.0),
        circle([85.0, 0.0, 20.0], Z, 20.0),
        point([0.0, 0.0, -5.0]),
        point([100.0, 100.0, 100.0]),
    ]
}

#[test]
fn a_3d_compound_two_circles_on_one_shaft_meshing_two_others() {
    let o = read(&compound(), None, &Simple { face_width: Some(8.0), ..Simple::default() }, &Spec::default());
    let p = planned(&o);
    assert_eq!(p.bodies.len(), 4);
    clean(&p, 2);
    // Members: the circles on their shafts; 5 and 6 one cluster; meshes 4–5 and 6–7.
    let m = &o.members;
    assert_eq!((m[3].shaft, m[4].shaft, m[5].shaft, m[6].shaft), (Some(0), Some(1), Some(1), Some(2)));
    assert!(m[4].cluster.is_some() && m[4].cluster == m[5].cluster && m[3].cluster.is_none());
    assert_eq!((m[3].meshes_with.clone(), m[5].meshes_with.clone()), (vec![4], vec![6]));
    assert_eq!((m[7].role, m[7].shaft, m[8].role), ("bearing", Some(0), "point"));
    assert_eq!(o.seats, vec![[0.0, 0.0, -5.0]]);
    assert!(m.iter().all(|m| m.error.is_none()));
    // The plan: one cluster of gears 2 and 3, held together rigidly; gear 4 20 mm up its shaft.
    assert_eq!(p.clusters, vec![vec![1, 2]]);
    assert!(p.assembly.links.iter().any(|l| matches!(l, AssemblyLink::Rigid { a: 1, b: 2 })));
    assert!((p.bodies[3].frame.origin[2] - 20.0).abs() < 1e-9 && (p.bodies[3].frame.origin[0] - 85.0).abs() < 1e-9, "{:?}", p.bodies[3].frame);
    let (z1, z2, z3, z4) = (m[3].teeth.unwrap(), m[4].teeth.unwrap(), m[5].teeth.unwrap(), m[6].teeth.unwrap());
    let ratio = p.facts.iter().find(|f| f.label == "train ratio gear 1 → gear 4").unwrap().value;
    assert!((ratio - f64::from(z2) / f64::from(z1) * f64::from(z4) / f64::from(z3)).abs() < 1e-12);
}

#[test]
fn per_member_module_and_teeth_are_kept_or_contradictions_named() {
    let mut items = compound();
    // Gear 3 asks module 1.25 (24 teeth on Ø30): its group (gears 3 and 4) follows.
    items[5].ask.module = Some(1.25);
    let o = read(&items, None, &Simple { face_width: Some(8.0), ..Simple::default() }, &Spec::default());
    planned(&o);
    assert_eq!((o.members[5].teeth, o.members[6].teeth, o.members[6].module), (Some(24), Some(32), Some(1.25)));
    // Gear 4 asks another module: both named.
    items[6].ask.module = Some(2.0);
    let o = read(&items, None, &Simple::default(), &Spec::default());
    let e = o.members[6].error.clone().unwrap();
    assert!(e.contains("member 7 asks module 2") && e.contains("member 6"), "{e}");
    assert!(o.solved.is_none() && o.error.is_some());
    // Teeth that don't fit the circle.
    let mut items = compound();
    items[3].ask = Ask { module: Some(2.5), teeth: Some(18), ..Ask::default() };
    let o = read(&items, None, &Simple::default(), &Spec::default());
    let e = o.members[3].error.clone().unwrap();
    assert!(e.contains("18 teeth at module 2.5 make Ø45") && e.contains("Ø40"), "{e}");
}

#[test]
fn a_ten_gear_train() {
    // Ten gears in a row along x, alternately Ø40 and Ø24, each touching the next.
    let radii: Vec<f64> = (0..10).map(|i| if i % 2 == 0 { 20.0 } else { 12.0 }).collect();
    let mut x = 0.0;
    let mut circles = Vec::new();
    for (i, r) in radii.iter().enumerate() {
        if i > 0 {
            x += radii[i - 1] + r;
        }
        circles.push(Circle { c: V2::new(x, 0.0), r: *r });
    }
    // Flat, through the sketch reader.
    let d = Drawn { points: vec![], circles: circles.clone(), lines: vec![] };
    let Ok(Reading::Train { circles: picked }) = crate::sketch_rules::read(Some(Kind::Spur), &d) else { panic!() };
    assert_eq!(picked.len(), 10);
    let (spec, _) = crate::sketch_rules::train_spec(&circles, Kind::Spur, &Simple::default(), &Spec::default()).unwrap();
    let p = crate::plan(&spec).unwrap();
    assert_eq!(p.bodies.len(), 10);
    clean(&p, 9);
    // Gear 10 turns -z1/z10 per turn of gear 1 (nine outside meshes).
    let t = spec.train.as_ref().unwrap();
    let want = -f64::from(t.gears[0].teeth) / f64::from(t.gears[9].teeth);
    assert!((p.preview[9].speed - want).abs() < 1e-12, "{} vs {want}", p.preview[9].speed);
    assert!((p.assembly.ratio(0, 9).unwrap() - want).abs() < 1e-12);
    // The same in 3D, on a plane tilted to the world.
    let tilt = [0.0, -0.6, 0.8];
    let x_dir = [1.0, 0.0, 0.0];
    let y_dir = [0.0, 0.8, 0.6];
    let items: Vec<Item> = circles.iter().map(|c| circle([c.c.x * x_dir[0] + c.c.y * y_dir[0], c.c.x * x_dir[1] + c.c.y * y_dir[1], c.c.x * x_dir[2] + c.c.y * y_dir[2]], tilt, c.r)).collect();
    let o = read(&items, None, &Simple::default(), &Spec::default());
    let p = planned(&o);
    clean(&p, 9);
    assert!(o.members.iter().all(|m| m.error.is_none() && m.teeth.is_some()));
    assert_eq!(o.members[4].meshes_with, vec![3, 5]);
}

#[test]
fn shafts_meeting_make_a_bevel_pair_and_the_circles_must_touch() {
    // Gear 1 on the z axis 30 below the apex, gear 2 on the x axis 20 out: they touch at
    // (20, 0, -30).
    let items = vec![line([0.0; 3], [0.0, 0.0, -60.0]), line([0.0; 3], [60.0, 0.0, 0.0]), circle([0.0, 0.0, -30.0], Z, 20.0), circle([20.0, 0.0, 0.0], [1.0, 0.0, 0.0], 30.0)];
    let o = read(&items, None, &Simple::default(), &Spec::default());
    let p = planned(&o);
    let spec = &o.solved.as_ref().unwrap().spec;
    assert_eq!(spec.kind, Kind::Bevel);
    assert!((spec.shaft_angle - 90.0).abs() < 1e-9);
    assert!((f64::from(spec.teeth2) / f64::from(spec.teeth) - 1.5).abs() < 1e-12);
    assert_eq!(p.bodies.len(), 2);
    assert_eq!((o.members[2].meshes_with.clone(), o.members[2].shaft), (vec![3], Some(0)));
    // Helical teeth asked: spiral bevels.
    let o = read(&items, Some(Kind::Helical), &Simple::default(), &Spec::default());
    assert_eq!(o.solved.as_ref().unwrap().spec.kind, Kind::SpiralBevel);
    // Circles that don't meet.
    let mut apart = items.clone();
    apart[3] = circle([20.0, 0.0, 0.0], [1.0, 0.0, 0.0], 25.0);
    let o = read(&apart, None, &Simple::default(), &Spec::default());
    assert!(o.members[3].error.as_deref().unwrap_or_default().contains("don't touch"), "{:?}", o.members);
}

#[test]
fn per_member_errors_name_the_member() {
    // A circle moved off its place (touching nothing now); a bearing point off every line; a
    // style mix.
    let mut items = compound();
    items[6] = circle([85.0, 5.0, 20.0], Z, 20.0);
    items[8].ask.role = Some(Role::Bearing);
    let o = read(&items, None, &Simple::default(), &Spec::default());
    assert!(o.members[6].error.as_deref().unwrap_or_default().contains("member 7 meshes with nothing"), "{:?}", o.members[6]);
    assert_eq!(o.members[6].shaft, None);
    assert!(o.members[8].error.as_deref().unwrap_or_default().contains("member 9: a bearing must lie on a shaft line"), "{:?}", o.members[8]);
    assert!(o.members[3].error.is_none() && o.solved.is_none());
    let e = o.error.unwrap();
    assert!(e.contains("member 7") && e.contains("member 9"), "{e}");
    let mut items = compound();
    items[4].ask.role = Some(Role::Gear(Kind::Helical));
    let o = read(&items, None, &Simple { face_width: Some(8.0), ..Simple::default() }, &Spec::default());
    assert!(o.members.iter().any(|m| m.error.as_deref().unwrap_or_default().contains("share one tooth style")), "{:?}", o.members);
}

#[test]
fn a_gear_on_a_selected_shaft_line_with_its_size() {
    // Two parallel shaft lines, one asked for a Ø40 gear, the other a circle Ø60 touching it.
    let mut items = vec![line([0.0; 3], [0.0, 0.0, 30.0]), circle([50.0, 0.0, 5.0], Z, 30.0), line([50.0, 0.0, 0.0], [50.0, 0.0, 30.0])];
    items[0].ask = Ask { pitch_diameter: Some(40.0), along: Some(5.0), ..Ask::default() };
    let o = read(&items, None, &Simple::default(), &Spec::default());
    let p = planned(&o);
    clean(&p, 1);
    assert_eq!((o.members[0].role, o.members[0].shaft, o.members[1].shaft), ("spur", Some(0), Some(2)));
    assert!((p.bodies[0].frame.origin[2] - 5.0).abs() < 1e-9);
}

/// The user's build 6 sketch: three circles on XZ, the first two touching, the third touching
/// nothing (its constraint missing); construction lines join the centres (the engine leaves
/// construction geometry out). Gear 3 is named, gears 1 and 2 are sized.
#[test]
fn build_6_sketch_names_the_circle_that_meshes_with_nothing() {
    let circles = vec![
        Circle { c: V2::new(0.0, 0.0), r: 15.0 },
        Circle { c: V2::new(-10.180755625393433, 20.064949911424925), r: 7.5 },
        Circle { c: V2::new(12.062125004376336, 21.214432234647948), r: 7.5 },
    ];
    let d = Drawn { points: vec![V2::new(-6.787170416928201, 13.376633274281524), V2::new(0.0, 21.214432234647948)], circles: circles.clone(), lines: vec![] };
    let Ok(Reading::Train { .. }) = crate::sketch_rules::read(Some(Kind::Spur), &d) else { panic!("not a train") };
    let e = crate::sketch_rules::train_spec(&circles, Kind::Spur, &Simple::default(), &Spec::default()).unwrap_err();
    assert!(e.starts_with("gear 3 meshes with nothing") && !e.contains("gear 1:") && !e.contains("gear 2:"), "{e}");
    // Touching, the three make a train.
    let fixed = vec![circles[0], circles[1], Circle { c: V2::new(0.0, 22.5).rot(-0.5), r: 7.5 }];
    let (spec, _) = crate::sketch_rules::train_spec(&fixed, Kind::Spur, &Simple::default(), &Spec::default()).unwrap();
    clean(&crate::plan(&spec).unwrap(), 2);
}

/// A sketch's flat geometry in model space: (x, y) on the plane through `origin` with axes
/// `x` and `y` (its normal x × y).
fn on_plane(origin: [f64; 3], x: [f64; 3], y: [f64; 3], p: (f64, f64)) -> [f64; 3] {
    [0, 1, 2].map(|i| origin[i] + p.0 * x[i] + p.1 * y[i])
}

/// One gear system in three ordinary sketches: gears 1 and 2 in a sketch on z = 0, gears 3 and
/// 4 in a sketch on z = 20 (gear 3 on gear 2's axis), and the shared shaft line in a third
/// sketch on the XZ plane. Read together: a compound of gears 2 and 3 on that shaft.
#[test]
fn a_compound_from_two_sketches_on_parallel_planes_and_a_shaft_line_in_a_third() {
    let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let a = |p| on_plane([0.0; 3], x, y, p);
    let b = |p| on_plane([0.0, 0.0, 20.0], x, y, p);
    // The third sketch is on XZ: its x along world X, its y along world -Z... any plane holding
    // the line; here (u, v) -> (u, 0, v).
    let c = |p: (f64, f64)| [p.0, 0.0, p.1];
    let items = vec![
        circle(a((0.0, 0.0)), Z, 20.0),
        circle(a((50.0, 0.0)), Z, 30.0),
        circle(b((50.0, 0.0)), Z, 15.0),
        circle(b((85.0, 0.0)), Z, 20.0),
        line(c((50.0, -10.0)), c((50.0, 40.0))),
        point(c((50.0, 30.0))),
    ];
    let o = read(&items, None, &Simple { face_width: Some(8.0), ..Simple::default() }, &Spec::default());
    let p = planned(&o);
    clean(&p, 2);
    let m = &o.members;
    assert!(m[1].cluster.is_some() && m[1].cluster == m[2].cluster && m[0].cluster.is_none() && m[3].cluster.is_none());
    assert_eq!((m[1].shaft, m[2].shaft, m[0].shaft), (Some(4), Some(4), None));
    assert_eq!((m[5].role, m[5].shaft), ("bearing", Some(4)));
    assert_eq!(p.clusters, vec![vec![1, 2]]);
    assert!(p.assembly.links.iter().any(|l| matches!(l, AssemblyLink::Rigid { a: 1, b: 2 })));
    // Without the shaft line, the shared axis alone makes the cluster.
    let o = read(&items[..4], None, &Simple { face_width: Some(8.0), ..Simple::default() }, &Spec::default());
    planned(&o);
    assert!(o.members[1].cluster.is_some() && o.members[1].cluster == o.members[2].cluster);
}
