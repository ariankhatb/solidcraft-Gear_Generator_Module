//! What the gear command tests share: running a command, a body's volume, the timeline's errors,
//! the Gears features, sketch entities.

use serde_json::{Value, json};
use solidcraft_doc::FeatureKind;

use crate::Session;

/// Runs command `id`; it must succeed.
pub(super) fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// A body's volume (mm³).
pub(super) fn volume(s: &mut Session, body: &str) -> f64 {
    run(s, "inspect.center_of_mass", json!({"bodies": [body]}))["volume"].as_f64().unwrap()
}

/// Every feature of the timeline evaluates without an error.
pub(super) fn no_errors(s: &Session) {
    for r in &s.model.results {
        assert!(r.error.is_none(), "{}: {:?}", r.name, r.error);
    }
}

/// The ids of the design's Gears features.
pub(super) fn gears_features(s: &Session) -> Vec<u64> {
    s.doc.features.iter().filter(|f| matches!(f.kind, FeatureKind::Gears { .. })).map(|f| f.id).collect()
}

/// A circle for a sketch, as a command and its parameters.
pub(super) fn circle(x: f64, y: f64, r: f64, id: &str) -> (&'static str, Value) {
    ("sketch.circle.center", json!({"center": [x, y], "radius": r, "id": id}))
}

/// A line for a sketch, as a command and its parameters.
pub(super) fn line(a: [f64; 2], b: [f64; 2], id: &str) -> (&'static str, Value) {
    ("sketch.line", json!({"points": [a, b], "ids": [id]}))
}
