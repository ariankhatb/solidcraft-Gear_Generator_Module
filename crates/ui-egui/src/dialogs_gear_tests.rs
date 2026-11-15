use serde_json::json;
use solidcraft_engine::Session;

use super::*;
use crate::Services;
use crate::dialogs::apply_commands;
use crate::test_support::run_all;

fn app() -> SolveApp {
    SolveApp::new(Session::default(), Services::default())
}

fn gear(app: &SolveApp) -> Gear {
    match app.dialog.as_ref().map(|d| &d.kind) {
        Some(Kind::Gear(g)) => g.clone(),
        other => panic!("not the Gears dialog: {other:?}"),
    }
}

fn type_of(k: GearKind) -> usize {
    TYPES.iter().position(|t| t.0 == k).unwrap()
}

/// Gears is a native dialog: drawn in a sketch, the set is made from it with gear.create
/// {sketch}, follows a native dimension change, and Edit Gears reopens the dialog on it.
#[test]
fn the_gears_dialog_makes_a_set_from_a_native_sketch_that_follows_it() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "pair", "gear": {"module": 2, "teeth": 20, "teeth2": 40}})).unwrap();
    let sk = made["sketch"].as_u64().unwrap();
    app.start("gear.create");
    let mut d = app.dialog.clone().expect("Gears opens a dialog");
    let Kind::Gear(g) = &mut d.kind else { panic!("the Gears dialog") };
    g.sketch = Some(sk);
    *g.value_mut("module") = "2 mm".into();
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].0, "gear.create");
    assert_eq!(c[0].1["sketch"], json!(sk));
    // The tooth style: the drawing decides the set (two touching circles, a pair).
    assert_eq!(c[0].1["type"], "spur", "{}", c[0].1);
    run_all(&mut app, c);
    app.dialog = None;
    let sets = solidcraft_engine::gear_sets(&app.session);
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].sketch, Some(sk));
    let group = sets[0].group;
    // A native dimension changed: the set follows.
    let d1 = made["dimensions"]["diameter1"].as_str().unwrap().to_string();
    app.run("parameters.change", json!({"name": d1, "expression": "60"})).unwrap();
    let st = solidcraft_engine::gear_sets(&app.session).into_iter().find(|s| s.group == group).unwrap();
    assert_eq!(st.params["teeth"].as_f64(), Some(30.0), "{}", st.params);
    // Edit Gears reopens the dialog on the set; OK updates it.
    assert!(is_set(&app, group));
    open_set(&mut app, group).unwrap();
    let g = gear(&app);
    assert_eq!((g.group, g.sketch), (Some(group), Some(sk)));
    let c = apply_commands(&app, app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(c[0].0, "gear.update");
    assert_eq!(c[0].1["group"], json!(group));
    assert_eq!(c[0].1["sketch"], json!(sk), "the set's sketch stays: {}", c[0].1);
}

/// Started while a sketch is being edited, Gears finishes it and takes it, as Extrude takes
/// the profile.
#[test]
fn gears_started_in_a_sketch_takes_that_sketch() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "spur", "gear": {"module": 2, "teeth": 24}})).unwrap();
    let sk = made["sketch"].as_u64().unwrap();
    app.edit_sketch(sk);
    assert_eq!(app.session.active_sketch, Some(sk));
    app.start("gear.create");
    assert_eq!(app.session.active_sketch, None);
    assert_eq!(gear(&app).sketch, Some(sk));
    let c = apply_commands(&app, app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(c[0].1["type"], "spur");
}

/// A bevel's shafts are lines meeting at the apex, their native angle dimension the shaft
/// angle; changing it, the set follows; Edit Gears reopens it as a bevel.
#[test]
fn a_bevel_is_made_in_the_dialog_and_follows_its_native_angle() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "bevel", "gear": {"module": 2, "teeth": 20, "teeth2": 30, "shaft_angle": 90}})).unwrap();
    let sk = made["sketch"].as_u64().unwrap();
    app.start("gear.create");
    let mut d = app.dialog.clone().expect("Gears opens a dialog");
    let Kind::Gear(g) = &mut d.kind else { panic!("the Gears dialog") };
    g.kind = type_of(GearKind::Bevel);
    g.sketch = Some(sk);
    for (key, v) in [("module", "2 mm"), ("teeth", "20"), ("teeth2", "30")] {
        *g.value_mut(key) = v.into();
    }
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].1["type"], "bevel");
    assert_eq!(c[0].1["teeth"], json!(20));
    run_all(&mut app, c);
    app.dialog = None;
    let group = solidcraft_engine::gear_sets(&app.session)[0].group;
    let angle = made["dimensions"]["shaft_angle"].as_str().unwrap().to_string();
    app.run("parameters.change", json!({"name": angle, "expression": "75 deg"})).unwrap();
    let st = solidcraft_engine::gear_sets(&app.session).into_iter().find(|s| s.group == group).unwrap();
    assert!((st.params["shaft_angle"].as_f64().unwrap() - 75.0).abs() < 1e-6, "{}", st.params);
    open_set(&mut app, group).unwrap();
    assert_eq!(gear(&app).gear_kind(), GearKind::Bevel);
}

