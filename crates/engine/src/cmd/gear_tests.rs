#[allow(unused_imports)]
use solidcraft_geom::prelude::*;
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

#[test]
fn hypoid_parameters_reach_the_spec() {
    let p: crate::schema::gear::GearParams =
        serde_json::from_value(json!({"type": "hypoid", "ring_diameter": 200, "teeth2": 40, "hand": "right", "offset": 0})).unwrap();
    let s = crate::gear_spec_of(&p).unwrap();
    assert!((s.module - 5.0).abs() < 1e-12);
    assert!(!s.left_hand);
    assert_eq!(s.offset, 0.0);
    // Zero offset survives the round trip (it is not the hypoid's default).
    let back: crate::schema::gear::GearParams = serde_json::from_value(crate::gear_params_of(&s)).unwrap();
    assert_eq!(crate::gear_spec_of(&back).unwrap(), s);
    let mut session = Session::default();
    let r = run(&mut session, "gear.calculate", json!({"type": "hypoid"}));
    let fact = |label: &str| r["facts"].as_array().unwrap().iter().find(|f| f["label"].as_str().unwrap().starts_with(label)).map(|f| f["value"].as_f64().unwrap());
    assert!(fact("pinion mean spiral angle").unwrap() > fact("gear mean spiral angle").unwrap());
    assert_eq!(fact("offset"), Some(25.0));
}

/// The default hypoid set builds both members, as the Gear Generator window previews and creates
/// it (slow: run with --ignored).
#[test]
#[ignore]
fn the_default_hypoid_set_builds() {
    let mut s = Session::default();
    let p = crate::gear_params_of(&solidcraft_gear::Spec::defaults_for(solidcraft_gear::Kind::Hypoid));
    let r = run(&mut s, "gear.create", p);
    let bodies = r["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 2, "{r}");
    for b in bodies {
        assert!(volume(&mut s, b.as_str().unwrap()) > 0.0);
    }
}

#[test]
fn sprockets_pulleys_splines_and_face_gears_build() {
    for (p, bodies) in [
        (json!({"type": "sprocket", "chain": "06b", "teeth": 13, "side_chamfer": false}), 1),
        (json!({"type": "pulley", "teeth": 16, "flanges": false}), 1),
        (json!({"type": "spline", "module": 1, "teeth": 12, "spline_diameter": 14}), 2),
        (json!({"type": "face_gear", "module": 1, "teeth": 12, "teeth2": 40, "face_width": 4}), 2),
    ] {
        let mut s = Session::default();
        let r = run(&mut s, "gear.create", p.clone());
        let made = r["bodies"].as_array().unwrap();
        assert_eq!(made.len(), bodies, "{p}: {r}");
        for b in made {
            assert!(volume(&mut s, b.as_str().unwrap()) > 0.0, "{p} {b}");
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

/// A herringbone's upper half is its lower half mirrored and joined (one sweep, not two): the
/// solid the two sweeps made (14043.12 mm³, kernel scripts of both), and it updates in place.
#[test]
fn a_herringbone_is_one_sweep_mirrored_and_updates_in_place() {
    let mut s = Session::default();
    // (A folder of features: the default Gears feature has no sweep or mirror of its own.)
    let r = run(&mut s, "gear.create", json!({"type": "herringbone", "build": "features"}));
    let group = r["group"].as_u64().unwrap();
    let body = r["bodies"][0].as_str().unwrap().to_string();
    let v = volume(&mut s, &body);
    assert!((v / 14043.1213 - 1.0).abs() < 1e-3, "{v}");
    let set = run(&mut s, "gear.inspect", json!({"group": group}))["sets"][0].clone();
    let features = set["features"].clone();
    // The lower half's profile, path and sweep, and the mirror.
    assert_eq!(features.as_array().map(Vec::len), Some(4), "{set}");
    let u = run(&mut s, "gear.update", json!({"group": group, "face_width": 20}));
    assert_eq!(u["updated"], "in_place", "{u}");
    let after = volume(&mut s, &body);
    assert!((after / (2.0 * v) - 1.0).abs() < 1e-3, "{v} -> {after}");
    assert_eq!(run(&mut s, "gear.inspect", json!({"group": group}))["sets"][0]["features"], features);
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
    assert_eq!(body, "Gear z20");
    assert_eq!(u["bodies"][0], "Gear z24", "the generator's name follows the tooth count");
    assert_eq!(u["group"].as_u64(), Some(group));
    let after = volume(&mut s, "Gear z24");
    assert!(after > before * 2.5, "{before} -> {after}");
    let set = run(&mut s, "gear.inspect", json!({"body": "Gear z24"}))["sets"][0].clone();
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
    // (A folder of features: a Gears feature is redefined in place whatever it becomes.)
    let r = run(&mut s, "gear.create", json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10, "build": "features"}));
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

#[test]
fn unknown_chain_and_belt_are_refused() {
    let mut s = Session::default();
    for p in [json!({"type": "sprocket", "chain": "99x"}), json!({"type": "pulley", "belt": "gt9"}), json!({"type": "sprocket", "strands": 9})] {
        assert!(s.execute("gear.calculate", &p).is_err(), "{p}");
    }
}

// ---------------------------------------------------------------------------------------------
// Teeth drawn from their formulas (`tooth_curves: equations`).
// ---------------------------------------------------------------------------------------------

/// The one body a request makes, and its volume.
fn made(s: &mut Session, p: Value) -> (Value, f64) {
    let r = run(s, "gear.create", p);
    let b = r["bodies"][0].as_str().unwrap().to_string();
    let v = volume(s, &b);
    (r, v)
}

fn no_errors(s: &Session) {
    for r in &s.model.results {
        assert!(r.error.is_none(), "{}: {:?}", r.name, r.error);
    }
}

/// How many equation curves and patterns the design's sketches hold.
fn formulas(s: &Session) -> (usize, usize) {
    let mut out = (0, 0);
    for f in &s.doc.features {
        if let solidcraft_doc::FeatureKind::Sketch { sketch, .. } = &f.kind {
            out.0 += sketch.equations.len();
            out.1 += sketch.patterns.len();
        }
    }
    out
}

#[test]
fn a_spur_gear_from_equations_has_the_fitted_gears_volume() {
    let spur = json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10});
    let (_, fitted) = made(&mut Session::default(), spur.clone());
    let mut s = Session::default();
    let mut p = spur.clone();
    p["tooth_curves"] = json!("equations");
    let (r, eq) = made(&mut s, p);
    no_errors(&s);
    assert!((eq / fitted - 1.0).abs() < 1e-3, "equations {eq} vs fitted {fitted}");
    // One tooth: four equation curves (two fillets, two flanks), and one pattern.
    assert_eq!(formulas(&s), (4, 1), "{r}");
    assert!(s.doc.params.is_empty(), "no parameters unless asked");
}

#[test]
fn a_parametric_spur_gear_follows_its_tooth_count_parameter_and_undoes() {
    let (_, fitted20) = made(&mut Session::default(), json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10}));
    let (_, fitted24) = made(&mut Session::default(), json!({"type": "spur", "module": 2, "teeth": 24, "face_width": 10}));
    let mut s = Session::default();
    let (r, v) = made(&mut s, json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10, "tooth_curves": "equations", "parametric": true, "parameter_prefix": "G"}));
    no_errors(&s);
    assert!((v / fitted20 - 1.0).abs() < 1e-3, "{v} vs {fitted20}");
    for name in ["G_module", "G_teeth", "G_pressure_angle", "G_shift", "G_backlash", "G_face_width"] {
        assert!(s.doc.param(name).is_some(), "{name} made: {:?}", s.doc.params.iter().map(|p| &p.name).collect::<Vec<_>>());
    }
    let body = r["bodies"][0].as_str().unwrap().to_string();
    run(&mut s, "parameters.change", json!({"name": "G_teeth", "expression": "24"}));
    no_errors(&s);
    let v24 = volume(&mut s, &body);
    assert!((v24 / fitted24 - 1.0).abs() < 1e-3, "24 teeth by the parameter: {v24} vs {fitted24}");
    // The face width drives the extrusion.
    run(&mut s, "parameters.change", json!({"name": "G_face_width", "expression": "5 mm"}));
    let v5 = volume(&mut s, &body);
    assert!((v5 / (fitted24 / 2.0) - 1.0).abs() < 1e-3, "half the face: {v5}");
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    let back = volume(&mut s, &body);
    assert!((back / fitted20 - 1.0).abs() < 1e-3, "undone: {back} vs {fitted20}");
    assert_eq!(s.doc.param("G_teeth").map(|p| p.expr.trim().to_string()), Some("20".into()));
}

#[test]
fn an_update_of_a_parametric_gear_reads_its_parameters_back() {
    let (_, fitted24) = made(&mut Session::default(), json!({"type": "spur", "module": 2, "teeth": 24, "face_width": 10}));
    let mut s = Session::default();
    let (r, _) = made(&mut s, json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 10, "tooth_curves": "equations", "parametric": true}));
    let group = r["group"].as_u64().unwrap();
    let kept = run(&mut s, "gear.inspect", json!({"group": group}));
    let prefix = kept["sets"][0]["request"]["parameter_prefix"].as_str().unwrap().to_string();
    run(&mut s, "parameters.change", json!({"name": format!("{prefix}_teeth"), "expression": "24"}));
    let u = run(&mut s, "gear.update", json!({"group": group}));
    no_errors(&s);
    let body = u["bodies"][0].as_str().unwrap().to_string();
    assert!((volume(&mut s, &body) / fitted24 - 1.0).abs() < 1e-3, "{u}");
    let set = run(&mut s, "gear.inspect", json!({"group": group}));
    assert_eq!(set["sets"][0]["request"]["teeth"], json!(24), "{set}");
    assert_eq!(set["sets"][0]["request"]["parameter_prefix"], json!(prefix));
    // One undo step takes the update back.
    run(&mut s, "edit.undo", json!({}));
    no_errors(&s);
}

#[test]
fn ring_rack_and_pair_teeth_from_equations_match_the_fitted_ones() {
    for p in [
        json!({"type": "internal", "module": 1.5, "teeth": 48, "face_width": 8}),
        json!({"type": "rack", "module": 2, "rack_teeth": 8, "face_width": 10}),
        json!({"type": "helical", "module": 2, "teeth": 18, "helix_angle": 20, "face_width": 10}),
        json!({"type": "pair", "module": 2, "teeth": 17, "teeth2": 31, "face_width": 8}),
    ] {
        let mut fs = Session::default();
        let rf = run(&mut fs, "gear.create", p.clone());
        let mut q = p.clone();
        q["tooth_curves"] = json!("equations");
        q["parametric"] = json!(true);
        let mut s = Session::default();
        let r = run(&mut s, "gear.create", q);
        no_errors(&s);
        assert!(formulas(&s).1 >= 1, "{p}: a pattern");
        for (a, b) in rf["bodies"].as_array().unwrap().iter().zip(r["bodies"].as_array().unwrap()) {
            let (va, vb) = (volume(&mut fs, a.as_str().unwrap()), volume(&mut s, b.as_str().unwrap()));
            assert!((vb / va - 1.0).abs() < 2e-3, "{p} {b}: equations {vb} vs fitted {va}");
        }
    }
}

