//! The Gears dialog's gear items and their settings (GEAR_UI_SPEC.md §1, §3, §6).
//!
//! A gear item is one thing the generator makes, defined by the geometry picked into its slots:
//! a spur gear by its pitch circle (and optionally its shaft line), a compound gear by its shaft
//! line and two or more circles on it (in parallel sketches, decision 6), a planetary set by its
//! sun, ring and planet circles, a bevel pair by two circles or by two axis lines and one circle,
//! a hypoid or worm pair by its circle and the other gear's axis line, a shaft by its line and the
//! bearing points on it. Pick a type, press New gear, then click the geometry the status line
//! asks for; a pick that doesn't fit is refused with the reason. In the dialog an item is called
//! a gear ("Gear B"), whatever number of gears it makes.
//!
//! The slots, the checks on a pick and the gear count are the gear crate's
//! ([`solidcraft_gear::items`]: `slots`, `assign`, `validate_slot`, `next_slot`, `gear_count`):
//! the dialog reads each pick as a world [`Thing`] (`dialogs_gear::thing_of`) and asks. Every
//! gear of an item has its own settings (§3), typed here as text (expressions with units, empty
//! for Auto) and read into the crate's [`items::GearSettings`]; `dialogs_gear_view` shows them.
//! The items are sent as `gear.create`'s `items`, with the `links` and `module_blocks` (§6).

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::items::{self, Bore, BevelStyle, FacePreset, FaceSides, Hand, Hub, ItemKind, SizeBy, Thing};

use crate::SolveApp;
use crate::dialogs_gear_links::Links;

/// An item's id as the engine names it (A, B, … Z, then A2 …).
pub use solidcraft_gear::items::item_id;

/// The index of item `id` (an id as [`item_id`] makes it, or a number).
pub fn item_index(id: &Value) -> Option<usize> {
    match id.as_u64() {
        Some(n) => Some(n as usize),
        None => items::item_number(id.as_str()?),
    }
}

/// A request read back with its items named by position (A, B, C, … as the dialog names them):
/// a request written elsewhere (the CLI, a script) may name its items W, G, R; every `item`
/// that refers to one (links, ratios, the drive) is renamed with it.
pub fn positional_ids(r: &mut serde_json::Map<String, Value>) {
    let ids: Vec<Option<String>> = r.get("items").and_then(Value::as_array).into_iter().flatten().map(|v| v.get("id").and_then(Value::as_str).map(str::to_string)).collect();
    let rename: std::collections::HashMap<String, String> =
        ids.iter().enumerate().filter_map(|(i, id)| Some((id.clone()?, item_id(i)))).filter(|(a, b)| a != b).collect();
    if rename.is_empty() {
        return;
    }
    fn walk(v: &mut Value, rename: &std::collections::HashMap<String, String>) {
        match v {
            Value::Object(o) => {
                for (k, x) in o.iter_mut() {
                    match x {
                        Value::String(s) if k == "item" => {
                            if let Some(n) = rename.get(s.as_str()) {
                                *s = n.clone();
                            }
                        }
                        _ => walk(x, rename),
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| walk(x, rename)),
            _ => {}
        }
    }
    for (k, v) in r.iter_mut() {
        if k == "items" {
            for (i, it) in v.as_array_mut().into_iter().flatten().enumerate() {
                if let Some(o) = it.as_object_mut() {
                    o.insert("id".into(), Value::String(item_id(i)));
                }
            }
        } else if k != "ui" {
            walk(v, &rename);
        }
    }
}

/// The face presets (decision 4), as the toggle shows them and `face.preset` names them.
pub const FACE_PRESETS: [(FacePreset, &str, &str); 4] = [
    (FacePreset::Symmetric, "symmetric", "Symmetric"),
    (FacePreset::AOnly, "a_only", "A only"),
    (FacePreset::BOnly, "b_only", "B only"),
    (FacePreset::Custom, "custom", "Custom"),
];

/// The name of gear `gear` of an item of `kind` where its gears have roles ("sun", "worm"); a
/// compound's or a pair's gears are numbered instead (`None`).
pub fn gear_part(kind: ItemKind, gear: usize) -> Option<&'static str> {
    let names: &[&str] = match kind {
        ItemKind::Planetary => &["sun", "ring", "planet"],
        ItemKind::Worm => &["wheel", "worm"],
        ItemKind::Hypoid => &["ring", "pinion"],
        ItemKind::Internal => &["ring", "pinion"],
        ItemKind::Rack => &["rack", "pinion"],
        _ => &[],
    };
    names.get(gear).copied()
}

/// A pick named for a person: "curve c3 of sketch 'Layer 2'", "point q of sketch 'Shaft'", "a model
/// edge", "axis Z".
pub fn pick_name(app: &SolveApp, sel: &Sel) -> String {
    let sketch = |id: Option<u64>| -> String {
        match id.and_then(|s| app.session.doc.feature(s)) {
            Some(f) => format!("sketch '{}'", f.name),
            None => "a sketch that is gone".to_string(),
        }
    };
    match sel {
        Sel::SketchCurve { id, sketch: s } => format!("curve {id} of {}", sketch(s.or(app.session.active_sketch))),
        Sel::ExternalSketchCurve { sketch: s, id } => format!("curve {id} of {}", sketch(Some(*s))),
        Sel::ExternalSketchPoint { sketch: s, id } => format!("point {id} of {}", sketch(Some(*s))),
        Sel::SketchPoint { id } => format!("point {id} of {}", sketch(app.session.active_sketch)),
        Sel::Edge { body, .. } => format!("an edge of {body}"),
        Sel::Face { body, .. } => format!("a face of {body}"),
        Sel::Axis { name } => format!("axis {name}"),
        _ => "a pick".to_string(),
    }
}

/// One row of a gear's checklist: the slot, whether it is required, and what fills it (the
/// pick's name: its sketch and size), if anything.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotRow {
    pub what: &'static str,
    pub required: bool,
    pub filled: Option<String>,
    /// Whether this is the slot the next pick goes to.
    pub next: bool,
    /// It takes any number (a compound's circles, bearing points).
    pub repeats: bool,
}

