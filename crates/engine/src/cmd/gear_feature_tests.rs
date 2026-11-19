//! `gear.create` / `gear.update` as one Gears feature: no section sketches, recomputed from its
//! sketches in place with its bodies, components and joints kept, a housing hollowed by its
//! keep-out in the same feature, and folder sets of earlier versions still read and updated.

use serde_json::{Value, json};
use solidcraft_doc::FeatureKind;

use crate::Session;
use super::super::test_util::{circle, gears_features, line, no_errors, run, volume};

fn sketches(s: &Session) -> usize {
    s.doc.features.iter().filter(|f| matches!(f.kind, FeatureKind::Sketch { .. })).count()
}

fn one_feature(p: Value) -> (Session, Value) {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", p);
    no_errors(&s);
    assert_eq!(s.doc.features.len(), 1, "one feature and nothing else: {r}");
    assert_eq!(gears_features(&s), vec![r["feature"].as_u64().unwrap()]);
    assert_eq!(r["group"], r["feature"]);
    assert_eq!(sketches(&s), 0);
    for b in r["bodies"].as_array().unwrap() {
        assert!(volume(&mut s, b.as_str().unwrap()) > 100.0, "{b}");
    }
    let sets = crate::gear_sets(&s);
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].feature, r["feature"].as_u64());
    (s, r)
}

#[test]
fn a_helical_pair_is_one_feature_without_sketches() {
    let (_, r) = one_feature(json!({"type": "pair", "helix_angle": 20, "teeth": 18, "teeth2": 30}));
    assert_eq!(r["bodies"].as_array().map(Vec::len), Some(2), "{r}");
}

#[test]
fn a_bevel_pair_is_one_feature_without_sketches() {
    let (_, r) = one_feature(json!({"type": "bevel"}));
    assert_eq!(r["bodies"].as_array().map(Vec::len), Some(2), "{r}");
}

#[test]
fn an_update_changes_the_feature_in_place() {
    let (mut s, r) = one_feature(json!({"type": "pair", "teeth": 18, "teeth2": 30}));
    let id = r["feature"].as_u64().unwrap();
    let u = run(&mut s, "gear.update", json!({"group": id, "teeth2": 36}));
    no_errors(&s);
    assert_eq!(u["updated"], "in_place", "{u}");
    assert_eq!(gears_features(&s), vec![id]);
    assert_eq!(s.doc.features.len(), 1);
}

/// Two touching circles (module 2: radius = teeth), gear 1's a dimension to edit.
fn pair_sketch(s: &mut Session) -> u64 {
    let sk = run(s, "sketch.create", json!({"plane": "XY"}))["sketch"].as_u64().unwrap();
    run(s, "sketch.circle.center", json!({"center": [0.0, 0.0], "radius": 20.0, "id": "g1"}));
    run(s, "sketch.circle.center", json!({"center": [50.0, 0.0], "radius": 30.0, "id": "g2"}));
    run(s, "sketch.finish", json!({}));
    sk
}

#[test]
fn a_sketch_edit_recomputes_the_set_with_its_bodies_components_and_joints() {
    let mut s = Session::default();
    let sk = pair_sketch(&mut s);
    let r = run(&mut s, "gear.create", json!({"sketch": sk, "type": "spur", "module": 2, "face_width": 8, "assembly": true}));
    no_errors(&s);
    let id = r["feature"].as_u64().unwrap();
    assert_eq!(gears_features(&s), vec![id]);
    assert_eq!(sketches(&s), 1, "only the user's sketch");
    let components = |r: &Value| r["assembly"]["members"].as_array().unwrap().iter().map(|m| m["component"].as_u64().unwrap()).collect::<Vec<_>>();
    let joints = |r: &Value| r["assembly"]["joints"].clone();
    let bodies = r["bodies"].clone();
    // Gear 2 grows (and gear 1 moves away from it): the circle's radius and centre edited.
    grow_gear_2(&mut s, sk);
    s.refresh();
    no_errors(&s);
    // The feature recomputed by itself; following the sketch moves the joints onto the shafts.
    let followed = crate::cmd::gear_follow_sketches(&mut s);
    assert!(followed.iter().all(|f| f.get("error").is_none()), "{followed:?}");
    let u = crate::gear_sets(&s).into_iter().next().unwrap();
    assert_eq!(gears_features(&s), vec![id], "the same one feature");
    assert_eq!(json!(u.bodies), bodies, "the same bodies");
    let r2 = run(&mut s, "gear.update", json!({"group": u.group}));
    no_errors(&s);
    assert_eq!(components(&r2), components(&r), "the same components");
    assert_eq!(joints(&r2), joints(&r), "the same joints");
    assert_eq!(r2["updated"], "in_place");
}