#[test]
fn tuned_tips_and_other_types_stay_fitted_with_a_note() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "spur", "tip_round": 0.2, "tooth_curves": "equations"}));
    assert_eq!(formulas(&s), (0, 0));
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("fitted")), "{r}");
    let r = run(&mut s, "gear.create", json!({"type": "bevel", "tooth_curves": "equations"}));
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("fitted")), "{r}");
}

/// A helical rack stays fitted because it is lofted, and the note says so (not tip shaping).
#[test]
fn a_helical_rack_says_why_its_teeth_stay_fitted() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "rack", "tooth_curves": "equations", "helix_angle": 15}));
    let notes: Vec<&str> = r["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).filter(|w| w.contains("fitted")).collect();
    assert_eq!(notes.len(), 1, "{r}");
    assert!(notes[0].contains("helical rack") && notes[0].contains("lofted") && !notes[0].contains("tip rounding"), "{}", notes[0]);
    // Tip rounding on a spur gear names tip rounding only.
    let r = run(&mut s, "gear.create", json!({"type": "spur", "tip_round": 0.2, "tooth_curves": "equations"}));
    let notes: Vec<&str> = r["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).filter(|w| w.contains("fitted")).collect();
    assert!(notes.len() == 1 && notes[0].contains("(tip rounding has no formulas here)"), "{r}");
}

#[test]
fn a_parametric_ring_keeps_its_rim_profile_through_a_tooth_count_change() {
    let mut s = Session::default();
    let (r, v) = made(&mut s, json!({"type": "internal", "module": 1.5, "teeth": 48, "face_width": 8, "tooth_curves": "equations", "parametric": true, "parameter_prefix": "R"}));
    let body = r["bodies"][0].as_str().unwrap().to_string();
    // More teeth in the same rim: a bigger tooth circle, less material; the extrusion's profile
    // is picked by the rim circle, which stays.
    run(&mut s, "parameters.change", json!({"name": "R_teeth", "expression": "50"}));
    no_errors(&s);
    let v50 = volume(&mut s, &body);
    assert!(v50 < v && v50 > 0.5 * v, "{v} then {v50}");
}

// ---- Sets drawn in one sketch: circles are pitch circles, lines are shafts (one per row of the
// sketch rules table). Not compiled by the window agent; run with the engine tests.

fn sketch_with(s: &mut Session, draw: &[(&str, Value)]) -> u64 {
    let sk = run(s, "sketch.create", json!({"plane": "XY"}))["sketch"].as_u64().unwrap();
    for (cmd, p) in draw {
        run(s, cmd, p.clone());
    }
    run(s, "sketch.finish", json!({}));
    sk
}

fn fact_opt(r: &Value, label: &str) -> Option<f64> {
    r["facts"].as_array()?.iter().find(|f| f["label"] == label)?["value"].as_f64()
}

fn circle(x: f64, y: f64, r: f64, id: &str) -> (&'static str, Value) {
    ("sketch.circle.center", json!({"center": [x, y], "radius": r, "id": id}))
}

fn line(a: [f64; 2], b: [f64; 2], id: &str) -> (&'static str, Value) {
    ("sketch.line", json!({"points": [a, b], "ids": [id]}))
}

#[test]
fn sketch_one_circle_is_a_single_gear() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk}));
    assert!(fact_opt(&r, "pitch diameter").is_none_or(|d| (d - 40.0).abs() < 0.3), "{r}");
}

#[test]
fn sketch_touching_circles_are_a_pair_on_their_centres() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g1"), circle(50.0, 0.0, 30.0, "g2")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "pair", "picks": [null, "g1", "g2"]}));
    assert!((fact_opt(&r, "axis distance").unwrap() - 50.0).abs() < 1e-6, "{r}");
    // Apart circles are refused, saying why.
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g1"), circle(70.0, 0.0, 30.0, "g2")]);
    let e = s.execute("gear.calculate", &json!({"sketch": sk, "type": "pair"})).unwrap_err().to_string();
    assert!(e.contains("must touch outside"), "{e}");
}

#[test]
fn sketch_internal_and_planetary() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 60.0, "ring"), circle(40.0, 0.0, 20.0, "pinion")]);
    run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "internal"}));
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 18.0, "sun"), circle(36.0, 0.0, 18.0, "planet"), circle(0.0, 0.0, 54.0, "ring")]);
    run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "planetary", "planets": 3}));
}

#[test]
fn sketch_tangent_line_is_a_worm_or_a_rack() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 30.0, "wheel"), line([-40.0, 30.0], [40.0, 30.0], "worm")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "worm"}));
    assert!(fact_opt(&r, "offset (from the axes)").is_some(), "{r}");
    run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "rack"}));
}

#[test]
fn sketch_bevel_side_view_at_80_degrees() {
    let a = 80f64.to_radians();
    let mut s = Session::default();
    let sk = sketch_with(
        &mut s,
        &[("sketch.point", json!({"point": [0, 0], "id": "apex"})), line([0.0, 0.0], [0.0, -50.0], "s1"), line([0.0, 0.0], [50.0 * a.sin(), -50.0 * a.cos()], "s2")],
    );
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "bevel", "picks": ["apex", "s1", "s2"]}));
    assert!((fact_opt(&r, "shaft angle (from the axes)").unwrap() - 80.0).abs() < 1e-6, "{r}");
}

#[test]
fn sketch_hypoid_and_crossed_helical_offsets() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 100.0, "ring"), line([-120.0, -25.0], [120.0, -25.0], "pinion")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "hypoid"}));
    assert!((fact_opt(&r, "offset (from the axes)").unwrap() - 25.0).abs() < 1e-6, "{r}");
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g1"), line([-60.0, 45.0], [60.0, 45.0], "s2")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "crossed_helical"}));
    assert!((fact_opt(&r, "offset (from the axes)").unwrap() - 45.0).abs() < 1e-6, "{r}");
    // Without a type, the drawing's look decides.
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk}));
    assert!(fact_opt(&r, "offset (from the axes)").is_some(), "{r}");
}

#[test]
fn sketch_picks_must_be_in_the_sketch() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g1")]);
    let e = s.execute("gear.calculate", &json!({"sketch": sk, "type": "spur", "picks": [null, "nope"]})).unwrap_err().to_string();
    assert!(e.contains("one sketch"), "{e}");
}

/// Three circles touching in a chain, typed only by their teeth: a three-gear train on the
/// drawn centres, its ratio from gear 1, and an assembly linked along it.
#[test]
fn sketch_three_touching_circles_are_a_gear_train() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g1"), circle(50.0, 0.0, 30.0, "g2"), circle(50.0, 45.0, 15.0, "g3")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "spur"}));
    // Decision 1: teeth closest to Ø / m at the auto module 1.5 (z27, z20), the drawn centres held
    // by profile shift: the ratio is the teeth's, not the diameters' (0.75).
    assert!((fact_opt(&r, "train ratio gear 1 → gear 3").unwrap() - 20.0 / 27.0).abs() < 1e-9, "{r}");
    let r = run(&mut s, "gear.create", json!({"sketch": sk, "type": "helical", "assembly": true}));
    assert_eq!(r["bodies"].as_array().unwrap().len(), 3, "{r}");
    assert_eq!(r["assembly"]["links"].as_array().map(Vec::len), Some(2), "{r}");
    // Picked: only gears 1 and 2 (a pair).
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "spur", "picks": ["g1", "g2"]}));
    assert!((fact_opt(&r, "axis distance").unwrap() - 50.0).abs() < 1e-6, "{r}");
    // A ring and the gear inside it, with a tooth style: both built.
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 60.0, "ring"), circle(40.0, 0.0, 20.0, "pinion")]);
    let r = run(&mut s, "gear.create", json!({"sketch": sk, "type": "spur"}));
    assert_eq!(r["bodies"].as_array().unwrap().len(), 2, "{r}");
}

/// A sketch point stands for the shaft through it, normal to its sketch.
#[test]
fn a_sketch_point_is_a_shaft_normal_to_its_sketch() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[("sketch.point", json!({"point": [30, 0], "id": "c"}))]);
    let r = run(&mut s, "gear.from_axes", json!({"axis1": {"sketch": sk, "point": "c"}, "axis2": "Z"}));
    assert_eq!(r["relation"]["kind"], "parallel", "{r}");
    assert!((r["relation"]["distance"].as_f64().unwrap() - 30.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------------
// The clearance body (keep-out volume for a housing)
// ---------------------------------------------------------------------------------------------

fn names(r: &Value) -> Vec<String> {
    r["bodies"].as_array().unwrap().iter().map(|b| b.as_str().unwrap().to_string()).collect()
}

/// `gear.create` with `clearance_body` makes exactly one more body, which cuts a housing block
/// in one Boolean without touching the gears, and which an update regenerates in place.
#[test]
fn a_clearance_body_comes_with_the_set_and_cuts_a_housing() {
    let set = json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 32, "face_width": 10, "name": "Drive",
        "bore": {"type": "round", "diameter": 8}, "bore2": {"type": "round", "diameter": 8}});
    let mut plain = Session::default();
    let bare = names(&run(&mut plain, "gear.create", set.clone()));
    assert_eq!(bare.len(), 2);
    let mut s = Session::default();
    let mut p = set.clone();
    p["clearance_body"] = json!({"radial": 0.5, "axial": 0.5, "bearing": "608", "bearing_fit": 0.1,
        "seats": [{"member": 1, "along": -14}, {"member": 2, "along": 14}, {"point": [0, 0, 20]}]});
    let r = run(&mut s, "gear.create", p.clone());
    let made = names(&r);
    assert_eq!(made.len(), bare.len() + 1, "{r}");
    assert_eq!(made[..2], bare[..]);
    assert_eq!(r["clearance"], "Drive clearance", "{r}");
    assert_eq!(made[2], "Drive clearance");
    let facts = r["facts"].as_array().unwrap();
    assert!(facts.iter().any(|f| f["label"] == "clearance bearing 608 outside diameter" && f["value"] == 22.0), "{r}");
    assert!(!r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("seat")), "{r}");
    let v = volume(&mut s, "Drive clearance");
    assert!(v > 0.0);
    // The set is kept with its clearance body.
    let group = r["group"].as_u64().unwrap();
    let kept = run(&mut s, "gear.inspect", json!({"group": group}));
    assert_eq!(kept["sets"][0]["bodies"].as_array().unwrap().len(), 3, "{kept}");
    assert!(kept["sets"][0]["params"]["clearance_body"]["seats"].as_array().is_some_and(|a| a.len() == 3), "{kept}");
    // A housing block (the shafts go through its walls) cut by it in one Boolean.
    run(&mut s, "solid.box", json!({"length": 150, "width": 90, "height": 30, "center": [26, 0, 5], "body_name": "Housing"}));
    let block = volume(&mut s, "Housing");
    run(&mut s, "solid.combine", json!({"target": "Housing", "tools": ["Drive clearance"], "operation": "cut", "keep_tools": true}));
    let housing = volume(&mut s, "Housing");
    assert!(housing > 0.0 && housing < block - 0.5 * v, "{block} -> {housing} (clearance {v})");
    no_errors(&s);
    // Nothing of the gears is in the housing.
    let i = run(&mut s, "inspect.interference", json!({"bodies": ["Housing", made[0], made[1]]}));
    let hits: Vec<&Value> = i["interferences"].as_array().unwrap().iter().filter(|h| h["a"] == "Housing" || h["b"] == "Housing").collect();
    assert!(hits.is_empty(), "{i}");
    assert!(i["unchecked"].as_array().unwrap().iter().all(|u| u["a"] != "Housing" && u["b"] != "Housing"), "{i}");
    // More clearance: regenerated in place, and the housing's cut follows.
    let u = run(&mut s, "gear.update", json!({"group": group, "clearance_body": {"radial": 1.0, "axial": 0.5, "bearing": "608", "bearing_fit": 0.1,
        "seats": [{"member": 1, "along": -14}, {"member": 2, "along": 14}, {"point": [0, 0, 20]}]}}));
    assert_eq!(u["updated"], "in_place", "{u}");
    assert_eq!(u["clearance"], "Drive clearance");
    assert!(u["broken"].as_array().is_none_or(|b| b.is_empty()), "{u}");
    assert!(volume(&mut s, "Drive clearance") > v);
    assert!(volume(&mut s, "Housing") < housing);
    // Without it: the set is rebuilt with only its gears.
    let u = run(&mut s, "gear.update", json!({"group": group, "clearance_body": null}));
    assert_eq!(names(&u).len(), 2, "{u}");
    assert!(u.get("clearance").is_none(), "{u}");
}

