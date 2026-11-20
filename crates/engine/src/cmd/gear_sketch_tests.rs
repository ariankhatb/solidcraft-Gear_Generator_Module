//! Gear sets drawn in a native sketch (`gear.sketch`) and following it: a dimension changed in
//! the sketch regenerates the set in place, in that change's undo step.

use serde_json::{Value, json};

use crate::Session;
use super::test_util::run;

fn set(s: &mut Session, group: u64) -> Value {
    let r = run(s, "gear.inspect", json!({"group": group}));
    r["sets"][0].clone()
}

fn num(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or_else(|| panic!("no {key} in {v}"))
}

/// A pair drawn as a native sketch, made from it, then a pitch diameter changed as a parameter.
fn pair(s: &mut Session) -> (u64, Value) {
    let made = run(s, "gear.sketch", json!({"type": "pair", "gear": {"module": 2, "teeth": 20, "teeth2": 40}}));
    let sk = made["sketch"].as_u64().expect("a sketch");
    assert!(made["dimensions"]["diameter1"].is_string() && made["dimensions"]["diameter2"].is_string(), "{made}");
    let r = run(s, "gear.create", json!({"type": "pair", "module": 2, "sketch": sk, "picks": made["picks"]}));
    let group = r["group"].as_u64().expect("a group");
    (group, made)
}

#[test]
fn gear_sketch_is_a_constrained_native_sketch_the_set_is_read_from() {
    let mut s = Session::default();
    let (group, made) = pair(&mut s);
    let sk = made["sketch"].as_u64().unwrap();
    let st = set(&mut s, group);
    assert_eq!(st["sketch"].as_u64(), Some(sk), "{st}");
    assert_eq!(st["sketch_changed"], false, "{st}");
    assert_eq!(num(&st["params"], "teeth"), 20.0, "{st}");
    assert_eq!(num(&st["params"], "teeth2"), 40.0, "{st}");
    // The native constraints: the circles stay tangent when one grows.
    assert!(!made["constraints"].as_array().unwrap().is_empty(), "{made}");
}

#[test]
fn changing_a_native_diameter_regenerates_the_set_in_place_in_one_undo_step() {
    let mut s = Session::default();
    let (group, made) = pair(&mut s);
    let before = set(&mut s, group);
    let d1 = made["dimensions"]["diameter1"].as_str().unwrap().to_string();
    let r = run(&mut s, "parameters.change", json!({"name": d1, "expression": "60"}));
    let followed = r["gears_followed"].as_array().unwrap_or_else(|| panic!("the set did not follow: {r}"));
    assert_eq!(followed[0]["group"].as_u64(), Some(group), "{r}");
    assert_eq!(followed[0]["updated"], "in_place", "{r}");
    let after = set(&mut s, group);
    assert_eq!(num(&after["params"], "teeth"), 30.0, "Ø60 at module 2: {after}");
    assert_eq!(num(&after["params"], "teeth2"), 40.0, "{after}");
    assert_eq!(after["features"], before["features"], "regenerated in place: the same features");
    assert_eq!(after["sketch_changed"], false, "{after}");
    // One step: Undo takes the diameter and the gears back together.
    run(&mut s, "edit.undo", json!({}));
    let undone = set(&mut s, group);
    assert_eq!(num(&undone["params"], "teeth"), 20.0, "{undone}");
    assert_eq!(undone["sketch_changed"], false, "{undone}");
}

#[test]
fn a_bevel_follows_its_native_shaft_angle() {
    let mut s = Session::default();
    let made = run(&mut s, "gear.sketch", json!({"type": "bevel", "gear": {"module": 2, "teeth": 20, "teeth2": 30, "shaft_angle": 90}}));
    let sk = made["sketch"].as_u64().unwrap();
    let angle = made["dimensions"]["shaft_angle"].as_str().unwrap_or_else(|| panic!("no shaft angle dimension: {made}")).to_string();
    let r = run(&mut s, "gear.create", json!({"type": "bevel", "module": 2, "teeth": 20, "teeth2": 30, "sketch": sk, "picks": made["picks"]}));
    let group = r["group"].as_u64().unwrap();
    assert!((num(&set(&mut s, group)["params"], "shaft_angle") - 90.0).abs() < 1e-6);
    let r = run(&mut s, "parameters.change", json!({"name": angle, "expression": "80 deg"}));
    assert!(r["gears_followed"].is_array(), "{r}");
    let st = set(&mut s, group);
    assert!((num(&st["params"], "shaft_angle") - 80.0).abs() < 1e-6, "{st}");
}

