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

#[test]
fn unknown_chain_and_belt_are_refused() {
    let mut s = Session::default();
    for p in [json!({"type": "sprocket", "chain": "99x"}), json!({"type": "pulley", "belt": "gt9"}), json!({"type": "sprocket", "strands": 9})] {
        assert!(s.execute("gear.calculate", &p).is_err(), "{p}");
    }
}