#[test]
fn clearance_parameters_round_trip_and_bad_ones_are_refused() {
    use solidcraft_gear::clearance::{BearingSize, Clearance, Seat};
    let mut spec = solidcraft_gear::Spec::defaults_for(solidcraft_gear::Kind::Worm);
    spec.clearance_body = Some(Clearance {
        radial: 0.3,
        axial: 0.8,
        shaft_extension: 25.0,
        ring_as_housing: false,
        bearing: None,
        bearing_size: Some(BearingSize { bore: 10.0, od: 26.0, width: 8.0 }),
        bearing_fit: 0.15,
        seats: vec![Seat::Along { member: 2, along: -30.0 }, Seat::Point([1.0, 2.0, 3.0])],
        travel: 0.0,
        keep_out: true,
        bearing_bodies: false,
    });
    let p = crate::gear_params_of(&spec);
    let back: crate::schema::gear::GearParams = serde_json::from_value(p.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    assert_eq!(crate::gear_spec_of(&back).unwrap(), spec, "{p}");
    let mut s = Session::default();
    for c in [
        json!({"bearing": "9999"}),
        json!({"seats": [{"member": 1}]}),
        json!({"seats": [{"member": 1, "along": 0}]}),
        json!({"radial": -1}),
        json!({"bearing": {"od": 0, "width": 5}}),
        json!({"bearing": "608", "seats": [{"member": 7, "along": 0}]}),
        json!({"nope": 1}),
    ] {
        assert!(s.execute("gear.calculate", &json!({"type": "pair", "clearance_body": c})).is_err(), "{c}");
    }
    // gear.calculate gives the clearance facts and builds nothing.
    let r = run(&mut s, "gear.calculate", json!({"type": "worm", "clearance_body": {"bearing": "6000", "seats": [{"member": 2, "along": 25}]}}));
    assert!(r["bodies"].as_array().unwrap().is_empty());
    assert!(r["facts"].as_array().unwrap().iter().any(|f| f["label"].as_str().unwrap().starts_with("clearance: ")), "{r}");
}

/// Every type builds its clearance body in the kernel (slow: run with --ignored).
#[test]
#[ignore]
fn every_type_builds_its_clearance_body() {
    for k in solidcraft_gear::Kind::ALL {
        let mut s = Session::default();
        let mut p = crate::gear_params_of(&solidcraft_gear::Spec::defaults_for(k));
        p["clearance_body"] = json!({});
        let r = s.execute("gear.create", &p).unwrap_or_else(|e| panic!("{k:?}: {e}"));
        let name = r["clearance"].as_str().unwrap_or_else(|| panic!("{k:?}: {r}")).to_string();
        let v = volume(&mut s, &name);
        assert!(v > 0.0, "{k:?}");
        eprintln!("{k:?} {name}: {v:.1} mm³");
    }
}

#[test]
fn a_worm_sets_loft_sketches_are_hidden() {
    let mut s = Session::default();
    // (A folder of features: a Gears feature makes no sketches.)
    run(&mut s, "gear.create", json!({"type": "worm", "build": "features"}));
    // Every sketch the set made (the wheel's loft sections, the thread) is hidden: shown, they
    // draw their outlines over the teeth.
    let sketches: Vec<u64> = s.doc.features.iter().filter(|f| matches!(f.kind, solidcraft_doc::FeatureKind::Sketch { .. })).map(|f| f.id).collect();
    assert!(sketches.len() > 3, "{sketches:?}");
    for id in sketches {
        assert!(s.visibility.hidden_sketches.contains(&id), "sketch {id} shows: {:?}", s.visibility.hidden_sketches);
    }
}

// ---- Bugs found by testing build 3 (sketch-driven sets).

#[test]
fn a_bevel_side_view_away_from_the_sketch_origin_is_recognised() {
    // Two shaft lines meeting at (30, 20), no dot: the sketch's own origin is not the apex.
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[line([30.0, 20.0], [30.0, -30.0], "s1"), line([30.0, 20.0], [80.0, 20.0], "s2")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk}));
    assert!(r["facts"].as_array().is_some_and(|f| !f.is_empty()), "{r}");
    // With a dot at the apex too.
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[("sketch.point", json!({"point": [30, 20], "id": "a"})), line([30.0, 20.0], [30.0, -30.0], "s1"), line([30.0, 20.0], [80.0, 20.0], "s2")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk}));
    assert!(r["facts"].as_array().is_some_and(|f| !f.is_empty()), "{r}");
}

#[test]
fn a_drawn_dot_is_the_origin_a_sketch_set_turns_about() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(10.0, 5.0, 20.0, "g1"), circle(10.0, 55.0, 30.0, "g2"), ("sketch.point", json!({"point": [100, 0], "id": "o"}))]);
    let auto = run(&mut s, "gear.create", json!({"sketch": sk, "sketch_turn": [0, 0, 90]}));
    let c = centre(&mut s, &auto["bodies"][0]);
    // Gear 1's centre (10, 5) turned 90° about the dot (100, 0): (95, -90).
    assert!((c[0] - 95.0).abs() < 0.5 && (c[1] + 90.0).abs() < 0.5, "{c:?}");
}

#[test]
fn a_sketch_set_keeps_the_face_width_asked() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 45.0, "g1"), circle(112.5, 0.0, 67.5, "g2")]);
    let r = run(&mut s, "gear.create", json!({"sketch": sk, "face_width": 8, "module": 3}));
    // 8 mm thick from the sketch's plane: the centre of mass 4 mm up (not 25 for 50 mm).
    let c = centre(&mut s, &r["bodies"][0]);
    assert!((c[2].abs() - 4.0).abs() < 0.5, "{c:?}");
}

#[test]
fn a_sketch_rack_lies_on_its_pitch_line() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(20.0, 10.0, 30.0, "p"), line([-20.0, 40.0], [60.0, 40.0], "l")]);
    let r = run(&mut s, "gear.create", json!({"sketch": sk, "type": "rack", "face_width": 8}));
    let c = centre(&mut s, &r["bodies"][0]);
    // Along the line (x from -20), below it (teeth up to the pitch line at y = 40, the body
    // under it, towards the pinion above? the pinion's centre is below at y = 10: teeth face it).
    assert!(c[0] > -20.0 && c[0] < 80.0, "{c:?}");
    assert!((c[1] - 40.0).abs() < 15.0, "rack centre {c:?} is not by the pitch line y = 40");
}

#[test]
fn a_sketch_crossed_helical_takes_its_drawn_sizes() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g"), line([-60.0, 45.0], [60.0, 45.0], "l")]);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "crossed_helical"}));
    let (d1, d2) = (fact(&r, "gear 1 pitch diameter"), fact(&r, "gear 2 pitch diameter"));
    // Gear 1 near the drawn Ø40, gear 2 near the Ø50 the 45 mm offset leaves.
    assert!((d2 / d1 - 50.0 / 40.0).abs() < 0.2, "{d1} / {d2}");
}

#[test]
fn a_bevel_planetary_at_60_degrees_builds_without_a_mode() {
    let mut s = Session::default();
    let a = 60f64.to_radians();
    let sk = sketch_with(&mut s, &[("sketch.point", json!({"point": [0, 0], "id": "apex"})), line([0.0, 0.0], [0.0, -50.0], "s1"), line([0.0, 0.0], [50.0 * a.sin(), -50.0 * a.cos()], "s2")]);
    let r = run(&mut s, "gear.create", json!({"sketch": sk, "type": "bevel_planetary"}));
    assert!(r["bodies"].as_array().is_some_and(|b| b.len() >= 3), "{r}");
}

// ---- Bugs found by testing build 3 (gear.create / gear.update options).

#[test]
fn a_planetary_ring_follows_its_sun_and_planets() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "planetary", "module": 0.5, "teeth": 24, "teeth2": 12}));
    let names: Vec<&str> = r["bodies"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(names.iter().any(|n| n.contains("z48")), "{names:?}");
    assert!(!r["warnings"].as_array().is_some_and(|w| w.iter().any(|x| x.as_str().is_some_and(|x| x.contains("ring teeth = sun")))), "{r}");
}

#[test]
fn equal_gears_and_a_second_set_get_distinct_names() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "teeth": 20, "teeth2": 20}));
    let a: Vec<String> = r["bodies"].as_array().unwrap().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    assert_eq!(a, ["Gear z20", "Gear z20 (2)"]);
    let r = run(&mut s, "gear.create", json!({"origin": [60, 0, 0]}));
    assert_eq!(r["bodies"][0].as_str(), Some("Gear z20 (3)"));
    assert!(!s.model.state().bodies.iter().any(|b| b.name.starts_with("Body")));
}

// ---------------------------------------------------------------------------------------------
// Parametric sets: what follows a parameter edit besides the teeth
// ---------------------------------------------------------------------------------------------

