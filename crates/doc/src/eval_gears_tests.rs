//! The Gears feature: a whole set built in memory as one feature, following its sketch, and its
//! keep-out cut from a housing in the same step. The planner here stands in for the app's
//! (`gear.create`'s reading): `params.set` names the set.

use serde_json::{Value, json};
use solidcore_record::{Vec2, Vec3};
use solidcraft_gear::plan::{Kind, Plan, Spec};
use solidcraft_kernel::measure;
use solidcraft_sketch::Sketch;

use super::{GearsInput, set_planner};
use crate::{BodyName, Document, FeatureKind, Model, Operation, PlaneRef, SketchId, point_expr};

/// The test planner: a helical pair, a bevel pair, a pair with its keep-out, or a pair whose
/// gear 1 is the sketch's circle (module 2: its diameter in mm is twice its teeth).
fn planner(input: &GearsInput<'_>) -> Result<Plan, String> {
    let mut spec = match input.params.get("set").and_then(Value::as_str) {
        Some("helical") => Spec { helix: 20.0, ..Spec::defaults_for(Kind::Pair) },
        Some("bevel") => Spec::defaults_for(Kind::Bevel),
        Some("keep_out") => Spec { clearance_body: Some(Default::default()), ..Spec::defaults_for(Kind::Pair) },
        Some("sketch") => {
            let id = *input.sketches.first().ok_or("no sketch")?;
            let ss = input.state.sketch(id).ok_or("no sketch")?;
            let r = ss.sketch.curves.iter().find_map(|c| if let solidcraft_sketch::CurveKind::Circle { r, .. } = c.kind { Some(r) } else { None }).ok_or("no circle")?;
            Spec { module: 2.0, teeth: (r.round() as u32).max(8), ..Spec::defaults_for(Kind::Pair) }
        }
        other => return Err(format!("no test set {other:?}")),
    };
    spec.name = String::new();
    solidcraft_gear::plan(&spec)
}

fn gears(params: Value, sketches: Vec<u64>, keep_out_cuts: Vec<&str>) -> FeatureKind {
    FeatureKind::Gears {
        params,
        sketches: sketches.into_iter().map(SketchId).collect(),
        operation: Operation::NewBody,
        targets: Vec::new(),
        keep_out_cuts: keep_out_cuts.into_iter().map(|n| BodyName(n.to_string())).collect(),
    }
}

fn evaluated(doc: &Document) -> Model {
    set_planner(planner);
    let mut m = Model::new();
    m.evaluate(doc);
    for r in &m.results {
        assert!(r.error.is_none(), "{}: {:?}", r.name, r.error);
    }
    m
}

fn one_feature_set(set: &str) {
    let mut doc = Document::new("gears");
    let id = doc.add_feature(gears(json!({"set": set}), Vec::new(), Vec::new()), Some("Gears")).unwrap();
    let m = evaluated(&doc);
    assert_eq!(doc.features.len(), 1, "one feature, no sketches");
    let st = m.state();
    assert_eq!(st.bodies.len(), 2, "{:?}", st.bodies.iter().map(|b| &b.name).collect::<Vec<_>>());
    assert!(st.bodies.iter().all(|b| b.feature == id));
    assert!(st.sketches.is_empty());
    for b in &st.bodies {
        let v = measure(&b.body).unwrap().volume;
        assert!(v > 100.0, "{}: {v}", b.name);
    }
    let saved = serde_json::to_value(&doc.features[0].kind).unwrap();
    assert_eq!(saved["type"], "gears");
    assert_eq!(serde_json::from_value::<FeatureKind>(saved).unwrap(), doc.features[0].kind, "saves and reads back");
}

#[test]
fn a_helical_pair_is_one_feature_with_no_sketches() {
    one_feature_set("helical");
}

#[test]
fn a_bevel_pair_is_one_feature_with_no_sketches() {
    one_feature_set("bevel");
}

#[test]
fn an_edited_sketch_recomputes_the_set_in_place_under_the_same_names() {
    let mut doc = Document::new("follow");
    let mut sk = Sketch::new();
    sk.add_circle(Vec2::new(0.0, 0.0), 20.0, None, None).unwrap();
    let s = doc.add_feature(FeatureKind::Sketch { plane: PlaneRef::Origin { name: "XY".into() }, sketch: sk }, None).unwrap();
    let id = doc.add_feature(gears(json!({"set": "sketch"}), vec![s], Vec::new()), Some("Gears")).unwrap();
    doc.feature_mut(id).unwrap().body_names = vec!["Input".into(), "Output".into()];
    let m = evaluated(&doc);
    let names = |m: &Model| m.state().bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&m), vec!["Input", "Output"]);
    let small = measure(&m.state().bodies[0].body).unwrap().volume;
    // The circle made bigger: gear 1 has more teeth, under the same name, from the same feature.
    if let Some(FeatureKind::Sketch { sketch, .. }) = doc.feature_mut(s).map(|f| &mut f.kind) {
        for curve in &mut sketch.curves {
            if let solidcraft_sketch::CurveKind::Circle { r, .. } = &mut curve.kind {
                *r = 30.0;
            }
        }
    }
    let m2 = evaluated(&doc);
    assert_eq!(names(&m2), vec!["Input", "Output"]);
    assert!(m2.state().bodies.iter().all(|b| b.feature == id));
    let big = measure(&m2.state().bodies[0].body).unwrap().volume;
    assert!(big > small * 1.5, "{small} -> {big}");
}

