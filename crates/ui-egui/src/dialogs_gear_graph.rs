//! The Gears dialog's items, their settings, and the node graph that links them
//! (GEAR_UI_SPEC.md §1–§4).
//!
//! A gear item is one thing the generator makes, defined by the geometry picked into its slots:
//! a spur gear by its pitch circle (and optionally its shaft line), a compound gear by its shaft
//! line and two or more circles on it, a bevel or hypoid pair or a worm and wheel by both gears'
//! geometry at once, a shaft by its line and the bearing points on it. Pick a type, press Create
//! New, then click the geometry the status line asks for; a pick that doesn't fit is refused
//! with the reason. Every gear of an item has its own settings (§3).
//!
//! Each item is a node of the graph (drawn with egui-snarl, vendored), one row per gear, each
//! with a Mesh pin and a Shaft pin: mesh to mesh says two gears mesh, shaft to shaft that they
//! share a shaft; an item's Module pin into a Module Block says its gears share that block's
//! module. Links the drawing implies come back from `gear.calculate` and are accepted with one
//! button; each link shows its ratio or its error. The items, links, blocks and the driver are
//! sent as `gear.create`'s `items`, `links`, `module_blocks` and `drive` (§6).

use egui_snarl::ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget};
use egui_snarl::{InPin, InPinId, NodeId, OutPin, OutPinId, Snarl};
use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;

use crate::SolveApp;
use crate::ui_kit::{self, choice_row, hint_row};

/// What an item can be: `gear.create`'s item types, with their labels.
pub const ITEM_TYPES: [(&str, &str); 11] = [
    ("spur", "Spur"),
    ("helical", "Helical"),
    ("herringbone", "Herringbone"),
    ("internal", "Ring Gear"),
    ("rack", "Rack"),
    ("compound", "Compound"),
    ("bevel", "Bevel Pair"),
    ("hypoid", "Hypoid Pair"),
    ("worm", "Worm and Wheel"),
    ("crossed_helical", "Crossed Helical"),
    ("shaft", "Shaft"),
];

/// What a slot takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Circle,
    Line,
    Point,
}

impl Slot {
    fn name(self) -> &'static str {
        match self {
            Slot::Circle => "pitch circle",
            Slot::Line => "shaft line",
            Slot::Point => "bearing point",
        }
    }
}

/// An item type's slots: (what, required); the last one repeats when `more` (a compound's
/// circles, a shaft's bearing points).
fn slots(t: &str) -> (Vec<(Slot, bool)>, bool) {
    use Slot::*;
    match t {
        "spur" | "helical" | "herringbone" => (vec![(Circle, true), (Line, false)], false),
        "internal" => (vec![(Circle, true)], false),
        "rack" => (vec![(Line, true)], false),
        "compound" => (vec![(Line, true), (Circle, true), (Circle, true)], true),
        "bevel" | "crossed_helical" => (vec![(Circle, true), (Circle, true)], false),
        "hypoid" | "worm" => (vec![(Circle, true), (Line, true)], false),
        _ => (vec![(Line, true), (Point, false)], true),
    }
}

/// An item's id as the engine names it (A, B, … Z, then A2 …).
pub fn item_id(i: usize) -> String {
    let letter = char::from(b'A' + (i % 26) as u8);
    if i < 26 { letter.to_string() } else { format!("{letter}{}", i / 26 + 1) }
}

/// The index of item `id` (an id as [`item_id`] makes it, or a number).
pub fn item_index(id: &Value) -> Option<usize> {
    if let Some(n) = id.as_u64() {
        return Some(n as usize);
    }
    let s = id.as_str()?;
    let mut c = s.chars();
    let letter = c.next()?;
    if !letter.is_ascii_uppercase() {
        return None;
    }
    let round: usize = c.as_str().parse().unwrap_or(1);
    Some((round - 1) * 26 + usize::from(letter as u8 - b'A'))
}

/// The slot names the engine gives an item type's components, in the order its kinds are
/// filled: (circles, lines, points), each list repeating its last name numbered on.
fn slot_names(t: &str) -> (&'static [&'static str], &'static [&'static str], &'static [&'static str]) {
    match t {
        "compound" => (&["pitch", "pitch2"], &["axis"], &[]),
        "bevel" | "crossed_helical" => (&["pitch", "pitch2"], &[], &[]),
        "hypoid" | "worm" => (&["pitch"], &["axis2"], &[]),
        "shaft" => (&[], &["axis"], &["bearing", "bearing2"]),
        "rack" => (&[], &["axis"], &[]),
        _ => (&["pitch"], &["axis"], &[]),
    }
}

