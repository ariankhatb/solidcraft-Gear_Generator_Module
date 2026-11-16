//! The Gears dialog's gear items and their settings (GEAR_UI_SPEC.md §1, §3, §6).
//!
//! A gear item is one thing the generator makes, defined by the geometry picked into its slots:
//! a spur gear by its pitch circle (and optionally its shaft line), a compound gear by its shaft
//! line and two or more circles on it (in parallel sketches, decision 6), a planetary set by its
//! sun, ring and planet circles, a bevel pair by two circles or by two axis lines and one circle,
//! a hypoid or worm pair by its circle and the other gear's axis line, a shaft by its line and the
//! bearing points on it. Pick a type, press Create New, then click the geometry the status line
//! asks for; a pick that doesn't fit is refused with the reason.
//!
//! The slots, the checks on a pick and the gear count are the gear crate's
//! ([`solidcraft_gear::items`]: `slots`, `assign`, `validate_slot`, `next_slot`, `gear_count`):
//! the dialog reads each pick as a world [`Thing`] (`dialogs_gear::thing_of`) and asks. Every
//! gear of an item has its own settings (§3), typed here as text (expressions with units, empty
//! for Auto) and read into the crate's [`items::GearSettings`]. The items are sent as
//! `gear.create`'s `items`, with the graph's `links` and `module_blocks` (§6).

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::items::{self, Bore, BevelStyle, FacePreset, FaceSides, Hand, Hub, ItemKind, SizeBy, Thing};

use crate::SolveApp;
use crate::dialogs_gear_graph::Graph;
use crate::ui_kit::{self, choice_row, hint_row};

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

/// A row with its hover guide (§5): the whole row shows it.
fn hinted(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str, tip: &str) -> bool {
    let mut enter = false;
    ui.scope(|ui| enter = hint_row(ui, label, value, hint)).response.on_hover_text(tip);
    enter
}

/// The face presets (decision 4), as the toggle shows them and `face.preset` names them.
pub const FACE_PRESETS: [(FacePreset, &str, &str); 4] = [
    (FacePreset::Symmetric, "symmetric", "Symmetric"),
    (FacePreset::AOnly, "a_only", "Side A Only"),
    (FacePreset::BOnly, "b_only", "Side B Only"),
    (FacePreset::Custom, "custom", "Custom"),
];

/// A gear's own settings (§3), as typed (empty: Auto); [`GearSettings::read`] makes the gear
/// crate's [`items::GearSettings`] of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearSettings {
    /// Sized by its teeth (the module follows from the circle), else by its module.
    pub by_teeth: bool,
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

fn eval(app: &SolveApp, text: &str, kind: ValueKind, what: &str) -> Result<Option<f64>, String> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let v = app.session.doc.eval(t, kind).map_err(|e| format!("{what}: {e}: type a number, or leave it empty for Auto"))?;
    Ok(Some(if matches!(kind, ValueKind::Angle) { v.to_degrees() } else { v }))
}