#[test]
fn the_keep_out_hollows_a_housing_in_the_same_feature() {
    let mut doc = Document::new("housing");
    doc.add_feature(
        FeatureKind::Box { corner: point_expr(Vec3::new(-40.0, -40.0, -10.0)), length: "140".into(), width: "80".into(), height: "40".into(), operation: Operation::NewBody, centred: false },
        Some("Case"),
    )
    .unwrap();
    doc.features[0].body_names = vec!["Case".into()];
    let solid = measure(&evaluated(&doc).state().bodies[0].body).unwrap().volume;
    let id = doc.add_feature(gears(json!({"set": "keep_out"}), Vec::new(), vec!["Case"]), Some("Gears")).unwrap();
    let m = evaluated(&doc);
    let st = m.state();
    let names: Vec<&str> = st.bodies.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names.len(), 3, "the case and two gears, no keep-out body: {names:?}");
    let case = st.body("Case").unwrap();
    let hollow = measure(&case.body).unwrap().volume;
    assert!(hollow < solid - 1000.0, "the case is hollowed where the gears turn: {solid} -> {hollow}");
    assert_eq!(st.bodies.iter().filter(|b| b.feature == id).count(), 2);
    assert_eq!(doc.features.len(), 2, "the box and one Gears feature");
}

#[test]
fn a_set_evaluated_again_makes_only_the_gears_whose_plan_changed() {
    let mut doc = Document::new("again");
    let mut sk = Sketch::new();
    sk.add_circle(Vec2::new(0.0, 0.0), 22.0, None, None).unwrap();
    let s = doc.add_feature(FeatureKind::Sketch { plane: PlaneRef::Origin { name: "XY".into() }, sketch: sk }, None).unwrap();
    doc.add_feature(gears(json!({"set": "sketch"}), vec![s], Vec::new()), Some("Gears")).unwrap();
    let bodies = |m: &Model| m.state().bodies.iter().map(|b| b.body.clone()).collect::<Vec<_>>();
    // A model of its own each time (nothing of the model's cache): the same plan, the same bodies.
    // (A circle no other test draws: a test evaluating the same set at once could make it too.)
    let first = bodies(&evaluated(&doc));
    let again = bodies(&evaluated(&doc));
    assert_eq!(first.len(), 2);
    assert!(first.iter().zip(&again).all(|(a, b)| a.same_shape(b)), "the gears made before are taken again");
    // Gear 1 grown: its plan changed, so it is made again.
    if let Some(FeatureKind::Sketch { sketch, .. }) = doc.feature_mut(s).map(|f| &mut f.kind) {
        for curve in &mut sketch.curves {
            if let solidcraft_sketch::CurveKind::Circle { r, .. } = &mut curve.kind {
                *r = 30.0;
            }
        }
    }
    let grown = bodies(&evaluated(&doc));
    assert!(!grown[0].same_shape(&first[0]), "the changed gear is made again");
    let (small, big) = (measure(&first[0]).unwrap().volume, measure(&grown[0]).unwrap().volume);
    assert!(big > small * 1.5, "{small} -> {big}");
}

#[test]
fn a_plan_differing_in_the_last_bits_is_known_as_the_same_body() {
    let plan = solidcraft_gear::plan(&Spec::defaults_for(Kind::Pair)).unwrap();
    let b = &plan.bodies[0];
    let key = super::plan_key(b);
    let mut noisy = b.clone();
    noisy.frame.origin[0] += 1e-13;
    noisy.frame.origin[1] = -0.0;
    assert_eq!(super::plan_key(&noisy), key, "noise in the last bits");
    let mut moved = b.clone();
    moved.frame.origin[0] += 1e-6;
    assert_ne!(super::plan_key(&moved), key, "a micron is another body");
    let mut renamed = b.clone();
    renamed.name = format!("{} 2", b.name);
    assert_ne!(super::plan_key(&renamed), key);
}

/// A gear with a bore, keyway, lightening holes, a hub, a shaft, chamfers and a set screw built
/// fused (fewer booleans on its teeth) is the gear its steps make in order: the same volume,
/// area and box.
#[test]
fn a_fused_build_is_the_planned_steps_solid() {
    use solidcraft_gear::hub::{Bore, BoreKind, Chamfer, Hardware, Shaft};
    let shaft = Shaft { diameter: 8.0, length: 10.0, chamfer: 0.5, ..Shaft::default() };
    let s = Spec {
        teeth: 30,
        module: 1.5,
        face_width: 8.0,
        bore: Bore { kind: BoreKind::Keyway, diameter: 5.0, ..Bore::default() },
        hub_diameter: 16.0,
        hub_length: 6.0,
        hub_chamfer: 0.8,
        bore_chamfer: 0.3,
        holes: 5,
        shaft_back: shaft,
        tooth_chamfer_front: Chamfer { size: 0.4, angle: 45.0 },
        tooth_chamfer_back: Chamfer { size: 0.4, angle: 45.0 },
        hardware: Hardware { set_screw: 3.0, set_screws: 2, ..Hardware::default() },
        ..Spec::defaults_for(Kind::Spur)
    };
    let plan = solidcraft_gear::plan(&s).unwrap();
    let b = &plan.bodies[0];
    let f = solidcraft_gear::plan::fused::fused(b).expect("fused");
    let mut notes = Vec::new();
    let fused = super::build_fused(b, &f, &mut notes).unwrap();
    let planned = super::run_steps(b, 0, None, &mut notes).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    let (mf, mp) = (measure(&fused).unwrap(), measure(&planned).unwrap());
    assert!((mf.volume - mp.volume).abs() <= 1e-6 * mp.volume, "{} vs {}", mf.volume, mp.volume);
    assert!((mf.area - mp.area).abs() <= 1e-6 * mp.area, "{} vs {}", mf.area, mp.area);
    let (bf, bp) = (fused.exact_bounds().unwrap(), planned.exact_bounds().unwrap());
    assert!((bf.min - bp.min).length() < 1e-6 && (bf.max - bp.max).length() < 1e-6, "{bf:?} vs {bp:?}");
}