/// The n-th name (0 first) of a slot list that repeats its last name numbered on (pitch2,
/// pitch3, …).
fn nth_slot(names: &[&str], n: usize) -> Option<String> {
    if let Some(s) = names.get(n) {
        return Some(s.to_string());
    }
    let last = names.last()?;
    let stem = last.trim_end_matches(|c: char| c.is_ascii_digit());
    Some(format!("{stem}{}", n + 1))
}

/// A row with its hover guide (§5): the whole row shows it.
fn hinted(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str, tip: &str) -> bool {
    let mut enter = false;
    ui.scope(|ui| enter = hint_row(ui, label, value, hint)).response.on_hover_text(tip);
    enter
}

/// A gear's own settings (§3), as typed (empty: Auto).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearSettings {
    pub teeth: String,
    /// Face preset: 0 Symmetric, 1 Side A only, 2 Side B only, 3 Custom.
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

/// One gear item: its type (an index into [`ITEM_TYPES`]), the geometry in its slots, whether
/// it is sized by module or teeth (and its module), each gear's settings, and the type's own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearItem {
    pub kind: usize,
    pub picks: Vec<Sel>,
    pub by_teeth: bool,
    pub module: String,
    pub gears: Vec<GearSettings>,
    /// Ring gear: rim; worm: starts and a throated wheel; bevel and hypoid: style (0 straight,
    /// 1 spiral, 2 chevron) and spiral angle; rack: back height; shaft: Ø and a keyway.
    pub rim: String,
    pub starts: String,
    pub throated: bool,
    pub bevel_style: usize,
    pub spiral: String,
    pub back_height: String,
    pub shaft_d: String,
    pub keyway: bool,
}

impl GearItem {
    pub fn new(kind: usize) -> GearItem {
        let mut it = GearItem { kind, ..Default::default() };
        if matches!(it.type_name(), "helical" | "herringbone") {
            it.gears.push(GearSettings { helix: "20 deg".into(), v: it.type_name() == "herringbone", ..Default::default() });
        }
        it
    }

    pub fn type_name(&self) -> &'static str {
        ITEM_TYPES.get(self.kind).map_or("spur", |t| t.0)
    }

    pub fn label(&self) -> &'static str {
        ITEM_TYPES.get(self.kind).map_or("", |t| t.1)
    }

    fn is_gear(&self) -> bool {
        self.type_name() != "shaft"
    }

    /// The slots: (slot, required, filled).
    pub fn slots_now(&self, app: &SolveApp) -> Vec<(Slot, bool, bool)> {
        let (list, more) = slots(self.type_name());
        let mut have: Vec<Slot> = self.picks.iter().filter_map(|s| slot_of(app, s)).collect();
        let mut out = Vec::new();
        for (slot, required) in &list {
            let filled = have.iter().position(|h| h == slot).map(|i| have.remove(i)).is_some();
            out.push((*slot, *required, filled));
        }
        if more && let Some(last) = list.last().map(|s| s.0) {
            for _ in have.iter().filter(|h| **h == last) {
                out.push((last, false, true));
            }
        }
        out
    }

    /// The next required slot, as the status line says it ("pitch circle 1 of 2+").
    pub fn next_slot(&self, app: &SolveApp) -> Option<String> {
        let now = self.slots_now(app);
        let (list, more) = slots(self.type_name());
        let (slot, _, _) = now.iter().find(|(_, required, filled)| *required && !filled)?;
        let of_kind = now.iter().filter(|(s, required, _)| s == slot && *required).count();
        let done = now.iter().filter(|(s, _, filled)| s == slot && *filled).count();
        let plus = if more && list.last().map(|s| s.0) == Some(*slot) { "+" } else { "" };
        Some(if of_kind > 1 { format!("{} {} of {of_kind}{plus}", slot.name(), done + 1) } else { slot.name().to_string() })
    }

    pub fn complete(&self, app: &SolveApp) -> bool {
        self.slots_now(app).iter().all(|(_, required, filled)| !required || *filled)
    }

    /// The gears it makes: one per circle picked; a pair two; a rack one; a shaft none.
    pub fn gear_count(&self, app: &SolveApp) -> usize {
        if !self.is_gear() {
            return 0;
        }
        let circles = self.picks.iter().filter(|s| slot_of(app, s) == Some(Slot::Circle)).count();
        match self.type_name() {
            "bevel" | "hypoid" | "worm" | "crossed_helical" => 2,
            "rack" => 1,
            _ => circles.max(1),
        }
    }

    /// Why `sel` can't be picked into this item (None: it fills a slot).
    pub fn refusal(&self, app: &SolveApp, sel: &Sel, others: &[(usize, &GearItem)]) -> Option<String> {
        if let Some((i, _)) = others.iter().find(|(_, it)| it.picks.contains(sel)) {
            return Some(format!("already used by Item {}", i + 1));
        }
        if crate::dialogs_gear::is_construction(app, sel) {
            return Some("a construction line is a helper, not a shaft".into());
        }
        let Some(kind) = slot_of(app, sel) else { return Some("pick a sketch circle, line or point, a model edge or a cylinder face".into()) };
        if kind == Slot::Point && !crate::dialogs_gear::point_on_picked_line(app, sel, &self.picks) {
            return Some("this point is not on a shaft line: pick the shaft line first, or put the point on it (Coincident)".into());
        }
        let (list, more) = slots(self.type_name());
        let now = self.slots_now(app);
        let open = now.iter().any(|(s, _, filled)| *s == kind && !filled) || (more && list.last().map(|s| s.0) == Some(kind));
        (!open).then(|| match now.iter().find(|(_, required, filled)| *required && !filled) {
            Some((want, _, _)) => format!("this item needs a {}, not a {}", want.name(), kind.name()),
            None => format!("a {} has no room for another {}", self.label(), kind.name()),
        })
    }

    /// The gear settings, one per gear (a new gear starts from the first one's).
    fn settings_mut(&mut self, n: usize) -> &mut Vec<GearSettings> {
        let seed = self.gears.first().cloned().unwrap_or_default();
        while self.gears.len() < n {
            let mut s = seed.clone();
            s.teeth.clear();
            self.gears.push(s);
        }
        &mut self.gears
    }
}