/// A circle's radius (0 for anything else).
fn radius(t: &Thing) -> f64 {
    match t {
        Thing::Circle { radius, .. } => *radius,
        _ => 0.0,
    }
}

/// Whether a `selection` component is a bore or hub circle (slot `bore` / `hub`).
fn is_detail(c: &Value) -> bool {
    matches!(c.get("slot").and_then(Value::as_str), Some("bore" | "hub"))
}

/// A picked thing named for its slot row: its sketch and its size ("Layer 1 · Ø40",
/// "Shaft · 120 long", "Side view · point"). `len` shows a length in the design's units.
pub fn thing_label(app: &SolveApp, t: &Thing, len: &dyn Fn(f64) -> String) -> String {
    let sketch = t.ids().sketch.and_then(|s| app.session.doc.feature(s)).map(|f| f.name.clone());
    let size = match t {
        Thing::Circle { radius, .. } => format!("Ø{}", len(2.0 * radius)),
        Thing::Line { a, b, .. } => {
            let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            format!("{} long", len(d))
        }
        Thing::Point { .. } => "point".to_string(),
    };
    match sketch {
        Some(s) => format!("{s} · {size}"),
        None => size,
    }
}

/// A gear's own settings (§3), as typed (empty: Auto); [`GearSettings::read`] makes the gear
/// crate's [`items::GearSettings`] of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearSettings {
    /// Sized by its teeth (the module follows from the circle), else by its module.
    pub by_teeth: bool,
    /// No circle gives its size (a bevel pair drawn as a side view): its module and its teeth are
    /// both typed, neither follows from the other.
    pub both: bool,
    pub module: String,
    pub teeth: String,
    /// An index into [`FACE_PRESETS`].
    pub face_preset: usize,
    pub face_a: String,
    pub face_b: String,
    pub helix: String,
    /// 0 Auto (opposite its mate), 1 Left, 2 Right.
    pub hand: usize,
    pub v: bool,
    pub profile_shift: String,
    /// 0 Auto (fits its shaft), 1 none, 2 the Ø typed.
    pub bore: usize,
    pub bore_d: String,
    pub hub: bool,
    pub hub_d: String,
    pub hub_a: String,
    pub hub_b: String,
}

pub(crate) fn eval(app: &SolveApp, text: &str, kind: ValueKind, what: &str) -> Result<Option<f64>, String> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let v = app.session.doc.eval(t, kind).map_err(|e| format!("{what}: {e}: type a number, or leave it empty for Auto"))?;
    Ok(Some(if matches!(kind, ValueKind::Angle) { v.to_degrees() } else { v }))
}

/// A value as a field shows it again (Edit): "2 mm", "20 deg", "0.3".
pub(crate) fn shown(v: Option<f64>, kind: ValueKind) -> String {
    let Some(v) = v else { return String::new() };
    let n = crate::dialogs_gear::fmt(v);
    match kind {
        ValueKind::Length => format!("{n} mm"),
        ValueKind::Angle => format!("{n} deg"),
        _ => n,
    }
}

impl GearSettings {
    /// The gear crate's settings of what is typed (`what` names the gear in a refusal).
    pub fn read(&self, app: &SolveApp, what: &dyn Fn(&str) -> String) -> Result<items::GearSettings, String> {
        let len = |t: &str, f: &str| eval(app, t, ValueKind::Length, &what(f));
        let preset = FACE_PRESETS.get(self.face_preset).map_or(FacePreset::Symmetric, |p| p.0);
        let (a, b) = (len(&self.face_a, "face side A")?, len(&self.face_b, "face side B")?);
        let face = match preset {
            FacePreset::Symmetric => FaceSides { preset, a, b: a },
            FacePreset::AOnly => FaceSides { preset, a, b: Some(0.0) },
            FacePreset::BOnly => FaceSides { preset, a: Some(0.0), b },
            FacePreset::Custom => FaceSides { preset, a, b },
        };
        let bore = match self.bore {
            1 => Bore::Solid,
            2 => Bore::Diameter(len(&self.bore_d, "bore Ø")?.ok_or_else(|| what("type the bore Ø, or choose Auto"))?),
            _ => Bore::Auto,
        };
        let hub = if self.hub {
            let diameter = len(&self.hub_d, "hub Ø")?;
            Some(Hub { diameter, length_a: len(&self.hub_a, "hub length A")?.unwrap_or(0.0), length_b: len(&self.hub_b, "hub length B")?.unwrap_or(0.0), auto: diameter.is_none() })
        } else {
            None
        };
        let (by_teeth, by_module) = (self.by_teeth || self.both, !self.by_teeth || self.both);
        Ok(items::GearSettings {
            size_by: if by_teeth { SizeBy::Teeth } else { SizeBy::Module },
            module: if by_module { len(&self.module, "module")? } else { None },
            teeth: if by_teeth { eval(app, &self.teeth, ValueKind::Unitless, &what("teeth"))?.map(|z| z.round().max(1.0) as u32) } else { None },
            face,
            helix: eval(app, &self.helix, ValueKind::Angle, &what("helix angle"))?,
            hand: match self.hand {
                1 => Some(Hand::Left),
                2 => Some(Hand::Right),
                _ => None,
            },
            v: self.v,
            profile_shift: eval(app, &self.profile_shift, ValueKind::Unitless, &what("profile shift"))?,
            bore,
            hub,
            ..Default::default()
        })
    }

