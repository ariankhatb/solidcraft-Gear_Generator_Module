//! `gear.create` / `gear.update` as one Gears feature: no section sketches, recomputed from its
//! sketches in place with its bodies, components and joints kept, a housing hollowed by its
//! keep-out in the same feature, and folder sets of earlier versions still read and updated.

use serde_json::{Value, json};
use solidcraft_doc::FeatureKind;

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn no_errors(s: &Session) {
    for r in &s.model.results {
        assert!(r.error.is_none(), "{}: {:?}", r.name, r.error);
    }
}

fn gears_features(s: &Session) -> Vec<u64> {
    s.doc.features.iter().filter(|f| matches!(f.kind, FeatureKind::Gears { .. })).map(|f| f.id).collect()
}

fn sketches(s: &Session) -> usize {
    s.doc.features.iter().filter(|f| matches!(f.kind, FeatureKind::Sketch { .. })).count()
}

fn volume(s: &mut Session, body: &str) -> f64 {
    run(s, "inspect.center_of_mass", json!({"bodies": [body]}))["volume"].as_f64().unwrap()
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