/// What a pick can fill.
fn slot_of(app: &SolveApp, sel: &Sel) -> Option<Slot> {
    match crate::dialogs_gear::geo_kind(app, sel)? {
        true => Some(Slot::Circle),
        false => match sel {
            Sel::SketchPoint { .. } | Sel::ExternalSketchPoint { .. } => Some(Slot::Point),
            _ => Some(Slot::Line),
        },
    }
}

/// A node of the graph: an item (by its index), or a module block (its module as typed; empty:
/// Auto).
#[derive(Clone, Debug, PartialEq)]
pub enum GNode {
    Item(usize),
    Block { module: String },
}

/// The graph, kept with the dialog (compared and printed by its nodes and wires only).
#[derive(Clone, Default)]
pub struct Graph(pub Snarl<GNode>);

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph").field("nodes", &self.0.nodes().count()).field("wires", &self.0.wires().count()).finish()
    }
}

impl PartialEq for Graph {
    fn eq(&self, other: &Self) -> bool {
        let wires = |g: &Graph| g.0.wires().map(|(o, i)| (o.node.0, o.output, i.node.0, i.input)).collect::<Vec<_>>();
        self.0.nodes().collect::<Vec<_>>() == other.0.nodes().collect::<Vec<_>>() && wires(self) == wires(other)
    }
}

/// A gear row's pins: 2 × gear (Mesh), 2 × gear + 1 (Shaft); an item's Module pin is its last
/// output (2 × gears).
pub fn pin(gear: usize, shaft: bool) -> usize {
    2 * gear + usize::from(shaft)
}

impl Graph {
    /// The node of item `i`, made where there is room when it is new.
    pub fn node_of(&mut self, i: usize) -> NodeId {
        if let Some((id, _)) = self.0.node_ids().find(|(_, n)| **n == GNode::Item(i)) {
            return id;
        }
        let n = self.0.nodes().count() as f32;
        self.0.insert_node(egui::pos2(40.0 + 190.0 * (n % 3.0), 30.0 + 150.0 * (n / 3.0).floor()), GNode::Item(i))
    }

    fn item_of(&self, id: NodeId) -> Option<usize> {
        match self.0.get_node(id)? {
            GNode::Item(i) => Some(*i),
            GNode::Block { .. } => None,
        }
    }