    /// What `gear_json` sent, as the fields show it (Edit reopens the set).
    pub fn from_json(g: &Value) -> GearSettings {
        let num = |k: &str| g.get(k).and_then(Value::as_f64);
        let face = g.get("face");
        let fnum = |k: &str| face.and_then(|f| f.get(k)).and_then(Value::as_f64);
        let preset = face.and_then(|f| f.get("preset")).and_then(Value::as_str).and_then(|p| FACE_PRESETS.iter().position(|x| x.1 == p)).unwrap_or(0);
        let hub = g.get("hub").filter(|h| h.is_object());
        let hnum = |k: &str| hub.and_then(|h| h.get(k)).and_then(Value::as_f64);
        let bore = match g.get("bore") {
            Some(Value::Null) => (1, String::new()),
            Some(Value::Number(n)) => (2, shown(n.as_f64(), ValueKind::Length)),
            _ => (0, String::new()),
        };
        GearSettings {
            by_teeth: g.get("size_by").and_then(Value::as_str) == Some("teeth"),
            both: num("module").is_some() && num("teeth").is_some(),
            module: shown(num("module"), ValueKind::Length),
            teeth: shown(num("teeth"), ValueKind::Unitless),
            face_preset: preset,
            face_a: if preset == 2 { String::new() } else { shown(fnum("a"), ValueKind::Length) },
            face_b: if preset < 2 { String::new() } else { shown(fnum("b"), ValueKind::Length) },
            helix: shown(num("helix_angle"), ValueKind::Angle),
            hand: match g.get("hand").and_then(Value::as_str) {
                Some("left") => 1,
                Some("right") => 2,
                _ => 0,
            },
            v: g.get("herringbone").and_then(Value::as_bool) == Some(true),
            profile_shift: shown(num("profile_shift"), ValueKind::Unitless),
            bore: bore.0,
            bore_d: bore.1,
            hub: hub.is_some(),
            hub_d: shown(hnum("diameter"), ValueKind::Length),
            hub_a: shown(hnum("length_a"), ValueKind::Length),
            hub_b: shown(hnum("length_b"), ValueKind::Length),
        }
    }
}

/// Per gear of an item made of `things`: whether no circle gives its size (a bevel pair drawn as a
/// side view), so its module and teeth are both typed.
pub fn unsized_gears(kind: ItemKind, things: &[Thing]) -> Vec<bool> {
    vec![items::sized_by_settings(kind, things); items::gear_count(kind, things)]
}

/// One gear's entry in an item's `gears` (§6), named as the gear crate's
/// [`items::GearSettings`] fields: `size_by` ("module" | "teeth"), `module` or `teeth` (the one it
/// is sized by; both where no circle sizes it; absent: Auto), `face` {`preset`, `a`, `b`} (mm, null: Auto), `helix_angle`
/// (degrees), `hand` ("left" | "right" | null: opposite its mate), `herringbone` (V), and
/// `profile_shift` (a value | "auto"), `bore` ("auto" | null: none | Ø) and `hub` ({diameter,
/// length_a, length_b} | absent).
pub fn gear_json(gear: usize, s: &items::GearSettings) -> Value {
    let mut e = json!({"gear": gear, "size_by": match s.size_by { SizeBy::Module => "module", SizeBy::Teeth => "teeth" }});
    // Each as read ([`GearSettings::read`] keeps the one it is not sized by only where no circle
    // sizes the gear, so both are its own then).
    if let Some(m) = s.module {
        e["module"] = json!(m);
    }
    if let Some(z) = s.teeth.filter(|_| s.size_by == SizeBy::Teeth) {
        e["teeth"] = json!(z);
    }
    let preset = FACE_PRESETS.iter().find(|p| p.0 == s.face.preset).map_or("symmetric", |p| p.1);
    e["face"] = json!({"preset": preset, "a": s.face.a, "b": s.face.b});
    if let Some(h) = s.helix {
        e["helix_angle"] = json!(h);
    }
    e["hand"] = match s.hand {
        Some(Hand::Left) => json!("left"),
        Some(Hand::Right) => json!("right"),
        None => Value::Null,
    };
    if s.v {
        e["herringbone"] = json!(true);
    }
    e["profile_shift"] = s.profile_shift.map_or(json!("auto"), |x| json!(x));
    e["bore"] = match s.bore {
        Bore::Auto => json!("auto"),
        Bore::Solid => Value::Null,
        Bore::Diameter(d) => json!(d),
    };
    if let Some(h) = s.hub {
        e["hub"] = json!({"diameter": h.diameter, "length_a": h.length_a, "length_b": h.length_b});
        if h.auto {
            e["hub"]["auto"] = json!(true);
        }
    }
    e
}

