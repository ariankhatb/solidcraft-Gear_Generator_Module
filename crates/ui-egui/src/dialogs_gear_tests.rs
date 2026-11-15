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

/// Gears is a native dialog: drawn in a sketch, the set is made from it with gear.create
/// {sketch}, follows a native dimension change, and reopens in the dialog to be edited.
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
    // Editing reopens the dialog on the set.
    crate::gear_window::open_group(&mut app, group).unwrap();
    let g = gear(&app);
    assert_eq!((g.group, g.sketch), (Some(group), Some(sk)));
    assert!(app.gear_window.is_none(), "the dialog, not the window");
    let c = apply_commands(&app, app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(c[0].0, "gear.update");
    assert_eq!(c[0].1["group"], json!(group));
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

/// Every family a sketch holds is the same dialog: a bevel's shafts are lines meeting at the
/// apex, their native angle dimension the shaft angle; changing it, the set follows.
#[test]
fn a_bevel_is_made_in_the_dialog_and_follows_its_native_angle() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "bevel", "gear": {"module": 2, "teeth": 20, "teeth2": 30, "shaft_angle": 90}})).unwrap();
    let sk = made["sketch"].as_u64().unwrap();
    app.start("gear.create");
    let mut d = app.dialog.clone().expect("Gears opens a dialog");
    let Kind::Gear(g) = &mut d.kind else { panic!("the Gears dialog") };
    g.kind = TYPES.iter().position(|t| t.0 == GearKind::Bevel).unwrap();
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
    // Reopened, it is the bevel's dialog again.
    crate::gear_window::open_group(&mut app, group).unwrap();
    assert_eq!(gear(&app).gear_kind(), GearKind::Bevel);
}

/// A type a sketch can't hold says so and offers the window; it makes nothing itself.
#[test]
fn a_spline_points_to_the_window() {
    let mut app = app();
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.kind = TYPES.iter().position(|t| t.0 == GearKind::Spline).unwrap();
    let e = apply_commands(&app, &d).unwrap_err();
    assert!(e.contains("Gear Generator window"), "{e}");
}

/// Shafts and bearings picked: the set goes on them (`selection`), the bearings sized from the
/// library; the drawing is not asked for then.
#[test]
fn picked_shafts_and_bearings_make_a_selection() {
    let mut app = app();
    let sk = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.line", &json!({"points": [[0, 0], [0, 40]], "ids": ["s1"]})).unwrap();
    app.session.execute("sketch.line", &json!({"points": [[50, 0], [50, 40]], "ids": ["s2"]})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [0, 10], "id": "b1"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    d.inputs[1].items = vec![Sel::SketchCurve { id: "s1".into(), sketch: Some(sk) }, Sel::SketchCurve { id: "s2".into(), sketch: Some(sk) }];
    d.inputs[2].items = vec![Sel::ExternalSketchPoint { sketch: sk, id: "b1".into() }];
    let c = apply_commands(&app, &d).unwrap();
    let p = &c[0].1;
    assert!(p.get("sketch").is_none(), "{p}");
    let sel = p["selection"].as_array().unwrap();
    assert_eq!(sel.len(), 3, "{p}");
    assert_eq!((sel[0]["curve"].as_str(), sel[0]["role"].as_str()), (Some("s1"), Some("shaft")));
    assert_eq!((sel[2]["point"].as_str(), sel[2]["role"].as_str(), sel[2]["bearing"].as_str()), (Some("b1"), Some("bearing"), Some("608")));
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
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(Default::default(), |ui| {
        rows(&mut app, ui, g, &d.inputs);
    });
    assert!(!g.shape.is_empty(), "the shape to draw: {:?}", g.calc);
    g.teeth = true;
    assert!(g.previews());
}