    /// The links the wires make: (item, gear, item, gear, "mesh" | "shaft").
    pub fn links(&self) -> Vec<(usize, usize, usize, usize, &'static str)> {
        let mut out: Vec<_> = self
            .0
            .wires()
            .filter_map(|(o, i)| {
                let (a, b) = (self.item_of(o.node)?, self.item_of(i.node)?);
                let kind = if o.output % 2 == 0 { "mesh" } else { "shaft" };
                Some((a, o.output / 2, b, i.input / 2, kind))
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// The module blocks: each its module as typed and the items wired into it.
    pub fn blocks(&self) -> Vec<(String, Vec<usize>)> {
        self.0
            .node_ids()
            .filter_map(|(id, n)| match n {
                GNode::Block { module } => {
                    let mut items: Vec<usize> = self.0.wires().filter(|(_, i)| i.node == id).filter_map(|(o, _)| self.item_of(o.node)).collect();
                    items.sort_unstable();
                    Some((module.clone(), items))
                }
                GNode::Item(_) => None,
            })
            .collect()
    }

    /// A suggested link accepted: the wire it stands for.
    pub fn accept(&mut self, link: &Suggested) {
        let (na, nb) = (self.node_of(link.a), self.node_of(link.b));
        let shaft = link.kind == "shaft";
        self.0.connect(OutPinId { node: na, output: pin(link.a_gear, shaft) }, InPinId { node: nb, input: pin(link.b_gear, shaft) });
    }

    /// Item `i` taken out: its node and wires go, and later items' nodes are renumbered.
    pub fn remove_item(&mut self, i: usize) {
        if let Some((id, _)) = self.0.node_ids().find(|(_, n)| **n == GNode::Item(i)) {
            self.0.remove_node(id);
        }
        for n in self.0.nodes_mut() {
            if let GNode::Item(j) = n
                && *j > i
            {
                *j -= 1;
            }
        }
    }
}

/// A link the drawing suggests (`gear.calculate`'s `suggested_links`).
#[derive(Clone, Debug, PartialEq)]
pub struct Suggested {
    pub a: usize,
    pub a_gear: usize,
    pub b: usize,
    pub b_gear: usize,
    pub kind: String,
}

/// What `gear.calculate` says of each link (its ratio, or its error with what to do: red) and
/// the links it suggests.
pub fn link_notes(r: &Value) -> (Vec<(String, bool)>, Vec<Suggested>) {
    // An end: {item, gear} (or the item alone; gear 0).
    let end = |v: Option<&Value>| -> (usize, usize) {
        match v {
            Some(Value::Object(o)) => (o.get("item").and_then(item_index).unwrap_or(0), o.get("gear").or(o.get("port")).and_then(Value::as_u64).unwrap_or(0) as usize),
            Some(other) => (item_index(other).unwrap_or(0), 0),
            None => (0, 0),
        }
    };
    let g = |l: &Value, k: &str| -> usize {
        match k {
            "a" => end(l.get("a")).0,
            "b" => end(l.get("b")).0,
            "a_gear" => end(l.get("a")).1,
            _ => end(l.get("b")).1,
        }
    };
    let notes = r
        .get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|l| {
            let name = format!("{}·{} – {}·{}", g(l, "a") + 1, g(l, "a_gear") + 1, g(l, "b") + 1, g(l, "b_gear") + 1);
            match (l.get("error").and_then(Value::as_str), l.get("ratio").and_then(Value::as_f64)) {
                (Some(e), _) => {
                    let hint = l.get("hint").and_then(Value::as_str).map(|h| format!(": {h}")).unwrap_or_default();
                    Some((format!("{name}: {e}{hint}"), true))
                }
                (None, Some(ratio)) => Some((format!("{name}: {} : 1", crate::dialogs_gear::fmt_ratio(ratio)), false)),
                _ => None,
            }
        })
        .collect();
    // "item A gear 1 and item B gear 1 are tangent: link them?" (gears counted from 1 there).
    let parse = |s: &str| -> Option<Suggested> {
        let w: Vec<&str> = s.split_whitespace().collect();
        let at = |k: usize| -> Option<(usize, usize)> {
            let item = item_index(&json!(w.get(k + 1)?.trim_matches(|c: char| !c.is_alphanumeric())))?;
            let gear = if w.get(k + 2) == Some(&"gear") { w.get(k + 3)?.trim_matches(|c: char| !c.is_ascii_digit()).parse::<usize>().ok()?.saturating_sub(1) } else { 0 };
            Some((item, gear))
        };
        let first = w.iter().position(|x| *x == "item")?;
        let second = w.iter().skip(first + 1).position(|x| *x == "item")? + first + 1;
        let ((a, ag), (b, bg)) = (at(first)?, at(second)?);
        Some(Suggested { a, a_gear: ag, b, b_gear: bg, kind: if s.contains("shaft") || s.contains("coaxial") { "shaft".into() } else { "mesh".into() } })
    };
    let suggested = r
        .get("suggestions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| match v {
            Value::String(s) => parse(s),
            Value::Object(_) => Some(Suggested { a: g(v, "a"), a_gear: g(v, "a_gear"), b: g(v, "b"), b_gear: g(v, "b_gear"), kind: v.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string() }),
            _ => None,
        })
        .collect();
    (notes, suggested)
}

fn eval(app: &SolveApp, text: &str, kind: ValueKind, what: &str) -> Result<Option<f64>, String> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let v = app.session.doc.eval(t, kind).map_err(|e| format!("{what}: {e}: type a number, or leave it empty for Auto"))?;
    Ok(Some(if matches!(kind, ValueKind::Angle) { v.to_degrees() } else { v }))
}

