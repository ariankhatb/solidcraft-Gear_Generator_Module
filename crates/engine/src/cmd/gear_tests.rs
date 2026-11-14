use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn volume(s: &mut Session, body: &str) -> f64 {
    run(s, "inspect.center_of_mass", json!({"bodies": [body]}))["volume"].as_f64().unwrap()
}

#[test]
fn a_spur_gear_is_one_body_of_the_right_volume_and_one_undo_step() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10}));
    let bodies = r["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 1, "{r}");
    let v = volume(&mut s, bodies[0].as_str().unwrap());
    // About the pitch circle's area times the face width.
    let pitch = std::f64::consts::PI * 20.0 * 20.0 * 10.0;
    assert!((v / pitch - 1.0).abs() < 0.06, "{v} vs {pitch}");
    assert!(r["group"].as_u64().is_some());
    run(&mut s, "edit.undo", json!({}));
    assert!(s.execute("inspect.center_of_mass", &json!({"bodies": [bodies[0]]})).is_err());
}

#[test]
fn a_bored_helical_gear_with_a_hub() {
    let mut s = Session::default();
    let r = run(
        &mut s,
        "gear.create",
        json!({"type": "helical", "module": 1.5, "teeth": 24, "helix_angle": 20, "face_width": 12, "bore": {"type": "keyway", "diameter": 12}, "hub_diameter": 22, "hub_length": 8}),
    );
    let b = r["bodies"][0].as_str().unwrap().to_string();
    assert!(volume(&mut s, &b) > 0.0);
}

#[test]
fn calculate_builds_nothing() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.calculate", json!({"type": "pair", "teeth": 12, "teeth2": 40}));
    assert!(r["bodies"].as_array().unwrap().is_empty());
    assert!(r["facts"].as_array().unwrap().iter().any(|f| f["label"] == "contact ratio"));
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("undercut")));
    assert!(s.undo.is_empty());
}

