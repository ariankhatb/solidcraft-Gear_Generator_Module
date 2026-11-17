//! Reference robustness of a Gears feature made of items (GEAR_UI_SPEC §3): a drivetrain built
//! from fully defined sketches (the `examples/gears/scripts`) is edited every way a user
//! edits a design — dimensions, drags, other sketch entities added and deleted, renames, timeline
//! reorders, picked entities and sketches deleted and redrawn, items added and taken away, undo
//! and redo, save and reopen, rollback — and after each the set evaluates again, its bodies keep
//! their names, its components and joints their ids, its motion links their joints, and a
//! feature built on a gear body still works. A pick that is gone fails the feature with an error
//! naming the item, the slot and the sketch: it is never quietly read as another entity.

use serde_json::{Value, json};
use solidcraft_doc::FeatureKind;

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn errors(s: &Session) -> Vec<String> {
    s.model.results.iter().filter_map(|r| r.error.as_ref().map(|e| format!("{}: {e}", r.name))).collect()
}

fn no_errors(s: &Session) {
    let e = errors(s);
    assert!(e.is_empty(), "{e:?}");
}

fn gears_features(s: &Session) -> Vec<u64> {
    s.doc.features.iter().filter(|f| matches!(f.kind, FeatureKind::Gears { .. })).map(|f| f.id).collect()
}

fn volume(s: &mut Session, body: &str) -> f64 {
    run(s, "inspect.center_of_mass", json!({"bodies": [body]}))["volume"].as_f64().unwrap()
}

/// A script of `examples/gears/scripts`, run up to (and with) its `gear.create`; the dimensions
/// it made, by sketch, entities and type, with the parameter holding each.
struct Fix {
    s: Session,
    create: Value,
    dims: Vec<(String, Vec<String>, String, String)>,
}

impl Fix {
    fn load(script: &str) -> Fix {
        let v: Value = serde_json::from_str(script).unwrap();
        let mut s = Session::default();
        let mut create = Value::Null;
        let mut dims = Vec::new();
        let mut sketch = String::new();
        for c in v["commands"].as_array().unwrap() {
            let (id, p) = (c["command"].as_str().unwrap(), c["params"].clone());
            if id == "gear.ratio" {
                continue;
            }
            let r = run(&mut s, id, p.clone());
            match id {
                "sketch.create" => sketch = p["name"].as_str().unwrap_or_default().to_string(),
                "sketch.dimension" => {
                    if let Some(param) = r["param"].as_str() {
                        let ents = p["entities"].as_array().unwrap().iter().map(|e| e.as_str().unwrap_or_default().to_string()).collect();
                        dims.push((sketch.clone(), ents, p["type"].as_str().unwrap_or("auto").to_string(), param.to_string()));
                    }
                }
                "gear.create" => create = r,
                _ => {}
            }
        }
        no_errors(&s);
        assert!(create["feature"].as_u64().is_some(), "{create}");
        Fix { s, create, dims }
    }

    /// The parameter of the dimension on `entities` in `sketch` (of `kind`).
    fn dim(&self, sketch: &str, entities: &[&str], kind: &str) -> String {
        self.dims
            .iter()
            .find(|(sk, e, k, _)| sk == sketch && e.iter().map(String::as_str).eq(entities.iter().copied()) && k == kind)
            .map(|d| d.3.clone())
            .unwrap_or_else(|| panic!("no {kind} dimension on {entities:?} in {sketch}: {:?}", self.dims))
    }

    fn feature(&self) -> u64 {
        self.create["feature"].as_u64().unwrap()
    }
}

const SPUR: &str = include_str!("../../../../examples/gears/scripts/01-spur-pair.json");
const COMPOUND: &str = include_str!("../../../../examples/gears/scripts/03-compound-train.json");
const WORM: &str = include_str!("../../../../examples/gears/scripts/05-worm-and-wheel.json");
const PLANETARY: &str = include_str!("../../../../examples/gears/scripts/06-planetary.json");
const EVERYTHING: &str = include_str!("../../../../examples/gears/scripts/09-everything-drivetrain.json");

/// What must not change across an edit: the set's feature, its bodies' names, each member's
/// component and joint, and the motion links (by their joints).
#[derive(Clone, Debug, PartialEq)]
struct Ids {
    feature: u64,
    bodies: Vec<String>,
    members: Vec<(String, u64, Option<u64>)>,
    links: Vec<(u64, u64)>,
}

/// The one set made of items (the record and the feature as they stand).
fn ids(s: &Session) -> Ids {
    let recs: Vec<super::Record> = super::records(&s.doc).into_iter().filter(|r| r.request.get("items").is_some()).collect();
    assert_eq!(recs.len(), 1, "one set made of items");
    ids_of(s, &recs[0])
}

fn ids_of(s: &Session, rec: &super::Record) -> Ids {
    let feature = rec.feature.unwrap();
    let bodies = super::current_names(s, rec);
    let members: Vec<(String, u64, Option<u64>)> = rec.assembly.as_ref().map(|a| a.members.iter().map(|m| (m.body.clone(), m.component, m.joint)).collect()).unwrap_or_default();
    let ours: Vec<u64> = members.iter().filter_map(|m| m.2).collect();
    let mut links: Vec<(u64, u64)> = s.doc.assembly.links.iter().filter(|l| ours.contains(&l.a) || ours.contains(&l.b)).map(|l| (l.a, l.b)).collect();
    links.sort_unstable();
    Ids { feature, bodies, members, links }
}