/// The overlap volume the kernel finds among `bodies`.
fn overlap(s: &mut Session, bodies: &[&str]) -> f64 {
    let i = run(s, "inspect.interference", json!({"bodies": bodies}));
    i["interferences"].as_array().unwrap().iter().map(|h| h["volume_mm3"].as_f64().unwrap_or(0.0)).sum()
}

/// A parametric pair's gear 2 sits at the centre distance its parameters give: a tooth-count or
/// module edit moves it, as gear.update would, and the pair still meshes.
#[test]
fn a_parametric_pairs_mate_follows_the_centre_distance() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 30, "face_width": 8, "tooth_curves": "equations", "parametric": true, "parameter_prefix": "P"}));
    no_errors(&s);
    let (g1, g2) = (r["bodies"][0].clone(), r["bodies"][1].clone());
    assert!((centre(&mut s, &g2)[0] - 50.0).abs() < 1e-3, "{r}");
    run(&mut s, "parameters.change", json!({"name": "P_teeth", "expression": "24"}));
    no_errors(&s);
    let c = centre(&mut s, &g2);
    assert!((c[0] - 54.0).abs() < 1e-3 && c[1].abs() < 1e-3, "gear 2 at {c:?}, wanted x = 54");
    let o = overlap(&mut s, &[g1.as_str().unwrap(), g2.as_str().unwrap()]);
    assert!(o < 0.05, "the pair overlaps by {o} mm³ after the tooth change");
    run(&mut s, "parameters.change", json!({"name": "P_module", "expression": "2.5 mm"}));
    no_errors(&s);
    let c = centre(&mut s, &g2);
    assert!((c[0] - 67.5).abs() < 1e-3, "gear 2 at {c:?}, wanted x = 67.5");
    // A profile shift pushes it out by the working centre distance.
    run(&mut s, "parameters.change", json!({"name": "P_shift", "expression": "0.3"}));
    no_errors(&s);
    let shifted = run(&mut s, "gear.calculate", json!({"type": "pair", "module": 2.5, "teeth": 24, "teeth2": 30, "profile_shift": 0.3, "face_width": 8}));
    let want = fact(&shifted, "centre distance");
    let c = centre(&mut s, &g2);
    assert!((c[0] - want).abs() < 1e-3, "gear 2 at {c:?}, wanted x = {want}");
}

/// A parametric planetary set's planets follow the module and the tooth counts (their places and
/// their mesh phases).
#[test]
fn a_parametric_planetary_sets_planets_follow_their_parameters() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "planetary", "module": 1, "teeth": 12, "teeth2": 24, "planets": 3, "face_width": 6, "tooth_curves": "equations", "parametric": true, "parameter_prefix": "S"}));
    no_errors(&s);
    let bodies: Vec<String> = names(&r);
    assert_eq!(bodies.len(), 5, "{r}");
    let radius = |c: [f64; 3]| c[0].hypot(c[1]);
    let p1 = json!(bodies[1]);
    assert!((radius(centre(&mut s, &p1)) - 18.0).abs() < 1e-3);
    run(&mut s, "parameters.change", json!({"name": "S_module", "expression": "1.25 mm"}));
    no_errors(&s);
    let c = centre(&mut s, &p1);
    assert!((radius(c) - 22.5).abs() < 1e-3, "planet 1 at {c:?}, wanted 22.5 from the sun");
    run(&mut s, "parameters.change", json!({"name": "S_module", "expression": "1 mm"}));
    // More sun and ring teeth: the planets go out and turn to mesh again.
    run(&mut s, "parameters.change", json!({"changes": {"S_teeth": "15", "S_teeth3": "63"}}));
    no_errors(&s);
    for p in &bodies[1..4] {
        let c = centre(&mut s, &json!(p));
        assert!((radius(c) - 19.5).abs() < 1e-3, "{p} at {c:?}, wanted 19.5 from the sun");
    }
    let refs: Vec<&str> = bodies.iter().map(String::as_str).collect();
    let o = overlap(&mut s, &refs);
    assert!(o < 0.05, "the set overlaps by {o} mm³ after the tooth change");
}

/// A tooth count edited into undercut leaves one body (the formulas' fillet and flank overlap
/// in small loops, which are not extruded).
#[test]
fn a_parametric_tooth_count_into_undercut_leaves_one_body() {
    for (bore, teeth) in [(None, 13), (Some(8.0), 13), (None, 4)] {
        let mut p = json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 8, "tooth_curves": "equations", "parametric": true, "parameter_prefix": "G", "name": "E"});
        if let Some(d) = bore {
            p["bore"] = json!({"type": "round", "diameter": d});
        }
        let mut s = Session::default();
        let (r, _) = made(&mut s, p.clone());
        let body = r["bodies"][0].as_str().unwrap().to_string();
        run(&mut s, "parameters.change", json!({"name": "G_teeth", "expression": teeth.to_string()}));
        no_errors(&s);
        let all: Vec<String> = s.model.state().bodies.iter().map(|b| b.name.clone()).collect();
        assert_eq!(all, vec![body.clone()], "bore {bore:?}, {teeth} teeth");
        let v = volume(&mut s, &body);
        assert!(v > 0.0);
        if teeth == 13 {
            let mut f = p.clone();
            f["teeth"] = json!(13);
            for k in ["tooth_curves", "parametric", "parameter_prefix"] {
                f.as_object_mut().unwrap().remove(k);
            }
            let (_, fitted) = made(&mut Session::default(), f);
            assert!((v / fitted - 1.0).abs() < 2e-3, "bore {bore:?}: {v} vs fitted {fitted}");
        }
    }
}

/// A parametric set's clearance body follows the parameters: its envelope grows with the tip
/// and the face, as the clearance of the set made with those numbers.
#[test]
fn a_parametric_clearance_body_follows_the_parameters() {
    let base = json!({"type": "spur", "module": 2, "teeth": 20, "face_width": 8, "name": "E", "clearance_body": {}});
    let clearance_of = |p: Value| {
        let mut s = Session::default();
        let r = run(&mut s, "gear.create", p);
        let name = r["clearance"].as_str().unwrap().to_string();
        volume(&mut s, &name)
    };
    let mut p = base.clone();
    p["tooth_curves"] = json!("equations");
    p["parametric"] = json!(true);
    p["parameter_prefix"] = json!("E");
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", p);
    no_errors(&s);
    let name = r["clearance"].as_str().unwrap().to_string();
    let mut q = base.clone();
    let v20 = clearance_of(q.clone());
    assert!((volume(&mut s, &name) / v20 - 1.0).abs() < 1e-6);
    run(&mut s, "parameters.change", json!({"name": "E_teeth", "expression": "30"}));
    no_errors(&s);
    q["teeth"] = json!(30);
    let v30 = clearance_of(q.clone());
    let v = volume(&mut s, &name);
    assert!((v / v30 - 1.0).abs() < 2e-3, "clearance {v} vs the z30 set's {v30}");
    run(&mut s, "parameters.change", json!({"name": "E_face_width", "expression": "12 mm"}));
    no_errors(&s);
    q["face_width"] = json!(12);
    let v12 = clearance_of(q);
    let v = volume(&mut s, &name);
    assert!((v / v12 - 1.0).abs() < 2e-3, "clearance {v} vs the 12 mm set's {v12}");
    // A housing cut from it does not touch the gear.
    let gear = r["bodies"][0].as_str().unwrap().to_string();
    run(&mut s, "solid.box", json!({"length": 90, "width": 90, "height": 20, "center": [0, 0, 6], "body_name": "Housing"}));
    run(&mut s, "solid.combine", json!({"target": "Housing", "tools": [name], "operation": "cut", "keep_tools": true}));
    assert!(overlap(&mut s, &["Housing", &gear]) < 1e-3);
}

/// An equation set made fitted again by gear.update takes away the design parameters it made,
/// except those something else uses.
#[test]
fn an_update_back_to_fitted_teeth_removes_the_sets_parameters() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 30, "face_width": 8, "tooth_curves": "equations", "parametric": true, "parameter_prefix": "P"}));
    let group = r["group"].as_u64().unwrap();
    assert!(s.doc.params.iter().filter(|p| p.name.starts_with("P_")).count() > 5);
    run(&mut s, "parameters.add", json!({"name": "Mine", "expression": "P_module * 2", "unit": "mm"}));
    run(&mut s, "gear.update", json!({"group": group, "tooth_curves": "fitted", "parametric": false}));
    no_errors(&s);
    let left: Vec<&str> = s.doc.params.iter().map(|p| p.name.as_str()).filter(|n| n.starts_with("P_")).collect();
    assert_eq!(left, vec!["P_module"], "only the one Mine uses stays");
    assert!(s.doc.param("Mine").is_some());
    assert_eq!(formulas(&s), (0, 0));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.doc.param("P_teeth").is_some(), "undo brings them back");
}

// ---------------------------------------------------------------------------------------------
// assembly: true: components, joints and motion links
// ---------------------------------------------------------------------------------------------

/// A joint's first value as `joint.list` gives it (degrees or mm), by id.
fn joint_value(s: &mut Session, id: u64) -> f64 {
    let l = run(s, "joint.list", json!({"quiet": true}));
    let j = l["joints"].as_array().into_iter().flatten().find(|j| j["id"].as_u64() == Some(id)).cloned().unwrap_or_default();
    j["values"][0]["value"].as_f64().unwrap_or(f64::NAN)
}

/// The members' joints of an assembled set, in body order (None for a grounded member).
fn member_joints(r: &Value) -> Vec<Option<u64>> {
    r["assembly"]["members"].as_array().map(|ms| ms.iter().map(|m| m["joint"].as_u64()).collect()).unwrap_or_default()
}

/// Drives joint `a` to `deg` and gives where joint `b` went.
fn drive_and_read(s: &mut Session, a: u64, deg: f64, b: u64) -> f64 {
    let d = run(s, "joint.drive", json!({"joint": a, "value": deg}));
    assert_eq!(d["conflicts"].as_array().map(Vec::len).unwrap_or(0), 0, "{d}");
    let got = joint_value(s, b);
    eprintln!("GATE   joint {a} driven to {deg}°: joint {b} measured {got:+.6}");
    got
}