/// What the dialog sends for its items (§6): each its type, components (`selection` locators),
/// and gears with their own settings; the links from the wires; the module blocks.
pub fn items_request(app: &SolveApp, items: &[GearItem], graph: &Graph) -> Result<Map<String, Value>, String> {
    let mut list = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let who = format!("Item {} · {}", i + 1, it.label());
        if let Some(next) = it.next_slot(app) {
            return Err(format!("{who} still needs a {next}: click one in the view"));
        }
        let components =
            it.picks.iter().map(|s| crate::dialogs_gear::locate(app, s).ok_or_else(|| format!("{who}: a pick that can't be part of a gear: remove it from the item"))).collect::<Result<Vec<_>, _>>()?;
        let module = if it.by_teeth { None } else { eval(app, &it.module, ValueKind::Length, &format!("{who}: module"))? };
        let mut gears = Vec::new();
        for g in 0..it.gear_count(app) {
            let s = it.gears.get(g).cloned().unwrap_or_else(|| it.gears.first().cloned().unwrap_or_default());
            let what = |f: &str| format!("{who} gear {}: {f}", g + 1);
            let mut e = json!({"gear": g});
            if it.by_teeth {
                if let Some(z) = eval(app, &s.teeth, ValueKind::Unitless, &what("teeth"))? {
                    e["teeth"] = json!(z.round().max(1.0) as u64);
                }
            } else if let Some(m) = module {
                e["module"] = json!(m);
            }
            let (a, b) = (eval(app, &s.face_a, ValueKind::Length, &what("face side A"))?, eval(app, &s.face_b, ValueKind::Length, &what("face side B"))?);
            e["face"] = match s.face_preset {
                0 => json!({"a": a, "b": a}),
                1 => json!({"a": a, "b": 0.0}),
                2 => json!({"a": 0.0, "b": b}),
                _ => json!({"a": a, "b": b}),
            };
            if let Some(h) = eval(app, &s.helix, ValueKind::Angle, &what("helix angle"))? {
                e["helix_angle"] = json!(h);
            }
            e["hand"] = match s.hand {
                1 => json!("left"),
                2 => json!("right"),
                _ => Value::Null,
            };
            if s.v {
                e["herringbone"] = json!(true);
            }
            e["profile_shift"] = match eval(app, &s.profile_shift, ValueKind::Unitless, &what("profile shift"))? {
                Some(x) => json!(x),
                None => json!("auto"),
            };
            e["bore"] = match s.bore {
                1 => Value::Null,
                2 => json!(eval(app, &s.bore_d, ValueKind::Length, &what("bore Ø"))?.ok_or_else(|| what("type the bore Ø, or choose Auto"))?),
                _ => json!("auto"),
            };
            if s.hub {
                e["hub"] = json!({
                    "diameter": eval(app, &s.hub_d, ValueKind::Length, &what("hub Ø"))?,
                    "length_a": eval(app, &s.hub_a, ValueKind::Length, &what("hub length A"))?,
                    "length_b": eval(app, &s.hub_b, ValueKind::Length, &what("hub length B"))?,
                });
            }
            gears.push(e);
        }
        // Each component named by its slot (pitch, pitch2, axis, bearing, …).
        let (circles, lines, points) = slot_names(it.type_name());
        let (mut nc, mut nl, mut np) = (0, 0, 0);
        let mut named = Vec::new();
        for (pick, mut c) in it.picks.iter().zip(components) {
            let slot = match slot_of(app, pick) {
                Some(Slot::Circle) => {
                    nc += 1;
                    nth_slot(circles, nc - 1)
                }
                Some(Slot::Line) => {
                    nl += 1;
                    nth_slot(lines, nl - 1)
                }
                Some(Slot::Point) => {
                    np += 1;
                    nth_slot(points, np - 1)
                }
                None => None,
            };
            if let Some(slot) = slot {
                c["slot"] = json!(slot);
            }
            named.push(c);
        }
        let mut item = json!({"id": item_id(i), "type": it.type_name(), "components": named, "gears": gears});
        match it.type_name() {
            "internal" => item["rim"] = json!(eval(app, &it.rim, ValueKind::Length, &format!("{who}: rim"))?),
            "worm" => {
                item["starts"] = json!(eval(app, &it.starts, ValueKind::Unitless, &format!("{who}: starts"))?.map(|v| v.round().max(1.0) as u64));
                if it.throated {
                    item["throated"] = json!(true);
                }
            }
            "bevel" | "hypoid" => {
                item["bevel_style"] = json!(["straight", "spiral", "chevron"][it.bevel_style.min(2)]);
                item["spiral_angle"] = json!(eval(app, &it.spiral, ValueKind::Angle, &format!("{who}: spiral angle"))?);
            }
            "rack" => item["back_height"] = json!(eval(app, &it.back_height, ValueKind::Length, &format!("{who}: back height"))?),
            "shaft" => {
                item["diameter"] = json!(eval(app, &it.shaft_d, ValueKind::Length, &format!("{who}: shaft Ø"))?);
                item["keyway"] = json!(it.keyway);
            }
            _ => {}
        }
        list.push(item);
    }
    let links: Vec<Value> = graph
        .links()
        .into_iter()
        .map(|(a, ag, b, bg, kind)| json!({"a": {"item": item_id(a), "port": ag}, "b": {"item": item_id(b), "port": bg}, "kind": kind}))
        .collect();
    let mut blocks = Vec::new();
    for (n, (module, members)) in graph.blocks().into_iter().enumerate() {
        let id = format!("M{}", n + 1);
        let mut b = json!({"id": id});
        if let Some(m) = eval(app, &module, ValueKind::Length, "module block")? {
            b["module"] = json!(m);
        }
        for i in members {
            if let Some(item) = list.get_mut(i) {
                item["module_block"] = json!(id);
            }
        }
        blocks.push(b);
    }
    let mut p = Map::new();
    p.insert("items".into(), Value::Array(list));
    p.insert("links".into(), Value::Array(links));
    p.insert("module_blocks".into(), Value::Array(blocks));
    Ok(p)
}