/// Every body the set's feature names is in the model, with some volume, and every member's
/// component and joint is in the design; the members' bodies are the feature's.
fn whole(s: &mut Session) -> Ids {
    no_errors(s);
    let now = ids(s);
    assert_eq!(gears_features(s), vec![now.feature]);
    for b in now.bodies.clone() {
        assert!(volume(s, &b) > 10.0, "{b}");
    }
    for (body, c, j) in &now.members {
        assert!(now.bodies.contains(body), "member {body} is one of the set's bodies {:?}", now.bodies);
        assert!(s.doc.components.iter().any(|k| k.id == *c), "component {c} of {body}");
        if let Some(j) = j {
            assert!(s.doc.assembly.joints.iter().any(|k| k.id == *j), "joint {j} of {body}");
        }
    }
    now
}

/// A hole cut through gear body `body` (centre `at`, on the XY plane at height `z`): a feature
/// built on a gear body, which must keep working through the set's edits. Its name: "Hole".
fn hole(s: &mut Session, body: &str, at: [f64; 2], z: f64) {
    run(s, "sketch.create", json!({"plane": {"origin": [0, 0, z], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}, "name": "Hole sketch"}));
    run(s, "sketch.circle.center", json!({"center": at, "radius": 2.0, "id": "h"}));
    run(s, "sketch.finish", json!({}));
    run(s, "solid.extrude", json!({"sketch": "Hole sketch", "distance": 200, "direction": "symmetric", "operation": "cut", "targets": [body], "name": "Hole"}));
    no_errors(s);
}

/// The volume the hole takes out of `body` now (its volume with the hole suppressed, less with).
fn hole_cuts(s: &mut Session, body: &str) -> bool {
    let with = volume(s, body);
    let id = s.doc.features.iter().find(|f| f.name == "Hole").map(|f| f.id).unwrap();
    run(s, "timeline.suppress", json!({"features": [id], "suppressed": true}));
    let without = volume(s, body);
    run(s, "edit.undo", json!({}));
    without - with > 1.0
}

fn set_dim(s: &mut Session, param: &str, expr: &str) -> Value {
    run(s, "parameters.change", json!({"name": param, "expression": expr}))
}

fn undo(s: &mut Session) {
    run(s, "edit.undo", json!({}));
}

fn redo(s: &mut Session) {
    run(s, "edit.redo", json!({}));
}

/// The error of the set's feature.
fn feature_error(s: &Session, id: u64) -> Option<String> {
    s.model.result(id).and_then(|r| r.error.clone())
}

fn sketch_named(s: &Session, name: &str) -> u64 {
    s.doc.features.iter().find(|f| f.name == name && matches!(f.kind, FeatureKind::Sketch { .. })).map(|f| f.id).unwrap_or_else(|| panic!("no sketch {name}"))
}

/// The set's request as the feature keeps it.
fn request(s: &Session) -> Value {
    super::records(&s.doc).into_iter().find(|r| r.request.get("items").is_some()).unwrap().request
}

// ---------------------------------------------------------------------------------------------
// Spur pair: every kind of edit
// ---------------------------------------------------------------------------------------------

fn spur() -> (Fix, Ids, f64) {
    let mut f = Fix::load(SPUR);
    let before = whole(&mut f.s);
    assert_eq!(before.members.len(), 2, "{before:?}");
    assert_eq!(before.links.len(), 1, "{before:?}");
    let a = before.bodies[0].clone();
    assert!(a.contains('A'), "{before:?}");
    hole(&mut f.s, &a, [0.0, 0.0], 0.0);
    let v = volume(&mut f.s, &a);
    (f, before, v)
}

#[test]
fn spur_dimension_edits_keep_every_id_and_the_hole() {
    let (mut f, before, v0) = spur();
    let a = before.bodies[0].clone();
    // Gear A's diameter 40 → 48: it grows (B stays tangent, so moves).
    let d = f.dim("Pitch circles", &["a"], "diameter");
    set_dim(&mut f.s, &d, "48 mm");
    let now = whole(&mut f.s);
    assert_eq!(now, before, "a diameter edit keeps every id");
    let v1 = volume(&mut f.s, &a);
    assert!(v1 > v0 * 1.2, "gear A grew: {v0} → {v1}");
    assert!(hole_cuts(&mut f.s, &a), "the hole still cuts gear A");
    // B's centre 0 → 12 mm up (a distance dimension): it moves round A.
    let d2 = f.dim("Pitch circles", &["origin", "b.center"], "vertical");
    set_dim(&mut f.s, &d2, "12 mm");
    assert_eq!(whole(&mut f.s), before, "a distance edit keeps every id");
    // Joint frames follow B's shaft.
    let rec = super::records(&f.s.doc).into_iter().next().unwrap();
    let b_axis = rec.assembly.as_ref().unwrap().members[1].axis.unwrap();
    assert!((b_axis[0][1] - 12.0).abs() < 1e-3, "B's joint is on its new shaft: {b_axis:?}");
    // Undo both, redo both.
    undo(&mut f.s);
    undo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
    assert!((volume(&mut f.s, &a) - v0).abs() < 1e-3 * v0, "back to the first size");
    redo(&mut f.s);
    redo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
    assert!(hole_cuts(&mut f.s, &a));
}