/// The types a sketch can't hold are made from their rows alone; the differential sends its
/// style, its drive and the bodies it makes, as a plain case.
#[test]
fn sketch_free_types_and_the_differential_are_rows() {
    let mut app = app();
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.kind = type_of(GearKind::Spline);
    *g.value_mut("teeth") = "18".into();
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!((c[0].1["type"].as_str(), c[0].1["teeth"].as_u64()), (Some("spline"), Some(18)), "{}", c[0].1);
    assert!(c[0].1.get("sketch").is_none());
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.kind = type_of(GearKind::Differential);
    g.diff_style = 2;
    g.diff_parts[4] = false;
    g.which_pinions = "1, 3".into();
    let c = apply_commands(&app, &d).unwrap();
    let df = &c[0].1["differential"];
    assert_eq!(df["style"], "torsen_b", "{df}");
    assert_eq!((df["windows"].as_bool(), df["ring_bolts"].as_bool()), (Some(false), Some(false)), "a plain case");
    assert_eq!(df["parts"]["case"], false);
    assert_eq!(df["parts"]["which_pinions"], json!([1, 3]));
}

/// Picks are rows, each with its role: circles gears of the dialog's type, a line a shaft, a
/// point on it a bearing; sent as `selection` (roles, locators, the row's index, its size).
/// A point off the picked lines can't be a bearing, and the row says why.
#[test]
fn picks_are_rows_with_roles_sent_as_the_selection() {
    let mut app = app();
    let sk = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    for (id, x, r) in [("c1", 0.0, 20.0), ("c2", 30.0, 10.0)] {
        app.session.execute("sketch.circle.center", &json!({"center": [x, 0], "radius": r, "id": id})).unwrap();
    }
    app.session.execute("sketch.line", &json!({"points": [[0, -40], [0, 40]], "ids": ["s1"]})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [0, 30], "id": "b1"})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [15, 30], "id": "b2"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    let curve = |id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    let point = |id: &str| Sel::ExternalSketchPoint { sketch: sk, id: id.into() };
    d.inputs[0].items = vec![curve("c1"), curve("c2"), curve("s1"), point("b1")];
    let c = apply_commands(&app, &d).unwrap();
    let p = &c[0].1;
    let sel = p["selection"].as_array().unwrap_or_else(|| panic!("{p}"));
    let roles: Vec<&str> = sel.iter().map(|i| i["role"].as_str().unwrap()).collect();
    assert_eq!(roles, ["spur", "spur", "shaft", "bearing"], "{p}");
    assert_eq!((sel[0]["curve"].as_str(), sel[0]["index"].as_u64()), (Some("c1"), Some(0)));
    assert_eq!((sel[3]["point"].as_str(), sel[3]["bearing"].as_str()), (Some("b1"), Some("608")));
    assert!(p.get("sketch").is_none(), "each entry names its own sketch: {p}");
    assert_eq!(sel[0]["sketch"], json!(sk));
    let name = app.session.doc.feature(sk).unwrap().name.clone();
    assert_eq!(row_name(&app, &geo(&app, &curve("c1")), "c1"), format!("c1 · {name}"));
    // Sized by teeth: each gear row its own teeth; the module is left to the engine.
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    *g.value_mut("module") = "2 mm".into();
    g.by_teeth = true;
    g.picks = rows_now(&app, g, &d.inputs).into_iter().map(|(p, _)| p).collect();
    g.picks[0].teeth = "40".into();
    let c = apply_commands(&app, &d).unwrap();
    let sel = c[0].1["selection"].as_array().unwrap().clone();
    assert_eq!(sel[0]["teeth"], json!(40));
    assert!(sel[0].get("module").is_none() && sel[1].get("module").is_none(), "{sel:?}");
    // A point off every picked line: not a bearing.
    d.inputs[0].items.push(point("b2"));
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    let rows = rows_now(&app, g, &d.inputs);
    assert_eq!(rows[4].0.role, UNUSED_ROLE, "a point off the lines is not taken as a bearing");
    let geos: Vec<Geo> = rows.iter().map(|(_, g)| g.clone()).collect();
    assert!(bearing_refused(&geos[4], &geos).is_some_and(|why| why.contains("on a picked shaft line")));
    assert!(bearing_refused(&geos[3], &geos).is_none());
}