/// One item's settings (§3), by gear, with the hover guides (§5).
pub fn item_settings(app: &SolveApp, ui: &mut egui::Ui, it: &mut GearItem, id: usize) -> bool {
    let mut enter = false;
    if !it.is_gear() {
        enter |= hint_row(ui, "Shaft Ø", &mut it.shaft_d, "auto");
        ui.checkbox(&mut it.keyway, solidcraft_ui_text::tr_label("Keyway"));
        return enter;
    }
    let mut size = usize::from(it.by_teeth);
    choice_row(ui, "Size By", &format!("gear_item_size_{id}"), &mut size, &["Module", "Teeth"])
        .on_hover_text("Set the module (tooth size) or the tooth count; the other follows from the pitch circle.");
    it.by_teeth = size == 1;
    if !it.by_teeth {
        enter |= hinted(ui, "Module", &mut it.module, "auto", "Tooth size in mm (pitch Ø ÷ teeth). Auto picks a standard module so the smallest gear has ≥17 teeth.");
    }
    match it.type_name() {
        "internal" => enter |= hint_row(ui, "Rim", &mut it.rim, "auto: 2.5 × module"),
        "worm" => {
            enter |= hinted(ui, "Starts", &mut it.starts, "1", "Thread starts on the worm. More starts = faster, less self-locking.");
            ui.checkbox(&mut it.throated, solidcraft_ui_text::tr_label("Throated Wheel"));
        }
        "bevel" | "hypoid" => {
            choice_row(ui, "Style", &format!("gear_item_bevel_{id}"), &mut it.bevel_style, &["Straight", "Spiral", "Chevron"])
                .on_hover_text("Chevron bevel: the spiral changes hand at mid-face.");
            if it.bevel_style > 0 || it.type_name() == "hypoid" {
                enter |= hinted(ui, "Spiral Angle", &mut it.spiral, "35 deg", "Spiral bevel/hypoid tooth angle at mid-face. 35° is typical.");
            }
        }
        "rack" => enter |= hint_row(ui, "Back Height", &mut it.back_height, "auto"),
        _ => {}
    }
    let n = it.gear_count(app);
    let by_teeth = it.by_teeth;
    let kind = it.type_name();
    for (g, s) in it.settings_mut(n).iter_mut().take(n).enumerate() {
        ui_kit::section(ui, ("gear_item_gear", id, g), &format!("Gear {}", g + 1), n == 1, |ui| {
            if by_teeth {
                enter |= hinted(ui, "Teeth", &mut s.teeth, "from the circle", "Number of teeth. The pitch circle stays as drawn, so the module follows.");
            }
            choice_row(ui, "Face", &format!("gear_face_{id}_{g}"), &mut s.face_preset, &["Symmetric", "Side A Only", "Side B Only", "Custom"])
                .on_hover_text("Symmetric: equal both sides. Side A/B only: the gear grows out of one face of the sketch.");
            let face_tip = "How far the teeth reach each side of the sketch plane, along the axis. Auto ≈ 10 × module, limited by the room on the shaft.";
            if s.face_preset != 2 {
                enter |= hinted(ui, "Face Side A", &mut s.face_a, "auto", face_tip);
            }
            if s.face_preset >= 2 {
                enter |= hinted(ui, "Face Side B", &mut s.face_b, "auto", face_tip);
            }
            if !matches!(kind, "bevel" | "hypoid" | "worm") {
                enter |= hinted(ui, "Helix Angle", &mut s.helix, "0 = straight", "Tooth angle to the axis. 0 = straight. Meshing gears get equal angles of opposite hand.");
                choice_row(ui, "Hand", &format!("gear_hand_{id}_{g}"), &mut s.hand, &["Auto", "Left", "Right"])
                    .on_hover_text("Left or right helix. The mating gear is set to the other hand for you.");
                ui.checkbox(&mut s.v, "V").on_hover_text("Herringbone: two opposite helices meeting in the middle — no axial thrust.");
            }
            enter |= hinted(ui, "Profile Shift", &mut s.profile_shift, "auto", "Moves the tooth out to avoid undercut on small gears. Auto does it below 17 teeth.");
            choice_row(ui, "Bore", &format!("gear_bore_{id}_{g}"), &mut s.bore, &["Auto", "None", "Ø"]).on_hover_text("Hole through the gear. Auto fits its shaft.");
            if s.bore == 2 {
                enter |= hinted(ui, "Bore Ø", &mut s.bore_d, "mm", "Hole through the gear. Auto fits its shaft.");
            }
            ui.checkbox(&mut s.hub, solidcraft_ui_text::tr_label("Hub"));
            if s.hub {
                enter |= hint_row(ui, "Hub Ø", &mut s.hub_d, "auto");
                enter |= hint_row(ui, "Hub Length A", &mut s.hub_a, "0");
                enter |= hint_row(ui, "Hub Length B", &mut s.hub_b, "0");
            }
        });
    }
    enter
}