#[test]
fn spur_unrelated_sketch_entities_added_and_deleted_keep_every_id() {
    let (mut f, before, _) = spur();
    let a = before.bodies[0].clone();
    let sk = sketch_named(&f.s, "Pitch circles");
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.circle.center", json!({"center": [-80, 0], "radius": 10.0, "id": "stray"}));
    run(&mut f.s, "sketch.line", json!({"points": [[-80, 30], [-40, 30]], "ids": ["stray_line"]}));
    run(&mut f.s, "sketch.finish", json!({}));
    assert_eq!(whole(&mut f.s), before, "an unrelated circle and line change nothing");
    // The circle drawn before the picked ones is taken away: picked entities' places shift.
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.delete", json!({"entities": ["stray", "stray_line"]}));
    run(&mut f.s, "sketch.finish", json!({}));
    assert_eq!(whole(&mut f.s), before);
    assert!(hole_cuts(&mut f.s, &a));
    undo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
    redo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
}

/// The picked circles come after an unrelated one in the sketch: deleting it moves every
/// picked circle down one place, and nothing may follow the places.
#[test]
fn deleting_an_entity_drawn_before_the_picks_keeps_every_pick() {
    let mut s = Session::default();
    let sk = run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Pitch"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.circle.center", json!({"center": [0, -90], "radius": 10.0, "id": "first"}));
    run(&mut s, "sketch.line", json!({"points": [[-60, -60], [60, -60]], "ids": ["l0"]}));
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 20.0, "id": "a"}));
    run(&mut s, "sketch.circle.center", json!({"center": [50, 0], "radius": 30.0, "id": "b"}));
    run(&mut s, "sketch.finish", json!({}));
    let r = run(
        &mut s,
        "gear.create",
        json!({"name": "Pair", "assembly": true, "items": [
            {"id": "A", "type": "spur", "components": [{"sketch": "Pitch", "curve": "a", "slot": "pitch"}]},
            {"id": "B", "type": "spur", "components": [{"sketch": "Pitch", "curve": "b", "slot": "pitch"}]}],
            "links": [{"a": {"item": "A"}, "b": {"item": "B"}, "kind": "mesh"}],
            "module_blocks": [{"id": "M", "module": 2, "members": ["A", "B"]}]}),
    );
    let before = whole(&mut s);
    let pd = |s: &Session| s.model.state().bodies.iter().filter(|b| before.bodies.contains(&b.name)).map(|b| b.name.clone()).collect::<Vec<_>>();
    assert_eq!(pd(&s).len(), 2, "{r}");
    let v_a = volume(&mut s, &before.bodies[0]);
    run(&mut s, "sketch.edit", json!({"sketch": sk}));
    run(&mut s, "sketch.delete", json!({"entities": ["first", "l0"]}));
    run(&mut s, "sketch.finish", json!({}));
    assert_eq!(whole(&mut s), before);
    assert!((volume(&mut s, &before.bodies[0]) - v_a).abs() < 1e-6 * v_a, "gear A is still read from circle a");
}

#[test]
fn spur_renames_keep_every_id_through_the_next_edit() {
    let (mut f, before, _) = spur();
    let (a, b) = (before.bodies[0].clone(), before.bodies[1].clone());
    let sk = sketch_named(&f.s, "Pitch circles");
    run(&mut f.s, "timeline.rename", json!({"feature": sk, "name": "Renamed circles"}));
    assert_eq!(whole(&mut f.s), before, "a sketch rename changes nothing");
    run(&mut f.s, "body.rename", json!({"body": a, "name": "Pinion"}));
    let comp_b = before.members[1].1;
    run(&mut f.s, "component.rename", json!({"component": comp_b, "name": "Wheel part"}));
    let renamed = whole(&mut f.s);
    assert_eq!(renamed.bodies, vec!["Pinion".to_string(), b.clone()], "{renamed:?}");
    let same_but_names = |x: &Ids| (x.feature, x.members.iter().map(|m| (m.1, m.2)).collect::<Vec<_>>(), x.links.clone());
    assert_eq!(same_but_names(&renamed), same_but_names(&before));
    // An edit after the renames: the set follows the sketch with the names kept.
    let d = f.dim("Pitch circles", &["a"], "diameter");
    set_dim(&mut f.s, &d, "44 mm");
    let after = whole(&mut f.s);
    assert_eq!(after.bodies, renamed.bodies, "the renamed body keeps its name");
    assert_eq!(same_but_names(&after), same_but_names(&before), "{after:?} vs {before:?}");
    assert_eq!(after.members, renamed.members);
    assert!(f.s.doc.components.iter().any(|c| c.id == comp_b && c.name == "Wheel part"), "the component keeps its new name");
    assert!(hole_cuts(&mut f.s, "Pinion"), "the hole follows the renamed body");
    // An update of the feature itself (the module) keeps them too.
    let req = request(&f.s);
    let group = super::records(&f.s.doc)[0].group;
    let mut blocks = req["module_blocks"].clone();
    blocks[0]["module"] = json!(1.0);
    run(&mut f.s, "gear.update", json!({"group": group, "module_blocks": blocks}));
    let up = whole(&mut f.s);
    assert_eq!(up.members, renamed.members, "{up:?}");
    assert_eq!(same_but_names(&up), same_but_names(&before));
}

