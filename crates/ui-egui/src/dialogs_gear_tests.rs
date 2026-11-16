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
    // Circles are Auto until given a type: the engine works out what each is.
    assert_eq!(roles, ["auto", "auto", "shaft", "bearing"], "{p}");
    assert_eq!(p["shafts"]["generate"], true, "shafts on every gear axis by default: {p}");
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
    assert_eq!(line, "m 2 · z 20 · Ø40 · b 10 · meshes 2 · one body with shaft 5 · compound A");
    let (line, bad) = member_line(&m[&2]);
    assert!(bad && line.contains("circle c3 touches no other gear"), "{line}");
}

/// An Auto row says what the engine made of it and its hand; a generated shaft is a row with
/// its gears; a contact is a point or a line.
#[test]
fn auto_rows_generated_shafts_and_contacts_from_the_engine() {
    let fixture = json!({
        "members": [{"index": 0, "role": "bevel", "inferred": true, "shaft_angle": 90.0, "hand": "LH", "module": 2.0, "teeth": 20, "shaft": 1}],
        "shafts": [{"index": 1, "name": "input", "diameter": null, "gears": [0, 2, 3], "length": 120.0, "line": 4}],
        "contacts": [{"a": [1.0, 2.0, 3.0], "b": [1.0, 2.0, 13.0]}, {"a": [0.0, 0.0, 0.0]}]
    });
    let (line, bad) = member_line(&members_of(&fixture)[&0]);
    assert!(!bad);
    assert!(line.starts_with("Auto → Bevel 90° · LH · m 2 · z 20"), "{line}");
    assert!(line.ends_with("one body with shaft 2"), "{line}");
    let shafts = shafts_of(&fixture);
    assert_eq!(shaft_line(&shafts[0]), "Shaft 2: input, Ø auto, 3 gears, 120 long (row 5)");
    let c = contacts_of(&fixture);
    assert_eq!(c.len(), 2);
    assert_eq!(c[1][0], c[1][1], "a point contact");
}

/// Parallel-axis rows take a helix angle and V; bevel rows a spiral angle and V.
#[test]
fn rows_send_their_helix_or_spiral_and_v() {
    let mut app = app();
    let sk = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    for (id, x, r) in [("c1", 0.0, 20.0), ("c2", 30.0, 10.0)] {
        app.session.execute("sketch.circle.center", &json!({"center": [x, 0], "radius": r, "id": id})).unwrap();
    }
    app.session.execute("sketch.finish", &json!({})).unwrap();
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    d.inputs[0].items = vec![Sel::SketchCurve { id: "c1".into(), sketch: Some(sk) }, Sel::SketchCurve { id: "c2".into(), sketch: Some(sk) }];
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.picks = rows_now(&app, g, &d.inputs).into_iter().map(|(p, _)| p).collect();
    g.picks[0].angle = "25 deg".into();
    g.picks[0].v = true;
    g.picks[1].role = ROLES.iter().position(|r| r.0 == "bevel").unwrap();
    g.picks[1].angle = "35".into();
    let c = apply_commands(&app, &d).unwrap();
    let sel = c[0].1["selection"].as_array().unwrap().clone();
    assert_eq!((sel[0]["helix_angle"].as_f64(), sel[0]["herringbone"].as_bool()), (Some(25.0), Some(true)), "{sel:?}");
    assert_eq!(sel[1]["spiral_angle"].as_f64(), Some(35.0), "{sel:?}");
    assert!(sel[1].get("helix_angle").is_none());
}

/// The preview is the set's outline (pitch, tip and root circles, shafts) drawn in the
/// viewport; the kernel never builds teeth before OK.
#[test]
fn the_preview_is_the_outline_and_no_teeth_before_ok() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "pair", "gear": {"module": 2, "teeth": 20, "teeth2": 40}})).unwrap();
    app.start("gear.create");
    let Some(Kind::Gear(g)) = app.dialog.as_mut().map(|d| &mut d.kind) else { panic!() };
    g.sketch = made["sketch"].as_u64();
    assert!(!g.previews(), "no kernel preview");
    let mut d = app.dialog.take().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    let mut hover = None;
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(Default::default(), |ui| {
        rows(&mut app, ui, g, &mut d.inputs, &mut hover, &[]);
    });
    assert!(!g.shape.is_empty(), "the shape to draw: {:?}", g.calc);
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