/// One gear item: its type, the geometry in its slots (in pick order), each gear's settings,
/// and the type's own.
#[derive(Clone, Debug, PartialEq)]
pub struct GearItem {
    pub kind: ItemKind,
    pub picks: Vec<Sel>,
    /// Its gears' bore and hub circles (`true`: a hub), picked beside its slots
    /// ([`items::DETAIL_SLOTS`]): sent as components with slot `bore` / `hub`; the engine reads
    /// which gear each belongs to from where it is drawn.
    pub details: Vec<(Sel, bool)>,
    /// Its bore and hub circles come from the drawing by themselves (a gear made in this dialog;
    /// one reopened keeps what it was made with), less those taken out by hand (`dropped`).
    pub auto_details: bool,
    pub dropped: Vec<Sel>,
    pub gears: Vec<GearSettings>,
    /// Ring gear: rim; worm: starts and a throated wheel; bevel and hypoid: style (an index into
    /// `BEVEL_STYLES`) and spiral angle; rack: back height; planetary: planets and a carrier;
    /// shaft: Ø and a keyway.
    pub rim: String,
    pub starts: String,
    pub throated: bool,
    pub bevel_style: usize,
    pub spiral: String,
    pub back_height: String,
    pub planets: String,
    pub carrier: bool,
    pub shaft_d: String,
    pub keyway: bool,
    /// Bevel and hypoid pairs: the differential its ring gear carries (`None`: none).
    pub diff: Option<DiffForm>,
}

/// A differential in a bevel or hypoid pair's ring gear (the item's `differential`), as typed
/// (empty: Auto).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffForm {
    /// An index into [`solidcraft_gear::diff::Style::ALL`]: open, biased, Torsen B, Torsen A.
    pub style: usize,
    pub pinions: String,
    pub module: String,
    pub axle_length: String,
    pub axle_d: String,
}

impl DiffForm {
    pub fn style(&self) -> solidcraft_gear::diff::Style {
        solidcraft_gear::diff::Style::ALL.get(self.style).copied().unwrap_or_default()
    }

    /// The item's `differential`: `{style, pinions?, module?, axle?: {length?, diameter?}}`.
    pub fn json(&self, app: &SolveApp, who: &str) -> Result<Value, String> {
        let num = |t: &str, kind: ValueKind, f: &str| eval(app, t, kind, &format!("{who}: differential {f}"));
        let mut d = json!({"style": self.style().name()});
        if let Some(n) = num(&self.pinions, ValueKind::Unitless, "pinions")? {
            d["pinions"] = json!(n.round().max(1.0) as u64);
        }
        if let Some(m) = num(&self.module, ValueKind::Length, "module")? {
            d["module"] = json!(m);
        }
        let mut axle = Map::new();
        if let Some(l) = num(&self.axle_length, ValueKind::Length, "half-shaft length")? {
            axle.insert("length".into(), json!(l));
        }
        if let Some(dia) = num(&self.axle_d, ValueKind::Length, "half-shaft Ø")? {
            axle.insert("diameter".into(), json!(dia));
        }
        if !axle.is_empty() {
            d["axle"] = Value::Object(axle);
        }
        Ok(d)
    }

    /// What an item's `differential` was made with (Edit).
    pub fn of(v: &Value) -> DiffForm {
        let style = v.get("style").and_then(Value::as_str).and_then(solidcraft_gear::diff::Style::parse).unwrap_or_default();
        let axle = v.get("axle");
        DiffForm {
            style: solidcraft_gear::diff::Style::ALL.iter().position(|s| *s == style).unwrap_or(0),
            pinions: shown(v.get("pinions").and_then(Value::as_f64), ValueKind::Unitless),
            module: shown(v.get("module").and_then(Value::as_f64), ValueKind::Length),
            axle_length: shown(axle.and_then(|a| a.get("length")).and_then(Value::as_f64), ValueKind::Length),
            axle_d: shown(axle.and_then(|a| a.get("diameter")).and_then(Value::as_f64), ValueKind::Length),
        }
    }
}

const BEVEL_STYLES: [BevelStyle; 3] = [BevelStyle::Straight, BevelStyle::Spiral, BevelStyle::Chevron];

impl Default for GearItem {
    fn default() -> GearItem {
        GearItem::new(ItemKind::Spur)
    }
}