#[test]
fn spur_reordering_the_timeline() {
    let (mut f, before, _) = spur();
    let sk = sketch_named(&f.s, "Pitch circles");
    let gears = f.feature();
    // The hole's sketch moved before the Gears feature: nothing of the set changes.
    let hole_sk = sketch_named(&f.s, "Hole sketch");
    let at = |s: &Session, id: u64| s.doc.features.iter().position(|x| x.id == id).unwrap();
    let pos = at(&f.s, gears);
    run(&mut f.s, "timeline.reorder", json!({"feature": hole_sk, "position": pos}));
    assert!(at(&f.s, hole_sk) < at(&f.s, gears));
    assert_eq!(whole(&mut f.s), before);
    // The picked sketch moved after the Gears feature: refused, or the feature fails naming it;
    // never a set read from something else.
    let end = f.s.doc.features.len() - 1;
    match f.s.execute("timeline.reorder", &json!({"feature": sk, "position": end})) {
        Err(e) => {
            let e = e.to_string();
            assert!(e.contains("Pitch circles") || e.contains("Gears") || e.contains("depend"), "{e}");
            assert_eq!(whole(&mut f.s), before);
        }
        Ok(_) => {
            let e = feature_error(&f.s, gears).unwrap_or_else(|| panic!("the set can't be read from a sketch after it: {:?}", errors(&f.s)));
            assert!(e.contains("Pitch circles") && e.contains("after"), "{e}");
            undo(&mut f.s);
            assert_eq!(whole(&mut f.s), before);
        }
    }
}

#[test]
fn spur_a_deleted_pick_fails_by_name_and_a_redrawn_one_recovers() {
    let (mut f, before, _) = spur();
    let gears = f.feature();
    let sk = sketch_named(&f.s, "Pitch circles");
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.delete", json!({"entities": ["b"]}));
    run(&mut f.s, "sketch.finish", json!({}));
    let e = feature_error(&f.s, gears).unwrap_or_else(|| panic!("a set missing gear B's circle must fail: {:?}", ids(&f.s)));
    for want in ["B", "pitch", "Pitch circles", "`b`"] {
        assert!(e.contains(want), "the error names {want}: {e}");
    }
    // The record keeps its components and joints (nothing was rebuilt from another pick).
    let rec = super::records(&f.s.doc).into_iter().next().unwrap();
    assert_eq!(ids_of(&f.s, &rec).members, before.members);
    // Redrawn with the same id (tangent to A again): the set recovers with every id.
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.circle.center", json!({"center": [50, 0], "radius": 30.0, "id": "b"}));
    run(&mut f.s, "sketch.constraint.tangent", json!({"a": "a", "b": "b"}));
    run(&mut f.s, "sketch.finish", json!({}));
    assert_eq!(whole(&mut f.s), before);
    // Undo the redraw: failed again; undo the delete: whole.
    undo(&mut f.s);
    assert!(feature_error(&f.s, gears).is_some());
    undo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
}

/// A pick deleted, and another entity drawn that takes its old place (and an id like it): the
/// set must not read it.
#[test]
fn a_deleted_pick_is_never_replaced_by_the_next_entity() {
    let (mut f, _, _) = spur();
    let gears = f.feature();
    let sk = sketch_named(&f.s, "Pitch circles");
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.delete", json!({"entities": ["b"]}));
    run(&mut f.s, "sketch.circle.center", json!({"center": [55, 0], "radius": 30.0}));
    run(&mut f.s, "sketch.finish", json!({}));
    let e = feature_error(&f.s, gears).unwrap_or_else(|| panic!("the new circle is not gear B's: {:?}", ids(&f.s)));
    assert!(e.contains("`b`") && e.contains("Pitch circles"), "{e}");
}

#[test]
fn spur_a_deleted_sketch_fails_by_name_and_undo_recovers() {
    let (mut f, before, _) = spur();
    let gears = f.feature();
    let sk = sketch_named(&f.s, "Pitch circles");
    // The timeline's delete may refuse a sketch others depend on; when it goes, the set names it.
    match f.s.execute("timeline.delete", &json!({"features": [sk]})) {
        Err(e) => panic!("deleting the sketch: {e}"),
        Ok(_) => {
            if f.s.doc.feature(gears).is_some() {
                let e = feature_error(&f.s, gears).unwrap_or_else(|| panic!("a set without its sketch must fail"));
                assert!(e.contains("Pitch circles") && e.contains('A'), "the error names the sketch and the item: {e}");
            }
            undo(&mut f.s);
            assert_eq!(whole(&mut f.s), before);
        }
    }
}