/// Operation and Targets: the set joined to or cut from bodies, its keep-out cut from a
/// housing; the driver is a row; Join without targets says what to pick.
#[test]
fn operation_targets_keep_out_and_driver() {
    let mut app = app();
    app.session.execute("solid.box", &json!({"length": 80, "width": 60, "height": 20})).unwrap();
    let body = solidcraft_engine::Sel::Body { name: crate::open_meshing::session_world(&app.session).bodies[0].name.clone() };
    let sk = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    for (id, x, r) in [("c1", 0.0, 20.0), ("c2", 30.0, 10.0)] {
        app.session.execute("sketch.circle.center", &json!({"center": [x, 0], "radius": r, "id": id})).unwrap();
    }
    app.session.execute("sketch.finish", &json!({})).unwrap();
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    d.inputs[0].items = vec![Sel::SketchCurve { id: "c1".into(), sketch: Some(sk) }, Sel::SketchCurve { id: "c2".into(), sketch: Some(sk) }];
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.operation = 2;
    let e = apply_commands(&app, &d).unwrap_err();
    assert!(e.contains("pick the bodies"), "{e}");
    d.inputs[1].items = vec![body.clone()];
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.cut_keep_out = true;
    g.assembly = true;
    g.driver = 1;
    let c = apply_commands(&app, &d).unwrap();
    let p = &c[0].1;
    assert_eq!(p["operation"], "cut");
    let name = match &body {
        Sel::Body { name } => name.clone(),
        _ => unreachable!(),
    };
    assert_eq!(p["targets"], json!([name]));
    assert_eq!(p["keep_out_cuts"], json!([name]));
    assert_eq!(p["assembly_driver"], json!(2), "the second gear drives (1-based)");
}

/// The engine's outlines draw each gear without teeth: tip, pitch and root circles at both
/// faces.
#[test]
fn outlines_are_drawn_from_the_engine() {
    let r = json!({"outlines": [{"index": 0, "center": [0.0, 0.0, 0.0], "axis": [0.0, 0.0, 1.0], "face_width": 10.0, "pitch_diameter": 40.0, "tip_diameter": 44.0, "root_diameter": 35.0}]});
    let o = outlines_of(&r);
    // tip ×2, root ×2, pitch ×2, four lines between.
    assert_eq!(o.len(), 10);
    let front = &o[5].1;
    assert!((front[0][2] - 10.0).abs() < 1e-9 && (front[0][0] - 20.0).abs() < 1e-9, "{front:?}");
}

/// The user's gearbox (a local copy in %TEMP%\geartest\u\GAERBOX.sc, not in the repository):
/// the picks the dialog would send, read by the engine. Run by hand (`--ignored --nocapture`).
#[test]
#[ignore]
fn the_users_gearbox_reads_sanely() {
    let path = std::env::temp_dir().join("geartest").join("u").join("GAERBOX.sc");
    if !path.exists() {
        return;
    }
    let mut app = app();
    app.session.execute("doc.open", &json!({"path": path})).unwrap();
    let st = crate::open_meshing::session_world(&app.session);
    let mut items = Vec::new();
    for ss in &st.sketches {
        let name = app.session.doc.feature(ss.feature).map(|f| f.name.clone()).unwrap_or_default();
        let Ok(gs) = solidcraft_engine::gear_sketch(&app.session, ss.feature) else { continue };
        for (id, c) in &gs.circles {
            eprintln!("{name} circle {id} Ø{:.2} at ({:.2}, {:.2})", 2.0 * c.r, c.c.x, c.c.y);
            let d = (2.0 * c.r * 100.0).round() / 100.0;
            if [30.0, 15.0, 25.0].contains(&d) {
                items.push(Sel::SketchCurve { id: id.clone(), sketch: Some(ss.feature) });
            }
        }
        for (id, l) in &gs.lines {
            eprintln!("{name} line {id} {:.2} long", l.len());
            if gs.circles.is_empty() {
                items.push(Sel::SketchCurve { id: id.clone(), sketch: Some(ss.feature) });
            }
        }
    }
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    d.inputs[0].items = items;
    let Kind::Gear(g) = &d.kind else { panic!() };
    for (i, (p, geo)) in rows_now(&app, g, &d.inputs).iter().enumerate() {
        eprintln!("row {}: {:?} as {}", i + 1, geo, ROLES[p.role].0);
    }
    let c = apply_commands(&app, &d).unwrap();
    eprintln!("request: {}", c[0].1);
    match app.session.execute("gear.calculate", &c[0].1) {
        Ok(r) => {
            for m in members_of(&r).values() {
                eprintln!("member: {}", member_line(m).0);
            }
            for s in shafts_of(&r) {
                eprintln!("{}", shaft_line(&s));
            }
            eprintln!("contacts: {}, outlines: {}", contacts_of(&r).len(), outlines_of(&r).len());
        }
        Err(e) => eprintln!("refused: {e}"),
    }
}

