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
    g.module = "2 mm".into();
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].0, "gear.create");
    assert_eq!(c[0].1["sketch"], json!(sk));
    // (Until the train reading lands: two touching circles are a pair.)
    assert_eq!(c[0].1["type"], "pair", "{}", c[0].1);
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