struct Viewer<'a> {
    items: &'a [GearItem],
    gears: &'a [usize],
    hovered: Option<usize>,
    driver: Option<(usize, usize)>,
    make_driver: Option<(usize, usize)>,
}

impl Viewer<'_> {
    fn gears_of(&self, node: &GNode) -> usize {
        match node {
            GNode::Item(i) => self.gears.get(*i).copied().unwrap_or(1).max(1),
            GNode::Block { .. } => 0,
        }
    }
}

impl SnarlViewer<GNode> for Viewer<'_> {
    fn title(&mut self, node: &GNode) -> String {
        match node {
            GNode::Item(i) => {
                let badge = if self.driver.is_some_and(|(d, _)| d == *i) { "▶ " } else { "" };
                format!("{badge}{} · {}", i + 1, self.items.get(*i).map_or("", GearItem::label))
            }
            GNode::Block { .. } => "Module Block".into(),
        }
    }

    fn inputs(&mut self, node: &GNode) -> usize {
        match node {
            GNode::Item(_) => 2 * self.gears_of(node),
            GNode::Block { .. } => 1,
        }
    }

    fn outputs(&mut self, node: &GNode) -> usize {
        match node {
            GNode::Item(_) => 2 * self.gears_of(node) + 1,
            GNode::Block { .. } => 0,
        }
    }

    fn show_input(&mut self, pin: &InPin, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) -> impl SnarlPin + 'static {
        match &mut snarl[pin.id.node] {
            GNode::Item(_) => {
                let g = pin.id.input / 2 + 1;
                if pin.id.input % 2 == 0 {
                    ui.label(format!("{g} ● Mesh")).on_hover_text("Drag to another gear's Mesh pin: they mesh. The distance between them is checked.");
                } else {
                    ui.label(format!("{g} ■ Shaft")).on_hover_text("Drag to another gear's Shaft pin: they share a shaft and turn together.");
                }
            }
            GNode::Block { module } => {
                ui.add(egui::TextEdit::singleline(module).desired_width(70.0).hint_text("module: auto"))
                    .on_hover_text("Gears in this block share one module, so they can mesh with each other.");
            }
        }
        pin_info(pin.id.input % 2 == 1, false)
    }

    fn show_output(&mut self, pin: &OutPin, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) -> impl SnarlPin + 'static {
        let module = pin.id.output == 2 * self.gears_of(&snarl[pin.id.node]);
        if module {
            ui.label("Module ▲").on_hover_text("Gears in this block share one module, so they can mesh with each other.");
        } else if pin.id.output % 2 == 0 {
            ui.label("Mesh ●").on_hover_text("Drag to another gear's Mesh pin: they mesh. The distance between them is checked.");
        } else {
            ui.label("Shaft ■").on_hover_text("Drag to another gear's Shaft pin: they share a shaft and turn together.");
        }
        pin_info(pin.id.output % 2 == 1, module)
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<GNode>) {
        // Mesh to mesh, shaft to shaft, Module into a block: nothing else joins.
        let module = from.id.output == 2 * self.gears_of(&snarl[from.id.node]);
        let block = matches!(snarl.get_node(to.id.node), Some(GNode::Block { .. }));
        let fits = if block { module } else { !module && from.id.output % 2 == to.id.input % 2 };
        if fits && from.id.node != to.id.node {
            snarl.connect(from.id, to.id);
        }
    }

    fn final_node_rect(&mut self, node: NodeId, rect: egui::Rect, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) {
        if ui.rect_contains_pointer(rect)
            && let Some(GNode::Item(i)) = snarl.get_node(node)
        {
            self.hovered = Some(*i);
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<GNode>) -> bool {
        true
    }

    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) {
        if ui.button(solidcraft_ui_text::tr_label("Add Module Block")).on_hover_text("Gears in this block share one module, so they can mesh with each other.").clicked() {
            snarl.insert_node(pos, GNode::Block { module: String::new() });
            ui.close();
        }
    }

    fn has_node_menu(&mut self, node: &GNode) -> bool {
        matches!(node, GNode::Item(_))
    }

    fn show_node_menu(&mut self, node: NodeId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) {
        let Some(GNode::Item(i)) = snarl.get_node(node).cloned() else { return };
        let n = self.gears_of(&GNode::Item(i));
        if ui.button(solidcraft_ui_text::tr_label("Make Driver")).on_hover_text("The gear you turn. Every other gear's speed and direction follow from it.").clicked() {
            self.make_driver = Some((i, 0));
            ui.close();
        }
        if ui.button(solidcraft_ui_text::tr_label("Remove Links")).clicked() {
            for p in 0..=2 * n {
                snarl.drop_outputs(OutPinId { node, output: p });
            }
            for p in 0..2 * n {
                snarl.drop_inputs(InPinId { node, input: p });
            }
            ui.close();
        }
    }
}