/// Edit on the Gears feature (its id differs from the set's folder id) opens THAT set, and OK
/// updates it in place: still one set, the same feature.
#[test]
fn editing_the_gears_feature_opens_that_set_and_updates_it_in_place() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "pair", "gear": {"module": 2, "teeth": 20, "teeth2": 40}})).unwrap();
    let sk = made["sketch"].as_u64().unwrap();
    app.session.execute("gear.create", &json!({"type": "spur", "module": 2, "sketch": sk})).unwrap();
    let set = solidcraft_engine::gear_sets(&app.session).into_iter().next().expect("a set");
    let feature = set.feature.unwrap_or(set.group);
    app.edit_feature(feature);
    let g = gear(&app);
    assert_eq!(g.group, Some(set.group), "the dialog is on that set");
    let c = apply_commands(&app, app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(c[0].0, "gear.update");
    run_all(&mut app, c);
    let after = solidcraft_engine::gear_sets(&app.session);
    assert_eq!(after.len(), 1, "no new set");
    assert_eq!(after[0].feature, set.feature, "updated in place");
}

/// A drawing for the gear items: in an XY sketch, circle c1 (r20) at the origin tangent to c2
/// (r10) at (30, 0), and a construction centre line k; in a parallel sketch 10 mm above, c3 (r5)
/// on the same axis as c2; in an upright sketch (XZ), the shaft line s through c2's and c3's
/// centres along Z, and a point q 10 mm off it. Returns the three sketches.
fn items_drawing(app: &mut SolveApp) -> (u64, u64, u64) {
    let sk = |app: &mut SolveApp, plane: Value| app.session.execute("sketch.create", &json!({"plane": plane})).unwrap()["sketch"].as_u64().unwrap();
    let s1 = sk(app, json!("XY"));
    for (id, x, r) in [("c1", 0.0, 20.0), ("c2", 30.0, 10.0)] {
        app.session.execute("sketch.circle.center", &json!({"center": [x, 0], "radius": r, "id": id})).unwrap();
    }
    app.session.execute("sketch.line", &json!({"points": [[0, 0], [30, 0]], "ids": ["k"], "construction": true})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let s2 = sk(app, json!({"origin": [0, 0, 10], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}));
    app.session.execute("sketch.circle.center", &json!({"center": [30, 0], "radius": 5, "id": "c3"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let s3 = sk(app, json!({"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}));
    app.session.execute("sketch.line", &json!({"points": [[30, -5], [30, 20]], "ids": ["s"]})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [40, 5], "id": "q"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    (s1, s2, s3)
}

/// A pick is checked by the gear crate (`items::validate_slot`): the slot it would fill, or what
/// is wrong and what to do; the checklist and next slot are the crate's too.
#[test]
fn picks_are_checked_by_the_gear_crates_slots() {
    use crate::dialogs_gear_graph::GearItem;
    use solidcraft_gear::items::ItemKind;
    let mut app = app();
    let (s1, s2, s3) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    let spur = GearItem { picks: vec![at(s1, "c1")], ..GearItem::new(ItemKind::Spur) };
    // A spur has room for one pitch circle.
    let why = spur.refusal(&app, &at(s1, "c2"), &[]).expect("a second circle is refused");
    assert!(why.contains("no room for another pitch circle"), "{why}");
    // Used by another item.
    let compound = GearItem { picks: vec![at(s1, "c2")], ..GearItem::new(ItemKind::Compound) };
    assert_eq!(compound.refusal(&app, &at(s1, "c1"), &[(0, &spur)]).as_deref(), Some("already used by Item 1"));
    // A construction line is a helper, not a shaft.
    let why = compound.refusal(&app, &at(s1, "k"), &[]).unwrap();
    assert!(why.starts_with("a construction line is a helper, not a shaft"), "{why}");
    // Compound (decision 6): two circles in parallel sketches and the shaft line through them.
    assert_eq!(compound.next_slot(&app).as_deref(), Some("shaft line"));
    assert_eq!(compound.refusal(&app, &at(s3, "s"), &[]), None);
    let mut compound = compound;
    compound.picks.push(at(s3, "s"));
    assert_eq!(compound.refusal(&app, &at(s2, "c3"), &[]), None);
    compound.picks.push(at(s2, "c3"));
    assert!(compound.complete(&app), "{:?}", compound.checklist(&app));
    assert_eq!(compound.gear_count(&app), 2);
    assert_eq!(compound.checklist(&app), vec![("shaft line", true, true), ("pitch circle", true, true), ("pitch circle", true, true)]);
    // Bearings: a point off the shaft line is refused with how far off it is.
    let shaft = GearItem { picks: vec![at(s3, "s")], ..GearItem::new(ItemKind::Shaft) };
    let q = Sel::ExternalSketchPoint { sketch: s3, id: "q".into() };
    let why = shaft.refusal(&app, &q, &[]).unwrap();
    assert!(why.contains("10 mm off the shaft line"), "{why}");
    // Planetary: sun, ring and planet.
    let planetary = GearItem::new(ItemKind::Planetary);
    assert_eq!(planetary.checklist(&app).iter().map(|c| c.0).collect::<Vec<_>>(), ["sun's pitch circle", "ring's pitch circle", "planet's pitch circle"]);
    assert_eq!(planetary.gear_count(&app), 3);
    // Not geometry an item takes.
    assert!(spur.refusal(&app, &Sel::Body { name: "Body1".into() }, &[]).is_some());
}

/// A bevel pair from two axis lines and one circle (GEAR_UI_SPEC §3): the second layout is
/// taken once a line is picked, and the components are named axis, axis2, pitch.
#[test]
fn a_bevel_pair_from_two_axis_lines_and_a_circle() {
    use crate::dialogs_gear_graph::GearItem;
    use solidcraft_gear::items::ItemKind;
    let mut app = app();
    let s1 = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.line", &json!({"points": [[10, 0], [50, 0]], "ids": ["l1"]})).unwrap();
    app.session.execute("sketch.line", &json!({"points": [[10, 0], [10, 40]], "ids": ["l2"]})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    // A circle square to l1 (in the plane x = 30), centred on it, 20 from the apex.
    let s2 = app.session.execute("sketch.create", &json!({"plane": {"origin": [30, 0, 0], "x_dir": [0, 1, 0], "y_dir": [0, 0, 1]}})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.circle.center", &json!({"center": [0, 0], "radius": 10, "id": "c"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    let mut bevel = GearItem::new(ItemKind::Bevel);
    for p in [at(s1, "l1"), at(s1, "l2"), at(s2, "c")] {
        assert_eq!(bevel.refusal(&app, &p, &[]), None, "{p:?}");
        bevel.picks.push(p);
    }
    assert!(bevel.complete(&app), "{:?}", bevel.checklist(&app));
    assert_eq!(bevel.checklist(&app).iter().map(|c| c.0).collect::<Vec<_>>(), ["axis line 1", "axis line 2", "pitch circle"]);
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.items = vec![bevel];
    let p = &apply_commands(&app, &d).unwrap()[0].1;
    let slots: Vec<&str> = p["items"][0]["components"].as_array().unwrap().iter().map(|c| c["slot"].as_str().unwrap()).collect();
    assert_eq!(slots, ["axis", "axis2", "pitch"], "{p}");
    assert_eq!(p["items"][0]["gears"].as_array().unwrap().len(), 2);
    assert_eq!(p["items"][0]["bevel_style"], "straight");
}

/// Per gear: Size by, module or teeth, face preset and sides, helix, hand, V, profile shift,
/// bore and hub, read into the gear crate's settings and sent by its field names; Edit reads them
/// back as typed.
#[test]
fn per_gear_settings_are_the_gear_crates_and_round_trip() {
    use crate::dialogs_gear_items::{GearSettings, gear_json};
    use solidcraft_gear::items::{Bore, FacePreset, Hand, SizeBy};
    let app = app();
    let typed = GearSettings {
        module: "2 mm".into(),
        face_preset: 3,
        face_a: "8 mm".into(),
        face_b: "3 mm".into(),
        helix: "15 deg".into(),
        hand: 2,
        v: true,
        profile_shift: "0.3".into(),
        bore: 2,
        bore_d: "8 mm".into(),
        hub: true,
        hub_d: "20 mm".into(),
        hub_a: "5 mm".into(),
        ..Default::default()
    };
    let s = typed.read(&app, &|f: &str| f.to_string()).unwrap();
    assert_eq!((s.size_by, s.module, s.teeth), (SizeBy::Module, Some(2.0), None));
    assert_eq!((s.face.preset, s.face.a, s.face.b), (FacePreset::Custom, Some(8.0), Some(3.0)));
    assert_eq!((s.hand, s.v, s.bore), (Some(Hand::Right), true, Bore::Diameter(8.0)));
    assert!((s.helix.unwrap() - 15.0).abs() < 1e-9);
    let j = gear_json(1, &s);
    assert_eq!(j["gear"], 1);
    assert_eq!(j["size_by"], "module");
    assert_eq!(j["module"], 2.0);
    assert_eq!(j["face"], json!({"preset": "custom", "a": 8.0, "b": 3.0}));
    assert_eq!((j["hand"].as_str(), j["herringbone"].as_bool(), j["profile_shift"].as_f64(), j["bore"].as_f64()), (Some("right"), Some(true), Some(0.3), Some(8.0)));
    assert_eq!(j["hub"], json!({"diameter": 20.0, "length_a": 5.0, "length_b": 0.0}));
    let back = GearSettings::from_json(&j);
    assert_eq!(back.read(&app, &|f: &str| f.to_string()).unwrap(), s, "Edit reads back what was sent");
    // By teeth: the teeth are sent, not the module; Side A only puts nothing on side B; Auto
    // everywhere else.
    let teeth = GearSettings { by_teeth: true, teeth: "24".into(), module: "3 mm".into(), face_preset: 1, ..Default::default() };
    let j = gear_json(0, &teeth.read(&app, &|f: &str| f.to_string()).unwrap());
    assert_eq!((j["size_by"].as_str(), j["teeth"].as_u64(), j.get("module")), (Some("teeth"), Some(24), None), "{j}");
    assert_eq!(j["face"], json!({"preset": "a_only", "a": null, "b": 0.0}));
    assert_eq!((j["profile_shift"].as_str(), j["bore"].as_str(), j["hand"].is_null()), (Some("auto"), Some("auto"), true));
    // A value that doesn't read says which gear and what.
    let bad = GearSettings { module: "two".into(), ..Default::default() };
    assert!(bad.read(&app, &|f: &str| format!("Item 1 gear 2: {f}")).unwrap_err().starts_with("Item 1 gear 2: module"));
}

/// Gear items: each fills its slots from the picks, has its own per-gear settings, and is sent
/// with the graph's links and module blocks and the driver (GEAR_UI_SPEC.md §6); Edit restores
/// them from the set's request.
#[test]
fn items_and_the_graph_make_the_request_and_edit_restores_them() {
    use crate::dialogs_gear_graph::{GearItem, GearSettings, Suggested};
    use solidcraft_gear::items::ItemKind;
    let mut app = app();
    let (s1, s2, s3) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.items = vec![
        GearItem { picks: vec![at(s1, "c1")], ..GearItem::new(ItemKind::Spur) },
        GearItem { picks: vec![at(s1, "c2"), at(s3, "s"), at(s2, "c3")], ..GearItem::new(ItemKind::Compound) },
    ];
    // Per gear: face side A only and left hand on the first gear; sized by module 2.
    g.items[0].gears = vec![GearSettings { module: "2 mm".into(), face_preset: 1, face_a: "8 mm".into(), hand: 1, ..Default::default() }];
    g.items[1].gears = vec![GearSettings::default(), GearSettings { by_teeth: true, teeth: "12".into(), ..Default::default() }];
    g.graph.accept(&Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
    let block = g.graph.add_block("2 mm");
    for i in [0usize, 1] {
        g.graph.put_in_block(block, i);
    }
    g.drive_item = Some((1, 0));
    g.rpm = "1000".into();
    let c = apply_commands(&app, &d).unwrap();
    let p = &c[0].1;
    assert_eq!(p["items"][1]["type"], "compound", "{p}");
    let g0 = &p["items"][0]["gears"][0];
    assert_eq!((g0["module"].as_f64(), g0["hand"].as_str()), (Some(2.0), Some("left")), "{g0}");
    assert_eq!(g0["face"], json!({"preset": "a_only", "a": 8.0, "b": 0.0}));
    assert_eq!(g0["profile_shift"], "auto");
    assert_eq!(p["items"][1]["gears"][1]["teeth"], 12);
    // The engine's names (gear-items): item ids, components by slot, links by item and port.
    assert_eq!((p["items"][0]["id"].as_str(), p["items"][1]["id"].as_str()), (Some("A"), Some("B")));
    let slots: Vec<&str> = p["items"][1]["components"].as_array().unwrap().iter().map(|c| c["slot"].as_str().unwrap()).collect();
    assert_eq!(slots, ["pitch", "axis", "pitch2"], "in pick order: c2, s, c3");
    assert_eq!(p["links"], json!([{"a": {"item": "A", "port": 0}, "b": {"item": "B", "port": 0}, "kind": "mesh"}]));
    assert_eq!(p["module_blocks"], json!([{"id": "M1", "module": 2.0}]));
    assert_eq!((p["items"][0]["module_block"].as_str(), p["items"][1]["module_block"].as_str()), (Some("M1"), Some("M1")));
    assert_eq!(p["drive"], json!({"item": "B", "gear": 0, "rpm": 1000.0}));
    assert!(p.get("selection").is_none() && p.get("sketch").is_none() && p.get("clearance_body").is_none(), "{p}");
    assert_eq!(p["ui"]["gear_items"].as_array().map(Vec::len), Some(2), "the picks kept for Edit");
    // Edit: the set's request reopens the dialog with its items, links, blocks and drive.
    let set = solidcraft_engine::schema::gear::GearSet {
        group: 7,
        feature: Some(8),
        name: "Gears".into(),
        bodies: Vec::new(),
        features: Vec::new(),
        request: p.clone(),
        params: json!({}),
        sketch: None,
        sketch_changed: false,
    };
    let e = for_set(&set);
    assert_eq!(e.items.len(), 2);
    assert_eq!(e.items[1].kind, ItemKind::Compound);
    assert_eq!(e.items[1].picks, vec![at(s1, "c2"), at(s3, "s"), at(s2, "c3")]);
    assert_eq!(e.items[0].gears[0].face_preset, 1);
    assert_eq!(e.items[0].gears[0].module, "2 mm");
    assert!(e.items[1].gears[1].by_teeth && e.items[1].gears[1].teeth == "12");
    assert_eq!(e.graph.links(), vec![(0, 0, 1, 0, "mesh")]);
    assert_eq!(e.graph.blocks(), vec![("2 mm".to_string(), vec![0, 1])]);
    assert_eq!((e.drive_item, e.rpm.as_str()), (Some((1, 0)), "1000"));
    // OK on the reopened dialog sends the same items.
    let mut d2 = d.clone();
    d2.kind = Kind::Gear(e);
    let again = &apply_commands(&app, &d2).unwrap()[0].1;
    assert_eq!(again["items"], p["items"]);
    assert_eq!(again["links"], p["links"]);
    assert_eq!(again["drive"], p["drive"]);
}

/// What the engine says (§6) is shown: per gear z, m, Ø, ratio, rpm, torque, contact ratio and
/// warnings in Results and on the graph rows; each link's ratio or its red error with the hint;
/// the suggestions dashed; `error` as the status line; `notes` under the graph.
#[test]
fn the_engine_reply_is_shown_per_gear_and_per_link() {
    use crate::dialogs_gear_graph::{Graph, Suggested, reply_text, reply_tip};
    let reply = json!({
        "gears": [
            {"item": "A", "gear": 0, "teeth": 20, "module": 2.0, "pitch_d": 40.0, "ratio": 1.0, "rpm": 1000.0, "torque": 2.0, "contact_ratio": 1.6, "warnings": []},
            {"item": "B", "gear": 0, "teeth": 10, "module": 2.0, "pitch_d": 20.0, "ratio": 2.0, "rpm": 2000.0, "torque": 0.98, "contact_ratio": 1.5, "warnings": ["undercut: raise teeth to 17+ or turn on Profile Shift Auto"]}
        ],
        "links": [
            {"a": {"item": "A", "gear": 0}, "b": {"item": "B", "gear": 0}, "kind": "mesh", "ratio": 2.0},
            {"a": {"item": "B", "gear": 0}, "b": {"item": "C", "gear": 0}, "kind": "mesh", "error": "centre distance 31.2, needs 30.0 ±0.05", "hint": "move the circles"}
        ],
        "suggestions": [{"a": {"item": "A", "gear": 0}, "b": {"item": "C", "gear": 0}, "kind": "mesh", "why": "tangent circles"}],
        "notes": ["module 2 from Module Block M1"],
        "error": "Item C: the pinion doesn't touch the ring"
    });
    let len = |v: f64| fmt(v);
    let rows = results_of(&reply, &len);
    assert_eq!(rows[1], ["2·1", "10", "2", "20", "2.00", "2000", "0.98", "1.5", "undercut: raise teeth to 17+ or turn on Profile Shift Auto"].map(String::from));
    let mut graph = Graph::default();
    for i in 0..3 {
        graph.node_of(i);
    }
    graph.note(&reply);
    let b = &graph.replies()[1];
    assert_eq!(reply_text(b, &len), "z10 m2 · Ø20 · 2.00 : 1 · 2000 rpm");
    assert!(reply_tip(b).contains("Contact ratio 1.5") && reply_tip(b).contains("⚠ undercut"), "{}", reply_tip(b));
    assert_eq!(graph.notes()[1].label(), Some(("centre distance 31.2, needs 30.0 ±0.05: move the circles".into(), true)));
    assert_eq!(graph.suggested(), &[Suggested { a: 0, a_gear: 0, b: 2, b_gear: 0, kind: "mesh".into() }]);
    let (lines, _) = crate::dialogs_gear_graph::link_notes(&reply);
    assert_eq!(lines[0], ("1·1 – 2·1: 2.00 : 1".to_string(), false));
    // `error`: the status line says it (nothing is built); `notes` are kept.
    assert_eq!(status_of(Ok::<_, String>(reply.clone())), Err("Item C: the pinion doesn't touch the ring".to_string()));
    assert_eq!(notes_of(&reply), ["module 2 from Module Block M1"]);
    assert!(status_of(Ok::<_, String>(json!({"facts": [{"label": "ratio", "value": 2.0}]}))).unwrap()[0].starts_with("ratio: 2"));
}

/// The ratio calculator: two gears picked (graph rows, the cards' chips, or the lists), and
/// `gear.ratio`'s reply as the readout; two gear bodies of one Gears feature selected in the
/// model ask it too (the quick readout), any other pair doesn't.
#[test]
fn the_ratio_readout_from_two_gears() {
    let mut app = app();
    app.start("gear.create");
    let mut k = gear(&app);
    pick_for_ratio(&mut k, (0, 0));
    assert_eq!(k.ratio_pick, [Some((0, 0)), None]);
    pick_for_ratio(&mut k, (2, 1));
    assert_eq!(k.ratio_pick, [Some((0, 0)), Some((2, 1))]);
    // Clicking a picked one takes it out; a third starts over.
    pick_for_ratio(&mut k, (2, 1));
    assert_eq!(k.ratio_pick, [Some((0, 0)), None]);
    pick_for_ratio(&mut k, (1, 0));
    pick_for_ratio(&mut k, (3, 0));
    assert_eq!(k.ratio_pick, [Some((3, 0)), None]);
    assert_eq!(gear_ref((1, 0)), json!({"item": "B", "gear": 0}));
    let r = json!({"ratio": 3.5, "direction": "opposite", "path": [{"item": "A", "gear": 0}, {"item": "B", "gear": 0}, {"item": "B", "gear": 1}, {"item": "C", "gear": 0}], "efficiency": 0.9604, "rack_mm_per_rev": 62.832});
    let lines = ratio_lines(&r, &|v: f64| fmt(v));
    assert_eq!(lines[0], ("Ratio".to_string(), "3.50 : 1, opposite direction".to_string()));
    assert_eq!(lines[1], ("Path".to_string(), "1·1 → 2·1 → 2·2 → 3·1".to_string()));
    assert_eq!(lines[2], ("Efficiency".to_string(), "96 %".to_string()));
    assert_eq!(lines[3], ("Rack travel".to_string(), "62.832 / rev".to_string()));
    // Not two gear bodies: no ratio asked.
    let two = [Sel::Body { name: "A".into() }, Sel::Body { name: "B".into() }];
    assert_eq!(ratio_bodies(&app.session, &two), None);
    assert_eq!(ratio_bodies(&app.session, &two[..1]), None);
}