fn assembled(s: &mut Session, p: Value) -> Value {
    let undo = s.undo.len();
    let r = run(s, "gear.create", p);
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    no_errors(s);
    let a = &r["assembly"];
    let members = a["members"].as_array().cloned().unwrap_or_default();
    assert_eq!(members.len(), r["bodies"].as_array().map_or(0, |b| b.len()) - usize::from(r["clearance"].is_string()), "{r}");
    // Every member is a component of its own, holding its body.
    for m in &members {
        let c = m["component"].as_u64().unwrap();
        assert!(s.doc.components.iter().any(|k| k.id == c), "{m}");
    }
    // The joints and links are there (joint.list) and in the set's folder.
    let l = run(s, "joint.list", json!({}));
    let listed: Vec<u64> = l["joints"].as_array().into_iter().flatten().filter_map(|j| j["id"].as_u64()).collect();
    for j in a["joints"].as_array().into_iter().flatten() {
        assert!(listed.contains(&j.as_u64().unwrap()), "{l}");
    }
    let group = r["group"].as_u64().unwrap();
    let folder = s.doc.timeline_groups.iter().find(|g| g.id == group).unwrap();
    for j in a["joints"].as_array().into_iter().flatten() {
        assert!(folder.joints.contains(&j.as_u64().unwrap()), "joint {j} is in the set's folder");
    }
    r
}

#[test]
fn an_assembled_pair_turns_gear_2_by_the_ratio_and_undoes_in_one_step() {
    let mut s = Session::default();
    let r = assembled(&mut s, json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 32, "face_width": 8, "assembly": true}));
    let j = member_joints(&r);
    let (g1, g2) = (j[0].unwrap(), j[1].unwrap());
    assert_eq!(r["assembly"]["links"].as_array().map(Vec::len), Some(1), "{r}");
    // Gear 1 at 32° turns gear 2 the other way by 20/32.
    let got = drive_and_read(&mut s, g1, 32.0, g2);
    assert!((got + 20.0).abs() < 1e-6, "{got}");
    // In the world too: a point on gear 2's pitch circle has gone round its axis by -20°.
    let occ = r["assembly"]["members"][1]["occurrence"].as_u64().unwrap();
    let p = solidcraft_doc::apply_point(&solidcraft_doc::joints::occurrence_world(&s.doc, occ), solidcore_record::Vec3::new(52.0 + 32.0, 0.0, 4.0));
    let t = (-20f64).to_radians();
    assert!((p.x - (52.0 + 32.0 * t.cos())).abs() < 1e-6 && (p.y - 32.0 * t.sin()).abs() < 1e-6, "{p:?}");
    // One undo takes the gears, their components and joints away.
    let made: Vec<u64> = r["assembly"]["members"].as_array().unwrap().iter().filter_map(|m| m["component"].as_u64()).collect();
    // (After the undo of the drive: joint.drive is a step of its own.)
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert!(!s.doc.components.iter().any(|c| made.contains(&c.id)));
    assert!(s.doc.assembly.joints.is_empty() && s.doc.assembly.links.is_empty());
}

#[test]
fn an_assembled_planetary_turns_its_planets_and_ring() {
    let mut s = Session::default();
    let r = assembled(&mut s, json!({"type": "planetary", "module": 2, "teeth": 18, "teeth2": 18, "planets": 3, "face_width": 8, "assembly": true}));
    let j = member_joints(&r);
    assert_eq!(j.len(), 5, "sun, three planets, ring: {r}");
    let (sun, planet, ring) = (j[0].unwrap(), j[1].unwrap(), j[4].unwrap());
    // The carrier is held: the sun at 36° turns each planet -36° and the ring -12°.
    let p = drive_and_read(&mut s, sun, 36.0, planet);
    assert!((p + 36.0).abs() < 1e-6, "{p}");
    for k in [j[2].unwrap(), j[3].unwrap()] {
        assert!((joint_value(&mut s, k) + 36.0).abs() < 1e-6);
    }
    assert!((joint_value(&mut s, ring) + 12.0).abs() < 1e-6, "{}", joint_value(&mut s, ring));
}

#[test]
fn an_assembled_bevel_set_and_its_keep_out() {
    let mut s = Session::default();
    let r = assembled(&mut s, json!({"type": "bevel", "module": 2, "teeth": 20, "teeth2": 30, "face_width": 10, "assembly": true, "clearance_body": {}}));
    let j = member_joints(&r);
    let got = drive_and_read(&mut s, j[0].unwrap(), 30.0, j[1].unwrap());
    assert!((got + 20.0).abs() < 1e-6, "{got}");
    // The keep-out body is a grounded component of its own, named after the set.
    let k = r["assembly"]["keep_out"].as_u64().expect("a keep-out component");
    let c = s.doc.components.iter().find(|c| c.id == k).unwrap();
    assert!(c.name.ends_with("keep-out"), "{}", c.name);
    let o = s.doc.occurrence_of(k).unwrap();
    assert!(o.grounded);
    assert!(r["assembly"]["members"].as_array().unwrap().iter().all(|m| m["grounded"] == json!(false)));
}

#[test]
fn an_assembled_worm_drives_its_wheel_by_starts_over_teeth() {
    let mut s = Session::default();
    let r = assembled(&mut s, json!({"type": "worm", "module": 2, "teeth": 30, "starts": 1, "face_width": 12, "assembly": true}));
    let j = member_joints(&r);
    // The wheel is body 1, the worm body 2: a right-hand worm turned 90° carries the wheel back 3°.
    let (wheel, worm) = (j[0].unwrap(), j[1].unwrap());
    let got = drive_and_read(&mut s, worm, 90.0, wheel);
    assert!((got + 3.0).abs() < 1e-6, "{got}");
}

#[test]
fn an_update_keeps_the_assemblys_components_joints_and_links_in_place() {
    let mut s = Session::default();
    let counts = |s: &Session| (s.doc.components.len(), s.doc.assembly.joints.len(), s.doc.assembly.links.len());
    let empty = counts(&s);
    let r = assembled(&mut s, json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 32, "face_width": 8, "assembly": true, "clearance_body": {}}));
    let group = r["group"].as_u64().unwrap();
    let before = counts(&s);
    assert_eq!(before, (empty.0 + 3, empty.1 + 2, empty.2 + 1), "two gears and the keep-out; two joints; one link");
    // (A member's body name and shaft follow the new tooth count; its component, occurrence
    // and joint stay.)
    let ids = |a: &Value| {
        let members: Vec<(Value, Value, Value)> = a["members"].as_array().unwrap().iter().map(|m| (m["component"].clone(), m["occurrence"].clone(), m["joint"].clone())).collect();
        (members, a["keep_out"].clone(), a["joints"].clone())
    };
    let made = ids(&r["assembly"]);
    // In place (the same steps) and rebuilt (another tooth count's steps): the same components,
    // occurrences, joints and keep-out each time, the link given the new ratio.
    for (p, teeth2) in [(json!({"group": group, "face_width": 10}), 32.0), (json!({"group": group, "teeth2": 40}), 40.0)] {
        let u = run(&mut s, "gear.update", p);
        no_errors(&s);
        assert_eq!(counts(&s), before, "{u}");
        assert_eq!(ids(&u["assembly"]), made, "{u}");
        let j = member_joints(&u);
        // (By 40° from where the joints stand: an update keeps their values.)
        let (a0, b0) = (joint_value(&mut s, j[0].unwrap()), joint_value(&mut s, j[1].unwrap()));
        let got = drive_and_read(&mut s, j[0].unwrap(), a0 + 40.0, j[1].unwrap()) - b0;
        assert!((got + 40.0 * 20.0 / teeth2).abs() < 1e-6, "{got} for {teeth2} teeth: {u}");
    }
    // Taking the assembly away leaves the gears as bodies again.
    run(&mut s, "gear.update", json!({"group": group, "assembly": null}));
    assert_eq!(counts(&s), empty);
    no_errors(&s);
}

/// Bevel teeth end on their back and front cones: the kernel takes the trim (a revolved blank
/// intersected with the teeth's taper) on ordinary and nearly flat cones, straight and spiral,
/// with gear 2's own face width and heel offset (slow: run with --ignored).
#[test]
#[ignore]
fn bevel_teeth_are_trimmed_by_their_cones() {
    for p in [
        json!({"type": "bevel"}),
        json!({"type": "spiral_bevel"}),
        json!({"type": "spiral_bevel", "module": 2.5, "teeth": 17, "teeth2": 34, "shaft_angle": 110}),
        json!({"type": "bevel", "module": 2.5, "teeth": 17, "teeth2": 34, "shaft_angle": 100}),
        json!({"type": "bevel", "face_width2": 8, "face_offset2": 1.5}),
    ] {
        let mut s = Session::default();
        let r = run(&mut s, "gear.create", p.clone());
        assert_eq!(r["bodies"].as_array().map_or(0, Vec::len), 2, "{p}: {r}");
        let trim_failed = r["warnings"].as_array().is_some_and(|w| w.iter().any(|x| x.as_str().is_some_and(|x| x.contains("could not be trimmed"))));
        assert!(!trim_failed, "{p}: {r}");
    }
}