#[test]
fn the_keep_out_hollows_a_housing_in_the_same_feature() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"corner": [-40, -40, -10], "length": 140, "width": 80, "height": 40, "body_name": "Case"}));
    let solid = volume(&mut s, "Case");
    let r = run(&mut s, "gear.create", json!({"type": "pair", "teeth": 18, "teeth2": 30, "clearance_body": {}, "keep_out_cuts": ["Case"]}));
    no_errors(&s);
    assert_eq!(gears_features(&s).len(), 1);
    assert_eq!(r["bodies"].as_array().map(Vec::len), Some(2), "two gears, no keep-out body: {r}");
    let hollow = volume(&mut s, "Case");
    assert!(hollow < solid - 1000.0, "the case is hollowed where the gears turn: {solid} -> {hollow}");
    assert_eq!(s.doc.features.len(), 2, "the box and one Gears feature");
}

#[test]
fn a_folder_set_still_opens_updates_and_becomes_one_feature() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "teeth": 18, "teeth2": 30, "build": "features"}));
    no_errors(&s);
    assert!(gears_features(&s).is_empty(), "a folder of ordinary features");
    assert!(s.doc.features.len() > 2);
    let group = r["group"].as_u64().unwrap();
    // Saved and opened again: the folder set is read as it was.
    let text = serde_json::to_string(&*s.doc).unwrap();
    let doc: solidcraft_doc::Document = serde_json::from_str(&text).unwrap();
    let mut s = Session::new(doc);
    no_errors(&s);
    let sets = crate::gear_sets(&s);
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].feature, None);
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth2": 32}));
    no_errors(&s);
    assert!(gears_features(&s).is_empty(), "an update leaves it a folder: {u}");
    // Asked to, it becomes one Gears feature under the same body names.
    let names = u["bodies"].clone();
    let c = run(&mut s, "gear.update", json!({"group": group, "build": "feature"}));
    no_errors(&s);
    assert_eq!(gears_features(&s).len(), 1, "{c}");
    assert_eq!(sketches(&s), 0);
    assert_eq!(c["bodies"], names);
}

/// A real drivetrain drawing (`GEAR_FEATURE_FILE`: a design whose sketches `GEAR_FEATURE_SKETCHES`
/// draw it, comma separated names): made as one feature with its assembly; prints what came out.
#[test]
#[ignore = "needs GEAR_FEATURE_FILE"]
fn a_drawn_drivetrain_is_one_feature() {
    let Ok(path) = std::env::var("GEAR_FEATURE_FILE") else { return };
    let names = std::env::var("GEAR_FEATURE_SKETCHES").unwrap_or_else(|_| "Sketch1,Sketch2,Sketch3,Sketch4,Sketch5".into());
    let mut s = Session::default();
    run(&mut s, "doc.open", json!({"path": path}));
    let features = s.doc.features.len();
    let drawn: Vec<&str> = names.split(',').collect();
    let t = std::time::Instant::now();
    let r = run(&mut s, "gear.create", json!({"sketch": drawn, "assembly": true}));
    let ms = t.elapsed().as_millis();
    no_errors(&s);
    let joints = s.doc.assembly.joints.len();
    let revolute = s.doc.assembly.joints.iter().filter(|j| format!("{:?}", j.kind).to_lowercase().contains("revolute")).count();
    println!(
        "GEARBOX: {} ms; features {} -> {} (Gears {}), sketches {}, bodies {:?}, components {}, joints {} (revolute {}), assembly {}",
        ms,
        features,
        s.doc.features.len(),
        gears_features(&s).len(),
        sketches(&s),
        r["bodies"],
        s.doc.components.len(),
        joints,
        revolute,
        r["assembly"]
    );
    assert_eq!(gears_features(&s).len(), 1);
}