/// A value as a field shows it again (Edit): "2 mm", "20 deg", "0.3".
fn shown(v: Option<f64>, kind: ValueKind) -> String {
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
            Some(Hub { diameter: len(&self.hub_d, "hub Ø")?, length_a: len(&self.hub_a, "hub length A")?.unwrap_or(0.0), length_b: len(&self.hub_b, "hub length B")?.unwrap_or(0.0) })
        } else {
            None
        };
        Ok(items::GearSettings {
            size_by: if self.by_teeth { SizeBy::Teeth } else { SizeBy::Module },
            module: if self.by_teeth { None } else { len(&self.module, "module")? },
            teeth: if self.by_teeth { eval(app, &self.teeth, ValueKind::Unitless, &what("teeth"))?.map(|z| z.round().max(1.0) as u32) } else { None },
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

/// One gear's entry in an item's `gears` (§6), named as the gear crate's
/// [`items::GearSettings`] fields: `size_by` ("module" | "teeth"), `module` or `teeth` (the one it
/// is sized by; absent: Auto), `face` {`preset`, `a`, `b`} (mm, null: Auto), `helix_angle`
/// (degrees), `hand` ("left" | "right" | null: opposite its mate), `herringbone` (V), and
/// `profile_shift` (a value | "auto"), `bore` ("auto" | null: none | Ø) and `hub` ({diameter,
/// length_a, length_b} | absent).
pub fn gear_json(gear: usize, s: &items::GearSettings) -> Value {
    let mut e = json!({"gear": gear, "size_by": match s.size_by { SizeBy::Module => "module", SizeBy::Teeth => "teeth" }});
    if let Some(m) = s.module.filter(|_| s.size_by == SizeBy::Module) {
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
    }
    e
}

/// One gear item: its type, the geometry in its slots (in pick order), each gear's settings,
/// and the type's own.
#[derive(Clone, Debug, PartialEq)]
pub struct GearItem {
    pub kind: ItemKind,
    pub picks: Vec<Sel>,
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

    fn is_gear(&self) -> bool {
        self.kind != ItemKind::Shaft
    }

    /// The picks as the gear crate takes them (one that can't be read is left out).
    pub fn things(&self, app: &SolveApp) -> Vec<Thing> {
        self.picks.iter().filter_map(|s| crate::dialogs_gear::thing_of(app, s)).collect()
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

    /// The next required slot ("shaft line"), `None` when every one is filled.
    pub fn next_slot(&self, app: &SolveApp) -> Option<String> {
        items::next_slot(self.kind, &self.things(app)).map(str::to_string)
    }

    pub fn complete(&self, app: &SolveApp) -> bool {
        let things = self.things(app);
        things.len() == self.picks.len() && items::assign(self.kind, &things).is_ok() && items::next_slot(self.kind, &things).is_none()
    }

    /// The gears it makes ([`items::gear_count`]).
    pub fn gear_count(&self, app: &SolveApp) -> usize {
        items::gear_count(self.kind, &self.things(app))
    }

    /// Why `sel` can't be picked into this item (None: it fills a slot): used by another item,
    /// not geometry an item takes, or the gear crate's check on the slot it would fill
    /// ([`items::validate_slot`]: what is wrong and what to do).
    pub fn refusal(&self, app: &SolveApp, sel: &Sel, others: &[(usize, &GearItem)]) -> Option<String> {
        if let Some((i, _)) = others.iter().find(|(_, it)| it.picks.contains(sel)) {
            return Some(format!("already used by Item {}", i + 1));
        }
        let Some(thing) = crate::dialogs_gear::thing_of(app, sel) else {
            return Some("pick a sketch circle, line or point, a model edge, a round face or an axis".into());
        };
        items::validate_slot(self.kind, &self.things(app), &thing).err()
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

    /// Gear `g`'s settings as typed: its own, else the first gear's (with no teeth).
    fn typed(&self, g: usize) -> GearSettings {
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
/// `carrier`, `diameter`, `keyway`) and its `module_block`; the `links` from the wires
/// ({a: {item, port}, b: {item, port}, kind}); the `module_blocks` ({id, module?}). The picks
/// themselves go in `ui.gear_items` so Edit gets them back exactly.
pub fn items_request(app: &SolveApp, items: &[GearItem], graph: &Graph) -> Result<Map<String, Value>, String> {
    let mut list = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let who = format!("Item {} · {}", i + 1, it.label());
        let things: Vec<Thing> = it
            .picks
            .iter()
            .map(|s| crate::dialogs_gear::thing_of(app, s).ok_or_else(|| format!("{who}: a pick that can't be part of a gear: remove it from the item")))
            .collect::<Result<_, _>>()?;
        if let Some(next) = items::next_slot(it.kind, &things) {
            return Err(format!("{who} still needs a {next}: click one in the view"));
        }
        let slots = items::assign(it.kind, &things).map_err(|e| format!("{who}: {e}"))?;
        let mut components = Vec::new();
        for (pick, slot) in it.picks.iter().zip(&slots) {
            let mut c = crate::dialogs_gear::locate(app, pick).ok_or_else(|| format!("{who}: a pick that can't be part of a gear: remove it from the item"))?;
            c["slot"] = json!(slot.name);
            components.push(c);
        }
        let mut gears = Vec::new();
        for g in 0..items::gear_count(it.kind, &things) {
            let s = it.typed(g).read(app, &|f: &str| format!("{who} gear {}: {f}", g + 1))?;
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
    let picks: Vec<Value> = items.iter().map(|it| serde_json::to_value(&it.picks).unwrap_or(Value::Null)).collect();
    let mut p = Map::new();
    p.insert("items".into(), Value::Array(list));
    p.insert("links".into(), Value::Array(links));
    p.insert("module_blocks".into(), Value::Array(blocks));
    p.insert("ui".into(), json!({"gear_items": picks}));
    Ok(p)
}

/// The items, graph and drive a set was made with (its request, as [`items_request`] and the
/// dialog's `drive` sent them): Edit reopens the dialog on them. The picks come from
/// `ui` (its `gear_items`); a request without them names each component by its locator.
pub fn items_of(r: &Map<String, Value>, ui: Option<&Value>, graph: &mut Graph) -> Vec<GearItem> {
    let picks_ui = ui.and_then(|u| u.get("gear_items")).and_then(Value::as_array);
    let mut out = Vec::new();
    for (i, v) in r.get("items").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let kind = v.get("type").and_then(Value::as_str).and_then(ItemKind::parse).unwrap_or(ItemKind::Spur);
        let mut it = GearItem::new(kind);
        it.picks = match picks_ui.and_then(|p| p.get(i)).and_then(|p| serde_json::from_value::<Vec<Sel>>(p.clone()).ok()) {
            Some(p) => p,
            None => v.get("components").and_then(Value::as_array).into_iter().flatten().filter_map(sel_of).collect(),
        };
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
        graph.node_of(i);
        out.push(it);
    }
    for l in r.get("links").and_then(Value::as_array).into_iter().flatten() {
        let end = |k: &str| -> Option<(usize, usize)> {
            let e = l.get(k)?;
            Some((item_index(e.get("item")?)?, e.get("port").or(e.get("gear")).and_then(Value::as_u64).unwrap_or(0) as usize))
        };
        if let (Some((a, a_gear)), Some((b, b_gear))) = (end("a"), end("b")) {
            let kind = l.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string();
            graph.accept(&crate::dialogs_gear_graph::Suggested { a, a_gear, b, b_gear, kind });
        }
    }
    let ids: Vec<Option<&str>> = r.get("items").and_then(Value::as_array).into_iter().flatten().map(|v| v.get("module_block").and_then(Value::as_str)).collect();
    for b in r.get("module_blocks").and_then(Value::as_array).into_iter().flatten() {
        let module = shown(b.get("module").and_then(Value::as_f64), ValueKind::Length);
        let k = graph.add_block(&module);
        let id = b.get("id").and_then(Value::as_str);
        for (i, _) in ids.iter().enumerate().filter(|(_, m)| id.is_some() && **m == id) {
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

/// One item's settings (§3), by gear, with the hover guides (§5).
pub fn item_settings(app: &SolveApp, ui: &mut egui::Ui, it: &mut GearItem, id: usize) -> bool {
    let mut enter = false;
    if !it.is_gear() {
        enter |= hint_row(ui, "Shaft Ø", &mut it.shaft_d, "auto");
        ui.checkbox(&mut it.keyway, solidcraft_ui_text::tr_label("Keyway"));
        return enter;
    }
    match it.kind {
        ItemKind::Internal => enter |= hint_row(ui, "Rim", &mut it.rim, "auto: 2.5 × module"),
        ItemKind::Worm => {
            enter |= hinted(ui, "Starts", &mut it.starts, "1", "Thread starts on the worm. More starts = faster, less self-locking.");
            ui.checkbox(&mut it.throated, solidcraft_ui_text::tr_label("Throated Wheel"));
        }
        ItemKind::Bevel | ItemKind::Hypoid => {
            choice_row(ui, "Style", &format!("gear_item_bevel_{id}"), &mut it.bevel_style, &["Straight", "Spiral", "Chevron"])
                .on_hover_text("Chevron bevel: the spiral changes hand at mid-face.");
            if it.bevel_style > 0 || it.kind == ItemKind::Hypoid {
                enter |= hinted(ui, "Spiral Angle", &mut it.spiral, "35 deg", "Spiral bevel/hypoid tooth angle at mid-face. 35° is typical.");
            }
        }
        ItemKind::Rack => enter |= hint_row(ui, "Back Height", &mut it.back_height, "auto"),
        ItemKind::Planetary => {
            enter |= hint_row(ui, "Planets", &mut it.planets, "auto: as many as fit, 3 at most");
            ui.checkbox(&mut it.carrier, solidcraft_ui_text::tr_label("Carrier"));
        }
        _ => {}
    }
    let n = it.gear_count(app);
    let kind = it.kind;
    let names: &[&str] = match kind {
        ItemKind::Planetary => &["Sun", "Ring", "Planet"],
        ItemKind::Worm => &["Wheel", "Worm"],
        ItemKind::Hypoid => &["Ring Gear", "Pinion"],
        ItemKind::Internal => &["Ring", "Pinion"],
        ItemKind::Rack => &["Rack", "Pinion"],
        _ => &[],
    };
    for (g, s) in it.settings_mut(n).iter_mut().take(n).enumerate() {
        let title = names.get(g).map_or_else(|| format!("Gear {}", g + 1), |t| solidcraft_ui_text::tr_label(t).to_string());
        ui_kit::section(ui, ("gear_item_gear", id, g), &title, n == 1, |ui| {
            let mut size = usize::from(s.by_teeth);
            choice_row(ui, "Size By", &format!("gear_item_size_{id}_{g}"), &mut size, &["Module", "Teeth"])
                .on_hover_text("Set the module (tooth size) or the tooth count; the other follows from the pitch circle.");
            s.by_teeth = size == 1;
            if s.by_teeth {
                enter |= hinted(ui, "Teeth", &mut s.teeth, "from the circle", "Number of teeth. The pitch circle stays as drawn, so the module follows.");
            } else {
                enter |= hinted(ui, "Module", &mut s.module, "auto", "Tooth size in mm (pitch Ø ÷ teeth). Auto picks a standard module so the smallest gear has ≥17 teeth.");
            }
            let labels: Vec<&str> = FACE_PRESETS.iter().map(|p| p.2).collect();
            choice_row(ui, "Face", &format!("gear_face_{id}_{g}"), &mut s.face_preset, &labels)
                .on_hover_text("Symmetric: equal both sides. Side A/B only: the gear grows out of one face of the sketch.");
            let face_tip = "How far the teeth reach each side of the sketch plane, along the axis. Auto ≈ 10 × module, limited by the room on the shaft.";
            if s.face_preset != 2 {
                enter |= hinted(ui, "Face Side A", &mut s.face_a, "auto", face_tip);
            }
            if s.face_preset >= 2 {
                enter |= hinted(ui, "Face Side B", &mut s.face_b, "auto", face_tip);
            }
            if !matches!(kind, ItemKind::Bevel | ItemKind::Hypoid | ItemKind::Worm) {
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