#[test]
fn spur_items_added_and_taken_away_keep_the_others() {
    let (mut f, before, _) = spur();
    let group = super::records(&f.s.doc)[0].group;
    // A third gear meshing B, drawn in a new sketch.
    run(&mut f.s, "sketch.create", json!({"plane": "XY", "name": "Third"}));
    run(&mut f.s, "sketch.circle.center", json!({"center": [50, 50], "radius": 20.0, "id": "c"}));
    run(&mut f.s, "sketch.finish", json!({}));
    let mut req = request(&f.s);
    let mut items = req["items"].as_array().unwrap().clone();
    items.push(json!({"id": "C", "type": "spur", "components": [{"sketch": "Third", "curve": "c", "slot": "pitch"}], "module_block": "M"}));
    let mut links = req["links"].as_array().unwrap().clone();
    links.push(json!({"a": {"item": "B", "gear": 0}, "b": {"item": "C", "gear": 0}, "kind": "mesh"}));
    run(&mut f.s, "gear.update", json!({"group": group, "items": items, "links": links}));
    let three = whole(&mut f.s);
    assert_eq!(three.members.len(), 3);
    assert_eq!(&three.members[..2], &before.members[..], "A and B keep theirs: {three:?}");
    assert_eq!(three.bodies[..2], before.bodies[..]);
    assert_eq!(three.links.len(), 2);
    assert!(three.links.contains(&before.links[0]));
    // A taken away: B and C keep theirs.
    req = request(&f.s);
    let items: Vec<Value> = req["items"].as_array().unwrap().iter().filter(|i| i["id"] != "A").cloned().collect();
    let links: Vec<Value> = req["links"].as_array().unwrap().iter().filter(|l| l["a"]["item"] != "A" && l["b"]["item"] != "A").cloned().collect();
    let r = f.s.execute("gear.update", &json!({"group": group, "items": items, "links": links, "drive": {"item": "B", "gear": 0, "rpm": 1000}}));
    // (The hole cut gear A: with A gone the hole has nothing to cut, which fails visibly.)
    r.unwrap_or_else(|e| panic!("{e}"));
    let two = ids(&f.s);
    let b_c: Vec<_> = three.members[1..].to_vec();
    assert_eq!(two.members, b_c, "B and C keep their components and joints");
    assert_eq!(two.links, vec![three.links.iter().copied().find(|l| !before.links.contains(l)).unwrap()]);
    // Undo: back to three.
    undo(&mut f.s);
    assert_eq!(whole(&mut f.s), three);
}

#[test]
fn spur_driver_change_keeps_the_ids() {
    let (mut f, before, _) = spur();
    let group = super::records(&f.s.doc)[0].group;
    run(&mut f.s, "gear.update", json!({"group": group, "drive": {"item": "B", "gear": 0, "rpm": 500}}));
    let now = whole(&mut f.s);
    assert_eq!(now.bodies, before.bodies);
    assert_eq!(now.members.iter().map(|m| (m.0.clone(), m.1)).collect::<Vec<_>>(), before.members.iter().map(|m| (m.0.clone(), m.1)).collect::<Vec<_>>());
    assert_eq!(now.members.iter().map(|m| m.2).collect::<Vec<_>>(), before.members.iter().map(|m| m.2).collect::<Vec<_>>(), "joints keep their ids");
    assert_eq!(now.links.len(), 1, "{now:?}");
}

#[test]
fn spur_save_reopen_then_edit() {
    let (f, before, _) = spur();
    let saved = solidcraft_io::write_design(&f.s.doc);
    let mut s = Session::new(solidcraft_io::read_design(&saved).unwrap());
    assert_eq!(whole(&mut s), before, "reopened as it was");
    let d = f.dim("Pitch circles", &["a"], "diameter");
    set_dim(&mut s, &d, "46 mm");
    assert_eq!(whole(&mut s), before, "edited after reopening");
    assert!(hole_cuts(&mut s, &before.bodies[0]));
    let group = super::records(&s.doc)[0].group;
    run(&mut s, "gear.update", json!({"group": group, "drive": {"item": "A", "gear": 0, "rpm": 2000}}));
    assert_eq!(whole(&mut s), before);
}

#[test]
fn spur_rollback_and_forward() {
    let (mut f, before, _) = spur();
    let gears = f.feature();
    let at = f.s.doc.features.iter().position(|x| x.id == gears).unwrap();
    run(&mut f.s, "timeline.rollback", json!({"position": at}));
    // Rolled back: a dimension edit there; the set follows when rolled forward.
    let d = f.dim("Pitch circles", &["a"], "diameter");
    set_dim(&mut f.s, &d, "44 mm");
    run(&mut f.s, "timeline.rollback", json!({"position": null}));
    assert_eq!(whole(&mut f.s), before);
    assert!(hole_cuts(&mut f.s, &before.bodies[0]));
    // The joints are on the shafts as they are now: B moved out by 2 mm.
    let rec = super::records(&f.s.doc).into_iter().next().unwrap();
    let b_axis = rec.assembly.as_ref().unwrap().members[1].axis.unwrap();
    assert!((b_axis[0][0] - 52.0).abs() < 1e-3, "{b_axis:?}");
}

#[test]
fn two_gears_features_read_one_sketch() {
    let mut f = Fix::load(SPUR);
    let first = whole(&mut f.s);
    let r = run(
        &mut f.s,
        "gear.create",
        json!({"name": "Second", "items": [
            {"id": "X", "type": "helical", "components": [{"sketch": "Pitch circles", "curve": "a"}], "gears": [{"helix_angle": 15, "face": {"a": 6, "b": 0}}], "module_block": "M"}],
            "module_blocks": [{"id": "M", "module": 2}]}),
    );
    no_errors(&f.s);
    let second = r["feature"].as_u64().unwrap();
    let names = |s: &Session, id: u64| s.doc.feature(id).unwrap().body_names.clone();
    let n2 = names(&f.s, second);
    let d = f.dim("Pitch circles", &["a"], "diameter");
    set_dim(&mut f.s, &d, "44 mm");
    no_errors(&f.s);
    let recs = super::records(&f.s.doc);
    let first_now = recs.iter().find(|r| r.feature == Some(first.feature)).map(|r| ids_of(&f.s, r)).unwrap();
    assert_eq!(first_now, first);
    assert_eq!(names(&f.s, second), n2);
}

// ---------------------------------------------------------------------------------------------
// The other fixtures: a compound across two sketches and a shaft line, a worm, a planetary set,
// the big drivetrain
// ---------------------------------------------------------------------------------------------

