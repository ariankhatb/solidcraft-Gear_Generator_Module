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

/// The type list's index of `kind`.
fn type_of(kind: ItemKind) -> usize {
    TYPES.iter().position(|t| t.0 == kind).unwrap()
}

/// The Gears dialog opened, as its kind and inputs.
fn opened(app: &mut SolveApp) -> (Gear, Vec<SelInput>) {
    app.start("gear.create");
    let d = app.dialog.clone().expect("Gears opens a dialog");
    let Kind::Gear(g) = d.kind else { panic!("the Gears dialog") };
    (g, d.inputs)
}

/// A click in the view while picks go to the gears: the pick lands in the Geometry input, and the
/// dialog lays it onto the gears on its next frame.
fn click(app: &SolveApp, g: &mut Gear, inputs: &mut [SelInput], sel: Sel) {
    if let Some(i) = inputs[0].items.iter().position(|s| *s == sel) {
        inputs[0].items.remove(i);
    } else {
        inputs[0].items.push(sel);
    }
    sync_picks(app, g, inputs);
}

/// The flow a first-time user follows: an empty dialog says what to do; New gear starts a gear of
/// the chosen type and the status line names the next sketch piece; picks tick the slots off in
/// the gear crate's words; a complete gear says so; a pick with no gear waiting starts one; a
/// pick that fits nowhere is refused with the reason; clicking a card makes it the active one,
/// clicking a pick again takes it out; × removes a gear and renumbers the rest.
#[test]
fn the_flow_new_gear_click_pieces_card_per_gear() {
    let mut app = app();
    let (s1, s2, s3) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    let (mut g, mut inputs) = opened(&mut app);
    assert!(g.items.is_empty());
    assert!(next_step(&app, &g).0.starts_with("Press New gear"), "{:?}", next_step(&app, &g));
    // Compound: New gear, then the status line asks for its shaft line first.
    g.new_kind = type_of(ItemKind::Compound);
    new_gear(&mut g, &mut inputs);
    assert_eq!((g.items.len(), g.active), (1, Some(0)));
    assert_eq!(next_step(&app, &g).0, "Gear A · Compound — click: shaft line");
    click(&app, &mut g, &mut inputs, at(s3, "s"));
    assert_eq!(next_step(&app, &g).0, "Gear A · Compound — click: pitch circle");
    click(&app, &mut g, &mut inputs, at(s1, "c2"));
    click(&app, &mut g, &mut inputs, at(s2, "c3"));
    assert!(g.items[0].complete(&app));
    assert!(next_step(&app, &g).0.starts_with("Gear A · Compound is complete"), "{:?}", next_step(&app, &g));
    // The checklist: each slot with what fills it (its sketch and size).
    let len = |v: f64| fmt(v);
    let rows = g.items[0].slot_rows(&app, &len);
    assert_eq!(rows.iter().map(|r| r.what).collect::<Vec<_>>(), ["shaft line", "pitch circle", "pitch circle", "bearing point"]);
    assert!(rows.iter().take(3).all(|r| r.filled.is_some()) && rows[3].repeats && !rows[3].required, "{rows:?}");
    assert!(rows[1].filled.as_deref().is_some_and(|f| f.ends_with("Ø20")), "{rows:?}");
    // The input holds the active gear's picks.
    assert_eq!(inputs[0].items, g.items[0].picks);
    // The gear is complete and a circle doesn't fit it: the pick starts a new gear of the type in
    // the list (a Spur now).
    g.new_kind = type_of(ItemKind::Spur);
    click(&app, &mut g, &mut inputs, at(s1, "c1"));
    assert_eq!((g.items.len(), g.active, g.items[1].kind), (2, Some(1), ItemKind::Spur));
    assert_eq!(inputs[0].items, vec![at(s1, "c1")]);
    // A pick another gear holds is refused, and says which.
    click(&app, &mut g, &mut inputs, at(s1, "c2"));
    assert_eq!(g.items.len(), 2);
    let (text, bad) = next_step(&app, &g);
    assert!(bad && text.contains("already used by Gear A"), "{text}");
    assert_eq!(inputs[0].items, vec![at(s1, "c1")], "the refused pick doesn't stay picked");
    // A construction line can't be a shaft: refused with the crate's reason.
    g.new_kind = type_of(ItemKind::Shaft);
    new_gear(&mut g, &mut inputs);
    click(&app, &mut g, &mut inputs, at(s1, "k"));
    assert!(next_step(&app, &g).0.contains("a construction line is a helper, not a shaft"), "{:?}", next_step(&app, &g));
    // × on the empty shaft: gone; clicking card A makes it the active one with its picks.
    remove_item(&mut g, 2, &mut inputs);
    assert_eq!(g.items.len(), 2);
    make_active(&mut g, 0, &mut inputs);
    assert_eq!(g.active, Some(0));
    assert_eq!(inputs[0].items, g.items[0].picks);
    // Clicking a pick again takes it out of its gear: the status line asks for it again.
    click(&app, &mut g, &mut inputs, at(s3, "s"));
    assert_eq!(g.items[0].picks.len(), 2);
    assert_eq!(next_step(&app, &g).0, "Gear A · Compound — click: shaft line");
    // Removing gear A renumbers B to A; its links and the driver follow.
    g.drive_item = Some((1, 0));
    remove_item(&mut g, 0, &mut inputs);
    assert_eq!((g.items.len(), g.items[0].kind, g.drive_item), (1, ItemKind::Spur, Some((0, 0))));
}

/// With nothing picked yet, a click starts a gear of the type in the list (no need to press New
/// gear first); a pick the type can't start with says what the type needs.
#[test]
fn a_pick_with_no_gear_waiting_starts_one() {
    let mut app = app();
    let (s1, _, s3) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    let (mut g, mut inputs) = opened(&mut app);
    g.new_kind = type_of(ItemKind::Worm);
    click(&app, &mut g, &mut inputs, at(s1, "c1"));
    assert_eq!((g.items.len(), g.items[0].kind), (1, ItemKind::Worm));
    assert_eq!(next_step(&app, &g).0, "Gear A · Worm and Wheel — click: worm's axis line");
    // A planetary set can't start from a shaft line: refused, nothing made.
    let (mut g, mut inputs) = opened(&mut app);
    g.new_kind = type_of(ItemKind::Planetary);
    click(&app, &mut g, &mut inputs, at(s3, "s"));
    assert!(g.items.is_empty());
    let (text, bad) = next_step(&app, &g);
    assert!(bad && text.contains("A Planetary gear can't start with that"), "{text}");
}