impl GearItem {
    pub fn new(kind: ItemKind) -> GearItem {
        let mut it = GearItem {
            kind,
            picks: Vec::new(),
            details: Vec::new(),
            auto_details: true,
            dropped: Vec::new(),
            gears: Vec::new(),
            rim: String::new(),
            starts: String::new(),
            throated: false,
            bevel_style: 0,
            spiral: String::new(),
            back_height: String::new(),
            planets: String::new(),
            carrier: true,
            shaft_d: String::new(),
            keyway: false,
            diff: None,
        };
        if matches!(kind, ItemKind::Helical | ItemKind::Herringbone) {
            it.gears.push(GearSettings { helix: "20 deg".into(), v: kind == ItemKind::Herringbone, ..Default::default() });
        }
        it
    }

    pub fn type_name(&self) -> &'static str {
        self.kind.name()
    }

    pub fn label(&self) -> &'static str {
        self.kind.label()
    }

    pub fn is_gear(&self) -> bool {
        self.kind != ItemKind::Shaft
    }

    /// The picks as the gear crate takes them (one that can't be read is left out).
    pub fn things(&self, app: &SolveApp) -> Vec<Thing> {
        self.picks.iter().filter_map(|s| crate::dialogs_gear::thing_of(app, s)).collect()
    }

    /// The picks that no longer read as geometry (a curve deleted from its sketch, a sketch
    /// deleted): each named, so the card can say which is gone. They are kept (never swapped for
    /// something else) until a new pick replaces one or the gear is removed.
    pub fn missing(&self, app: &SolveApp) -> Vec<String> {
        let details = self.details.iter().map(|(s, _)| s);
        self.picks.iter().chain(details).filter(|s| crate::dialogs_gear::thing_of(app, s).is_none()).map(|s| pick_name(app, s)).collect()
    }

    /// Everything picked into it: its slots' picks, then its bore and hub circles.
    pub fn all_picks(&self) -> Vec<Sel> {
        self.picks.iter().cloned().chain(self.details.iter().map(|(s, _)| s.clone())).collect()
    }

    /// Its bore and hub circles as they are now: (gear, hub?, the circle, the pick), each by the
    /// gear whose pitch circle it sits in ([`items::detail_gear`]); one that no longer reads, or
    /// sits in no gear now, is left out (see [`GearItem::missing`]).
    pub fn detail_things(&self, app: &SolveApp) -> Vec<(usize, bool, Thing, Sel)> {
        let it = self.crate_item(app, "");
        self.details
            .iter()
            .filter_map(|(sel, hub)| {
                let t = crate::dialogs_gear::thing_of(app, sel)?;
                Some((items::detail_gear(&it, &t)?, *hub, t, sel.clone()))
            })
            .collect()
    }

    /// `sel` taken as a bore or hub circle when it is one for one of this item's gears (a circle
    /// concentric with its pitch circle, inside it): the smaller of a gear's two is its bore, the
    /// larger its hub. `None`: not such a circle (it goes to the slots); `Some(Err)`: that gear
    /// has both already.
    pub fn take_detail(&mut self, app: &SolveApp, sel: &Sel) -> Option<Result<(), String>> {
        if !items::takes_details(self.kind) || self.details.iter().any(|(s, _)| s == sel) {
            return None;
        }
        let t = crate::dialogs_gear::thing_of(app, sel)?;
        let g = items::detail_gear(&self.crate_item(app, ""), &t)?;
        let mut mine: Vec<(Sel, f64)> = self
            .detail_things(app)
            .into_iter()
            .filter(|d| d.0 == g)
            .map(|(_, _, t, s)| (s, radius(&t)))
            .collect();
        if mine.len() >= 2 {
            return Some(Err("this gear has a bore and a hub circle already: click one of them to take it out first".into()));
        }
        mine.push((sel.clone(), radius(&t)));
        mine.sort_by(|a, b| a.1.total_cmp(&b.1));
        self.details.retain(|(s, _)| !mine.iter().any(|m| m.0 == *s));
        for (k, (s, _)) in mine.into_iter().enumerate() {
            self.details.push((s, k == 1));
        }
        Some(Ok(()))
    }

    /// A new pick taken in: it replaces the first missing pick, if any (a re-pick).
    pub fn take(&mut self, app: &SolveApp, sel: Sel) {
        if let Some(i) = self.picks.iter().position(|s| crate::dialogs_gear::thing_of(app, s).is_none()) {
            self.picks.remove(i);
        }
        self.picks.push(sel);
    }

    /// The checklist (§1): each slot of the layout the picks fill (a bevel pair: two circles, or
    /// two axis lines and a circle) as (what, required, filled), a repeating slot once more per
    /// extra pick.
    pub fn checklist(&self, app: &SolveApp) -> Vec<(&'static str, bool, bool)> {
        let things = self.things(app);
        let filled = items::assign(self.kind, &things).unwrap_or_default();
        let layouts = items::slots(self.kind);
        let list = layouts[filled.first().map_or(0, |s| s.layout).min(layouts.len() - 1)];
        let mut out = Vec::new();
        for (i, s) in list.iter().enumerate() {
            let n = filled.iter().filter(|f| f.slot == i).count();
            out.push((s.what, s.required, n > 0));
            out.extend((1..n).map(|_| (s.what, false, true)));
        }
        out
    }

    /// The checklist with what fills each slot ([`SlotRow`]): the layout the picks fill (a bevel
    /// pair: two circles, two axis lines and a circle, or a side view), each slot once, a
    /// repeating slot once more per extra pick, and the next one to fill marked.
    pub fn slot_rows(&self, app: &SolveApp, len: &dyn Fn(f64) -> String) -> Vec<SlotRow> {
        let things = self.things(app);
        let filled = items::assign(self.kind, &things).unwrap_or_default();
        let layouts = items::slots(self.kind);
        let list = layouts[filled.first().map_or(0, |s| s.layout).min(layouts.len() - 1)];
        let next = items::next_slot(self.kind, &things);
        let mut out = Vec::new();
        let mut marked = false;
        for (i, s) in list.iter().enumerate() {
            let picks: Vec<String> = filled.iter().zip(&things).filter(|(f, _)| f.slot == i).map(|(_, t)| thing_label(app, t, len)).collect();
            if picks.is_empty() {
                let is_next = !marked && next == Some(s.what);
                marked |= is_next;
                out.push(SlotRow { what: s.what, required: s.required, filled: None, next: is_next, repeats: s.repeats });
            }
            for (n, p) in picks.into_iter().enumerate() {
                out.push(SlotRow { what: s.what, required: s.required && n == 0, filled: Some(p), next: false, repeats: s.repeats });
            }
        }
        out
    }

    /// The next required slot ("shaft line"), `None` when every one is filled.
    pub fn next_slot(&self, app: &SolveApp) -> Option<String> {
        items::next_slot(self.kind, &self.things(app)).map(str::to_string)
    }

    /// What it can still take once complete ("shaft line", "another planet's pitch circle"),
    /// `None` when nothing.
    pub fn optional_slot(&self, app: &SolveApp) -> Option<String> {
        let things = self.things(app);
        let filled = items::assign(self.kind, &things).ok()?;
        let layouts = items::slots(self.kind);
        let list = layouts[filled.first().map_or(0, |s| s.layout).min(layouts.len() - 1)];
        let open = list.iter().enumerate().find(|(i, s)| !s.required && !filled.iter().any(|f| f.slot == *i)).map(|(_, s)| s.what.to_string());
        open.or_else(|| list.iter().rev().find(|s| s.repeats).map(|s| format!("another {}", s.what)))
    }

    pub fn complete(&self, app: &SolveApp) -> bool {
        let things = self.things(app);
        things.len() == self.picks.len() && items::assign(self.kind, &things).is_ok() && items::next_slot(self.kind, &things).is_none()
    }

    /// The gears it makes ([`items::gear_count`]).
    pub fn gear_count(&self, app: &SolveApp) -> usize {
        items::gear_count(self.kind, &self.things(app))
    }

    /// Why `sel` can't be picked into this item (None: it fills a slot): a circle or point
    /// another item holds (a line is a shaft several gears can share), not geometry an item
    /// takes, or the gear crate's check on the slot it would fill ([`items::validate_slot`]:
    /// what is wrong and what to do).
    pub fn refusal(&self, app: &SolveApp, sel: &Sel, others: &[(usize, &GearItem)]) -> Option<String> {
        let Some(thing) = crate::dialogs_gear::thing_of(app, sel) else {
            return Some("pick a sketch circle, line or point, a model edge, a round face or an axis".into());
        };
        let shared = matches!(thing, items::Thing::Line { .. });
        if let Some((i, _)) = others.iter().find(|(_, it)| it.all_picks().contains(sel)).filter(|_| !shared) {
            return Some(format!("already used by Gear {}: click its card to change it", item_id(*i)));
        }
        items::validate_slot(self.kind, &self.things(app), &thing).err()
    }

    /// The gear settings, one per gear (a new gear starts from the first one's).
    pub fn settings_mut(&mut self, n: usize) -> &mut Vec<GearSettings> {
        let seed = self.gears.first().cloned().unwrap_or_default();
        while self.gears.len() < n {
            let mut s = seed.clone();
            s.teeth.clear();
            self.gears.push(s);
        }
        &mut self.gears
    }

    /// Gear `g`'s settings as typed: its own, else the first gear's (with no teeth).
    pub fn typed(&self, g: usize) -> GearSettings {
        self.gears.get(g).cloned().unwrap_or_else(|| GearSettings { teeth: String::new(), ..self.gears.first().cloned().unwrap_or_default() })
    }

    /// The gear crate's item (id `id`): its things and each gear's settings (a value that
    /// doesn't read is Auto here; [`items_request`] refuses it).
    pub fn crate_item(&self, app: &SolveApp, id: &str) -> items::GearItem {
        let mut it = items::GearItem::new(id, self.kind).with(self.things(app));
        let n = it.gear_count();
        it.gears = (0..n).map(|g| self.typed(g).read(app, &|f: &str| f.to_string()).unwrap_or_default()).collect();
        it
    }
}