/// The edits every fixture goes through: a diameter, a distance, an unrelated circle added and
/// deleted, its sketches renamed, undo and redo of each, save and reopen.
fn common(script: &str, sketch: &str, diameters: &[(&str, &str)], distance: Option<([&str; 2], &str, &str)>) {
    let mut f = Fix::load(script);
    let before = whole(&mut f.s);
    // The diameters changed together (one rebuild), as a set that stays possible.
    let changes: serde_json::Map<String, Value> = diameters.iter().map(|(ent, to)| (f.dim(sketch, &[ent], "diameter"), json!(to))).collect();
    let (d, to) = (f.dim(sketch, &[diameters[0].0], "diameter"), diameters[0].1);
    run(&mut f.s, "parameters.change", json!({"changes": changes}));
    assert_eq!(whole(&mut f.s), before, "{sketch} {diameters:?}");
    undo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
    redo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
    if let Some((ents, kind, to)) = distance {
        let d = f.dim(sketch, &ents, kind);
        set_dim(&mut f.s, &d, to);
        assert_eq!(whole(&mut f.s), before, "{sketch} {ents:?} {kind} {to}");
    }
    let sk = sketch_named(&f.s, sketch);
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.circle.center", json!({"center": [-300, -300], "radius": 5.0, "id": "stray"}));
    run(&mut f.s, "sketch.finish", json!({}));
    assert_eq!(whole(&mut f.s), before);
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.delete", json!({"entities": ["stray"]}));
    run(&mut f.s, "sketch.finish", json!({}));
    assert_eq!(whole(&mut f.s), before);
    let renamed: Vec<u64> = f.s.doc.features.iter().filter(|x| matches!(x.kind, FeatureKind::Sketch { .. })).map(|x| x.id).collect();
    for (k, id) in renamed.iter().enumerate() {
        run(&mut f.s, "timeline.rename", json!({"feature": id, "name": format!("Renamed {k}")}));
    }
    assert_eq!(whole(&mut f.s), before);
    let saved = solidcraft_io::write_design(&f.s.doc);
    let mut s = Session::new(solidcraft_io::read_design(&saved).unwrap());
    assert_eq!(whole(&mut s), before);
    set_dim(&mut s, &d, to);
    let group = super::records(&s.doc).into_iter().find(|r| r.request.get("items").is_some()).unwrap().group;
    run(&mut s, "gear.update", json!({"group": group}));
    assert_eq!(whole(&mut s), before);
}

#[test]
fn compound_across_two_sketches_keeps_its_references() {
    common(COMPOUND, "Layer 2", &[("d", "56 mm")], Some((["origin", "d.center"], "vertical", "10 mm")));
}

#[test]
fn worm_keeps_its_references() {
    common(WORM, "Worm stage", &[("w", "56 mm")], Some((["origin", "axis.start"], "vertical", "36 mm")));
}

#[test]
fn planetary_keeps_its_references() {
    // (Sun, planet and ring sizes are tied: (ring - sun) / 2 is the planet.)
    common(PLANETARY, "Planetary", &[("sun", "33 mm"), ("ring", "93 mm")], None);
}

#[test]
fn everything_drivetrain_keeps_its_references() {
    let mut f = Fix::load(EVERYTHING);
    let before = whole(&mut f.s);
    // (The internal pair's pinion: its ring moves round it; nothing shares the ring's shaft.)
    let d = f.dim("L-90 internal", &["rp"], "diameter");
    set_dim(&mut f.s, &d, "44 mm");
    assert_eq!(whole(&mut f.s), before);
    let sk = sketch_named(&f.s, "L-30 herringbone");
    run(&mut f.s, "timeline.rename", json!({"feature": sk, "name": "Herringbone layer"}));
    run(&mut f.s, "sketch.edit", json!({"sketch": sk}));
    run(&mut f.s, "sketch.circle.center", json!({"center": [-300, -300], "radius": 5.0, "id": "stray"}));
    run(&mut f.s, "sketch.finish", json!({}));
    assert_eq!(whole(&mut f.s), before);
    // Item E taken away, then put back: the others keep theirs, E's come back new.
    let group = super::records(&f.s.doc).into_iter().find(|r| r.request.get("items").is_some()).unwrap().group;
    let req = request(&f.s);
    let items: Vec<Value> = req["items"].as_array().unwrap().iter().filter(|i| i["id"] != "E").cloned().collect();
    let links: Vec<Value> = req["links"].as_array().unwrap().iter().filter(|l| l["a"]["item"] != "E" && l["b"]["item"] != "E").cloned().collect();
    run(&mut f.s, "gear.update", json!({"group": group, "items": items, "links": links}));
    let without = whole(&mut f.s);
    let e_bodies: Vec<&String> = before.bodies.iter().filter(|b| b.contains("E·")).collect();
    assert!(!e_bodies.is_empty());
    for m in &before.members {
        if !m.0.contains("E·") {
            assert!(without.members.contains(m), "{m:?} kept: {without:?}");
        }
    }
    undo(&mut f.s);
    assert_eq!(whole(&mut f.s), before);
    let saved = solidcraft_io::write_design(&f.s.doc);
    let mut s = Session::new(solidcraft_io::read_design(&saved).unwrap());
    assert_eq!(whole(&mut s), before);
}

// ---------------------------------------------------------------------------------------------
// Sketch-driven details: bore and hub circles, side A, shaft lines and bearings
// ---------------------------------------------------------------------------------------------