/// Edited in the sketch itself (Edit Sketch, a dimension changed, Finish): the set follows on
/// Finish, not while the sketch is open.
#[test]
fn editing_the_sketch_natively_updates_the_set_on_finish() {
    let mut s = Session::default();
    let (group, made) = pair(&mut s);
    let sk = made["sketch"].as_u64().unwrap();
    let d2 = made["dimensions"]["diameter2"].as_str().unwrap().to_string();
    run(&mut s, "sketch.edit", json!({"sketch": sk}));
    run(&mut s, "parameters.change", json!({"name": d2, "expression": "100"}));
    let r = run(&mut s, "sketch.finish", json!({}));
    assert!(r["gears_followed"].is_array(), "{r}");
    let st = set(&mut s, group);
    assert_eq!(num(&st["params"], "teeth2"), 50.0, "Ø100 at module 2: {st}");
}

/// `sketch: "new"`: the drawing and the set in one step, the set linked to the drawing.
#[test]
fn a_set_made_with_a_new_sketch_is_linked_to_it() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "module": 2, "teeth": 18, "teeth2": 36, "sketch": "new"}));
    let group = r["group"].as_u64().unwrap();
    let st = set(&mut s, group);
    assert!(st["sketch"].as_u64().is_some(), "{st}");
    assert_eq!(num(&st["params"], "teeth"), 18.0, "{st}");
    // One step for both.
    run(&mut s, "edit.undo", json!({}));
    assert!(run(&mut s, "gear.inspect", json!({}))["sets"].as_array().unwrap().is_empty());
}

/// Why gear.calculate can't make the set: its refusal, or (a drawing it reads member by member) what
/// it says is wrong, in its warnings.
fn refusal(s: &mut Session, p: Value) -> String {
    match s.execute("gear.calculate", &p) {
        Err(e) => e.to_string(),
        Ok(r) => r["warnings"].to_string(),
    }
}

/// The user's own sketch: two tangent circles, a third equal one touching nothing, and
/// construction lines joining the centres. Construction lines are helpers, never shafts; the
/// lone circle is named in the refusal; tangent, the three make a train.
#[test]
fn construction_lines_are_helpers_and_a_lone_circle_is_named() {
    let mut s = Session::default();
    let sk = run(&mut s, "sketch.create", json!({"plane": "XZ"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 15, "id": "c1"}));
    run(&mut s, "sketch.circle.center", json!({"center": [22.5, 0], "radius": 7.5, "id": "c2"}));
    run(&mut s, "sketch.circle.center", json!({"center": [5, 40], "radius": 7.5, "id": "c3"}));
    for (id, a, b) in [("l1", [0.0, 0.0], [22.5, 0.0]), ("l2", [22.5, 0.0], [5.0, 40.0]), ("l3", [5.0, 40.0], [0.0, 0.0])] {
        run(&mut s, "sketch.line", json!({"points": [a, b], "ids": [id], "construction": true}));
    }
    run(&mut s, "sketch.finish", json!({}));
    let e = refusal(&mut s, json!({"sketch": sk, "type": "spur"}));
    assert!(e.contains("circle c3 touches no other gear"), "{e}");
    assert!(!e.contains("shaft line"), "construction lines read as shafts: {e}");
    // A centre line (not construction) between circles that don't touch says what to do.
    run(&mut s, "sketch.edit", json!({"sketch": sk}));
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [5, 40]], "ids": ["m"]}));
    run(&mut s, "sketch.finish", json!({}));
    let e = refusal(&mut s, json!({"sketch": sk, "type": "spur"}));
    assert!(e.contains("joined by the centre line m but don't touch"), "{e}");
}

/// Three circles in a row, each tangent to the next: a train, whatever construction lines are
/// drawn with them.
#[test]
fn tangent_circles_with_construction_helpers_make_a_train() {
    let mut s = Session::default();
    let sk = run(&mut s, "sketch.create", json!({"plane": "XY"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 15, "id": "c1"}));
    run(&mut s, "sketch.circle.center", json!({"center": [22.5, 0], "radius": 7.5, "id": "c2"}));
    run(&mut s, "sketch.circle.center", json!({"center": [42.5, 0], "radius": 12.5, "id": "c3"}));
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [42.5, 0]], "ids": ["axis"], "construction": true}));
    run(&mut s, "sketch.finish", json!({}));
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "spur", "module": 1}));
    assert!(r["params"].is_object(), "{r}");
}