/// A joint the user made from a part of their own to gear 1 is still there, on gear 1, after
/// the set is edited, and the set still turns by its new ratio.
#[test]
fn a_users_joint_to_an_assembled_gear_survives_an_update() {
    let mut s = Session::default();
    let r = assembled(&mut s, json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 32, "face_width": 8, "assembly": true}));
    let group = r["group"].as_u64().unwrap();
    let gear1 = r["assembly"]["members"][0]["occurrence"].as_u64().unwrap();
    // A bracket of the user's, fixed to gear 1's front face at its centre.
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "component.create", json!({"name": "Bracket", "grounded": false}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 4, "corner": [-5, -5, 8]}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    let bracket = s.doc.occurrence_of(s.doc.find_component("Bracket").unwrap()).unwrap().id;
    let at = |o: u64| json!({"occurrence": o, "point": [0, 0, 8], "z": [0, 0, 1]});
    let mine = run(&mut s, "joint.create", json!({"type": "rigid", "a": at(gear1), "b": at(bracket), "flip": true, "name": "Bracket on gear 1"}))["joint"].as_u64().unwrap();
    // More teeth on gear 1: a bigger gear, gear 2 further out.
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth": 24}));
    no_errors(&s);
    let j = s.doc.assembly.joints.iter().find(|j| j.id == mine).unwrap_or_else(|| panic!("the user's joint is gone: {u}"));
    assert!(j.a.occurrence == gear1 || j.b.occurrence == gear1, "{u}");
    assert_eq!(u["assembly"]["members"][0]["occurrence"].as_u64(), Some(gear1));
    assert!(!u["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap_or_default().contains("gone")), "{u}");
    let jj = member_joints(&u);
    let got = drive_and_read(&mut s, jj[0].unwrap(), 32.0, jj[1].unwrap());
    assert!((got + 24.0).abs() < 1e-6, "{got}");
}

/// Bevels up to a 70° pitch cone (their teeth ending on planes square to the axis) pass the
/// kernel's own check of every step; the flatter ones, trimmed by their back and front cones,
/// are refused by SMLib only for near-cusps in the boolean's 2D curves ("Bad CurveCusp") and
/// nothing else (slow: run with --ignored).
#[test]
#[ignore]
fn bevel_bodies_pass_the_kernels_check() {
    let check = |p: Value| -> std::result::Result<(), String> {
        let mut s = Session::default();
        run(&mut s, "diagnostics.validation", json!({"mode": "each_step"}));
        run(&mut s, "gear.create", p.clone());
        // (A feature the check refuses fails the command or is listed in its reply.)
        match s.execute("timeline.compute_all", &json!({})) {
            Err(e) => Err(e.to_string()),
            Ok(r) => {
                let text = r.to_string();
                if text.contains("SMLib") { Err(text) } else { Ok(()) }
            }
        }
    };
    for p in [
        json!({"type": "bevel"}),
        json!({"type": "spiral_bevel"}),
        json!({"type": "bevel", "face_width2": 8, "face_offset2": 1.5}),
        json!({"type": "bevel", "shaft_angle": 60, "teeth": 16, "teeth2": 40}),
    ] {
        check(p.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    let p = json!({"type": "bevel", "module": 2.5, "teeth": 17, "teeth2": 34, "shaft_angle": 100, "only": 2});
    if let Err(e) = check(p.clone()) {
        assert!(e.lines().filter(|l| l.contains("Test #")).all(|l| l.contains("CurveCusp")), "{p}: {e}");
    }
}

// ---------------------------------------------------------------------------------------------
// Names after an update: the generator's follow the set, the user's stay
// ---------------------------------------------------------------------------------------------

/// A gear still called what the generator named it takes the new tooth count's name, and what
/// names it follows: a user's cut through it, and (as an assembly) its component and the
/// component's placement. One undo step takes the names back with the rest.
#[test]
fn an_update_renames_generated_bodies_and_what_names_them() {
    // Without the user's hole, for comparison.
    let mut plain = Session::default();
    run(&mut plain, "gear.create", json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 40, "face_width": 8}));
    let uncut = volume(&mut plain, "Gear z40");

    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 32, "face_width": 8}));
    let group = r["group"].as_u64().unwrap();
    assert_eq!(r["bodies"], json!(["Gear z20", "Gear z32"]));
    // The user's feature on gear 2, by its name: a hole through its centre (x = 52, then 60).
    run(&mut s, "solid.cylinder", json!({"base": [0, 0, -1], "axis": [0, 0, 1], "radius": 3, "height": 10, "operation": "cut", "targets": ["Gear z20"]}));
    no_errors(&s);
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth2": 40}));
    no_errors(&s);
    assert_eq!(u["bodies"], json!(["Gear z20", "Gear z40"]), "{u}");
    assert!(s.model.state().body("Gear z40").is_some() && s.model.state().body("Gear z32").is_none());
    assert!((volume(&mut s, "Gear z40") / uncut - 1.0).abs() < 1e-3);
    // Gear 1 more teeth: the cut on it follows the rename.
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth": 24}));
    no_errors(&s);
    assert_eq!(u["bodies"], json!(["Gear z24", "Gear z40"]), "{u}");
    let cut = volume(&mut s, "Gear z24");
    let mut plain = Session::default();
    run(&mut plain, "gear.create", json!({"type": "pair", "module": 2, "teeth": 24, "teeth2": 40, "face_width": 8}));
    let whole = volume(&mut plain, "Gear z24");
    assert!((whole - cut - std::f64::consts::PI * 9.0 * 8.0).abs() < 1.0, "the hole is still cut: {whole} vs {cut}");
    // A second update finds the set by the new name, and keeps it.
    let u = run(&mut s, "gear.update", json!({"body": "Gear z24", "face_width": 8}));
    assert_eq!(u["bodies"], json!(["Gear z24", "Gear z40"]), "{u}");
    // One undo takes a rename back with its update.
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.model.state().body("Gear z20").is_some() && s.model.state().body("Gear z24").is_none());

    // As an assembly: the component and its placement follow, joints stay.
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 32, "face_width": 8, "assembly": true}));
    let group = r["group"].as_u64().unwrap();
    let c2 = r["assembly"]["members"][1]["component"].as_u64().unwrap();
    let joints = r["assembly"]["joints"].clone();
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth2": 40}));
    no_errors(&s);
    assert_eq!(u["bodies"], json!(["Gear z20", "Gear z40"]), "{u}");
    assert_eq!(s.doc.components.iter().find(|c| c.id == c2).unwrap().name, "Gear z40");
    assert_eq!(s.doc.occurrence_of(c2).unwrap().name, "Gear z40:1");
    assert_eq!(u["assembly"]["joints"], joints);
    assert_eq!(u["assembly"]["members"][1]["component"].as_u64(), Some(c2));
    // A component the user renamed keeps its name.
    run(&mut s, "component.rename", json!({"component": c2, "name": "Output"}));
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth2": 36}));
    assert_eq!(u["bodies"][1], "Gear z36", "{u}");
    assert_eq!(s.doc.components.iter().find(|c| c.id == c2).unwrap().name, "Output");
}

/// A body the user renamed keeps its name through updates; its mate, still the generator's,
/// follows its tooth count.
#[test]
fn a_users_body_name_stays_through_an_update() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "module": 2, "teeth": 20, "teeth2": 32, "face_width": 8}));
    let group = r["group"].as_u64().unwrap();
    run(&mut s, "body.rename", json!({"body": "Gear z20", "name": "Drive gear"}));
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth": 24, "teeth2": 40}));
    no_errors(&s);
    assert_eq!(u["bodies"], json!(["Drive gear", "Gear z40"]), "{u}");
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth": 18}));
    assert_eq!(u["bodies"], json!(["Drive gear", "Gear z40"]), "{u}");
    // Two equal gears: the second stays "(2)" when its count is unchanged.
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "pair", "teeth": 20, "teeth2": 20}));
    let group = r["group"].as_u64().unwrap();
    let u = run(&mut s, "gear.update", json!({"group": group, "teeth": 24}));
    assert_eq!(u["bodies"], json!(["Gear z24", "Gear z20 (2)"]), "{u}");
    assert!(!u["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap_or_default().contains("keeps its name")), "{u}");
}

/// Selected shafts (sketch points, a circle, an axis) and a bearing on one: a train on the
/// shafts, the bearing a keep-out seat and, with assembly, two ring bodies.
#[test]
fn selected_shafts_and_a_bearing_make_a_train_with_a_bearing() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[("sketch.point", json!({"point": [0, 0], "id": "p1"})), ("sketch.point", json!({"point": [50, 0], "id": "p2"})), circle(50.0, 45.0, 15.0, "c3")]);
    let sel = json!([
        {"sketch": sk, "point": "p1", "pitch_diameter": 40},
        {"sketch": sk, "point": "p2"},
        {"sketch": sk, "curve": "c3"},
        {"axis": {"origin": [0, 0, 0], "dir": [0, 0, 1]}, "bearing": "608", "along": -15}
    ]);
    let r = run(&mut s, "gear.calculate", json!({"selection": sel, "type": "spur"}));
    assert!(fact_opt(&r, "train ratio gear 1 → gear 3").is_some(), "{r}");
    let r = run(&mut s, "gear.create", json!({"selection": sel, "type": "spur", "assembly": true}));
    // Three gears, the bearing's outer and inner ring, and the keep-out body.
    assert!(r["bodies"].as_array().unwrap().len() >= 5, "{r}");
    // One shaft: a gear on it.
    run(&mut s, "gear.calculate", json!({"selection": [{"axis": "Z", "pitch_diameter": 40}]}));
    // Clear errors.
    let e = s.execute("gear.calculate", &json!({"selection": [{"axis": "Z", "face": [0, 0, 0]}]})).unwrap_err().to_string();
    assert!(e.contains("exactly one of"), "{e}");
    let e = s.execute("gear.calculate", &json!({"selection": [{"axis": "Z"}, {"axis": "X", "bearing": "nope"}]})).unwrap_err().to_string();
    assert!(e.contains("no standard bearing"), "{e}");
}

// ---------------------------------------------------------------------------------------------
// A whole sketch-driven system: ten gears, two compound clusters, one assembly
// ---------------------------------------------------------------------------------------------

/// Ten circles (module 2: radius = teeth): 1 drives 2; 2 and 3 share a shaft; 3 → 4 → 5; 5 and
/// 6 share a shaft; 6 → 7 → 8 → 9 → 10.
fn ten_gears(s: &mut Session) -> u64 {
    let c = [
        (0.0, 0.0, 20.0),
        (50.0, 0.0, 30.0),
        (50.0, 0.0, 15.0),
        (50.0, 35.0, 20.0),
        (50.0, 80.0, 25.0),
        (50.0, 80.0, 12.0),
        (80.0, 80.0, 18.0),
        (118.0, 80.0, 20.0),
        (118.0, 110.0, 10.0),
        (118.0, 136.0, 16.0),
    ];
    let draw: Vec<(&str, Value)> = c.iter().enumerate().map(|(i, (x, y, r))| circle(*x, *y, *r, &format!("g{}", i + 1))).collect();
    sketch_with(s, &draw)
}

/// Each gear's turn per turn of gear 1, from the teeth by hand.
fn ten_by_hand() -> Vec<f64> {
    let z = [20.0, 30.0, 15.0, 20.0, 25.0, 12.0, 18.0, 20.0, 10.0, 16.0];
    let mut w = vec![1.0; 10];
    for (i, shared) in [(1, false), (2, true), (3, false), (4, false), (5, true), (6, false), (7, false), (8, false), (9, false)] {
        w[i] = if shared { w[i - 1] } else { -w[i - 1] * z[i - 1] / z[i] };
    }
    w
}

/// How far member `i`'s component has turned about its own shaft (degrees, right hand about the
/// shaft's direction), read back from where the engine has placed it (its components start
/// unturned).
fn turned_about(s: &Session, r: &Value, i: usize) -> f64 {
    let m = &r["assembly"]["members"][i];
    let occ = m["occurrence"].as_u64().unwrap();
    let d: Vec<f64> = m["axis"][1].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let d = solidcore_record::Vec3::new(d[0], d[1], d[2]);
    let x0 = if d.z.abs() < 0.9 { d.cross(solidcore_record::Vec3::Z) } else { d.cross(solidcore_record::Vec3::X) };
    let x0 = x0 * (1.0 / x0.len());
    let x1 = solidcraft_doc::apply_vector(&solidcraft_doc::joints::occurrence_world(&s.doc, occ), x0);
    x0.cross(x1).dot(d).atan2(x0.dot(x1)).to_degrees()
}

/// The member whose shaft runs through the point (x, y) of the XY plane (a circle's centre).
fn member_on(r: &Value, x: f64, y: f64) -> usize {
    let ms = r["assembly"]["members"].as_array().unwrap();
    ms.iter()
        .position(|m| {
            let o = &m["axis"][0];
            (o[0].as_f64().unwrap() - x).abs() < 1e-6 && (o[1].as_f64().unwrap() - y).abs() < 1e-6
        })
        .unwrap_or_else(|| panic!("no member on ({x}, {y}): {r}"))
}