/// What the dialog sends for its items (§6): each its `id`, `type`, `components` (`selection`
/// locators, each with its `slot` from [`items::assign`]) and `gears` ([`gear_json`]); the type's
/// own (`rim`, `starts`, `throated`, `bevel_style`, `spiral_angle`, `back_height`, `planets`,
/// `carrier`, `diameter`, `keyway`) and its `module_block`; the `links`
/// ({a: {item, port}, b: {item, port}, kind}); the `module_blocks` ({id, name?, module?, and the
/// block's other settings}). The picks themselves go in `ui.gear_items` so Edit gets them back
/// exactly.
pub fn items_request(app: &SolveApp, items: &[GearItem], links: &Links) -> Result<Map<String, Value>, String> {
    let mut list = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let who = format!("Gear {} · {}", item_id(i), it.label());
        let things: Vec<Thing> = it
            .picks
            .iter()
            .map(|s| crate::dialogs_gear::thing_of(app, s).ok_or_else(|| format!("{who}: its {} is missing (deleted or renamed in its sketch): click its replacement in the view", pick_name(app, s))))
            .collect::<Result<_, _>>()?;
        if let Some(next) = items::next_slot(it.kind, &things) {
            return Err(format!("{who} still needs a {next}: click one in the view (or remove the gear)"));
        }
        let slots = items::assign(it.kind, &things).map_err(|e| format!("{who}: {e}"))?;
        let mut components = Vec::new();
        for (pick, slot) in it.picks.iter().zip(&slots) {
            let mut c = crate::dialogs_gear::locate(app, pick).ok_or_else(|| format!("{who}: a pick that can't be part of a gear: remove the gear and make it again"))?;
            c["slot"] = json!(slot.name);
            components.push(c);
        }
        // Its gears' bore and hub circles (the engine finds the gear each sits in).
        for (sel, hub) in &it.details {
            if crate::dialogs_gear::thing_of(app, sel).is_none() {
                return Err(format!("{who}: its {} is missing (deleted or renamed in its sketch): click its replacement in the view", pick_name(app, sel)));
            }
            let mut c = crate::dialogs_gear::locate(app, sel).ok_or_else(|| format!("{who}: a bore or hub pick that can't be read: click the gear's card and pick it again"))?;
            c["slot"] = json!(if *hub { "hub" } else { "bore" });
            components.push(c);
        }
        let mut gears = Vec::new();
        let free = unsized_gears(it.kind, &things);
        for g in 0..items::gear_count(it.kind, &things) {
            let typed = GearSettings { both: free.get(g).copied().unwrap_or(false), ..it.typed(g) };
            let s = typed.read(app, &|f: &str| format!("{who} gear {}: {f}", g + 1))?;
            gears.push(gear_json(g, &s));
        }
        let mut item = json!({"id": item_id(i), "type": it.type_name(), "components": components, "gears": gears});
        let num = |t: &str, kind: ValueKind, f: &str| eval(app, t, kind, &format!("{who}: {f}"));
        match it.kind {
            ItemKind::Internal => item["rim"] = json!(num(&it.rim, ValueKind::Length, "rim")?),
            ItemKind::Worm => {
                item["starts"] = json!(num(&it.starts, ValueKind::Unitless, "starts")?.map(|v| v.round().max(1.0) as u64));
                if it.throated {
                    item["throated"] = json!(true);
                }
            }
            ItemKind::Bevel | ItemKind::Hypoid => {
                item["bevel_style"] = json!(BEVEL_STYLES[it.bevel_style.min(2)].name());
                item["spiral_angle"] = json!(num(&it.spiral, ValueKind::Angle, "spiral angle")?);
                if let Some(d) = &it.diff {
                    item["differential"] = d.json(app, &who)?;
                }
            }
            ItemKind::Rack => item["back_height"] = json!(num(&it.back_height, ValueKind::Length, "back height")?),
            ItemKind::Planetary => {
                item["planets"] = json!(num(&it.planets, ValueKind::Unitless, "planets")?.map(|v| v.round().max(1.0) as u64));
                item["carrier"] = json!(it.carrier);
            }
            ItemKind::Shaft => {
                item["diameter"] = json!(num(&it.shaft_d, ValueKind::Length, "shaft Ø")?);
                item["keyway"] = json!(it.keyway);
            }
            _ => {}
        }
        list.push(item);
    }
    let blocks_of = links;
    let links: Vec<Value> = links
        .all()
        .into_iter()
        .map(|(a, ag, b, bg, kind)| json!({"a": {"item": item_id(a), "port": ag}, "b": {"item": item_id(b), "port": bg}, "kind": kind}))
        .collect();
    let mut blocks = Vec::new();
    for (n, b) in blocks_of.blocks().iter().enumerate() {
        let v = b.json(app, n)?;
        for i in &b.members {
            if let Some(item) = list.get_mut(*i) {
                item["module_block"] = v["id"].clone();
            }
        }
        blocks.push(v);
    }
    let picks: Vec<Value> = items.iter().map(|it| serde_json::to_value(&it.picks).unwrap_or(Value::Null)).collect();
    let mut p = Map::new();
    p.insert("items".into(), Value::Array(list));
    p.insert("links".into(), Value::Array(links));
    p.insert("module_blocks".into(), Value::Array(blocks));
    let details: Vec<Value> = items.iter().map(|it| serde_json::to_value(&it.details).unwrap_or(Value::Null)).collect();
    p.insert("ui".into(), json!({"gear_items": picks, "gear_details": details}));
    Ok(p)
}