/// Gear A's pitch circle `a` (Ø40) with a Ø8 circle `ab` and a Ø16 circle `ah` drawn round its
/// centre, and gear B's `b` (Ø60) touching it; the small circles' sizes as parameters.
fn detailed_sketch(s: &mut Session) -> u64 {
    let sk = run(s, "sketch.create", json!({"plane": "XY", "name": "Detailed"}))["sketch"].as_u64().unwrap();
    run(s, "sketch.circle.center", json!({"center": [0, 0], "radius": 20.0, "id": "a"}));
    run(s, "sketch.circle.center", json!({"center": [0, 0], "radius": 4.0, "id": "ab"}));
    run(s, "sketch.circle.center", json!({"center": [0, 0], "radius": 8.0, "id": "ah"}));
    run(s, "sketch.circle.center", json!({"center": [50, 0], "radius": 30.0, "id": "b"}));
    run(s, "sketch.constraint.coincident", json!({"a": "a.center", "b": "origin"}));
    run(s, "sketch.constraint.concentric", json!({"a": "ab", "b": "a"}));
    run(s, "sketch.constraint.concentric", json!({"a": "ah", "b": "a"}));
    run(s, "sketch.dimension", json!({"entities": ["ab"], "type": "diameter", "value": "bore_d = 8 mm"}));
    run(s, "sketch.dimension", json!({"entities": ["ah"], "type": "diameter", "value": "hub_d = 16 mm"}));
    run(s, "sketch.finish", json!({}));
    sk
}

fn detailed_request(details: &[(&str, &str)]) -> Value {
    let mut a = vec![json!({"sketch": "Detailed", "curve": "a", "slot": "pitch"})];
    a.extend(details.iter().map(|(curve, slot)| json!({"sketch": "Detailed", "curve": curve, "slot": slot})));
    json!({"name": "Detailed pair", "assembly": true, "items": [
        {"id": "A", "type": "spur", "components": a, "gears": [{"face": {"preset": "a_only", "a": 10}, "hub": {"length_a": 6}}]},
        {"id": "B", "type": "spur", "components": [{"sketch": "Detailed", "curve": "b", "slot": "pitch"}]}],
        "links": [{"a": {"item": "A"}, "b": {"item": "B"}, "kind": "mesh"}],
        "module_blocks": [{"id": "M", "module": 2, "members": ["A", "B"]}]})
}

/// Gear A's volume made with these detail picks, and the reply.
fn detailed_volume(details: &[(&str, &str)]) -> (f64, Value) {
    let mut s = Session::default();
    detailed_sketch(&mut s);
    let r = run(&mut s, "gear.create", detailed_request(details));
    no_errors(&s);
    let a = r["gears"][0]["body"].as_str().unwrap().to_string();
    (volume(&mut s, &a), r)
}

#[test]
fn bore_and_hub_circles_shape_the_gear() {
    let (plain, r0) = detailed_volume(&[]);
    assert!(r0["gears"][0]["bore"].is_null() && r0["gears"][0]["hub"].is_null(), "{r0}");
    let (bored, r1) = detailed_volume(&[("ab", "bore")]);
    assert_eq!(r1["gears"][0]["bore"], json!(8.0), "{r1}");
    let hole = std::f64::consts::PI * 4.0 * 4.0 * 10.0;
    assert!((plain - bored - hole).abs() < 0.02 * hole, "a bore of 8 through a 10 mm face: {plain} - {bored} vs {hole}");
    let (hubbed, r2) = detailed_volume(&[("ab", "bore"), ("ah", "hub")]);
    assert_eq!((r2["gears"][0]["bore"].clone(), r2["gears"][0]["hub"].clone()), (json!(8.0), json!(16.0)), "{r2}");
    let hub = std::f64::consts::PI * (8.0 * 8.0 - 4.0 * 4.0) * 6.0;
    assert!((hubbed - bored - hub).abs() < 0.03 * hub, "a hub of 16 reaching 6 mm on side A: {hubbed} - {bored} vs {hub}");
    // Side A: the sketch's normal (XY: +Z).
    assert_eq!(r2["gears"][0]["side_a"], json!([0.0, 0.0, 1.0]), "{r2}");
}

#[test]
fn bore_circle_follows_its_dimension_and_a_deleted_one_fails_by_name() {
    let mut s = Session::default();
    let sk = detailed_sketch(&mut s);
    let r = run(&mut s, "gear.create", detailed_request(&[("ab", "bore"), ("ah", "hub")]));
    let before = whole(&mut s);
    let a = r["gears"][0]["body"].as_str().unwrap().to_string();
    let v8 = volume(&mut s, &a);
    run(&mut s, "parameters.change", json!({"name": "bore_d", "expression": "10 mm"}));
    assert_eq!(whole(&mut s), before);
    let v10 = volume(&mut s, &a);
    let more = std::f64::consts::PI * (25.0 - 16.0) * 16.0;
    assert!((v8 - v10 - more).abs() < 0.05 * more, "the bore grew from 8 to 10 through gear and hub: {v8} - {v10} vs {more}");
    // The bore circle deleted: the feature names it; never another circle read as the bore.
    run(&mut s, "sketch.edit", json!({"sketch": sk}));
    run(&mut s, "sketch.delete", json!({"entities": ["ab"]}));
    run(&mut s, "sketch.finish", json!({}));
    let e = feature_error(&s, before.feature).expect("the set fails without its bore circle");
    assert!(e.contains("item A, bore") && e.contains("`ab`") && e.contains("Detailed"), "{e}");
    undo(&mut s);
    assert_eq!(whole(&mut s), before);
}