fn pin_info(shaft: bool, module: bool) -> PinInfo {
    let mut p = if module {
        PinInfo::triangle()
    } else if shaft {
        PinInfo::square()
    } else {
        PinInfo::circle()
    };
    p.fill = Some(if module {
        egui::Color32::from_rgb(230, 140, 50)
    } else if shaft {
        egui::Color32::from_rgb(160, 160, 160)
    } else {
        egui::Color32::from_rgb(70, 140, 220)
    });
    p
}

/// What drawing the graph found: the item a hovered node stands for, and a driver chosen there.
#[derive(Default)]
pub struct GraphOut {
    pub hovered: Option<usize>,
    pub driver: Option<(usize, usize)>,
}

/// The graph, `gears` the gears of each item (docked in the dialog, or in its window).
pub fn graph_ui(ui: &mut egui::Ui, items: &[GearItem], gears: &[usize], graph: &mut Graph, driver: Option<(usize, usize)>, size: egui::Vec2, salt: &str) -> GraphOut {
    for i in 0..items.len() {
        graph.node_of(i);
    }
    if items.is_empty() {
        ui.label(egui::RichText::new("Items appear here. Drag from a ● Mesh pin to another to make them mesh.").small().weak());
        return GraphOut::default();
    }
    let mut viewer = Viewer { items, gears, hovered: None, driver, make_driver: None };
    SnarlWidget::new().id_salt(salt).style(SnarlStyle::new()).min_size(size).max_size(size).show(&mut graph.0, &mut viewer, ui);
    GraphOut { hovered: viewer.hovered, driver: viewer.make_driver }
}

/// The popped-out graph's window (⤢ Pop out), while the dialog is open.
pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(d) = app.dialog.as_mut() else { return };
    let crate::dialogs::Kind::Gear(g) = &mut d.kind else { return };
    if !g.graph_popped {
        return;
    }
    let mut open = true;
    let mut out = GraphOut::default();
    let (items, gears, driver) = (g.items.clone(), g.gear_counts.clone(), g.drive_item);
    egui::Window::new(solidcraft_ui_text::tr_label("Gear Graph")).id(egui::Id::unique("sc_gear_graph")).open(&mut open).default_size(egui::vec2(640.0, 420.0)).resizable(true).show(ctx, |ui| {
        let size = ui.available_size().max(egui::vec2(320.0, 240.0));
        out = graph_ui(ui, &items, &gears, &mut g.graph, driver, size, "sc_gear_graph_window");
    });
    if out.driver.is_some() {
        g.drive_item = out.driver;
    }
    if out.hovered.is_some() {
        g.graph_hovered = out.hovered;
    }
    g.graph_popped = open;
}