/// The items, links and blocks a set was made with (its request, as [`items_request`] and the
/// dialog's `drive` sent them): Edit reopens the dialog on them. The picks come from
/// `ui` (its `gear_items`); a request without them names each component by its locator.
pub fn items_of(r: &Map<String, Value>, ui: Option<&Value>, graph: &mut Links) -> Vec<GearItem> {
    let picks_ui = ui.and_then(|u| u.get("gear_items")).and_then(Value::as_array);
    let mut out = Vec::new();
    for (i, v) in r.get("items").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let kind = v.get("type").and_then(Value::as_str).and_then(ItemKind::parse).unwrap_or(ItemKind::Spur);
        let mut it = GearItem::new(kind);
        it.auto_details = false;
        it.picks = match picks_ui.and_then(|p| p.get(i)).and_then(|p| serde_json::from_value::<Vec<Sel>>(p.clone()).ok()) {
            Some(p) => p,
            None => v.get("components").and_then(Value::as_array).into_iter().flatten().filter(|c| !is_detail(c)).filter_map(sel_of).collect(),
        };
        // Its bore and hub circles: as the dialog kept them, else the components so named.
        let details_ui = ui.and_then(|u| u.get("gear_details")).and_then(|d| d.get(i)).and_then(|d| serde_json::from_value::<Vec<(Sel, bool)>>(d.clone()).ok());
        it.details = details_ui.unwrap_or_else(|| {
            v.get("components")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|c| is_detail(c))
                .filter_map(|c| Some((sel_of(c)?, c.get("slot").and_then(Value::as_str) == Some("hub"))))
                .collect()
        });
        it.gears = v.get("gears").and_then(Value::as_array).into_iter().flatten().map(GearSettings::from_json).collect();
        let s = |k: &str, kind: ValueKind| shown(v.get(k).and_then(Value::as_f64), kind);
        it.rim = s("rim", ValueKind::Length);
        it.starts = s("starts", ValueKind::Unitless);
        it.throated = v.get("throated").and_then(Value::as_bool) == Some(true);
        it.bevel_style = v.get("bevel_style").and_then(Value::as_str).and_then(|b| BEVEL_STYLES.iter().position(|x| x.name() == b)).unwrap_or(0);
        it.spiral = s("spiral_angle", ValueKind::Angle);
        it.back_height = s("back_height", ValueKind::Length);
        it.planets = s("planets", ValueKind::Unitless);
        it.carrier = v.get("carrier").and_then(Value::as_bool) != Some(false);
        it.shaft_d = s("diameter", ValueKind::Length);
        it.keyway = v.get("keyway").and_then(Value::as_bool) == Some(true);
        it.diff = v.get("differential").filter(|d| d.is_object()).map(DiffForm::of);
        // Saved with the circles it keeps out: it takes any new one the drawing gets by itself.
        if let Some(d) = ui.and_then(|u| u.get("gear_dropped")).and_then(|d| d.get(i)).and_then(|d| serde_json::from_value::<Vec<Sel>>(d.clone()).ok()) {
            it.auto_details = true;
            it.dropped = d;
        }
        out.push(it);
    }
    for l in r.get("links").and_then(Value::as_array).into_iter().flatten() {
        let end = |k: &str| -> Option<(usize, usize)> {
            let e = l.get(k)?;
            Some((item_index(e.get("item")?)?, e.get("port").or(e.get("gear")).and_then(Value::as_u64).unwrap_or(0) as usize))
        };
        if let (Some((a, a_gear)), Some((b, b_gear))) = (end("a"), end("b")) {
            let kind = l.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string();
            graph.accept(&crate::dialogs_gear_links::Suggested { a, a_gear, b, b_gear, kind });
        }
    }
    let ids: Vec<Option<&str>> = r.get("items").and_then(Value::as_array).into_iter().flatten().map(|v| v.get("module_block").and_then(Value::as_str)).collect();
    // (A set saved before blocks had more than a module reads back with only its module.)
    for b in r.get("module_blocks").and_then(Value::as_array).into_iter().flatten() {
        let k = graph.add_block(&[]);
        if let Some(block) = graph.block_mut(k) {
            *block = crate::dialogs_gear_links::Block::of(b);
        }
        let id = b.get("id").and_then(Value::as_str);
        let mut members: Vec<usize> = ids.iter().enumerate().filter(|(_, m)| id.is_some() && **m == id).map(|(i, _)| i).collect();
        members.extend(b.get("members").and_then(Value::as_array).into_iter().flatten().filter_map(item_index));
        for i in members {
            graph.put_in_block(k, i);
        }
    }
    out
}

/// A `selection` locator back as a pick (without `ui.gear_items`): a sketch's curve or point, an
/// axis; a model edge or face named by a point can't be found again this way.
fn sel_of(c: &Value) -> Option<Sel> {
    let sketch = c.get("sketch").and_then(Value::as_u64);
    if let (Some(sketch), Some(id)) = (sketch, c.get("curve").and_then(Value::as_str)) {
        return Some(Sel::SketchCurve { id: id.to_string(), sketch: Some(sketch) });
    }
    if let (Some(sketch), Some(id)) = (sketch, c.get("point").and_then(Value::as_str)) {
        return Some(Sel::ExternalSketchPoint { sketch, id: id.to_string() });
    }
    c.get("axis").and_then(Value::as_str).map(|name| Sel::Axis { name: name.to_string() })
}