/// Angles in degrees, equal round a turn within 1e-6 relative (of the larger, at least 1°).
fn same_angle(got: f64, want: f64) -> bool {
    let d = ((got - want + 540.0).rem_euclid(360.0)) - 180.0;
    d.abs() <= 1e-6 * want.abs().max(1.0)
}

/// The kinds of `r`'s joints, as joint.list names them: (revolute, rigid, other).
fn joint_kinds(s: &mut Session, r: &Value) -> (usize, usize, usize) {
    let l = run(s, "joint.list", json!({}));
    let ours: Vec<u64> = r["assembly"]["joints"].as_array().unwrap().iter().filter_map(Value::as_u64).collect();
    let kind_of = |id: u64| l["joints"].as_array().unwrap().iter().find(|j| j["id"].as_u64() == Some(id)).map(|j| j["type"].as_str().unwrap_or_default().to_lowercase()).unwrap_or_default();
    let (mut rev, mut rig, mut other) = (0, 0, 0);
    for k in ours.iter().map(|j| kind_of(*j)) {
        match k.as_str() {
            "revolute" => rev += 1,
            "rigid" => rig += 1,
            _ => other += 1,
        }
    }
    (rev, rig, other)
}

const TEN: [(f64, f64, f64); 10] = [
    (0.0, 0.0, 20.0),
    (50.0, 0.0, 30.0),
    (50.0, 0.0, 15.0),
    (50.0, 35.0, 20.0),
    (50.0, 80.0, 25.0),
    (50.0, 80.0, 12.0),
    (80.0, 80.0, 18.0),
    (118.0, 80.0, 20.0),
    (118.0, 110.0, 10.0),
    (118.0, 136.0, 16.0),
];

/// Gate: ten gears with two compound clusters. One revolute per shaft (eight), each gear turned
/// by the engine to its tooth-count angle, from gear 1 and from gear 7.
#[test]
fn a_ten_gear_sketch_system_turns_every_gear_from_any_driver() {
    let mut s = Session::default();
    let sk = ten_gears(&mut s);
    let r = assembled(&mut s, json!({"sketch": sk, "type": "spur", "module": 2, "face_width": 6, "assembly": true, "clearance_body": {}}));
    let a = &r["assembly"];
    let n = a["members"].as_array().map_or(0, Vec::len);
    assert!(n == 8 || n == 10, "one member per shaft (or per gear, clusters rigid): {r}");
    assert_eq!(a["driver"], 1);
    assert!(a.get("issues").is_none(), "{r}");
    let (rev, rig, other) = joint_kinds(&mut s, &r);
    assert_eq!((rev, rig, other), (8, n - 8, 0), "one revolute per shaft");
    assert_eq!(a["links"].as_array().map(Vec::len), Some(7), "a gear link per mesh: {r}");
    let j = member_joints(&r);
    let driver = member_on(&r, 0.0, 0.0);
    run(&mut s, "joint.drive", json!({"joint": j[driver].unwrap(), "value": 30}));
    let want = ten_by_hand();
    eprintln!("GATE ten-gear train, gear 1 by 30°:");
    for (k, (x, y, _)) in TEN.iter().enumerate() {
        let got = turned_about(&s, &r, member_on(&r, *x, *y));
        eprintln!("GATE   gear {:2}: expected {:+.6}°, measured {:+.6}°", k + 1, 30.0 * want[k], got);
        assert!(same_angle(got, 30.0 * want[k]), "gear {}: turned {got}°, by the teeth {}°", k + 1, 30.0 * want[k]);
    }
    // Driven from gear 7 instead: its joint drives, gear 1 follows up the train.
    let group = r["group"].as_u64().unwrap();
    let u = run(&mut s, "gear.update", json!({"group": group, "assembly_driver": member_on(&r, 80.0, 80.0) + 1}));
    no_errors(&s);
    let comps = |v: &Value| v["assembly"]["members"].as_array().unwrap().iter().map(|m| m["component"].clone()).collect::<Vec<_>>();
    assert_eq!(comps(&u), comps(&r), "the same components");
    let (g1, g7) = (member_on(&u, 0.0, 0.0), member_on(&u, 80.0, 80.0));
    let (b1, b7) = (turned_about(&s, &u, g1), turned_about(&s, &u, g7));
    let j = member_joints(&u);
    let now = joint_value(&mut s, j[g7].unwrap());
    run(&mut s, "joint.drive", json!({"joint": j[g7].unwrap(), "value": now + 18.0}));
    let (m1, m7) = (turned_about(&s, &u, g1) - b1, turned_about(&s, &u, g7) - b7);
    assert!(same_angle(m7, 18.0), "gear 7 moved {m7}°");
    assert!(same_angle(m1, 18.0 / want[6]), "gear 1 moved {m1}°, expected {}°", 18.0 / want[6]);
}

/// Gate: the user's twin-countershaft gearbox (module 1, diameter = teeth): input Ø30, Ø15 + Ø25
/// on each countershaft, one Ø15 output meshing both Ø25s. The loop agrees; the input by 30°
/// turns each countershaft −60° and the output +100°.
#[test]
fn the_gaerbox_power_split_turns_by_its_teeth() {
    // ±45.98° round the input: the two Ø25s clear each other, the output reaches both, and the
    // loop's teeth close (the train planner refuses a loop that isn't a whole number of teeth).
    let a = 45.98f64.to_radians();
    let (cx, cy) = (22.5 * a.cos(), 22.5 * a.sin());
    let x0 = cx + (400.0 - cy * cy).sqrt();
    let mut s = Session::default();
    let draw = [
        circle(0.0, 0.0, 15.0, "in"),
        circle(cx, cy, 7.5, "a15"),
        circle(cx, cy, 12.5, "a25"),
        circle(cx, -cy, 7.5, "b15"),
        circle(cx, -cy, 12.5, "b25"),
        circle(x0, 0.0, 7.5, "out"),
    ];
    let sk = sketch_with(&mut s, &draw);
    let r = assembled(&mut s, json!({"sketch": sk, "type": "spur", "module": 1, "face_width": 5, "assembly": true, "clearance_body": {}}));
    let am = &r["assembly"];
    assert!(am.get("issues").is_none(), "a power split agrees: {r}");
    let n = am["members"].as_array().map_or(0, Vec::len);
    let (rev, rig, other) = joint_kinds(&mut s, &r);
    assert_eq!((rev, rig, other), (4, n - 4, 0), "input, countershaft A, countershaft B, output: {r}");
    // Three of the four meshes as motion links: the joints' links drive each joint once, so the
    // mesh closing the loop holds by the ratios (checked below by the output's turn).
    assert_eq!(am["links"].as_array().map(Vec::len), Some(3), "{r}");
    let j = member_joints(&r);
    let input = member_on(&r, 0.0, 0.0);
    run(&mut s, "joint.drive", json!({"joint": j[input].unwrap(), "value": 30}));
    eprintln!("GATE gaerbox, input by 30°:");
    for (what, x, y, want) in [("input", 0.0, 0.0, 30.0), ("countershaft A", cx, cy, -60.0), ("countershaft B", cx, -cy, -60.0), ("output", x0, 0.0, 100.0)] {
        let got = turned_about(&s, &r, member_on(&r, x, y));
        eprintln!("GATE   {what}: expected {want:+.6}°, measured {got:+.6}°");
        assert!(same_angle(got, want), "{what}: {got}° vs {want}°");
    }
}

/// Gate: a pinion inside a ring (drawn as circles): the ring turns the same way by 20/60.
#[test]
fn an_internal_mesh_turns_the_ring_the_same_way() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 60.0, "ring"), circle(40.0, 0.0, 20.0, "pinion")]);
    let r = assembled(&mut s, json!({"sketch": sk, "type": "spur", "module": 2, "face_width": 6, "assembly": true}));
    let (ring, pinion) = (member_on(&r, 0.0, 0.0), member_on(&r, 40.0, 0.0));
    let j = member_joints(&r);
    // The motion links turn the driven member from the driver (member 1, here the ring drawn
    // first): driven, the ring by 10° turns the pinion 30° the same way.
    let driver = r["assembly"]["driver"].as_u64().unwrap() as usize - 1;
    let (moved, read, by, want) = if driver == pinion { (pinion, ring, 30.0, 10.0) } else { (ring, pinion, 10.0, 30.0) };
    run(&mut s, "joint.drive", json!({"joint": j[moved].unwrap(), "value": by}));
    let got = turned_about(&s, &r, read);
    eprintln!("GATE internal, z20 in z60, member {moved} by {by}°: expected {want:+}°, measured {got:+.6}°");
    assert!(same_angle(got, want), "{got}");
}

/// Gate: bevels at 90° and 70°, gear 1 z20 by 30°: gear 2 z30 turns −20° about its own axis
/// (both axes pointing at the apex), the engine's joint angle read back.
#[test]
fn bevels_at_any_shaft_angle_turn_by_their_teeth() {
    for angle in [90.0, 70.0] {
        let mut s = Session::default();
        let r = assembled(&mut s, json!({"type": "bevel", "module": 2, "teeth": 20, "teeth2": 30, "face_width": 10, "shaft_angle": angle, "assembly": true}));
        let j = member_joints(&r);
        run(&mut s, "joint.drive", json!({"joint": j[0].unwrap(), "value": 30}));
        let got = turned_about(&s, &r, 1);
        eprintln!("GATE bevel {angle}°, gear 1 z20 by 30°: gear 2 z30 expected -20°, measured {got:+.6}°");
        assert!(same_angle(got, -20.0), "{angle}°: {got}");
        assert!(same_angle(turned_about(&s, &r, 0), 30.0));
    }
}

/// Gate: an open differential with its case held (the drive pinion at rest): side gear A by
/// 30° turns side gear B the other way by as much, about the axle.
#[test]
fn an_open_differential_turns_its_sides_opposite_with_the_case_held() {
    let mut s = Session::default();
    let r = run(&mut s, "gear.create", json!({"type": "differential", "assembly": true}));
    no_errors(&s);
    let ms = r["assembly"]["members"].as_array().unwrap().clone();
    let find = |part: &str| ms.iter().position(|m| m["body"].as_str().unwrap_or_default().contains(part)).unwrap_or_else(|| panic!("no {part}: {r}"));
    let (a, b) = (find("Side gear A"), find("Side gear B"));
    let j = member_joints(&r);
    run(&mut s, "joint.drive", json!({"joint": j[a].unwrap(), "value": 30}));
    // Side B's turn about side A's axle direction.
    let axis_a = r["assembly"]["members"][a]["axis"][1].clone();
    let axis_b = r["assembly"]["members"][b]["axis"][1].clone();
    let along = (0..3).map(|k| axis_a[k].as_f64().unwrap() * axis_b[k].as_f64().unwrap()).sum::<f64>().signum();
    let got = turned_about(&s, &r, b) * along;
    eprintln!("GATE open differential, case held, side A by 30°: side B expected -30°, measured {got:+.6}°");
    assert!(same_angle(got, -30.0), "{got}: {r}");
}