/// The dialog reads its slots from the gear crate, never its own list: every type's checklist is
/// `items::slots`' first layout, and a bevel pair drawn in a side view (apex point, two shaft
/// lines, an optional contact line) takes that layout as soon as the point is picked.
#[test]
fn slots_are_the_gear_crates() {
    let mut app = app();
    let len = |v: f64| fmt(v);
    for (kind, _) in TYPES {
        let it = GearItem::new(kind);
        let rows: Vec<&str> = it.slot_rows(&app, &len).iter().map(|r| r.what).collect();
        let crate_rows: Vec<&str> = gi::slots(kind)[0].iter().map(|s| s.what).collect();
        assert_eq!(rows, crate_rows, "{kind:?}");
        // The first required slot is the one marked next.
        assert_eq!(it.slot_rows(&app, &len).iter().find(|r| r.next).map(|r| r.what), gi::next_slot(kind, &[]), "{kind:?}");
    }
    // A side view: the apex where the shafts meet, two shaft lines from it, a contact line.
    let s = app.session.execute("sketch.create", &json!({"plane": "XZ"})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.point", &json!({"point": [0, 0], "id": "apex"})).unwrap();
    app.session.execute("sketch.line", &json!({"points": [[0, 0], [60, 0]], "ids": ["s1"]})).unwrap();
    app.session.execute("sketch.line", &json!({"points": [[0, 0], [0, 40]], "ids": ["s2"]})).unwrap();
    app.session.execute("sketch.line", &json!({"points": [[40, 0], [0, 20]], "ids": ["c"], "construction": true})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let (mut g, mut inputs) = opened(&mut app);
    g.new_kind = type_of(ItemKind::Bevel);
    new_gear(&mut g, &mut inputs);
    // (The gear crate checks a side view as a whole: the two shaft lines first, then the apex.)
    for id in ["s1", "s2"] {
        click(&app, &mut g, &mut inputs, Sel::SketchCurve { id: id.into(), sketch: Some(s) });
    }
    click(&app, &mut g, &mut inputs, Sel::ExternalSketchPoint { sketch: s, id: "apex".into() });
    assert_eq!(g.items[0].picks.len(), 3, "{:?}", g.refused);
    let rows: Vec<&str> = g.items[0].slot_rows(&app, &len).iter().map(|r| r.what).collect();
    let side: Vec<&str> = gi::slots(ItemKind::Bevel).iter().find(|l| l.iter().any(|s| s.kind == gi::SlotKind::Point)).unwrap().iter().map(|s| s.what).collect();
    assert_eq!(rows, side);
    assert!(g.items[0].complete(&app), "{:?} {:?}", g.items[0].slot_rows(&app, &len), g.refused);
    assert_eq!(g.items[0].optional_slot(&app).as_deref(), Some("contact line"));
}

/// A message from the engine lands on the card of the gear it names, worded as the dialog names
/// gears; one about no gear goes above the cards.
#[test]
fn engine_messages_go_on_their_cards() {
    assert_eq!(about_item("Item B: the pinion doesn't touch the ring"), Some((1, "the pinion doesn't touch the ring".to_string())));
    assert_eq!(about_item("Gear C · Compound still needs a pitch circle: click one"), Some((2, "Gear C · Compound still needs a pitch circle: click one".to_string())));
    assert_eq!(about_item("centre distance 31.2, needs 30.0"), None);
    assert_eq!(worded("item A gear 1 and Item B"), "Gear A gear 1 and Gear B");
    // A gear's own name for its parts: sun, ring, planet; a compound's numbered.
    let mut g = new(None);
    g.items = vec![GearItem::new(ItemKind::Planetary), GearItem::new(ItemKind::Compound), GearItem::new(ItemKind::Spur)];
    g.gear_counts = vec![3, 2, 1];
    assert_eq!((g.gear_name((0, 0)), g.gear_name((0, 2)), g.gear_name((1, 1)), g.gear_name((2, 0))), ("A sun".into(), "A planet".into(), "B2".into(), "C".into()));
}

/// Edit on a set made before gear items (one type read from a sketch) opens it as it is (no new
/// set): OK updates it in place, its request kept; Convert turns its circles into gears, and OK
/// then sends gear items in place of the old type, still on the same set.
#[test]
fn an_old_set_opens_as_it_is_and_converts() {
    let mut app = app();
    let made = app.session.execute("gear.sketch", &json!({"type": "pair", "gear": {"module": 2, "teeth": 20, "teeth2": 40}})).unwrap();
    let sk = made["sketch"].as_u64().unwrap();
    app.session.execute("gear.create", &json!({"type": "spur", "module": 2, "sketch": sk})).unwrap();
    let set = solidcraft_engine::gear_sets(&app.session).into_iter().next().expect("a set");
    let feature = set.feature.unwrap_or(set.group);
    app.edit_feature(feature);
    let g = gear(&app);
    assert_eq!(g.group, Some(set.group), "the dialog is on that set");
    assert!(g.legacy.as_ref().is_some_and(|l| l.convertible), "{:?}", g.legacy);
    let c = apply_commands(&app, app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(c[0].0, "gear.update");
    assert_eq!((c[0].1["sketch"].as_u64(), c[0].1["type"].as_str()), (Some(sk), Some("spur")), "kept as it was: {}", c[0].1);
    // Convert: its two circles become two spur gears.
    let mut d = app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    assert_eq!(convert_legacy(&app, g), Ok(2));
    assert!(g.legacy.is_none() && g.items.iter().all(|it| it.kind == ItemKind::Spur && it.picks.len() == 1));
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].0, "gear.update");
    let p = &c[0].1;
    assert_eq!(p["items"].as_array().map(Vec::len), Some(2), "{p}");
    assert!(p.get("sketch").is_none() && p.get("type").is_none() && p.get("module").is_none(), "the old keys go: {p}");
    crate::dialogs::cancel(&mut app);
    run_all(&mut app, c);
    let after = solidcraft_engine::gear_sets(&app.session);
    assert_eq!(after.len(), 1, "no new set");
    assert_eq!(after[0].feature, set.feature, "updated in place");
}

/// Several sketch pieces picked at once (a box selection, Ctrl-clicks): one gear per circle of the
/// type in the list, in one go; touching ones are suggested as meshes; one OK makes them all as
/// one Gears feature.
#[test]
fn several_circles_at_once_make_one_gear_each() {
    let mut app = app();
    let (s1, s2, _) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    let (mut g, mut inputs) = opened(&mut app);
    inputs[0].items = vec![at(s1, "c1"), at(s1, "c2"), at(s2, "c3")];
    sync_picks(&app, &mut g, &mut inputs);
    assert_eq!(g.items.len(), 3, "{:?}", g.refused);
    assert!(g.items.iter().all(|it| it.kind == ItemKind::Spur && it.picks.len() == 1));
    refresh(&mut app, &mut g, &inputs);
    let s = g.links.suggested();
    assert!(s.iter().any(|s| s.kind == "mesh" && [s.a, s.b] == [0, 1]), "c1 and c2 touch: {s:?}");
    g.links.accept_all();
    let mut d = app.dialog.clone().unwrap();
    d.kind = Kind::Gear(g);
    d.inputs = inputs;
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].0, "gear.create");
    assert_eq!(c[0].1["items"].as_array().map(Vec::len), Some(3));
    crate::dialogs::cancel(&mut app);
    run_all(&mut app, c);
    assert_eq!(solidcraft_engine::gear_sets(&app.session).len(), 1, "one Gears feature");
}

/// A picked circle deleted from its sketch: the gear keeps the pick and names it as missing (it is
/// never swapped for something else), the status line asks for its replacement, OK says which is
/// gone; a new pick replaces it.
#[test]
fn a_deleted_pick_is_named_missing_until_re_picked() {
    let mut app = app();
    let (s1, _, _) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    let (mut g, mut inputs) = opened(&mut app);
    g.items = vec![GearItem { picks: vec![at(s1, "gone")], ..GearItem::new(ItemKind::Spur) }];
    make_active(&mut g, 0, &mut inputs);
    // The host drops a pick it can't show: the gear keeps it.
    inputs[0].items.clear();
    sync_picks(&app, &mut g, &mut inputs);
    assert_eq!(g.items[0].picks, vec![at(s1, "gone")]);
    let gone = g.items[0].missing(&app);
    assert_eq!(gone.len(), 1);
    assert!(gone[0].starts_with("curve gone of sketch"), "{gone:?}");
    let (text, bad) = next_step(&app, &g);
    assert!(bad && text.contains("is missing") && text.contains("curve gone"), "{text}");
    let mut d = app.dialog.clone().unwrap();
    d.kind = Kind::Gear(g.clone());
    let e = apply_commands(&app, &d).unwrap_err();
    assert!(e.contains("Gear A") && e.contains("curve gone") && e.contains("is missing"), "{e}");
    // Re-picked: the new circle takes its place.
    click(&app, &mut g, &mut inputs, at(s1, "c1"));
    assert_eq!(g.items[0].picks, vec![at(s1, "c1")]);
    assert!(g.items[0].missing(&app).is_empty() && g.items[0].complete(&app));
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
    use crate::dialogs_gear_links::GearItem;
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
    assert_eq!(compound.refusal(&app, &at(s1, "c1"), &[(0, &spur)]).as_deref(), Some("already used by Gear A: click its card to change it"));
    // A construction line is a helper, not a shaft.
    let why = compound.refusal(&app, &at(s1, "k"), &[]).unwrap();
    assert!(why.starts_with("a construction line is a helper, not a shaft"), "{why}");
    // Compound (decision 6): two circles in parallel sketches and the shaft line through them.
    assert_eq!(compound.next_slot(&app).as_deref(), Some("shaft line"));
    assert_eq!(compound.refusal(&app, &at(s3, "s"), &[]), None);
    // A shaft line another gear holds can be picked again: gears share a shaft.
    let on_line = GearItem { picks: vec![at(s3, "s")], ..GearItem::new(ItemKind::Shaft) };
    assert_eq!(compound.refusal(&app, &at(s3, "s"), &[(0, &on_line)]), None);
    let mut compound = compound;
    compound.picks.push(at(s3, "s"));
    assert_eq!(compound.refusal(&app, &at(s2, "c3"), &[]), None);
    compound.picks.push(at(s2, "c3"));
    assert!(compound.complete(&app), "{:?}", compound.checklist(&app));
    assert_eq!(compound.gear_count(&app), 2);
    assert_eq!(compound.checklist(&app), vec![("shaft line", true, true), ("pitch circle", true, true), ("pitch circle", true, true), ("bearing point", false, false)]);
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
    use crate::dialogs_gear_links::GearItem;
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
    // No circle sizes the gear (a side view's bevel pair): its teeth and its module are both sent,
    // and Edit reads both back.
    let both = GearSettings { both: true, teeth: "30".into(), module: "0.6 mm".into(), ..Default::default() };
    let j = gear_json(0, &both.read(&app, &|f: &str| f.to_string()).unwrap());
    assert_eq!((j["size_by"].as_str(), j["teeth"].as_u64(), j["module"].as_f64()), (Some("teeth"), Some(30), Some(0.6)), "{j}");
    let back = GearSettings::from_json(&j);
    assert!(back.both && back.teeth == "30" && back.module == "0.6 mm", "{back:?}");
    // A value that doesn't read says which gear and what.
    let bad = GearSettings { module: "two".into(), ..Default::default() };
    assert!(bad.read(&app, &|f: &str| format!("Item 1 gear 2: {f}")).unwrap_err().starts_with("Item 1 gear 2: module"));
}