#[test]
fn concentric_circles_are_suggested_as_bore_and_hub() {
    let mut s = Session::default();
    let sk = detailed_sketch(&mut s);
    let r = run(&mut s, "gear.calculate", detailed_request(&[]));
    let d = r["detail_suggestions"].as_array().unwrap_or_else(|| panic!("{r}"));
    assert_eq!(d.len(), 2, "{r}");
    assert_eq!((d[0]["item"].clone(), d[0]["slot"].clone(), d[0]["curve"].clone(), d[0]["diameter"].clone(), d[0]["sketch"].clone()), (json!("A"), json!("bore"), json!("ab"), json!(8.0), json!(sk)));
    assert_eq!((d[1]["slot"].clone(), d[1]["curve"].clone(), d[1]["diameter"].clone()), (json!("hub"), json!("ah"), json!(16.0)));
    // Picked, nothing more is suggested for it; nothing is used until picked.
    let r = run(&mut s, "gear.calculate", detailed_request(&[("ab", "bore")]));
    let d = r["detail_suggestions"].as_array().cloned().unwrap_or_default();
    assert_eq!(d.len(), 1, "{r}");
    assert_eq!(d[0]["slot"], "hub");
    assert_eq!(r["gears"][0]["hub"], Value::Null, "{r}");
}

#[test]
fn a_bore_circle_off_centre_is_refused_by_name() {
    let mut s = Session::default();
    detailed_sketch(&mut s);
    let e = s.execute("gear.calculate", &detailed_request(&[("b", "bore")])).unwrap_err().to_string();
    assert!(e.contains("item A, bore") && e.contains("concentric"), "{e}");
}

#[test]
fn a_shaft_line_sets_the_shafts_length_and_a_point_on_it_is_a_bearing() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Pitch"}));
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 20.0, "id": "a"}));
    run(&mut s, "sketch.circle.center", json!({"center": [50, 0], "radius": 30.0, "id": "b"}));
    run(&mut s, "sketch.finish", json!({}));
    // The shaft line in the XZ plane through A's centre: z -15 to 35 (sketch y is world Z).
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Shaft"}));
    run(&mut s, "sketch.line", json!({"points": [[0, -15], [0, 35]], "ids": ["s"]}));
    run(&mut s, "sketch.point", json!({"point": [0, 25], "id": "brg"}));
    run(&mut s, "sketch.finish", json!({}));
    // A spur with its shaft line (Generate Shafts on by a drawn line): the shaft runs between
    // the line's ends.
    let req = json!({"items": [
        {"id": "A", "type": "spur", "components": [{"sketch": "Pitch", "curve": "a", "slot": "pitch"}, {"sketch": "Shaft", "curve": "s", "slot": "axis"}]},
        {"id": "B", "type": "spur", "components": [{"sketch": "Pitch", "curve": "b", "slot": "pitch"}]}],
        "links": [{"a": {"item": "A"}, "b": {"item": "B"}, "kind": "mesh"}],
        "module_blocks": [{"id": "M", "module": 2, "members": ["A", "B"]}]});
    let (_, plan) = super::ask(&s, "gear.calculate", &req).unwrap();
    assert_eq!(plan.shafts.len(), 1, "one shaft, on A's drawn line only");
    assert!((plan.shafts[0].length - 50.0).abs() < 1e-6, "the line's length: {}", plan.shafts[0].length);
    let f = plan.bodies[plan.shafts[0].body].frame;
    assert!(f.origin.iter().zip([0.0, 0.0, -15.0]).all(|(a, b)| (a - b).abs() < 1e-6) && (f.z[2] - 1.0).abs() < 1e-9, "from the line's first end along it: {f:?}");
    // A bearing point goes on a Shaft item's line (a gear item refuses it, saying so).
    let e = s.execute("gear.calculate", &json!({"items": [
        {"id": "A", "type": "spur", "components": [{"sketch": "Pitch", "curve": "a"}, {"sketch": "Shaft", "curve": "s"}, {"sketch": "Shaft", "point": "brg", "slot": "bearing"}]}]})).unwrap_err().to_string();
    assert!(e.contains("Shaft item"), "{e}");
    let with_shaft = json!({"items": [
        {"id": "A", "type": "spur", "components": [{"sketch": "Pitch", "curve": "a", "slot": "pitch"}]},
        {"id": "B", "type": "spur", "components": [{"sketch": "Pitch", "curve": "b", "slot": "pitch"}]},
        {"id": "S", "type": "shaft", "components": [{"sketch": "Shaft", "curve": "s", "slot": "axis"}, {"sketch": "Shaft", "point": "brg", "slot": "bearing"}]}],
        "links": [{"a": {"item": "A"}, "b": {"item": "B"}, "kind": "mesh"}],
        "module_blocks": [{"id": "M", "module": 2, "members": ["A", "B"]}]});
    let r = s.execute("gear.calculate", &with_shaft);
    let (_, plan) = super::ask(&s, "gear.calculate", &with_shaft).unwrap_or_else(|e| panic!("{e}"));
    eprintln!("BEARING reply {r:?}
BEARING plan shafts {:?} clearance {:?} warnings {:?}", plan.shafts, plan.clearance.as_ref().map(|c| c.name.clone()), plan.warnings);
    let r = r.unwrap();
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(plan.shafts.len(), 1, "the Shaft item's line is A's shaft");
}