#[test]
fn adding_a_gear_to_the_sketch_keeps_the_systems_components_and_joints() {
    let mut s = Session::default();
    let sk = ten_gears(&mut s);
    let r = assembled(&mut s, json!({"sketch": sk, "type": "spur", "module": 2, "face_width": 6, "assembly": true}));
    let group = r["group"].as_u64().unwrap();
    // An eleventh gear meshing gear 10.
    run(&mut s, "sketch.edit", json!({"sketch": sk}));
    run(&mut s, "sketch.circle.center", json!({"center": [148, 136], "radius": 14, "id": "g11"}));
    run(&mut s, "sketch.finish", json!({}));
    let u = run(&mut s, "gear.update", json!({"group": group}));
    no_errors(&s);
    let before = r["assembly"]["members"].as_array().unwrap().clone();
    let members = u["assembly"]["members"].as_array().unwrap().clone();
    assert_eq!(members.len(), before.len() + 1, "{u}");
    // Every shaft that was there keeps its component and joint (found by where it turns).
    for m in &before {
        let o = &m["axis"][0];
        let (x, y) = (o[0].as_f64().unwrap(), o[1].as_f64().unwrap());
        let now = &members[member_on(&u, x, y)];
        assert_eq!((&now["component"], &now["joint"]), (&m["component"], &m["joint"]), "the shaft at ({x}, {y}) keeps its component and joint: {u}");
    }
    assert_eq!(u["assembly"]["links"].as_array().map(Vec::len), Some(8), "{u}");
    let new = member_on(&u, 148.0, 136.0);
    // And taken away again: the eleventh's component and joint go, the rest stay.
    run(&mut s, "sketch.edit", json!({"sketch": sk}));
    run(&mut s, "sketch.delete", json!({"entities": ["g11"]}));
    run(&mut s, "sketch.finish", json!({}));
    let v = run(&mut s, "gear.update", json!({"group": group}));
    no_errors(&s);
    assert_eq!(v["assembly"]["members"].as_array().map(Vec::len), Some(before.len()), "{v}");
    let gone = members[new]["component"].as_u64().unwrap();
    assert!(!s.doc.components.iter().any(|c| c.id == gone));
}

// ---- The sketch-driven reader: construction geometry, members, 3D sketches, clusters.

/// The user's build 6 sketch: three circles, the first two touching, the third touching nothing,
/// and construction lines joining the centres. Construction geometry is never a gear or shaft;
/// the circle meshing with nothing is named, member by member.
#[test]
fn build_6_construction_lines_are_left_out_and_the_loose_circle_named() {
    let mut s = Session::default();
    let (c2, c3) = ([-10.180755625393433, 20.064949911424925], [12.062125004376336, 21.214432234647948]);
    let sk = sketch_with(
        &mut s,
        &[
            circle(0.0, 0.0, 15.0, "c1"),
            circle(c2[0], c2[1], 7.5, "c2"),
            circle(c3[0], c3[1], 7.5, "c3"),
            ("sketch.line", json!({"points": [[0, 0], c2], "construction": true, "ids": ["l1"]})),
            ("sketch.line", json!({"points": [c2, c3], "construction": true, "ids": ["l2"]})),
            ("sketch.line", json!({"points": [[0, 0], c3], "construction": true, "ids": ["l3"]})),
        ],
    );
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "type": "spur"}));
    let members = r["members"].as_array().unwrap_or_else(|| panic!("{r}"));
    assert_eq!(members.len(), 3, "{r}");
    assert!(members[0]["error"].is_null() && members[1]["error"].is_null(), "{r}");
    assert_eq!(members[0]["meshes_with"], json!([1]), "{r}");
    assert!(members[2]["error"].as_str().unwrap_or_default().contains("circle c3 touches no other gear"), "{r}");
    let e = s.execute("gear.create", &json!({"sketch": sk, "type": "spur"})).unwrap_err().to_string();
    assert!(e.contains("circle c3 touches no other gear") && !e.contains("rack"), "{e}");
}

/// A 3D sketch of a drivetrain: three shaft lines, gear 1 on the first meshing gear 2 on the
/// second, gear 3 on the second further along (a cluster with gear 2) meshing gear 4 on the
/// third, and a bearing point on the first shaft.
fn drivetrain_3d(s: &mut Session) -> u64 {
    let arc = |c: [f64; 3], r: f64| ("sketch.arc3d", json!({"start": [c[0] + r, c[1], c[2]], "through": [c[0], c[1] + r, c[2]], "end": [c[0] - r, c[1], c[2]]}));
    sketch_with(
        s,
        &[
            ("sketch.options", json!({"sketch_3d": true})),
            ("sketch.line3d", json!({"points": [[0, 0, -10], [0, 0, 40]]})),
            ("sketch.line3d", json!({"points": [[50, 0, -10], [50, 0, 40]]})),
            // (Starts above gear 2: down to z −10 the Ø18 shaft would run through its teeth.)
            ("sketch.line3d", json!({"points": [[85, 0, 10], [85, 0, 40]]})),
            arc([0.0, 0.0, 0.0], 20.0),
            arc([50.0, 0.0, 0.0], 30.0),
            arc([50.0, 0.0, 20.0], 15.0),
            arc([85.0, 0.0, 20.0], 20.0),
            ("sketch.point3d", json!({"point": [0, 0, -5]})),
        ],
    )
}

#[test]
fn a_3d_sketch_drivetrain_with_a_cluster_and_a_bearing() {
    let mut s = Session::default();
    let sk = drivetrain_3d(&mut s);
    let r = run(&mut s, "gear.calculate", json!({"sketch": sk, "face_width": 8, "clearance_body": {"bearing": "608"}}));
    let m = r["members"].as_array().unwrap_or_else(|| panic!("{r}"));
    // Circles first (members 1–4), then the lines, then the point.
    assert_eq!(m.len(), 8, "{r}");
    assert!(m.iter().all(|m| m["error"].is_null()), "{r}");
    assert!(m[1]["cluster"].is_number() && m[1]["cluster"] == m[2]["cluster"], "{r}");
    assert_eq!((m[0]["meshes_with"].clone(), m[2]["meshes_with"].clone()), (json!([1]), json!([3])), "{r}");
    assert_eq!((m[7]["role"].as_str(), m[7]["shaft"].as_u64()), (Some("bearing"), Some(4)), "{r}");
    let r = run(&mut s, "gear.create", json!({"sketch": sk, "face_width": 8, "clearance_body": {"bearing": "608"}, "assembly": true}));
    assert!(r["bodies"].as_array().unwrap().len() >= 4, "{r}");
    assert!(r["members"].as_array().is_some_and(|m| m.len() == 8), "{r}");
}

/// Circles of a flat sketch selected with roles and per-member sizes: each member's numbers back.
#[test]
fn selected_circles_with_roles_and_sizes_report_each_member() {
    let mut s = Session::default();
    let sk = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g1"), circle(50.0, 0.0, 30.0, "g2"), circle(50.0, 45.0, 15.0, "g3")]);
    let sel = json!([
        {"sketch": sk, "curve": "g1", "role": "spur", "module": 2.5},
        {"sketch": sk, "curve": "g2", "role": "spur"},
        {"sketch": sk, "curve": "g3", "role": "spur", "face_width": 6}
    ]);
    let r = run(&mut s, "gear.calculate", json!({"selection": sel}));
    let m = r["members"].as_array().unwrap_or_else(|| panic!("{r}"));
    assert_eq!(m.iter().map(|m| m["teeth"].as_u64().unwrap()).collect::<Vec<_>>(), vec![16, 24, 12], "{r}");
    assert_eq!(m[2]["face_width"].as_f64(), Some(6.0), "{r}");
    // A module that doesn't fit a circle: that member's error.
    let sel = json!([{"sketch": sk, "curve": "g1", "role": "spur", "teeth": 18, "module": 2.5}, {"sketch": sk, "curve": "g2"}]);
    let r = run(&mut s, "gear.calculate", json!({"selection": sel}));
    // Decision 1: no refusal; the drawn centre distance is held by profile shift, and said so.
    assert!(r["members"][0]["error"].is_null(), "{r}");
    assert!(r["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap_or_default().contains("18 teeth at module 2.5")), "{r}");
    // A bearing on no shaft line.
    let sel = json!([{"sketch": sk, "curve": "g1"}, {"sketch": sk, "curve": "g2"}, {"axis": {"origin": [0, 0, 0], "dir": [0, 0, 1]}, "role": "shaft"}, {"sketch": sk, "point": "origin", "role": "bearing", "bearing": "608"}]);
    let r = run(&mut s, "gear.calculate", json!({"selection": sel}));
    assert!(r["members"].as_array().is_some(), "{r}");
}

/// One gear system in three ordinary sketches: gears 1 and 2 on XY, gears 3 and 4 on a plane
/// 20 mm up (gear 3 on gear 2's axis), the shared shaft line in a sketch on XZ. Read together
/// (`sketch` a list): a compound of gears 2 and 3; the set follows all three sketches.
#[test]
fn a_compound_from_two_sketches_on_parallel_planes_and_a_shaft_in_a_third() {
    let mut s = Session::default();
    let low = sketch_with(&mut s, &[circle(0.0, 0.0, 20.0, "g1"), circle(50.0, 0.0, 30.0, "g2")]);
    let high = run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 20], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.circle.center", json!({"center": [50, 0], "radius": 15, "id": "g3"}));
    run(&mut s, "sketch.circle.center", json!({"center": [85, 0], "radius": 20, "id": "g4"}));
    run(&mut s, "sketch.finish", json!({}));
    // XZ: sketch y along world -Z.
    let side = run(&mut s, "sketch.create", json!({"plane": "XZ"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.line", json!({"points": [[50, 10], [50, -40]], "ids": ["shaft"]}));
    run(&mut s, "sketch.finish", json!({}));
    let ask = json!({"sketch": [low, high, side], "face_width": 8});
    let r = run(&mut s, "gear.calculate", ask.clone());
    let m = r["members"].as_array().unwrap_or_else(|| panic!("{r}"));
    assert_eq!(m.len(), 5, "{r}");
    assert!(m.iter().all(|m| m["error"].is_null()), "{r}");
    assert!(m[1]["cluster"].is_number() && m[1]["cluster"] == m[2]["cluster"] && m[0]["cluster"].is_null(), "{r}");
    assert_eq!((m[1]["shaft"].as_u64(), m[2]["shaft"].as_u64()), (Some(4), Some(4)), "{r}");
    let made = run(&mut s, "gear.create", ask);
    let group = made["group"].as_u64().unwrap();
    // Linked to every sketch it read: moving gear 4's circle in the second regenerates it.
    let st = run(&mut s, "gear.inspect", json!({"group": group}));
    assert_eq!(st["sets"][0]["sketch_changed"], false, "{st}");
}