/// What `gear.calculate` says per row (`members`) is shown on the rows: the worked-out size,
/// the meshes, the shared shaft of a compound gear, and a row's own error.
#[test]
fn rows_show_what_the_engine_worked_out_for_them() {
    let fixture = json!({"members": [
        {"index": 0, "role": "spur", "module": 2.0, "teeth": 20, "pitch_diameter": 40.0, "face_width": 10.0, "meshes_with": [1], "shaft": 4, "cluster": "A", "error": null},
        {"index": 1, "role": "spur", "module": 2.0, "teeth": 10, "pitch_diameter": 20.0, "face_width": 10.0, "meshes_with": [0], "shaft": null, "cluster": null, "error": null},
        {"index": 2, "role": "spur", "module": null, "teeth": null, "pitch_diameter": 15.0, "face_width": null, "meshes_with": [], "shaft": null, "cluster": null, "error": "circle c3 touches no other gear: make it tangent"}
    ]});
    let m = members_of(&fixture);
    let (line, bad) = member_line(&m[&0]);
    assert!(!bad);
    assert_eq!(line, "m 2 · z 20 · Ø40 · b 10 · meshes 2 · on shaft 5 · compound A");
    let (line, bad) = member_line(&m[&2]);
    assert!(bad && line.contains("circle c3 touches no other gear"), "{line}");
}

/// The preview is the abstract set (shafts and pitch surfaces) drawn in the viewport; the
/// teeth are only previewed when asked for.
#[test]
fn the_preview_is_the_abstract_shape_unless_teeth_are_asked_for() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "pair", "gear": {"module": 2, "teeth": 20, "teeth2": 40}})).unwrap();
    app.start("gear.create");
    let Some(Kind::Gear(g)) = app.dialog.as_mut().map(|d| &mut d.kind) else { panic!() };
    g.sketch = made["sketch"].as_u64();
    assert!(!g.previews(), "no teeth preview by default");
    let mut d = app.dialog.take().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    let mut hover = None;
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(Default::default(), |ui| {
        rows(&mut app, ui, g, &mut d.inputs, &mut hover, &[]);
    });
    assert!(!g.shape.is_empty(), "the shape to draw: {:?}", g.calc);
    g.teeth = true;
    assert!(g.previews());
}

/// A set drawn across several sketches on different planes: each row's entry names its sketch.
#[test]
fn rows_may_come_from_several_sketches() {
    let mut app = app();
    let mut sketch = |plane: &str, circle: (&str, f64)| {
        let sk = app.session.execute("sketch.create", &json!({"plane": plane})).unwrap()["sketch"].as_u64().unwrap();
        app.session.execute("sketch.circle.center", &json!({"center": [0, 0], "radius": circle.1, "id": circle.0})).unwrap();
        app.session.execute("sketch.finish", &json!({})).unwrap();
        sk
    };
    let (a, b) = (sketch("XY", ("g1", 20.0)), sketch("XZ", ("g2", 10.0)));
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    d.inputs[0].items = vec![Sel::SketchCurve { id: "g1".into(), sketch: Some(a) }, Sel::SketchCurve { id: "g2".into(), sketch: Some(b) }];
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.kind = type_of(GearKind::Bevel);
    let c = apply_commands(&app, &d).unwrap();
    let sel = c[0].1["selection"].as_array().unwrap().clone();
    assert_eq!((sel[0]["sketch"].as_u64(), sel[1]["sketch"].as_u64()), (Some(a), Some(b)), "{sel:?}");
    assert_eq!((sel[0]["role"].as_str(), sel[1]["role"].as_str()), (Some("bevel"), Some("bevel")));
}