/// Gear items: each fills its slots from the picks, has its own per-gear settings, and is sent
/// with the links and blocks and the driver (GEAR_UI_SPEC.md §6); Edit restores them from the
/// set's request.
#[test]
fn items_and_the_links_make_the_request_and_edit_restores_them() {
    use crate::dialogs_gear_links::{GearItem, GearSettings, Suggested};
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
    g.links.accept(&Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
    let block = g.links.add_block(&[0, 1]);
    g.links.block_mut(block).unwrap().module = "2 mm".into();
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
    assert_eq!(e.links.all(), vec![(0, 0, 1, 0, "mesh")]);
    assert_eq!(e.links.blocks().iter().map(|b| (b.module.clone(), b.members.clone())).collect::<Vec<_>>(), vec![("2 mm".to_string(), vec![0, 1])]);
    assert_eq!((e.drive_item, e.rpm.as_str()), (Some((1, 0)), "1000"));
    // OK on the reopened dialog sends the same items.
    let mut d2 = d.clone();
    d2.kind = Kind::Gear(e);
    let again = &apply_commands(&app, &d2).unwrap()[0].1;
    assert_eq!(again["items"], p["items"]);
    assert_eq!(again["links"], p["links"]);
    assert_eq!(again["drive"], p["drive"]);
    // A request written elsewhere names its items W and R: read back by position, its links
    // and drive with them.
    let mut named = p.clone();
    named["items"][0]["id"] = json!("W");
    named["items"][1]["id"] = json!("R");
    let swap = |v: &mut Value| {
        if let Some(s) = v.as_str() {
            *v = json!(if s == "A" { "W" } else { "R" });
        }
    };
    for l in named["links"].as_array_mut().unwrap() {
        swap(&mut l["a"]["item"]);
        swap(&mut l["b"]["item"]);
    }
    swap(&mut named["drive"]["item"]);
    let e = for_set(&solidcraft_engine::schema::gear::GearSet { request: named, ..set.clone() });
    assert_eq!(e.links.all(), vec![(0, 0, 1, 0, "mesh")]);
    assert_eq!(e.drive_item, Some((1, 0)));
    assert_eq!(e.base["items"][1]["id"], json!("B"));
}

/// What the engine says (§6) is shown: per gear z, m, Ø, ratio, rpm, torque, contact ratio and
/// warnings in Results and on the gear rows; each link's ratio or its red error with the hint;
/// the suggestions; `error` as the status line; `notes` under the links.
#[test]
fn the_engine_reply_is_shown_per_gear_and_per_link() {
    use crate::dialogs_gear_links::{Links, Suggested, reply_text, reply_tip};
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
    let mut graph = Links::default();
    graph.note(&reply);
    let b = &graph.replies()[1];
    assert_eq!(reply_text(b, &len), "z10 m2 · Ø20 · 2.00 : 1 · 2000 rpm");
    assert!(reply_tip(b).contains("Contact ratio 1.5") && reply_tip(b).contains("⚠ undercut"), "{}", reply_tip(b));
    assert_eq!(graph.notes()[1].label(), Some(("centre distance 31.2, needs 30.0 ±0.05: move the circles".into(), true)));
    assert_eq!(graph.suggested(), &[Suggested { a: 0, a_gear: 0, b: 2, b_gear: 0, kind: "mesh".into() }]);
    let (lines, _) = crate::dialogs_gear_links::link_notes(&reply);
    assert_eq!(lines[0], ("A – B: 2.00 : 1".to_string(), false));
    // `error`: the status line says it (nothing is built); `notes` are kept.
    assert_eq!(status_of(Ok::<_, String>(reply.clone())), Err("Item C: the pinion doesn't touch the ring".to_string()));
    assert_eq!(notes_of(&reply), ["module 2 from Module Block M1"]);
    assert_eq!(status_of(Ok::<_, String>(json!({"warnings": ["thin rim"]}))).unwrap(), vec!["⚠ thin rim".to_string()]);
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
    // As gear.ratio replies (schema GearRatioResult): steps with their end gears.
    let step = |f: (&str, u64), t: (&str, u64), kind: &str| {
        json!({"from": "x", "to": "y", "from_gear": {"item": f.0, "gear": f.1}, "to_gear": {"item": t.0, "gear": t.1}, "kind": kind, "ratio": 1.0, "efficiency": 0.98})
    };
    let r = json!({"ratio": 3.5, "direction": "opposite", "path": [step(("A", 0), ("B", 0), "spur"), step(("B", 0), ("B", 1), "shaft"), step(("B", 1), ("C", 0), "spur")], "efficiency": 0.9604, "mm_per_rev": 62.832});
    let lines = ratio_lines(&r, &|v: f64| fmt(v));
    assert_eq!(lines[0], ("Ratio".to_string(), "3.50 : 1, opposite direction".to_string()));
    assert_eq!(lines[1], ("Path".to_string(), "A → B → B2 → C".to_string()));
    assert_eq!(lines[2], ("Efficiency".to_string(), "96 %".to_string()));
    assert_eq!(lines[3], ("Rack travel".to_string(), "62.832 / rev".to_string()));
    // Not two gear bodies: no ratio asked.
    let two = [Sel::Body { name: "A".into() }, Sel::Body { name: "B".into() }];
    assert_eq!(ratio_bodies(&app.session, &two), None);
    assert_eq!(ratio_bodies(&app.session, &two[..1]), None);
}

/// The contract between the dialog and the engine: what the dialog sends for its items, links,
/// module blocks and drive reads as the engine's `GearParams` (every name known), `gear.calculate`
/// answers it per gear and per link, and `gear.ratio` takes the same request with two {item, gear}
/// ends and its reply is the readout.
#[test]
fn the_dialogs_request_is_the_engines_schema() {
    use crate::dialogs_gear_links::{GearItem, GearSettings, Suggested, gear_replies};
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
    g.items[0].gears = vec![GearSettings { face_preset: 1, face_a: "6 mm".into(), ..Default::default() }];
    g.links.accept(&Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
    let block = g.links.add_block(&[0, 1]);
    let bl = g.links.block_mut(block).unwrap();
    (bl.name, bl.module, bl.shaft_d, bl.backlash) = ("Main".into(), "1 mm".into(), "4 mm".into(), "0.05 mm".into());
    g.drive_item = Some((0, 0));
    g.rpm = "1000".into();
    g.torque = "2".into();
    g.ratio_pick = [Some((0, 0)), Some((1, 1))];
    let p = apply_commands(&app, &d).unwrap()[0].1.clone();
    let parsed: Result<solidcraft_engine::schema::gear::GearParams, _> = serde_json::from_value(p.clone());
    assert!(parsed.is_ok(), "the engine reads every name the dialog sends: {:?}\n{p}", parsed.err());
    let r = app.session.execute("gear.calculate", &p).unwrap();
    assert!(r.get("error").is_none(), "{r}");
    let replies = gear_replies(&r);
    assert_eq!(replies.len(), 3, "{r}");
    let a = replies.iter().find(|x| (x.item, x.gear) == (0, 0)).unwrap();
    assert_eq!((a.teeth, a.module), (40, 1.0), "Ø40 at module 1 from the block: {r}");
    assert!(a.rpm.is_some_and(|x| (x - 1000.0).abs() < 1e-6), "{a:?}");
    let (lines, _) = crate::dialogs_gear_links::link_notes(&r);
    assert!(lines.iter().any(|(t, bad)| t.starts_with("A – B:") && !bad), "{lines:?} {r}");
    // gear.ratio on the dialog's request: A·1 (z40) → B·2, one body with B·1 (z20): 2 : 1, the sign said in words.
    let q = json!({"request": p, "a": gear_ref((0, 0)), "b": gear_ref((1, 1))});
    let ratio = app.session.execute("gear.ratio", &q).unwrap();
    let lines = ratio_lines(&ratio, &|v: f64| fmt(v));
    assert!(lines[0].1.starts_with("2.00 : 1, opposite"), "{lines:?} {ratio}");
    assert_eq!(lines[1], ("Path".to_string(), "A → B → B2".to_string()), "a compound is one body; the path names both its gears: {ratio}");
}

/// A set made of items in the dialog (Create New + picks): every way to edit it opens the
/// dialog on THAT set with its items (not a new dialog), and OK updates it in place.
#[test]
fn every_edit_entry_point_opens_an_items_set() {
    use crate::dialogs_gear_links::{GearItem, Suggested};
    use solidcraft_gear::items::ItemKind;
    let mut app = app();
    let (s1, s2, s3) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.items = vec![GearItem { picks: vec![at(s1, "c1")], ..GearItem::new(ItemKind::Spur) }, GearItem { picks: vec![at(s1, "c2"), at(s3, "s"), at(s2, "c3")], ..GearItem::new(ItemKind::Compound) }];
    g.links.accept(&Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
    g.assembly = true;
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].0, "gear.create");
    crate::dialogs::cancel(&mut app);
    run_all(&mut app, c);
    let sets = solidcraft_engine::gear_sets(&app.session);
    assert_eq!(sets.len(), 1, "{sets:?}");
    let set = sets[0].clone();
    let feature = set.feature.expect("a Gears feature");
    let check = |app: &SolveApp, how: &str| {
        let g = match app.dialog.as_ref().map(|d| &d.kind) {
            Some(Kind::Gear(g)) => g.clone(),
            other => panic!("{how}: not the Gears dialog: {other:?}"),
        };
        assert_eq!(g.group, Some(set.group), "{how}: the dialog is on the set");
        assert_eq!(g.items.len(), 2, "{how}: its items are restored");
    };
    app.edit_feature(feature);
    check(&app, "the Gears feature's Edit");
    crate::dialogs::cancel(&mut app);
    open_set(&mut app, set.group).unwrap();
    check(&app, "the folder's Edit Gears");
    crate::dialogs::cancel(&mut app);
    // A body of the set selected, then the Gears command.
    let body = set.bodies.first().cloned().expect("a body");
    app.session.selection = vec![solidcraft_engine::Sel::Body { name: body.clone() }];
    app.start("gear.create");
    check(&app, "a body selected + Gears");
    crate::dialogs::cancel(&mut app);
    // The feature selected, then the Gears command.
    app.session.selection = vec![solidcraft_engine::Sel::Feature { id: feature }];
    app.start("gear.create");
    check(&app, "the feature selected + Gears");
    crate::dialogs::cancel(&mut app);
    // The set's folder picked in the timeline (not a model selection), then the Gears command.
    app.session.selection.clear();
    app.tree.picked_timeline_groups = vec![set.group];
    app.start("gear.create");
    check(&app, "the folder picked + Gears");
    crate::dialogs::cancel(&mut app);
    app.tree.picked_timeline_groups.clear();
    // Edit on the body's feature (a double-click on the body in the viewport).
    let made_by = app.session.model.state().bodies.iter().find(|b| b.name == body).map(|b| b.feature);
    if let Some(f) = made_by {
        app.edit_feature(f);
        check(&app, "the body's feature");
        crate::dialogs::cancel(&mut app);
    }
    // An occurrence of the set's assembly selected, then the Gears command.
    if let Some(o) = app.session.doc.occurrences.first().map(|o| o.id) {
        app.session.selection = vec![solidcraft_engine::Sel::Occurrence { path: vec![o] }];
        app.start("gear.create");
        check(&app, "an occurrence selected + Gears");
        crate::dialogs::cancel(&mut app);
    }
    app.session.selection.clear();
    // OK on the reopened dialog: the same set, updated in place.
    app.edit_feature(feature);
    let c = apply_commands(&app, app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(c[0].0, "gear.update", "{c:?}");
    crate::dialogs::cancel(&mut app);
    run_all(&mut app, c);
    let after = solidcraft_engine::gear_sets(&app.session);
    assert_eq!(after.len(), 1, "no new set");
    assert_eq!(after[0].feature, Some(feature), "updated in place");
}

/// The same set edited the way a person does it, frames running: a double-click on the Gears
/// feature's timeline row or on its folder, and the dialog stays on the set over later frames.
#[test]
fn double_clicks_in_the_timeline_open_an_items_set_and_it_stays_open() {
    use crate::dialogs_gear_links::{GearItem, Suggested};
    use solidcraft_gear::items::ItemKind;
    let mut h = crate::scenario::Harness::new();
    h.app.home.open = false;
    let (s1, s2, s3) = items_drawing(&mut h.app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    h.app.start("gear.create");
    let mut d = h.app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.items = vec![GearItem { picks: vec![at(s1, "c1")], ..GearItem::new(ItemKind::Spur) }, GearItem { picks: vec![at(s1, "c2"), at(s3, "s"), at(s2, "c3")], ..GearItem::new(ItemKind::Compound) }];
    g.links.accept(&Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
    g.assembly = true;
    let c = apply_commands(&h.app, &d).unwrap();
    crate::dialogs::cancel(&mut h.app);
    run_all(&mut h.app, c);
    h.frames(3);
    let set = solidcraft_engine::gear_sets(&h.app.session).into_iter().next().expect("a set");
    let feature = set.feature.expect("a Gears feature");
    let state = |h: &crate::scenario::Harness| match h.app.dialog.as_ref().map(|d| &d.kind) {
        Some(Kind::Gear(g)) => Some((g.group, g.items.len())),
        _ => None,
    };
    let fname = h.app.session.doc.feature(feature).map(|f| f.name.clone()).unwrap();
    let gname = h.app.session.doc.timeline_groups.iter().find(|g| g.id == set.group).map(|g| g.name.clone());
    let mut tried = Vec::new();
    for handle in [Some(format!("timeline:{fname}")), gname.map(|n| format!("timeline-folder:{n}"))].into_iter().flatten() {
        if h.step(&json!({"click": {"handle": handle}, "double": true})).is_err() {
            continue;
        }
        h.frames(5);
        assert_eq!(state(&h), Some((Some(set.group), 2)), "{handle}: the dialog on the set with its items, frames later");
        tried.push(handle);
        crate::dialogs::cancel(&mut h.app);
        h.frames(2);
    }
    assert!(!tried.is_empty(), "no timeline handle to double-click");
    // Edit from the API, frames later.
    h.app.edit_feature(feature);
    h.frames(5);
    assert_eq!(state(&h), Some((Some(set.group), 2)), "edit_feature, frames later");
}

/// The entry points the real app showed opening the wrong thing: a double-click on a gear in the
/// view (in an assembly set it activated the gear's component instead of opening the set), a
/// feature of the set's folder (a body transfer), and Gears pressed while a gear's component is
/// active. Each opens THIS set with its gears.
#[test]
fn a_gear_double_clicked_or_its_component_active_opens_its_set() {
    use crate::dialogs_gear_links::{GearItem, Suggested};
    let mut app = app();
    let (s1, s2, s3) = items_drawing(&mut app);
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    app.start("gear.create");
    let mut d = app.dialog.clone().unwrap();
    let Kind::Gear(g) = &mut d.kind else { panic!() };
    g.items = vec![GearItem { picks: vec![at(s1, "c1")], ..GearItem::new(ItemKind::Spur) }, GearItem { picks: vec![at(s1, "c2"), at(s3, "s"), at(s2, "c3")], ..GearItem::new(ItemKind::Compound) }];
    g.links.accept(&Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
    g.assembly = true;
    let c = apply_commands(&app, &d).unwrap();
    crate::dialogs::cancel(&mut app);
    run_all(&mut app, c);
    let set = solidcraft_engine::gear_sets(&app.session).into_iter().next().expect("a set");
    let check = |app: &SolveApp, how: &str| {
        let g = gear(app);
        assert_eq!((g.group, g.items.len()), (Some(set.group), 2), "{how}");
    };
    // A double-click on a gear body.
    let body = set.bodies.first().cloned().expect("a body");
    assert!(double_click_body(&mut app, &body));
    check(&app, "double-click on a gear");
    crate::dialogs::cancel(&mut app);
    assert!(!double_click_body(&mut app, "not a gear"), "another body is left to the view");
    // Every feature in the set's folder belongs to it.
    let folder: Vec<u64> = app.session.doc.timeline_groups.iter().filter(|g| g.id == set.group || Some(g.id) == set.feature).flat_map(|g| g.features.clone()).collect();
    for f in folder {
        assert_eq!(set_of_feature(&app, f), Some(set.group), "feature {f} of the folder");
    }
    // A gear's component active, nothing selected, then Gears.
    let component = crate::open_meshing::session_world(&app.session).body(&body).map(|b| b.component).filter(|c| *c != 0);
    if let Some(component) = component {
        app.session.active_component = component;
        app.session.selection.clear();
        app.start("gear.create");
        check(&app, "a gear's component active + Gears");
    }
}

/// Bore and hub circles: a circle drawn on a spur's centre inside its pitch circle, picked while
/// the gear is active, is its bore (the smaller) or hub (the larger) — not a new gear; sent as
/// components with slot bore / hub; the engine's detail suggestions are offered and Use takes
/// them; Edit gets them back; the reply's bore, hub and side A are read.
#[test]
fn bore_and_hub_circles_from_picks_and_suggestions() {
    let mut app = app();
    let s = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    for (id, x, r) in [("a", 0.0, 20.0), ("b", 50.0, 30.0), ("bore", 0.0, 4.0), ("hub", 0.0, 8.0), ("bbore", 50.0, 4.0)] {
        app.session.execute("sketch.circle.center", &json!({"center": [x, 0], "radius": r, "id": id})).unwrap();
    }
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let at = |id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(s) };
    let (mut g, mut inputs) = opened(&mut app);
    click(&app, &mut g, &mut inputs, at("a"));
    // The hub first, then the bore: the smaller is the bore whichever came first.
    click(&app, &mut g, &mut inputs, at("hub"));
    click(&app, &mut g, &mut inputs, at("bore"));
    assert_eq!(g.items.len(), 1, "no new gear for a circle inside: {:?}", g.refused);
    assert_eq!(g.items[0].details, vec![(at("bore"), false), (at("hub"), true)]);
    assert_eq!(inputs[0].items.len(), 3, "the input holds the gear's pitch circle, bore and hub");
    assert!(g.items[0].take_detail(&app, &at("bbore")).is_none(), "another gear's circle is no detail of this one");
    // Gear B by a pick, then the request.
    click(&app, &mut g, &mut inputs, at("b"));
    assert_eq!(g.items.len(), 2);
    let mut d = app.dialog.clone().unwrap();
    d.kind = Kind::Gear(g.clone());
    d.inputs = inputs.clone();
    let p = apply_commands(&app, &d).unwrap()[0].1.clone();
    let slots: Vec<&str> = p["items"][0]["components"].as_array().unwrap().iter().map(|c| c["slot"].as_str().unwrap()).collect();
    assert_eq!(slots, ["pitch", "bore", "hub"], "{p}");
    // The engine's reply: B's bore circle suggested; A's bore and hub as built, and side A.
    let r = app.session.execute("gear.calculate", &p).unwrap();
    let offers = details_suggested_of(&r);
    let offer = offers.iter().find(|o| o.item == 1).unwrap_or_else(|| panic!("B's bore offered: {r}"));
    assert!(!offer.hub && (offer.diameter - 8.0).abs() < 1e-6 && offer.sel == at("bbore"), "{offer:?}");
    let built = built_of(&r);
    let a = built.iter().find(|b| b.item == 0).unwrap_or_else(|| panic!("{r}"));
    assert_eq!((a.bore, a.hub), (Some(8.0), Some(16.0)), "{r}");
    assert!(a.side_a.is_some_and(|v| (v[2].abs() - 1.0).abs() < 1e-9), "{a:?}");
    // Use: B's bore taken.
    use_detail(&app, &mut g, offer, &mut inputs);
    assert_eq!(g.items[1].details, vec![(at("bbore"), false)]);
    // Edit restores them.
    d.kind = Kind::Gear(g.clone());
    let p = apply_commands(&app, &d).unwrap()[0].1.clone();
    let set = solidcraft_engine::schema::gear::GearSet { group: 7, feature: Some(8), name: "Gears".into(), bodies: Vec::new(), features: Vec::new(), request: p, params: json!({}), sketch: None, sketch_changed: false };
    let e = for_set(&set);
    assert_eq!(e.items[0].details, g.items[0].details);
    assert_eq!(e.items[1].details, g.items[1].details);
}

/// The housing cutout: off by default; switched on, a bearing point picked on a gear's shaft line
/// joins that gear (not refused) and is shown as an optional repeating row, the request carries
/// `clearance_body` with the bearing, the preview has the swept envelope and a seat ring at the
/// point, and Edit gets the settings back.
#[test]
fn the_housing_cutout_and_bearing_points_on_shaft_lines() {
    let mut app = app();
    let s1 = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.circle.center", &json!({"center": [0, 0], "radius": 20.0, "id": "a"})).unwrap();
    app.session.execute("sketch.circle.center", &json!({"center": [50, 0], "radius": 30.0, "id": "b"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let s2 = app.session.execute("sketch.create", &json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.line", &json!({"points": [[0, -40], [0, 40]], "ids": ["s1"]})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [0, -25], "id": "p1"})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [8, -25], "id": "off"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let (mut g, mut inputs) = opened(&mut app);
    assert!(!g.cutout.on, "off by default");
    click(&app, &mut g, &mut inputs, Sel::SketchCurve { id: "a".into(), sketch: Some(s1) });
    click(&app, &mut g, &mut inputs, Sel::SketchCurve { id: "s1".into(), sketch: Some(s2) });
    click(&app, &mut g, &mut inputs, Sel::ExternalSketchPoint { sketch: s2, id: "p1".into() });
    assert_eq!((g.items.len(), g.items[0].picks.len()), (1, 3), "the point joins gear A: {:?}", g.refused);
    // A point off the shaft line: refused with why.
    click(&app, &mut g, &mut inputs, Sel::ExternalSketchPoint { sketch: s2, id: "off".into() });
    assert!(next_step(&app, &g).0.contains("off the shaft line"), "{:?}", next_step(&app, &g));
    let len = |v: f64| fmt(v);
    let rows = g.items[0].slot_rows(&app, &len);
    assert!(rows.iter().any(|r| r.what == "bearing point" && r.filled.is_some() && r.repeats), "{rows:?}");
    click(&app, &mut g, &mut inputs, Sel::SketchCurve { id: "b".into(), sketch: Some(s1) });
    g.cutout.on = true;
    let mut d = app.dialog.clone().unwrap();
    d.kind = Kind::Gear(g.clone());
    d.inputs = inputs.clone();
    let p = apply_commands(&app, &d).unwrap()[0].1.clone();
    assert_eq!(p["clearance_body"]["bearing"], json!("608"), "{p}");
    let slots: Vec<&str> = p["items"][0]["components"].as_array().unwrap().iter().map(|c| c["slot"].as_str().unwrap()).collect();
    assert_eq!(slots, ["pitch", "axis", "bearing"], "{p}");
    refresh(&mut app, &mut g, &inputs);
    assert!(g.cutout_rings.iter().any(|(i, _)| i.is_none()), "the swept envelope");
    let seat = g.cutout_rings.iter().find(|(i, _)| *i == Some(0)).expect("a seat ring for gear A");
    // A 608's seat: 11 mm round A's axis at the point.
    let p0 = seat.1[0];
    assert!(((p0[0] * p0[0] + p0[1] * p0[1]).sqrt() - 11.0).abs() < 1e-6, "{p0:?}");
    // Edit: the settings come back.
    let set = solidcraft_engine::schema::gear::GearSet { group: 7, feature: Some(8), name: "Gears".into(), bodies: Vec::new(), features: Vec::new(), request: p, params: json!({}), sketch: None, sketch_changed: false };
    let e = for_set(&set);
    assert!(e.cutout.on && e.cutout.bearing == g.cutout.bearing, "{:?}", e.cutout);
    // The tooth chamfer: one setting for the set, sent as `tooth_chamfer`, back on Edit, gone
    // when emptied.
    assert!(set.request.get("tooth_chamfer").is_none(), "off by default");
    let mut c = g.clone();
    c.chamfer = "0.5 mm".into();
    c.chamfer_faces = 1;
    d.kind = Kind::Gear(c);
    let p = apply_commands(&app, &d).unwrap()[0].1.clone();
    assert_eq!(p["tooth_chamfer"], json!({"size": 0.5, "faces": "a"}), "{p}");
    let e = for_set(&solidcraft_engine::schema::gear::GearSet { request: p.clone(), ..set.clone() });
    assert_eq!((e.chamfer.as_str(), e.chamfer_faces), ("0.5 mm", 1));
    let mut off = e.clone();
    off.chamfer.clear();
    d.kind = Kind::Gear(off);
    let p = apply_commands(&app, &d).unwrap()[0].1.clone();
    assert!(p.get("tooth_chamfer").is_none(), "{p}");
}

/// QA round: the sketch defines the details — a gear made here takes the bore and hub circles
/// drawn on its centre by itself (× takes one out for good); a side view clicked as its two shaft
/// lines is complete; a collision the drawing's suggestion resolves isn't said; two circles a
/// little apart get a note; an Edit with nothing changed runs nothing.
#[test]
fn qa_details_by_themselves_side_view_by_lines_and_quiet_edits() {
    let mut app = app();
    let s = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    for (id, x, r) in [("a", 0.0, 20.0), ("abore", 0.0, 4.0), ("ahub", 0.0, 8.0), ("b", 50.0, 30.0), ("bbore", 50.0, 5.0), ("c", 0.0, 0.0)] {
        if r > 0.0 {
            app.session.execute("sketch.circle.center", &json!({"center": [x, 0], "radius": r, "id": id})).unwrap();
        }
    }
    app.session.execute("sketch.circle.center", &json!({"center": [-58, 0], "radius": 30.0, "id": "near"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let at = |id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(s) };
    let (mut g, mut inputs) = opened(&mut app);
    click(&app, &mut g, &mut inputs, at("a"));
    click(&app, &mut g, &mut inputs, at("b"));
    for _ in 0..3 {
        refresh(&mut app, &mut g, &inputs);
        auto_details(&app, &mut g, &mut inputs);
    }
    assert_eq!(g.items[0].details, vec![(at("abore"), false), (at("ahub"), true)], "A's bore and hub from the drawing");
    assert_eq!(g.items[1].details, vec![(at("bbore"), false)], "B's too, though B was made after A");
    // × on B's bore: out, and not taken again.
    drop_detail(&mut g, 1, &at("bbore"), &mut inputs);
    refresh(&mut app, &mut g, &inputs);
    auto_details(&app, &mut g, &mut inputs);
    assert!(g.items[1].details.is_empty() && g.items[1].dropped == vec![at("bbore")]);
    // A circle 8 mm from A, not linked: a note on its card.
    click(&app, &mut g, &mut inputs, at("near"));
    refresh(&mut app, &mut g, &inputs);
    let notes: Vec<&str> = g.item_msgs[2].iter().map(|m| m.text.as_str()).collect();
    assert!(notes.iter().any(|n| n.starts_with("Gears A and C are 8 mm apart, so they won't mesh")), "{notes:?}");
    // Collisions named "Gear D gear 1 and Gear E gear 1" (or B1/B2) read as the items they name.
    assert_eq!(gears_named("Gear D gear 1 and Gear E gear 1 collide: link them"), vec![3, 4]);
    assert_eq!(gears_named("Gear B1 and Gear C overlap"), vec![1, 2]);
    // A bevel side view by its two shaft lines meeting at a corner: complete, sized by settings.
    let v = app.session.execute("sketch.create", &json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.line", &json!({"points": [[0, 0], [0, 60]], "ids": ["s1"]})).unwrap();
    app.session.execute("sketch.line", &json!({"points": [[0, 0], [60, 0]], "ids": ["s2"]})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let (mut b, mut binputs) = opened(&mut app);
    b.new_kind = type_of(ItemKind::Bevel);
    new_gear(&mut b, &mut binputs);
    for id in ["s1", "s2"] {
        click(&app, &mut b, &mut binputs, Sel::SketchCurve { id: id.into(), sketch: Some(v) });
    }
    assert!(b.items[0].complete(&app), "{:?}", b.refused);
    assert!(next_step(&app, &b).0.contains("sized by its gears' teeth and module"), "{:?}", next_step(&app, &b));
    // Edit with nothing changed: OK runs nothing.
    let mut spur = new(None);
    spur.items = vec![GearItem { picks: vec![at("a")], ..GearItem::new(ItemKind::Spur) }];
    spur.group = Some(1);
    let p = request(&app, &spur, &inputs).unwrap();
    spur.original = Some(comparable(&p));
    assert!(commands(&app, &spur, &inputs, &Map::new()).unwrap().is_empty());
    spur.items[0].gears = vec![crate::dialogs_gear_items::GearSettings { module: "3 mm".into(), ..Default::default() }];
    assert_eq!(commands(&app, &spur, &inputs, &Map::new()).unwrap()[0].0, "gear.update");
}

/// Round 2: a refusal's suggestions are one click each on the card; "item" is reworded only
/// where it names an item; a gear made here keeps the circles it dropped through save and Edit,
/// and takes a new one drawn later by itself.
#[test]
fn round_two_fixes_wording_and_new_circles_on_edit() {
    let mut app = app();
    let s = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    for (id, x, r) in [("a", 0.0, 20.0), ("abore", 0.0, 4.0), ("b", 50.0, 30.0)] {
        app.session.execute("sketch.circle.center", &json!({"center": [x, 0], "radius": r, "id": id})).unwrap();
    }
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let (mut g, mut inputs) = opened(&mut app);
    click(&app, &mut g, &mut inputs, Sel::SketchCurve { id: "a".into(), sketch: Some(s) });
    click(&app, &mut g, &mut inputs, Sel::SketchCurve { id: "b".into(), sketch: Some(s) });
    for _ in 0..3 {
        refresh(&mut app, &mut g, &inputs);
        auto_details(&app, &mut g, &mut inputs);
    }
    assert_eq!(g.items.len(), 2);
    // The fixes a refusal names.
    let text = "holding the drawn centre distances needs more profile shift than these teeth take at module 2 (Gear A z18 and Gear B z12): Gear A would be undercut. These would work: module 1.5 (Gear A z24, Gear B z16), Gear B with 14 teeth (or draw it Ø28)";
    let fixes = fixes_in(&g, text);
    assert_eq!(fixes, vec![Fix::Module(1.5), Fix::Teeth { item: 1, gear: 0, teeth: 14 }], "{text}");
    assert_eq!(fixes[1].label(&g), "Use 14 teeth on B");
    apply_fix(&mut g, 0, &fixes[1], &inputs);
    assert!(g.items[1].gears[0].by_teeth && g.items[1].gears[0].teeth == "14");
    apply_fix(&mut g, 0, &fixes[0], &inputs);
    assert_eq!(g.items[0].gears[0].module, "1.5 mm");
    // "item" only where an item's letter follows.
    assert_eq!(worded("item B has no gear 2; Item C meshes nothing; items here"), "Gear B has no gear 2; Gear C meshes nothing; items here");
    // A's bore was taken by itself: drop it, save, and the reopened set keeps it out but takes a
    // circle drawn on B later.
    assert!(g.items[0].details.iter().any(|(sel, _)| *sel == Sel::SketchCurve { id: "abore".into(), sketch: Some(s) }), "{:?}", g.items[0].details);
    let abore = Sel::SketchCurve { id: "abore".into(), sketch: Some(s) };
    drop_detail(&mut g, 0, &abore, &mut inputs);
    g.items[0].gears[0] = Default::default();
    g.items[1].gears[0] = Default::default();
    let mut d = app.dialog.clone().unwrap();
    d.kind = Kind::Gear(g.clone());
    d.inputs = inputs.clone();
    let p = apply_commands(&app, &d).unwrap()[0].1.clone();
    assert_eq!(p["ui"]["gear_dropped"][0], json!([abore]), "{p}");
    app.session.execute("sketch.edit", &json!({"sketch": s})).unwrap();
    app.session.execute("sketch.circle.center", &json!({"center": [50, 0], "radius": 5.0, "id": "bbore"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let set = solidcraft_engine::schema::gear::GearSet { group: 7, feature: Some(8), name: "Gears".into(), bodies: Vec::new(), features: Vec::new(), request: p, params: json!({}), sketch: None, sketch_changed: false };
    let mut e = for_set(&set);
    let mut inputs2 = inputs.clone();
    for _ in 0..3 {
        refresh(&mut app, &mut e, &inputs2);
        auto_details(&app, &mut e, &mut inputs2);
    }
    assert!(e.items[0].details.is_empty(), "the dropped bore stays out: {:?}", e.items[0].details);
    assert!(e.items[1].details.iter().any(|(sel, _)| *sel == Sel::SketchCurve { id: "bbore".into(), sketch: Some(s) }), "the new circle on B is taken: {:?}", e.items[1].details);
}

/// Where a point sits on a line (a bearing point on a shaft line, a line's end such as a bevel's
/// apex), a click there picks the point, not the line; a point on no line (a free point, a
/// circle's centre) leaves the click to the curve under it.
#[test]
fn points_on_lines_win_over_the_line_in_picking() {
    use crate::viewport::Hit;
    use solidcore_record::{Vec2, Vec3};
    let mut app = app();
    let s = app.session.execute("sketch.create", &json!({"plane": "XZ"})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.line", &json!({"points": [[0, 0], [0, 40]], "ids": ["s"]})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [0, 25], "id": "brg"})).unwrap();
    app.session.execute("sketch.point", &json!({"point": [10, 25], "id": "free"})).unwrap();
    app.session.execute("sketch.circle.center", &json!({"center": [30, 30], "radius": 5, "id": "c"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    assert!(takes_point_hit(&app.session, s, "brg"), "a point on the line");
    assert!(!takes_point_hit(&app.session, s, "free"), "a point on no line");
    let st = crate::open_meshing::session_world(&app.session);
    let sk = &st.sketch(s).unwrap().sketch;
    let end = sk.points[match sk.curves.iter().find(|c| c.id == "s").unwrap().kind {
        solidcraft_engine::sketch::CurveKind::Line { a, .. } => a,
        _ => unreachable!(),
    }]
    .id
    .clone();
    let centre = sk.points[match sk.curves.iter().find(|c| c.id == "c").unwrap().kind {
        solidcraft_engine::sketch::CurveKind::Circle { c, .. } => c,
        _ => unreachable!(),
    }]
    .id
    .clone();
    drop(st);
    assert!(takes_point_hit(&app.session, s, &end), "a line's end (an apex)");
    assert!(!takes_point_hit(&app.session, s, &centre), "a circle's centre");
    app.start("gear.create");
    let d = app.dialog.as_ref().unwrap();
    assert!(d.inputs[0].accept & crate::selection::POINTS != 0 && d.inputs[0].accept & crate::selection::EXTERNAL_POINTS != 0);
    let point = Hit::SketchPoint { sketch: s, id: "brg".into(), at: Vec2::new(0.0, 25.0) };
    let line = Hit::SketchCurve { sketch: s, id: "s".into(), straight: true, point: Vec3::new(0.0, 0.0, 25.0) };
    let free = Hit::SketchPoint { sketch: s, id: "free".into(), at: Vec2::new(10.0, 25.0) };
    // Points come before curves in a pick's hits: the bearing point wins; a free point yields.
    let (_, sel) = crate::viewport::candidate(&app, &[point.clone(), line.clone()]).unwrap();
    assert_eq!(sel, Sel::ExternalSketchPoint { sketch: s, id: "brg".into() });
    let (h, _) = crate::viewport::candidate(&app, &[free, line.clone()]).unwrap();
    assert_eq!(h, line);
}

/// The tooth chamfer (All gears) shows in the preview: on each chamfered end the tip circle steps
/// in by the size, with a second circle the size back from the face.
#[test]
fn the_tooth_chamfer_shows_in_the_preview() {
    let mut app = app();
    let (s1, _, _) = items_drawing(&mut app);
    let (mut g, mut inputs) = opened(&mut app);
    g.new_kind = type_of(ItemKind::Spur);
    new_gear(&mut g, &mut inputs);
    click(&app, &mut g, &mut inputs, Sel::SketchCurve { id: "c1".into(), sketch: Some(s1) });
    refresh(&mut app, &mut g, &inputs);
    let rings = |g: &Gear| g.shape.iter().filter(|(_, l)| l.len() == 49).count();
    let plain = rings(&g);
    assert!(plain > 0, "the outline");
    g.chamfer = "0.5 mm".into();
    refresh(&mut app, &mut g, &inputs);
    assert_eq!(rings(&g), plain + 2, "a second circle at each end");
    let radii: Vec<f64> = g.shape.iter().filter(|(_, l)| l.len() == 49).map(|(_, l)| (l[0][0] * l[0][0] + l[0][1] * l[0][1]).sqrt()).collect();
    let tip = radii.iter().copied().fold(0.0, f64::max);
    assert!(radii.iter().any(|r| (r - (tip - 0.5)).abs() < 1e-6), "the tip stepped in by 0.5: {radii:?}");
    g.chamfer_faces = 1;
    refresh(&mut app, &mut g, &inputs);
    assert_eq!(rings(&g), plain + 1, "side A only");
}

/// The Gears dialog as the harness draws it, and its state.
fn drawn(h: &crate::scenario::Harness) -> Gear {
    gear(&h.app)
}

/// Changes the open Gears dialog's state (as the form would) and draws it again.
fn with_dialog(h: &mut crate::scenario::Harness, f: impl FnOnce(&mut Gear, &mut Vec<SelInput>)) {
    if let Some(d) = h.app.dialog.as_mut()
        && let Kind::Gear(g) = &mut d.kind
    {
        f(g, &mut d.inputs);
    }
    h.frames(3);
}

/// The wheel turned over the dialog's tab body: it scrolls down to its foot.
fn scroll_down(h: &mut crate::scenario::Harness) {
    let Some(body) = crate::scenario::rect_of("dialog:body:viewport") else { return };
    for _ in 0..10 {
        h.app.synthetic.push(egui::Event::PointerMoved(body.center()));
        h.app.synthetic.push(egui::Event::MouseWheel {
            source: egui::MouseWheelSource::Unknown,
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -400.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.frame();
    }
    h.app.synthetic.push(egui::Event::PointerGone);
    h.frames(60);
}

/// A bevel pair from two axis lines and a circle, in the harness's design.
fn bevel_pair(app: &mut SolveApp) -> crate::dialogs_gear_links::GearItem {
    let s1 = app.session.execute("sketch.create", &json!({"plane": "XY"})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.line", &json!({"points": [[10, 0], [50, 0]], "ids": ["l1"]})).unwrap();
    app.session.execute("sketch.line", &json!({"points": [[10, 0], [10, 40]], "ids": ["l2"]})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let s2 = app.session.execute("sketch.create", &json!({"plane": {"origin": [30, 0, 0], "x_dir": [0, 1, 0], "y_dir": [0, 0, 1]}})).unwrap()["sketch"].as_u64().unwrap();
    app.session.execute("sketch.circle.center", &json!({"center": [0, 0], "radius": 10, "id": "c"})).unwrap();
    app.session.execute("sketch.finish", &json!({})).unwrap();
    let at = |sk: u64, id: &str| Sel::SketchCurve { id: id.into(), sketch: Some(sk) };
    crate::dialogs_gear_links::GearItem { picks: vec![at(s1, "l1"), at(s1, "l2"), at(s2, "c")], ..crate::dialogs_gear_links::GearItem::new(ItemKind::Bevel) }
}

/// The tab row: the dialog opens on Gears with its type list and New gear; each tab clicked shows
/// its own groups and none of the others'; the choice stays while the dialog is open. Without a
/// differential there is no Differential tab, and a tab that goes away falls back to Gears.
#[test]
fn tabs_switch_and_show_their_own_groups() {
    use crate::scenario::handle_at;
    use crate::test_support::click as tap;
    crate::dialogs_gear::DIFF_SUPPORTED.with(|d| d.set(Some(false)));
    let mut h = crate::scenario::Harness::new();
    h.app.start("gear.create");
    h.frames(3);
    assert_eq!(drawn(&h).tab, Tab::Gears);
    for t in [Tab::Gears, Tab::Teeth, Tab::Shafts, Tab::Motion, Tab::Housing, Tab::Results] {
        assert!(handle_at(crate::dialogs_gear_view::tab_handle(t)).is_some(), "{t:?} is in the tab row");
    }
    assert!(handle_at("gear:tab:differential").is_none(), "no differential, no tab");
    assert!(handle_at("gear:new").is_some() && crate::scenario::rect_of("gear:status").is_some());
    // Teeth: the set's tooth form and chamfer; Gears' New gear is gone, the status line stays.
    tap(&mut h, "gear:tab:teeth");
    assert_eq!(drawn(&h).tab, Tab::Teeth);
    assert!(handle_at("gear:new").is_none(), "Gears' rows are not drawn on Teeth");
    assert!(crate::scenario::rect_of("gear:status").is_some(), "the status line stays at the top");
    assert!(handle_at("gear:group:chamfer").is_some());
    assert!(handle_at("dialog:section:Tooth form · every gear").is_some());
    // Housing & Output: the cutout's switch and the output; Teeth's chamfer is gone.
    tap(&mut h, "gear:tab:housing");
    assert_eq!(drawn(&h).tab, Tab::Housing);
    assert!(handle_at("gear:group:housing_cutout").is_some() && handle_at("gear:group:keep_out").is_some());
    assert!(handle_at("gear:group:chamfer").is_none());
    // Links & Motion with no gears says to make some; Results says nothing is worked out yet.
    tap(&mut h, "gear:tab:motion");
    assert_eq!(drawn(&h).tab, Tab::Motion);
    assert!(handle_at("gear:accept").is_none() && handle_at("gear:add_link").is_none(), "no links without two gears");
    tap(&mut h, "gear:tab:results");
    assert_eq!(drawn(&h).tab, Tab::Results);
    h.frames(2);
    assert_eq!(drawn(&h).tab, Tab::Results, "the tab chosen stays");
    tap(&mut h, "gear:tab:gears");
    assert!(handle_at("gear:new").is_some());
    // A tab that isn't offered (Differential with no differential) shows Gears.
    with_dialog(&mut h, |g, _| g.tab = Tab::Differential);
    assert_eq!(drawn(&h).tab, Tab::Gears);
    crate::dialogs_gear::DIFF_SUPPORTED.with(|d| d.set(None));
}

/// Groups a checkbox switches on show their rows only while on: the tooth chamfer (switched off,
/// its size goes, so no chamfer is sent), the housing cutout, the keep-out and a gear's hub; the
/// per-gear tabs follow the gear chosen on them, which becomes the one picks go to.
#[test]
fn checkbox_groups_switch_their_rows_and_the_request() {
    use crate::scenario::handle_at;
    use crate::test_support::click as tap;
    let mut h = crate::scenario::Harness::new();
    let (s1, _, _) = items_drawing(&mut h.app);
    h.app.start("gear.create");
    h.frames(3);
    with_dialog(&mut h, |g, inputs| {
        g.new_kind = type_of(ItemKind::Spur);
        new_gear(g, inputs);
        inputs[0].items.push(Sel::SketchCurve { id: "c1".into(), sketch: Some(s1) });
    });
    assert_eq!(drawn(&h).items.len(), 1);
    // Tooth chamfer: off, then on (its size row), a size typed, then off again: no chamfer.
    tap(&mut h, "gear:tab:teeth");
    assert!(!drawn(&h).chamfer_open);
    tap(&mut h, "gear:group:chamfer");
    assert!(drawn(&h).chamfer_open, "the group is on");
    with_dialog(&mut h, |g, _| g.chamfer = "0.4 mm".into());
    let p = request(&h.app, &drawn(&h), &h.app.dialog.as_ref().unwrap().inputs).unwrap();
    assert_eq!(p["tooth_chamfer"]["size"].as_f64(), Some(0.4));
    tap(&mut h, "gear:group:chamfer");
    let g = drawn(&h);
    assert!(!g.chamfer_open && g.chamfer.is_empty(), "off clears the size");
    assert!(request(&h.app, &g, &h.app.dialog.as_ref().unwrap().inputs).unwrap().get("tooth_chamfer").is_none());
    // Shafts & Bores: the selected gear's bore and its hub group.
    tap(&mut h, "gear:tab:shafts");
    assert!(handle_at("gear:group:hub:A:0").is_some(), "the selected gear's hub switch");
    tap(&mut h, "gear:group:hub:A:0");
    assert!(drawn(&h).items[0].gears[0].hub, "the hub is on");
    tap(&mut h, "gear:group:hub:A:0");
    assert!(!drawn(&h).items[0].gears[0].hub);
    // No gear selected: the per-gear rows wait for one; the set's rows still show.
    with_dialog(&mut h, |g, _| g.active = None);
    assert!(handle_at("gear:group:hub:A:0").is_none());
    tap(&mut h, "gear:tab:teeth");
    assert!(handle_at("gear:group:chamfer").is_some());
    // Housing cutout and keep-out.
    tap(&mut h, "gear:tab:housing");
    tap(&mut h, "gear:group:housing_cutout");
    assert!(drawn(&h).cutout.on);
    tap(&mut h, "gear:group:keep_out");
    assert!(drawn(&h).cut_keep_out);
    assert!(handle_at("gear:targets").is_some(), "keep-out asks for its Targets");
    tap(&mut h, "gear:group:keep_out");
    tap(&mut h, "gear:group:housing_cutout");
    let g = drawn(&h);
    assert!(!g.cutout.on && !g.cut_keep_out);
}

/// A bevel or hypoid pair carries a differential in its ring gear when the engine takes one: its
/// switch on the Gears tab adds the Differential tab, which sends the item's `differential`
/// ({style, pinions, module, axle}); Edit restores it; switched off, nothing is sent and the tab
/// goes. With an engine that doesn't take one, neither the switch nor the tab shows.
#[test]
fn a_differential_switch_on_a_ring_gear_and_its_tab() {
    use crate::dialogs_gear_items::DiffForm;
    use crate::scenario::handle_at;
    use crate::test_support::click as tap;
    let mut h = crate::scenario::Harness::new();
    let bevel = bevel_pair(&mut h.app);
    assert!(bevel.complete(&h.app));
    h.app.start("gear.create");
    h.frames(3);
    with_dialog(&mut h, |g, inputs| {
        g.items = vec![bevel.clone()];
        make_active(g, 0, inputs);
    });
    // An engine without differentials: no switch, no tab, nothing sent.
    crate::dialogs_gear::DIFF_SUPPORTED.with(|d| d.set(Some(false)));
    h.frames(3);
    assert!(handle_at("gear:group:differential").is_none() && handle_at("gear:tab:differential").is_none());
    let inputs = h.app.dialog.as_ref().unwrap().inputs.clone();
    let p = request(&h.app, &drawn(&h), &inputs).unwrap();
    assert!(p["items"][0].get("differential").is_none(), "{p}");
    // One that takes them: the switch on the bevel's group adds the tab.
    crate::dialogs_gear::DIFF_SUPPORTED.with(|d| d.set(Some(true)));
    h.frames(3);
    assert!(handle_at("gear:tab:differential").is_none());
    scroll_down(&mut h);
    tap(&mut h, "gear:group:differential");
    assert_eq!(drawn(&h).items[0].diff, Some(DiffForm::default()));
    assert!(handle_at("gear:tab:differential").is_some());
    scroll_down(&mut h);
    tap(&mut h, "gear:to_differential");
    assert_eq!(drawn(&h).tab, Tab::Differential);
    assert!(handle_at("dialog:section:Half-shafts").is_some());
    with_dialog(&mut h, |g, _| {
        g.items[0].diff = Some(DiffForm { style: 2, pinions: "4".into(), module: "1.5 mm".into(), axle_length: "60 mm".into(), axle_d: String::new() });
    });
    let p = request(&h.app, &drawn(&h), &inputs).unwrap();
    assert_eq!(p["items"][0]["differential"], json!({"style": "torsen_b", "pinions": 4, "module": 1.5, "axle": {"length": 60.0}}), "{p}");
    // Edit reads it back as typed.
    let mut graph = crate::dialogs_gear_links::Links::default();
    let back = crate::dialogs_gear_items::items_of(p.as_object().unwrap(), p.get("ui"), &mut graph);
    assert_eq!(back[0].diff, drawn(&h).items[0].diff);
    // Switched off on its own tab: nothing sent, the tab goes and Gears shows.
    scroll_down(&mut h);
    tap(&mut h, "gear:group:differential");
    let g = drawn(&h);
    assert_eq!((g.items[0].diff.clone(), g.tab), (None, Tab::Gears));
    assert!(handle_at("gear:tab:differential").is_none());
    assert!(request(&h.app, &g, &inputs).unwrap()["items"][0].get("differential").is_none());
    crate::dialogs_gear::DIFF_SUPPORTED.with(|d| d.set(None));
}

/// Two spur gears on the items drawing's touching circles c1 and c2, meshing.
fn meshing_pair(app: &mut SolveApp) -> (Gear, Vec<SelInput>) {
    let (s1, _, _) = items_drawing(app);
    let (mut g, mut inputs) = opened(app);
    g.new_kind = type_of(ItemKind::Spur);
    for id in ["c1", "c2"] {
        new_gear(&mut g, &mut inputs);
        click(app, &mut g, &mut inputs, Sel::SketchCurve { id: id.into(), sketch: Some(s1) });
    }
    g.links.accept(&crate::dialogs_gear_links::Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
    (g, inputs)
}

/// A block on the Gears tab: one button puts every gear in it; its module, shaft Ø, face width
/// and tooth chamfer go in the request's `module_blocks` with its gears naming it; its shaft Ø
/// makes one shaft per axis (listed on the Shafts tab); Edit reads it all back; × removes it.
#[test]
fn a_block_puts_gears_under_shared_settings() {
    use crate::scenario::handle_at;
    use crate::test_support::click as tap;
    let mut h = crate::scenario::Harness::new();
    let (pair, _) = meshing_pair(&mut h.app);
    h.app.dialog = None;
    h.app.start("gear.create");
    h.frames(3);
    with_dialog(&mut h, |g, _| {
        g.items = pair.items.clone();
        g.links = pair.links.clone();
        g.active = None;
    });
    assert!(drawn(&h).links.blocks().is_empty());
    // (No gear selected: the blocks are the foot of the Gears tab.)
    scroll_down(&mut h);
    tap(&mut h, "gear:block:all");
    let g = drawn(&h);
    assert_eq!(g.links.blocks().len(), 1);
    assert_eq!(g.links.blocks()[0].members, vec![0, 1]);
    with_dialog(&mut h, |g, _| {
        let b = g.links.block_mut(0).unwrap();
        (b.name, b.module, b.shaft_d, b.face, b.chamfer) = ("Main".into(), "1 mm".into(), "6 mm".into(), "8 mm".into(), "0.3 mm".into());
    });
    let inputs = h.app.dialog.as_ref().unwrap().inputs.clone();
    let p = request(&h.app, &drawn(&h), &inputs).unwrap();
    assert_eq!(p["module_blocks"], json!([{"id": "M1", "name": "Main", "module": 1.0, "shaft_diameter": 6.0, "face_width": 8.0, "tooth_chamfer": 0.3}]), "{p}");
    assert_eq!((p["items"][0]["module_block"].as_str(), p["items"][1]["module_block"].as_str()), (Some("M1"), Some("M1")));
    // One shaft per axis: the two gears are on two axes.
    let g = drawn(&h);
    assert_eq!(g.block_shafts.len(), 2, "{:?}", g.block_shafts);
    assert!(g.block_shafts.iter().all(|s| s.block == 0 && s.diameter == 6.0 && s.existing.is_none()));
    tap(&mut h, "gear:tab:shafts");
    assert!(handle_at("dialog:section:Shafts · 2").is_some(), "the Shafts tab lists both");
    assert!(handle_at("dialog:section:Gear bores · 2").is_some(), "the bores apart from the shafts");
    // Make shaft: each gear fused to its shaft (S1, S2), so no bore is drawn.
    let app = &h.app;
    let (d, from) = bore_of(app, &g, &g.block_shafts, 0, 0);
    assert_eq!((d, from.as_str()), (None, "fused to shaft S1: one body, no bore"));
    assert!(g.bores.is_empty(), "{:?}", g.bores);
    // Bore only: every gear bored to the Ø, no shaft made.
    with_dialog(&mut h, |g, _| g.links.block_mut(0).unwrap().shaft_mode = 1);
    let g = drawn(&h);
    assert!(g.block_shafts.is_empty());
    assert_eq!(bore_of(&h.app, &g, &g.block_shafts, 1, 0), (Some(6.0), "Main (Bore only)".to_string()));
    assert_eq!(g.bores.len(), 2, "a bore drawn on each gear");
    let pb = request(&h.app, &g, &inputs).unwrap();
    assert_eq!((pb["module_blocks"][0]["shaft"].as_str(), pb["module_blocks"][0]["shaft_diameter"].as_f64()), (Some("bore"), Some(6.0)));
    let set_of = |request: Value| solidcraft_engine::schema::gear::GearSet {
        group: 7,
        feature: Some(8),
        name: "Gears".into(),
        bodies: Vec::new(),
        features: Vec::new(),
        request,
        params: json!({}),
        sketch: None,
        sketch_changed: false,
    };
    assert_eq!(for_set(&set_of(pb)).links.blocks()[0].shaft_mode, 1);
    // None: no Ø sent, the gears keep their own bores.
    with_dialog(&mut h, |g, _| g.links.block_mut(0).unwrap().shaft_mode = 2);
    let pn = request(&h.app, &drawn(&h), &inputs).unwrap();
    assert!(pn["module_blocks"][0].get("shaft_diameter").is_none() && pn["module_blocks"][0]["shaft"] == "none", "{pn}");
    with_dialog(&mut h, |g, _| g.links.block_mut(0).unwrap().shaft_mode = 0);
    // Edit: the block back as typed, with its gears (an old block with a Ø is Make shaft).
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
    assert_eq!(e.links.blocks(), drawn(&h).links.blocks());
    // × takes the block away: its gears keep their own settings.
    tap(&mut h, "gear:tab:gears");
    scroll_down(&mut h);
    tap(&mut h, "gear:block:1:remove");
    assert!(drawn(&h).links.blocks().is_empty());
    let p = request(&h.app, &drawn(&h), &inputs).unwrap();
    assert_eq!(p["module_blocks"], json!([]));
}

/// Each link is drawn in the view from gear to gear (a mesh through the contact of its pitch
/// circles) with its block; hovering its row on Links & Motion lights it there.
#[test]
fn links_are_drawn_in_the_view_and_a_hovered_row_lights_its_link() {
    let mut app = app();
    let (mut g, inputs) = meshing_pair(&mut app);
    g.links.add_block(&[0, 1]);
    refresh(&mut app, &mut g, &inputs);
    let lines: Vec<&LinkLine> = g.link_lines.iter().filter(|l| !l.suggested).collect();
    assert_eq!(lines.len(), 1, "{:?}", g.link_lines);
    let l = lines[0];
    assert_eq!((l.link, l.block), ((0, 0, 1, 0, "mesh"), Some(0)));
    assert_eq!(l.points.len(), 3, "centre, contact, centre");
    assert!((l.points[1][0] - 20.0).abs() < 1e-6 && l.points[1][1].abs() < 1e-6, "the contact: {:?}", l.points[1]);
    // The row hovered on Links & Motion.
    let mut h = crate::scenario::Harness::new();
    let (pair, _) = meshing_pair(&mut h.app);
    h.app.dialog = None;
    h.app.start("gear.create");
    h.frames(3);
    with_dialog(&mut h, |k, _| {
        k.items = pair.items.clone();
        k.links = pair.links.clone();
        k.tab = Tab::Motion;
    });
    let row = crate::scenario::rect_of("gear:link:1").expect("the link's row");
    h.app.synthetic.push(egui::Event::PointerMoved(row.center()));
    h.frames(2);
    assert_eq!(drawn(&h).link_hovered, Some((0, 0, 1, 0, "mesh")));
    h.app.synthetic.push(egui::Event::PointerGone);
    h.frames(2);
    assert_eq!(drawn(&h).link_hovered, None);
}

/// The dialog is worked out again only when what it is worked out from changes; with the app
/// running work off its thread, `gear.calculate` runs there and its reply is taken in later.
#[test]
fn refresh_works_out_only_what_changed_and_calculates_off_the_thread() {
    let mut app = app();
    let (mut g, inputs) = meshing_pair(&mut app);
    refresh(&mut app, &mut g, &inputs);
    assert!(g.calc.is_some() && !g.shape.is_empty());
    // Nothing changed: nothing is worked out again.
    g.shape.clear();
    refresh(&mut app, &mut g, &inputs);
    assert!(g.shape.is_empty(), "worked out again with nothing changed");
    // A setting changed: worked out again.
    g.chamfer = "0.2 mm".into();
    refresh(&mut app, &mut g, &inputs);
    assert!(!g.shape.is_empty());
    // Off the thread: the last reply stays until the new one is in.
    app.apply_off_thread = Some(std::time::Duration::ZERO);
    let before = g.calc.clone();
    g.chamfer = "0.3 mm".into();
    refresh(&mut app, &mut g, &inputs);
    assert_eq!(g.calc, before, "the last reply shown while the new one is worked out");
    let start = std::time::Instant::now();
    while g.calc == before {
        assert!(start.elapsed().as_secs() < 60, "gear.calculate never came back");
        std::thread::sleep(std::time::Duration::from_millis(20));
        refresh(&mut app, &mut g, &inputs);
    }
    assert!(g.calc.as_ref().is_some_and(|(key, r)| key.contains("0.3") && r.is_ok()), "{:?}", g.calc);
}

/// Helical and herringbone are not types in the list: any gear's Teeth toggle makes them. An
/// older set's Helical or Herringbone gear reopens as a Spur with its teeth so (20° where it gave
/// no helix), and a new one made of those kinds is a Spur the same way.
#[test]
fn helical_and_herringbone_are_a_spurs_teeth_not_types() {
    use crate::dialogs_gear_items::{GearItem, items_of};
    use solidcraft_gear::items::ItemKind;
    assert!(!TYPES.iter().any(|t| matches!(t.0, ItemKind::Helical | ItemKind::Herringbone)));
    let new = GearItem::new(ItemKind::Herringbone);
    assert_eq!((new.kind, new.gears[0].helix.as_str(), new.gears[0].v), (ItemKind::Spur, "20 deg", true));
    let r = json!({"items": [
        {"id": "A", "type": "helical", "components": [], "gears": [{"gear": 0, "size_by": "module", "helix_angle": 15.0}]},
        {"id": "B", "type": "herringbone", "components": [], "gears": [{"gear": 0, "size_by": "module"}]},
        {"id": "C", "type": "spur", "components": [], "gears": [{"gear": 0, "size_by": "module"}]}
    ]});
    let mut graph = crate::dialogs_gear_links::Links::default();
    let back = items_of(r.as_object().unwrap(), None, &mut graph);
    assert!(back.iter().all(|it| it.kind == ItemKind::Spur), "{:?}", back.iter().map(|it| it.kind).collect::<Vec<_>>());
    assert_eq!((back[0].gears[0].helix.as_str(), back[0].gears[0].v), ("15 deg", false));
    assert_eq!((back[1].gears[0].helix.as_str(), back[1].gears[0].v), ("20 deg", true));
    assert_eq!((back[2].gears[0].helix.as_str(), back[2].gears[0].v), ("", false));
}

/// A block's Teeth toggle (under More), as a gear's: Herringbone gives the helix angle 20° where
/// none is typed and sends `herringbone`; Helical keeps the angle and drops it; Straight clears
/// the angle (each gear's own teeth). Edit reads it back.
#[test]
fn a_blocks_teeth_toggle_sends_helical_or_herringbone() {
    use crate::dialogs_gear_view::{set_teeth_style, teeth_style};
    let mut app = app();
    let (mut g, inputs) = meshing_pair(&mut app);
    g.links.add_block(&[0, 1]);
    let b = g.links.block_mut(0).unwrap();
    assert_eq!(teeth_style(&b.helix, b.herringbone), 0);
    set_teeth_style(&mut b.helix, &mut b.herringbone, 2);
    assert_eq!((b.helix.as_str(), b.herringbone, b.has_more()), ("20 deg", true, true));
    let p = request(&app, &g, &inputs).unwrap();
    assert_eq!((p["module_blocks"][0]["helix_angle"].as_f64(), p["module_blocks"][0]["herringbone"].as_bool()), (Some(20.0), Some(true)), "{p}");
    let back = crate::dialogs_gear_links::Block::of(&p["module_blocks"][0]);
    assert_eq!((teeth_style(&back.helix, back.herringbone), back.helix.as_str()), (2, "20 deg"));
    let b = g.links.block_mut(0).unwrap();
    b.helix = "15 deg".into();
    set_teeth_style(&mut b.helix, &mut b.herringbone, 1);
    let p = request(&app, &g, &inputs).unwrap();
    assert!(p["module_blocks"][0].get("herringbone").is_none() && p["module_blocks"][0]["helix_angle"].as_f64().is_some_and(|h| (h - 15.0).abs() < 1e-9), "{p}");
    let b = g.links.block_mut(0).unwrap();
    set_teeth_style(&mut b.helix, &mut b.herringbone, 0);
    let p = request(&app, &g, &inputs).unwrap();
    assert!(p["module_blocks"][0].get("helix_angle").is_none() && p["module_blocks"][0].get("herringbone").is_none(), "{p}");
}