// ---------------------------------------------------------------------------------------------
// A drivetrain of gear items (GEAR_UI_SPEC §3, §6): one feature, its assembly kept by item+gear
// ---------------------------------------------------------------------------------------------

/// A finished sketch on `plane` with `draw` in it.
fn sketch_on(s: &mut Session, plane: Value, draw: &[(&str, Value)]) -> u64 {
    let sk = run(s, "sketch.create", json!({"plane": plane, "project_edges": false}))["sketch"].as_u64().unwrap();
    for (id, p) in draw {
        run(s, id, p.clone());
    }
    run(s, "sketch.finish", json!({}));
    sk
}

fn plane(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Value {
    json!({"origin": origin, "x_dir": x, "y_dir": y})
}

fn pick(sketch: u64, curve: &str) -> Value {
    json!({"sketch": sketch, "curve": curve})
}

fn mesh(a: &str, ag: usize, b: &str, bg: usize) -> Value {
    json!({"a": {"item": a, "port": ag}, "b": {"item": b, "port": bg}, "kind": "mesh"})
}

fn shaft(a: &str, ag: usize, b: &str, bg: usize) -> Value {
    json!({"a": {"item": a, "gear": ag}, "b": {"item": b, "gear": bg}, "kind": "shaft"})
}

/// Six sketches: a spur pair A–B (z 0); a compound E (its shaft line in an XZ sketch, gear 1 on
/// z 0 meshing A, gear 2 on z −25); a bevel pair C on B's shaft (pinion on z 40, gear on the
/// plane x 65) and a worm and wheel D whose wheel shares the bevel gear's shaft (plane x 110).
struct Mixed {
    items: Value,
    links: Value,
    s2: u64,
}

fn mixed(s: &mut Session) -> Mixed {
    let s1 = sketch_on(s, json!("XY"), &[circle(0.0, 0.0, 20.0, "a"), circle(50.0, 0.0, 30.0, "b"), circle(-50.0, 0.0, 30.0, "e1")]);
    let s2 = sketch_on(s, plane([0.0, 0.0, -25.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), &[circle(-50.0, 0.0, 15.0, "e2")]);
    let s3 = sketch_on(s, plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), &[line([-50.0, -35.0], [-50.0, 10.0], "shaft_e")]);
    let s4 = sketch_on(s, plane([0.0, 0.0, 40.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), &[circle(50.0, 0.0, 15.0, "c1")]);
    let s5 = sketch_on(s, plane([65.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), &[circle(0.0, 60.0, 20.0, "c2")]);
    let s6 = sketch_on(s, plane([110.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), &[circle(0.0, 60.0, 30.0, "d1"), line([-40.0, 98.0], [40.0, 98.0], "worm")]);
    let items = json!([
        {"id": "A", "type": "spur", "components": [pick(s1, "a")], "gears": [{"gear": 0, "face": {"a": null, "b": null}, "profile_shift": "auto", "bore": "auto"}]},
        {"id": "B", "type": "spur", "components": [pick(s1, "b")]},
        {"id": "C", "type": "bevel", "components": [pick(s4, "c1"), pick(s5, "c2")], "bevel_style": "straight"},
        {"id": "D", "type": "worm", "components": [pick(s6, "d1"), pick(s6, "worm")], "starts": 1},
        {"id": "E", "type": "compound", "components": [pick(s3, "shaft_e"), pick(s1, "e1"), pick(s2, "e2")]},
    ]);
    let links = json!([mesh("A", 0, "B", 0), shaft("B", 0, "C", 0), shaft("C", 1, "D", 0), mesh("A", 0, "E", 0)]);
    Mixed { items, links, s2 }
}

fn components_of(r: &Value) -> Vec<(String, u64, Option<u64>)> {
    r["assembly"]["members"].as_array().unwrap().iter().map(|m| (m["body"].as_str().unwrap().to_string(), m["component"].as_u64().unwrap(), m["joint"].as_u64())).collect()
}

#[test]
fn an_items_request_reads_back_as_it_was_sent() {
    let p: crate::schema::gear::GearParams = serde_json::from_value(json!({
        "items": [{"id": "A", "type": "compound", "components": [{"sketch": 3, "curve": "l", "slot": "axis"}, {"sketch": 3, "curve": "c1", "slot": "pitch"}],
                   "gears": [{"gear": 0, "teeth": 18, "face": {"a": 4.0, "b": 0.0}, "helix_angle": 15.0, "hand": "left", "herringbone": true, "profile_shift": 0.2, "bore": null,
                              "hub": {"diameter": 20.0, "length_a": 5.0, "length_b": null}}],
                   "module_block": "M1", "starts": 2, "bevel_style": "spiral", "spiral_angle": 30.0}],
        "links": [{"a": {"item": "A", "port": 0}, "b": {"item": "A", "port": 1}, "kind": "shaft"}],
        "module_blocks": [{"id": "M1", "module": 1.5}],
        "drive": {"item": "A", "gear": 1, "rpm": 100.0, "torque": 2.0},
        "keep_out_gap": 0.5,
    }))
    .unwrap();
    let items = p.items.as_ref().unwrap();
    assert_eq!(items[0].gears[0].bore, Value::Null, "null bore: no hole, not Auto");
    assert_eq!(items[0].gears[0].teeth, Some(18));
    assert_eq!(p.links.as_ref().unwrap()[0].b.gear, 1, "`port` is the gear");
    let back: crate::schema::gear::GearParams = serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
    assert_eq!(back, p);
    assert!(serde_json::from_value::<crate::schema::gear::GearParams>(json!({"items": [{"type": "spur", "colour": 1}]})).is_err(), "unknown fields are refused");
}

#[test]
fn a_mixed_drivetrain_is_one_feature_with_components_joints_and_links_kept_by_item() {
    let mut s = Session::default();
    let m = mixed(&mut s);
    let calc = run(&mut s, "gear.calculate", json!({"items": m.items, "links": m.links, "drive": {"item": "A", "gear": 0, "rpm": 1000.0, "torque": 2.0}}));
    assert!(calc.get("error").is_none(), "{calc}");
    let gears = calc["gears"].as_array().unwrap().clone();
    assert_eq!(gears.len(), 8, "A, B, C×2, D×2, E×2: {calc}");
    assert!(gears.iter().all(|g| g["teeth"].as_u64().unwrap() > 0 && g["rpm"].is_number()), "{calc}");
    assert_eq!(calc["links"].as_array().map(Vec::len), Some(4), "{calc}");
    assert!(calc["links"].as_array().unwrap().iter().all(|l| l.get("error").is_none() && l["ratio"].is_number()), "{calc}");
    assert!(calc["outlines"].as_array().is_some_and(|o| !o.is_empty()), "an outline per gear for the preview");

    let r = run(&mut s, "gear.create", json!({"items": m.items, "links": m.links, "drive": {"item": "A", "gear": 0}, "assembly": true, "name": "Box"}));
    no_errors(&s);
    assert_eq!(gears_features(&s).len(), 1, "{r}");
    let bodies: Vec<String> = r["bodies"].as_array().unwrap().iter().map(|b| b.as_str().unwrap().to_string()).collect();
    // A; B with the bevel pinion (one shaft); the bevel gear with the wheel (one shaft); the
    // worm; E's two gears.
    assert_eq!(bodies.len(), 5, "{bodies:?}");
    assert!(bodies.contains(&"Gear A·1".to_string()) && bodies.iter().any(|b| b.contains("E·1")), "{bodies:?}");
    for b in &bodies {
        assert!(volume(&mut s, b) > 100.0, "{b}");
    }
    let members = components_of(&r);
    assert_eq!(members.len(), 5, "{r}");
    assert!(members.iter().filter(|m| m.2.is_some()).count() >= 4, "a joint per moving member: {r}");
    assert!(!r["assembly"]["links"].as_array().unwrap().is_empty(), "{r}");
    // The drive gear drives: its member is the assembly's driver.
    let a_member = r["gears"].as_array().unwrap().iter().find(|g| g["item"] == "A").and_then(|g| g["body"].as_str()).unwrap().to_string();
    let driver = r["assembly"]["driver"].as_u64().unwrap() as usize;
    assert_eq!(r["assembly"]["members"][driver - 1]["body"], json!(a_member), "{r}");
    let feature = r["feature"].as_u64().unwrap();
    let (params, sketch_count) = match s.doc.feature(feature).map(|f| &f.kind) {
        Some(FeatureKind::Gears { params, sketches, .. }) => (params.clone(), sketches.len()),
        _ => panic!("no Gears feature"),
    };
    assert_eq!(sketch_count, 6, "every sketch picked in is the feature's");
    assert_eq!(params["body_keys"].as_array().map(Vec::len), Some(5));
    let group = r["group"].as_u64().unwrap();

    // The ratio across the compound, and across the bevel stage.
    let t = |item: &str, gear: usize| gears.iter().find(|g| g["item"] == item && g["gear"] == gear).unwrap()["teeth"].as_f64().unwrap();
    let q = run(&mut s, "gear.ratio", json!({"a": {"item": "E", "gear": 1}, "b": {"item": "B", "gear": 0}}));
    let want = t("E", 0) / t("B", 0);
    assert!((q["ratio"].as_f64().unwrap().abs() - want).abs() < 1e-9, "{q} vs {want}");
    assert_eq!(q["direction"], "same", "two external meshes: {q}");
    assert_eq!(q["a"], "Gear E·1+2", "E's two gears are one body: {q}");
    // The path names both of the compound's gears: along its shaft from gear 2 to gear 1,
    // which meshes A.
    let p0 = &q["path"][0];
    assert_eq!((p0["kind"].as_str(), p0["from_gear"].clone(), p0["to_gear"].clone()), (Some("shaft"), json!({"item": "E", "gear": 1}), json!({"item": "E", "gear": 0})), "{q}");
    assert_eq!(q["path"][1]["from_gear"], json!({"item": "E", "gear": 0}), "{q}");
    // The worm stage is a worm (its lead-angle efficiency), not a crossed helical pair.
    let wq = run(&mut s, "gear.ratio", json!({"a": {"item": "A", "gear": 0}, "b": {"item": "D", "gear": 1}}));
    let last = wq["path"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["kind"], "worm", "{wq}");
    assert!(last["efficiency"].as_f64().unwrap() < 0.9 || last["self_locking"] == true, "{wq}");
    // gear.create's replies carry each meshing spur's contact ratio.
    let a_reply = r["gears"].as_array().unwrap().iter().find(|g| g["item"] == "A").unwrap();
    assert!(a_reply["contact_ratio"].as_f64().is_some_and(|c| c > 1.0), "{a_reply}");
    // Its sketch isn't dimensioned: the item says so (once, on its first gear).
    let loose = |g: &Value| g["warnings"].as_array().unwrap().iter().filter(|w| w.as_str().unwrap_or_default().contains("isn't fully defined")).count();
    assert_eq!(loose(a_reply), 1, "{a_reply}");
    let w = run(&mut s, "gear.ratio", json!({"group": group, "a": a_member, "b": {"item": "D", "gear": 0}}));
    let want = t("A", 0) / t("B", 0) * t("C", 0) / t("C", 1);
    assert!((w["ratio"].as_f64().unwrap().abs() - want).abs() < 1e-9, "{w} vs {want}");
    assert!(w["path"].as_array().unwrap().iter().any(|p| p["kind"] == "bevel" || p["kind"] == "spiral_bevel"), "{w}");
    assert!(w["efficiency"].as_f64().unwrap() < 1.0);

    // The compound's second gear grows: a sketch edit re-plans the feature, which keeps its
    // bodies, components and joints.
    if let Some(FeatureKind::Sketch { sketch, .. }) = s.doc_mut().feature_mut(m.s2).map(|f| &mut f.kind) {
        for c in &mut sketch.curves {
            if let solidcraft_sketch::CurveKind::Circle { r, .. } = &mut c.kind {
                *r = 17.0;
            }
        }
    }
    s.refresh();
    no_errors(&s);
    let followed = crate::cmd::gear_follow_sketches(&mut s);
    assert!(!followed.is_empty() && followed.iter().all(|f| f.get("error").is_none()), "{followed:?}");
    let u = run(&mut s, "gear.update", json!({"group": group}));
    no_errors(&s);
    assert_eq!(components_of(&u), members, "the same components and joints after a sketch edit");
    assert_eq!(gears_features(&s), vec![feature]);

    // A module block's module: the feature is updated in place.
    let mut blocked = m.items.clone();
    for i in [0, 1, 4] {
        blocked[i]["module_block"] = json!("M1");
    }
    let u = run(&mut s, "gear.update", json!({"group": group, "items": blocked, "module_blocks": [{"id": "M1", "module": 2.5}]}));
    no_errors(&s);
    assert_eq!(u["updated"], "in_place", "{u}");
    assert_eq!(components_of(&u), members, "the same components and joints after a module change");
    let a = u["gears"].as_array().unwrap().iter().find(|g| g["item"] == "A").unwrap().clone();
    assert_eq!(a["module"], json!(2.5), "{a}");
    assert_eq!(a["teeth"], json!(16), "Ø40 at module 2.5: {a}");

    // Another item first in the list (a spur meshing E's second gear, so every other item's
    // place changes) keeps every other member's component and joint.
    let s7 = sketch_on(&mut s, plane([0.0, 0.0, -25.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), &[circle(-50.0, -42.0, 25.0, "f")]);
    let mut more = blocked.clone();
    more.as_array_mut().unwrap().insert(0, json!({"id": "F", "type": "spur", "components": [pick(s7, "f")]}));
    let mut links = m.links.as_array().unwrap().clone();
    links.push(mesh("E", 1, "F", 0));
    let u2 = run(&mut s, "gear.update", json!({"group": group, "items": more, "links": links}));
    no_errors(&s);
    let after = components_of(&u2);
    assert_eq!(after.len(), 6, "{u2}");
    for m in &members {
        assert!(after.contains(m), "{m:?} kept: {after:?}");
    }
    // Taken away again: the others still keep theirs.
    let u3 = run(&mut s, "gear.update", json!({"group": group, "items": blocked, "links": m.links}));
    no_errors(&s);
    assert_eq!(components_of(&u3), members, "{u3}");
}

/// Two spur items on one sketch, linked, as an assembly driven from B.
fn spur_items(s: &mut Session) -> (u64, Value) {
    let sk = pair_sketch(s);
    let r = run(
        s,
        "gear.create",
        json!({"items": [{"id": "A", "type": "spur", "components": [pick(sk, "g1")]}, {"id": "B", "type": "spur", "components": [pick(sk, "g2")]}],
               "links": [mesh("A", 0, "B", 0)], "drive": {"item": "B", "gear": 0}, "assembly": true}),
    );
    no_errors(s);
    (sk, r)
}

fn joint_text(s: &Session, id: u64) -> String {
    serde_json::to_string(&s.doc.assembly.joints.iter().find(|j| j.id == id).unwrap().a).unwrap()
}

/// Gear 2 of [`pair_sketch`] grows, its centre moved out to x 56.
fn grow_gear_2(s: &mut Session, sk: u64) {
    if let Some(FeatureKind::Sketch { sketch, .. }) = s.doc_mut().feature_mut(sk).map(|f| &mut f.kind) {
        for c in &mut sketch.curves {
            if c.id == "g2"
                && let solidcraft_sketch::CurveKind::Circle { r, .. } = &mut c.kind
            {
                *r = 36.0;
            }
        }
        if let Some(p) = sketch.points.iter_mut().find(|p| (p.pos.x - 50.0).abs() < 1e-9) {
            p.pos.x = 56.0;
        }
    }
}

#[test]
fn finishing_a_sketch_edit_moves_the_items_joints() {
    let mut s = Session::default();
    let (sk, r) = spur_items(&mut s);
    let before = components_of(&r);
    let b = before.iter().find(|m| m.0 == "Gear B·1").cloned().unwrap_or_else(|| panic!("{r}"));
    let driver = r["assembly"]["driver"].as_u64().unwrap() as usize;
    assert_eq!(r["assembly"]["members"][driver - 1]["body"], "Gear B·1", "B drives: {r}");
    run(&mut s, "sketch.edit", json!({"sketch": sk}));
    grow_gear_2(&mut s, sk);
    let f = run(&mut s, "sketch.finish", json!({}));
    no_errors(&s);
    assert!(f["gears_followed"].as_array().is_some_and(|g| !g.is_empty() && g.iter().all(|x| x.get("error").is_none())), "{f}");
    assert!(joint_text(&s, b.2.unwrap()).contains("56"), "B's joint is on its new shaft: {}", joint_text(&s, b.2.unwrap()));
    let u = run(&mut s, "gear.update", json!({"group": r["group"]}));
    assert_eq!(components_of(&u), before);
}

#[test]
fn a_sketch_edited_while_rolled_back_is_followed_when_rolled_forward() {
    let mut s = Session::default();
    let (sk, r) = spur_items(&mut s);
    let b = components_of(&r).into_iter().find(|m| m.0 == "Gear B·1").unwrap();
    run(&mut s, "timeline.roll_to", json!({"position": 1}));
    grow_gear_2(&mut s, sk);
    s.refresh();
    let back = run(&mut s, "timeline.roll_to", json!({"position": "end"}));
    no_errors(&s);
    assert!(back["gears_followed"].as_array().is_some_and(|g| !g.is_empty()), "{back}");
    assert!(joint_text(&s, b.2.unwrap()).contains("56"), "{}", joint_text(&s, b.2.unwrap()));
}

/// A bevel pair from one 2D side view: the apex point, two shaft lines from it and the contact
/// line between them, no circles; and a sketch left free to move is named on the item.
#[test]
fn a_bevel_pair_from_a_side_view_sketch() {
    let mut s = Session::default();
    let a = (2.0f64).atan();
    let c = [50.0 * a.sin(), 50.0 * a.cos()];
    let sk = sketch_on(
        &mut s,
        plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        &[("sketch.point", json!({"point": [0, 0], "id": "apex"})), line([0.0, 0.0], [0.0, 60.0], "s1"), line([0.0, 0.0], [60.0, 0.0], "s2"), line([0.0, 0.0], c, "k")],
    );
    let items = json!([{"id": "C", "type": "bevel", "components": [{"sketch": sk, "point": "apex"}, pick(sk, "s1"), pick(sk, "s2"), pick(sk, "k")]}]);
    let r = run(&mut s, "gear.calculate", json!({"items": items}));
    assert!(r.get("error").is_none(), "{r}");
    let gears = r["gears"].as_array().unwrap();
    assert_eq!(gears.len(), 2, "{r}");
    let (z1, z2) = (gears[0]["teeth"].as_f64().unwrap(), gears[1]["teeth"].as_f64().unwrap());
    assert!((z1 / z2 - 2.0).abs() < 0.15, "about 2:1 as drawn: {r}");
    assert!(gears[0]["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap_or_default().contains("isn't fully defined")), "{r}");
    let made = run(&mut s, "gear.create", json!({"items": items}));
    no_errors(&s);
    assert_eq!(made["bodies"].as_array().map(Vec::len), Some(2), "{made}");
}

/// A set with its assembly sits in a timeline folder beside its Gears feature: `group` names the
/// set by either id (the timeline lists the feature), for gear.inspect and gear.update alike.
#[test]
fn group_names_a_set_by_its_folder_or_its_gears_feature() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "teeth": 18, "teeth2": 30, "assembly": true}));
    no_errors(&s);
    let feature = r["feature"].as_u64().unwrap();
    let folder = r["group"].as_u64().unwrap();
    assert_ne!(folder, feature, "the assembly's folder is not the feature: {r}");
    for id in [folder, feature] {
        let i = run(&mut s, "gear.inspect", json!({"group": id}));
        assert_eq!(i["sets"].as_array().map(Vec::len), Some(1), "{id}: {i}");
    }
    let u = run(&mut s, "gear.update", json!({"group": feature, "teeth2": 36}));
    no_errors(&s);
    assert!(u["updated"].is_string(), "{u}");
}