#[test]
fn parameters_round_trip_through_the_spec() {
    for k in solidcraft_gear::Kind::ALL {
        let spec = solidcraft_gear::Spec::defaults_for(k);
        let p = crate::gear_params_of(&spec);
        let back: crate::schema::gear::GearParams = serde_json::from_value(p.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
        assert_eq!(crate::gear_spec_of(&back).unwrap(), spec, "{k:?}");
    }
}

#[test]
fn bad_values_are_refused() {
    let mut s = Session::default();
    for p in [json!({"module": -1}), json!({"teeth": 1}), json!({"type": "spur", "nope": 1}), json!({"module": 2, "face_width": 0})] {
        assert!(s.execute("gear.create", &p).is_err(), "{p}");
    }
}

/// Every type builds (slow: run with --ignored).
#[test]
#[ignore]
fn every_type_builds() {
    for k in solidcraft_gear::Kind::ALL {
        let mut s = Session::default();
        let p = crate::gear_params_of(&solidcraft_gear::Spec::defaults_for(k));
        let r = s.execute("gear.create", &p).unwrap_or_else(|e| panic!("{k:?}: {e}"));
        for b in r["bodies"].as_array().unwrap() {
            let v = volume(&mut s, b.as_str().unwrap());
            assert!(v > 0.0, "{k:?} {b}");
            eprintln!("{k:?} {b}: {v:.1} mm³");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Axis-first: gear.from_axes, and gear.create / gear.calculate with axis1 and axis2
// ---------------------------------------------------------------------------------------------

fn centre(s: &mut Session, body: &Value) -> [f64; 3] {
    let r = run(s, "inspect.center_of_mass", json!({"bodies": [body]}));
    serde_json::from_value(r["center"].clone()).unwrap_or_else(|e| panic!("{r}: {e}"))
}

/// How far `p` is from the line through `o` along `d`.
fn off_axis(p: [f64; 3], o: [f64; 3], d: [f64; 3]) -> f64 {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let d = d.map(|v| v / l);
    let w = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let t = w[0] * d[0] + w[1] * d[1] + w[2] * d[2];
    (0..3).map(|i| (w[i] - t * d[i]).powi(2)).sum::<f64>().sqrt()
}

fn fact(r: &Value, label: &str) -> f64 {
    r["facts"].as_array().unwrap().iter().find(|f| f["label"] == label).unwrap_or_else(|| panic!("no fact {label} in {r}"))["value"].as_f64().unwrap()
}

#[test]
fn from_axes_classifies_and_solves_without_building() {
    let mut s = Session::default();
    let par = json!({"axis1": "Z", "axis2": {"origin": [40, 0, 0], "dir": [0, 0, 1]}, "ratio": 1.5});
    let r = run(&mut s, "gear.from_axes", par.clone());
    assert_eq!(r["relation"]["kind"], "parallel", "{r}");
    assert!((r["relation"]["distance"].as_f64().unwrap() - 40.0).abs() < 1e-9);
    assert_eq!(r["type"], "pair");
    assert_eq!((r["params"]["module"].as_f64(), r["params"]["teeth"].as_u64(), r["params"]["teeth2"].as_u64()), (Some(2.0), Some(16), Some(24)), "{r}");
    assert!(r["derived"].as_array().unwrap().contains(&json!("module")));
    assert_eq!(r["frames"].as_array().unwrap().len(), 2);
    let fits = |r: &Value, t: &str| r["candidates"].as_array().unwrap().iter().find(|c| c["type"] == t).unwrap()["fits"].as_bool().unwrap();
    assert!(fits(&r, "pair") && !fits(&r, "bevel") && !fits(&r, "worm") && fits(&r, "spur"));
    assert!(s.undo.is_empty());
    // The solved parameters are plain gear.create parameters giving the same set.
    let again = run(&mut s, "gear.calculate", r["params"].clone());
    assert!((fact(&again, "centre distance") - 40.0).abs() < 1e-9);
    assert!((fact(&run(&mut s, "gear.calculate", par), "centre distance") - 40.0).abs() < 1e-9);

    // A type that doesn't fit: the classification still comes back, with why.
    let r = run(&mut s, "gear.from_axes", json!({"type": "bevel", "axis1": "Z", "axis2": {"origin": [25, 0, 0], "dir": [0, 1, 0]}}));
    assert_eq!(r["relation"]["kind"], "skew");
    assert!(r["error"].as_str().unwrap().contains("intersecting"), "{r}");
    assert!(r.get("params").is_none());
    let r = run(&mut s, "gear.from_axes", json!({"axis1": "Z", "axis2": {"origin": [25, 0, 0], "dir": [0, 1, 0]}, "ratio": 30}));
    assert_eq!(r["type"], "worm");
    assert!((fact(&r, "centre distance") - 25.0).abs() < 1e-9);
    let r = run(&mut s, "gear.from_axes", json!({"axis1": "Z", "axis2": "X"}));
    assert_eq!((r["relation"]["kind"].as_str(), r["type"].as_str()), (Some("intersecting"), Some("bevel")));
    assert!((fact(&r, "shaft angle") - 90.0).abs() < 1e-9);
    let r = run(&mut s, "gear.from_axes", json!({"axis1": "Z"}));
    assert_eq!((r["relation"]["kind"].as_str(), r["type"].as_str()), (Some("single"), Some("spur")));
}

#[test]
fn axis_requests_that_make_no_sense_are_refused() {
    let mut s = Session::default();
    for p in [
        json!({"axis2": "Z"}),
        json!({"ratio": 2}),
        json!({"axis1": "nope"}),
        json!({"axis1": {"dir": [0, 0, 0]}}),
        json!({"axis1": 7}),
        json!({"axis1": "Z", "axis2": "X", "ratio": -1}),
        json!({"axis1": "Z", "axis2": "X", "type": "pair"}),
        json!({"axis1": "Z", "placement": {"origin": [0, 0, 0], "x": [1, 0, 0], "z": [0, 0, 1]}}),
        json!({"placement": {"origin": [0, 0, 0], "x": [0, 0, 2], "z": [0, 0, 1]}}),
        json!({"axis1": {"sketch": "none", "curve": "L1"}}),
    ] {
        assert!(s.execute("gear.create", &p).is_err(), "{p}");
        assert!(s.execute("gear.calculate", &p).is_err(), "{p}");
    }
    assert!(s.execute("gear.from_axes", &json!({})).is_err());
}

/// Gears made on construction axes turn about them: each body's centre of mass is on its axis
/// (a gear is symmetric about its axis), and the pair is the axes' distance apart.
#[test]
fn a_pair_is_built_on_two_parallel_construction_axes() {
    let mut s = Session::default();
    run(&mut s, "construct.axis.two_points", json!({"a": {"point": [10, 0, 5]}, "b": {"point": [10, 30, 5]}, "name": "Shaft A"}));
    run(&mut s, "construct.axis.two_points", json!({"a": {"point": [10, 0, 46.3]}, "b": {"point": [10, 30, 46.3]}, "name": "Shaft B"}));
    let r = run(&mut s, "gear.create", json!({"axis1": "Shaft A", "axis2": "Shaft B", "ratio": 1.5, "face_width": 8}));
    let bodies = r["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 2, "{r}");
    assert!((fact(&r, "axis distance") - 41.3).abs() < 1e-9 && (fact(&r, "centre distance") - 41.3).abs() < 1e-6, "{r}");
    assert!(!r["notes"].as_array().unwrap().is_empty());
    let (c1, c2) = (centre(&mut s, &bodies[0]), centre(&mut s, &bodies[1]));
    assert!(off_axis(c1, [10.0, 0.0, 5.0], [0.0, 1.0, 0.0]) < 0.02, "{c1:?}");
    assert!(off_axis(c2, [10.0, 0.0, 46.3], [0.0, 1.0, 0.0]) < 0.02, "{c2:?}");
    run(&mut s, "edit.undo", json!({}));
    assert!(s.execute("inspect.center_of_mass", &json!({"bodies": [bodies[0]]})).is_err());
}

/// A sketch line and a cylinder's face serve as shafts.
#[test]
fn sketch_lines_and_round_faces_are_axes() {
    let mut s = Session::default();
    let sk = run(&mut s, "sketch.create", json!({"plane": "XY"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [0, 50]], "ids": ["shaft"]}));
    run(&mut s, "sketch.finish", json!({}));
    let id = "shaft";
    let r = run(&mut s, "gear.from_axes", json!({"axis1": {"sketch": sk, "curve": id}, "axis2": {"origin": [30, 0, 0], "dir": [0, 1, 0]}}));
    assert_eq!(r["relation"]["kind"], "parallel", "{r}");
    assert!((r["relation"]["distance"].as_f64().unwrap() - 30.0).abs() < 1e-9);
}

/// Slow (bevels, a worm): run with --ignored.
#[test]
#[ignore]
fn sets_are_built_on_intersecting_and_skew_axes() {
    let r3 = 3f64.sqrt() / 2.0;
    // Intersecting at 90° and at 60°, away from the origin; skew at 90° (a worm) and 60°.
    let cases = [
        (json!({"origin": [5, 5, 0], "dir": [0, 0, 1]}), json!({"origin": [5, 5, 20], "dir": [1, 0, 0]}), json!({"ratio": 2, "module": 2})),
        (json!({"origin": [5, 5, 0], "dir": [0, 0, 1]}), json!({"origin": [5, 5, 20], "dir": [r3, 0, 0.5]}), json!({"ratio": 1.5, "cone_distance": 40})),
        (json!({"origin": [0, 0, 0], "dir": [0, 0, 1]}), json!({"origin": [30, 0, 0], "dir": [0, 1, 0]}), json!({"ratio": 20})),
        (json!({"origin": [0, 0, 0], "dir": [0, 0, 1]}), json!({"origin": [30, 0, 0], "dir": [0, r3, 0.5]}), json!({"ratio": 2})),
    ];
    for (a1, a2, extra) in cases {
        let mut s = Session::default();
        let mut p = json!({"axis1": a1, "axis2": a2});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let r = s.execute("gear.create", &p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let bodies = r["bodies"].as_array().unwrap();
        assert_eq!(bodies.len(), 2, "{r}");
        for (b, a) in bodies.iter().zip([&a1, &a2]) {
            let c = centre(&mut s, b);
            let v = |k: &str| -> [f64; 3] { serde_json::from_value(a[k].clone()).unwrap() };
            assert!(off_axis(c, v("origin"), v("dir")) < 0.05, "{p}: {b} at {c:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Sets kept with the design: gear.inspect and gear.update (written without a test run: the
// engine's tests were not allowed to run on the branch that added them)
// ---------------------------------------------------------------------------------------------

/// A set is kept with what it was asked for, found by its folder or by a body, and gone with
/// an undo of the command that made it.
#[test]
fn a_set_is_kept_and_inspected() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10, "ui": {"note": "mine"}}));
    let group = r["group"].as_u64().unwrap();
    let body = r["bodies"][0].as_str().unwrap().to_string();
    let by_group = run(&mut s, "gear.inspect", json!({"group": group}));
    let set = &by_group["sets"][0];
    assert_eq!(set["request"]["teeth"], 20, "{by_group}");
    assert_eq!(set["request"]["ui"]["note"], "mine");
    assert_eq!(set["params"]["module"].as_f64(), Some(2.0));
    assert_eq!(set["bodies"][0], body.as_str());
    assert!(!set["features"].as_array().unwrap().is_empty());
    assert_eq!(run(&mut s, "gear.inspect", json!({"body": body}))["sets"][0]["group"].as_u64(), Some(group));
    assert_eq!(run(&mut s, "gear.inspect", json!({}))["sets"].as_array().unwrap().len(), 1);
    assert!(s.execute("gear.inspect", &json!({"body": "nope"})).is_err());
    // `group`, `body` and `reset` belong to gear.update.
    assert!(s.execute("gear.create", &json!({"group": group})).is_err());
    run(&mut s, "edit.undo", json!({}));
    assert!(run(&mut s, "gear.inspect", json!({}))["sets"].as_array().unwrap().is_empty());
}

/// Changing a number regenerates the set in place: the same body, folder and features, another
/// size, one undo step.
#[test]
fn an_update_changes_the_set_in_place() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10}));
    let group = r["group"].as_u64().unwrap();
    let body = r["bodies"][0].as_str().unwrap().to_string();
    let features = run(&mut s, "gear.inspect", json!({"group": group}))["sets"][0]["features"].clone();
    let before = volume(&mut s, &body);
    let u = run(&mut s, "gear.update", json!({"group": group, "face_width": 20, "teeth": 24}));
    assert_eq!(u["updated"], "in_place", "{u}");
    assert_eq!(u["bodies"][0], body.as_str(), "the body keeps its name");
    assert_eq!(u["group"].as_u64(), Some(group));
    let after = volume(&mut s, &body);
    assert!(after > before * 2.5, "{before} -> {after}");
    let set = run(&mut s, "gear.inspect", json!({"body": body}))["sets"][0].clone();
    assert_eq!(set["features"], features, "the sketches and features keep their ids");
    assert_eq!((set["request"]["teeth"].as_u64(), set["request"]["face_width"].as_f64()), (Some(24), Some(20.0)));
    run(&mut s, "edit.undo", json!({}));
    assert!((volume(&mut s, &body) - before).abs() < 1e-6);
    assert!(s.execute("gear.update", &json!({"face_width": 20})).is_err(), "which set?");
    assert!(s.execute("gear.update", &json!({"group": group, "teeth": 1})).is_err());
}

/// A change of structure (another type: other bodies) rebuilds the set in the same folder.
#[test]
fn an_update_that_changes_the_steps_rebuilds_under_the_same_names() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10}));
    let group = r["group"].as_u64().unwrap();
    let body = r["bodies"][0].as_str().unwrap().to_string();
    let before = volume(&mut s, &body);
    let u = run(&mut s, "gear.update", json!({"body": body, "type": "pair", "teeth2": 30}));
    assert_eq!(u["updated"], "rebuilt", "{u}");
    assert!(u["rebuilt_because"].as_str().is_some());
    assert_eq!(u["bodies"].as_array().unwrap().len(), 2);
    assert_eq!(u["group"].as_u64(), Some(group), "{u}");
    let _ = before;
    // And back to one gear, with everything else as it was last asked.
    let u = run(&mut s, "gear.update", json!({"group": group, "type": "spur", "teeth2": null}));
    assert_eq!(u["bodies"].as_array().unwrap().len(), 1, "{u}");
}
