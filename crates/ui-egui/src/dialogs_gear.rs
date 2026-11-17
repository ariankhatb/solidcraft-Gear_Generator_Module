//! Gears: the Gear Generator as a native command dialog (MODEL › CREATE › Gears).
//!
//! The drivetrain is drawn first, in ordinary sketches, with the app's own sketch tools: circles
//! are pitch circles, lines are shafts, points are bearings or a bevel's apex, and construction
//! curves are helpers. The dialog then makes one gear at a time (GEAR_UI_SPEC.md §1):
//!
//! 1. pick a type in the list (each with its icon) and press **New gear**, or just click the
//!    geometry: a pick with no gear waiting for it starts a gear of the chosen type;
//! 2. the status line says what to click next ("Gear B · Compound — click: shaft line"), and the
//!    gear's card ticks off its slots as the picks fill them (the slots, the checks on a pick and
//!    the gear count are the gear crate's: `solidcraft_gear::items`);
//! 3. a complete gear's card folds to one line with its teeth and module; clicking a card makes it
//!    the one picks go to and shows its own settings below the cards;
//! 4. the Drivetrain section lists the links (meshes and shared shafts), the links the drawing
//!    suggests (dashed, accepted with one button), the driver and the ratio between any two gears;
//!    the node graph is an optional view of the same links.
//!
//! OK makes the set with `gear.create {items, links, module_blocks, drive}` (one Gears feature);
//! editing the set (its timeline row or folder, Edit Gears, a gear body or component selected
//! with the Gears command, a double-click on a gear) reopens this dialog on it with its gears
//! restored, and OK updates it in place with `gear.update`. A set made before gear items (one
//! type read from a whole sketch, or rows with roles) opens as it is, with a button that turns
//! its circles into gears where that can be done.
//!
//! The preview is the gears' outline over the viewport (never their teeth: those are solved on
//! OK only), each gear's picked geometry in its own colour, each mesh's contact and the active
//! gear's side-A arrow (decision 4).

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::items::{self as gi, ItemKind};

use crate::SolveApp;
use crate::dialogs::{Kind, OPS};
use crate::dialogs_gear_graph::{GearItem, Graph};
use crate::selection::{AXES, BODIES, CURVES, EDGES, FACES, SKETCH_POINTS, SelInput};

/// Each gear item's colour: its dot on the card, its picked geometry and its outline in the view
/// (an item's index picks one, round the list).
pub(crate) const PALETTE: [egui::Color32; 6] = [
    egui::Color32::from_rgb(70, 140, 220),
    egui::Color32::from_rgb(230, 140, 50),
    egui::Color32::from_rgb(80, 170, 100),
    egui::Color32::from_rgb(190, 90, 170),
    egui::Color32::from_rgb(200, 180, 60),
    egui::Color32::from_rgb(120, 170, 190),
];

/// Item `i`'s colour.
pub(crate) fn item_color(i: usize) -> egui::Color32 {
    PALETTE[i % PALETTE.len()]
}

/// The types in the list, in its order, with their icons.
pub(crate) const TYPES: [(ItemKind, &str); 12] = [
    (ItemKind::Spur, "gear_spur"),
    (ItemKind::Helical, "gear_helical"),
    (ItemKind::Herringbone, "gear_herringbone"),
    (ItemKind::Internal, "gear_internal"),
    (ItemKind::Rack, "gear_rack"),
    (ItemKind::Compound, "gear_pair"),
    (ItemKind::Planetary, "gear_planetary"),
    (ItemKind::Bevel, "gear_bevel"),
    (ItemKind::Hypoid, "gear_hypoid"),
    (ItemKind::Worm, "gear_worm"),
    (ItemKind::CrossedHelical, "gear_crossed_helical"),
    (ItemKind::Shaft, "axis_through_cylinder"),
];

/// A type's icon.
pub(crate) fn icon_of(kind: ItemKind) -> &'static str {
    TYPES.iter().find(|t| t.0 == kind).map_or("gear_generator", |t| t.1)
}

/// A set made before gear items (one type read from a sketch, or picked rows with roles): it
/// opens as it is; OK keeps it, with the output settings changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Legacy {
    /// What it is, in words ("Spur set read from sketch 'Pitch circles'").
    pub what: String,
    /// Whether its circles can be turned into gear items.
    pub convertible: bool,
}

/// A message shown on a gear's card (or above the cards): what is wrong and what to do.
#[derive(Clone, Debug, PartialEq)]
pub struct Msg {
    pub text: String,
    /// An error (nothing can be built) rather than a warning.
    pub error: bool,
}

/// What the dialog asks the app to do once its frame is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Open a sketch for editing (a gear's sketch that isn't fully defined); the dialog comes back
    /// when its editing ends.
    EditSketch(u64),
}

/// A sketch opened from the dialog: when its editing ends, the dialog comes back on it.
#[derive(Clone, Debug, PartialEq)]
pub struct AfterSketch {
    pub sketch: u64,
    /// The sketch has been seen open (its editing has started).
    pub opened: bool,
    pub dialog: Gear,
}

/// The Advanced section's rows: the defaults every gear takes (its `gear.create` key, label,
/// kind, hint, hover text).
pub(crate) const ADVANCED: [(&str, &str, ValueKind, &str, &str); 4] = [
    ("pressure_angle", "Pressure Angle", ValueKind::Angle, "20 deg", "Tooth flank angle. 20° is standard."),
    ("backlash", "Backlash", ValueKind::Length, "0", "Play between meshing teeth, taken from each tooth's thickness."),
    ("tip_round", "Tip Rounding", ValueKind::Unitless, "× module", "Round the tooth tips, as a fraction of the module."),
    ("xy_compensation", "XY Compensation", ValueKind::Length, "0", "Shrink (or grow) every tooth flank for a printer's over-extrusion."),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Gear {
    /// The sketch being edited when the dialog opened (it is finished and kept for Edit Sketch).
    pub sketch: Option<u64>,
    /// Editing this set (its timeline folder): OK updates it.
    pub group: Option<u64>,
    /// The set's request when editing: what the dialog is laid over.
    pub base: Map<String, Value>,
    /// An old set, opened as it is.
    pub legacy: Option<Legacy>,
    /// The gear items, the one picks go to, the type a new one is made as (an index into
    /// [`TYPES`]), and which of the active item's gears its settings show.
    pub items: Vec<GearItem>,
    pub active: Option<usize>,
    pub new_kind: usize,
    pub gear_tab: usize,
    /// The links between the items (and its module blocks), drawn as rows or as a graph.
    pub graph: Graph,
    pub show_graph: bool,
    /// The item whose card or graph node is under the pointer (its geometry lights up).
    pub graph_hovered: Option<usize>,
    pub card_hovered: Option<usize>,
    /// Each item's gear count (worked out once a frame).
    pub gear_counts: Vec<usize>,
    /// The Add Link row: its two gears (indices into the gear list) and its kind (0 mesh, 1 shaft).
    pub link_add: (usize, usize, usize),
    /// The driving item·gear, its input rpm and torque.
    pub drive_item: Option<(usize, usize)>,
    pub rpm: String,
    pub torque: String,
    /// The two gears picked for the ratio calculator, and its last request and readout.
    pub ratio_pick: [Option<(usize, usize)>; 2],
    pub ratio: Option<(String, Result<Vec<String>, String>)>,
    pub assembly: bool,
    /// Teeth from the equations (fitted curves otherwise).
    pub equations: bool,
    /// The Advanced rows, by key, as typed (empty: the default).
    pub values: BTreeMap<String, String>,
    /// What the set does to the Targets bodies (an index into the dialogs' `OPS`), and whether its
    /// keep-out is cut from them (a housing) with what gap.
    pub operation: usize,
    pub cut_keep_out: bool,
    pub keep_out_gap: String,
    /// Why the last pick was refused, and the item it was for.
    pub refused: Option<(Option<usize>, String)>,
    /// The last request worked out and its summary or the reason it can't be built.
    pub calc: Option<(String, Result<Vec<String>, String>)>,
    /// What the engine and the dialog say of each item (by index), and of the set as a whole.
    pub item_msgs: Vec<Vec<Msg>>,
    pub set_msgs: Vec<Msg>,
    pub notes: Vec<String>,
    /// The results table's rows (one per gear).
    pub results: Vec<[String; 9]>,
    /// The outline drawn over the viewport, by item: polylines in the world.
    pub shape: Vec<(usize, Vec<[f64; 3]>)>,
    /// Each item's picked geometry in the world (a point is a polyline of one point).
    pub picked: Vec<(usize, Vec<[f64; 3]>, String)>,
    /// Each mesh's contact, in the world: a line or (two equal ends) a point.
    pub contacts: Vec<[[f64; 3]; 2]>,
    /// Each gear's side-A arrow (decision 4), in the world, by item.
    pub arrows: Vec<(usize, [[f64; 3]; 2])>,
    /// Whether the last reply gave the outlines and the suggestions itself.
    pub from_engine: (bool, bool),
    pub action: Option<Action>,
    /// What the engine says each gear is built with (side A, bore, hub), and the bore and hub
    /// circles the drawing suggests.
    pub built: Vec<Built>,
    pub details_suggested: Vec<DetailSuggested>,
    /// The housing cutout (`clearance_body`): one body holding the space the drivetrain needs to
    /// turn, with a seat of the bearing at every bearing point; off by default.
    pub cutout: Cutout,
    /// The cutout's preview: the gears' swept envelopes as rings (drawn translucent) and each
    /// bearing seat as a ring in its gear's colour, in the world, by item.
    pub cutout_rings: Vec<(Option<usize>, Vec<[f64; 3]>)>,
    /// Where the form scrolls to on its next frame: the top (the dialog just opened), or the
    /// active gear's settings (a card just made active).
    pub scroll: Scroll,
    /// The set's request as the dialog first made it when opened on the set (Edit), its numbers as
    /// f64 and without `ui`: an OK with nothing changed since closes without running.
    pub original: Option<Value>,
}

/// Where the form scrolls to on its next frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scroll {
    #[default]
    None,
    Top,
    Settings,
}

/// A request as compared for "nothing changed": numbers as f64, `ui`, `group` and `reset` left out.
pub(crate) fn comparable(v: &Value) -> Value {
    fn norm(v: &Value) -> Value {
        match v {
            Value::Number(n) => n.as_f64().map_or(Value::Null, |f| json!(f)),
            Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
            Value::Object(o) => Value::Object(o.iter().filter(|(k, v)| !matches!(k.as_str(), "ui" | "group" | "reset") && !v.is_null()).map(|(k, v)| (k.clone(), norm(v))).collect()),
            other => other.clone(),
        }
    }
    norm(v)
}

/// The housing cutout's settings as typed (empty: the default).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cutout {
    pub on: bool,
    /// An index into `hub::BEARINGS`; one past the last: a custom size.
    pub bearing: usize,
    pub od: String,
    pub width: String,
    pub bore: String,
    pub radial: String,
    pub axial: String,
    pub fit: String,
    pub run_on: String,
}

impl Cutout {
    /// The bearing chosen: its name, bore, OD and width (a custom one as typed, 0 where empty).
    pub fn bearing_size(&self, app: &SolveApp) -> Option<(String, f64, f64, f64)> {
        match solidcraft_gear::hub::BEARINGS.get(self.bearing) {
            Some(b) => Some((b.0.to_string(), b.1, b.2, b.3)),
            None => {
                let n = |t: &str| number(app, t, ValueKind::Length, "bearing").ok().flatten();
                Some(("custom".into(), n(&self.bore).unwrap_or(0.0), n(&self.od)?, n(&self.width)?))
            }
        }
    }

    /// The `clearance_body` parameters.
    fn params(&self, app: &SolveApp) -> Result<Value, String> {
        let mut c = Map::new();
        for (key, text, what) in [("radial", &self.radial, "Radial gap"), ("axial", &self.axial, "Axial gap"), ("bearing_fit", &self.fit, "Seat fit"), ("shaft_extension", &self.run_on, "Shaft run-on")] {
            if let Some(v) = number(app, text, ValueKind::Length, what)? {
                c.insert(key.into(), json!(v));
            }
        }
        match solidcraft_gear::hub::BEARINGS.get(self.bearing) {
            Some(b) => {
                c.insert("bearing".into(), json!(b.0));
            }
            None => {
                let od = number(app, &self.od, ValueKind::Length, "Bearing OD")?.ok_or("Housing cutout: type the custom bearing's OD and width")?;
                let width = number(app, &self.width, ValueKind::Length, "Bearing width")?.ok_or("Housing cutout: type the custom bearing's width")?;
                let mut b = json!({"od": od, "width": width});
                if let Some(bore) = number(app, &self.bore, ValueKind::Length, "Bearing bore")? {
                    b["bore"] = json!(bore);
                }
                c.insert("bearing".into(), b);
            }
        }
        Ok(Value::Object(c))
    }

    /// The settings a set's `clearance_body` was made with.
    fn of(v: Option<&Value>) -> Cutout {
        let mut c = Cutout { bearing: default_bearing(), ..Cutout::default() };
        let Some(v) = v.filter(|v| v.is_object()) else { return c };
        c.on = true;
        let mm = |k: &str| v.get(k).and_then(Value::as_f64).map(|x| shown(x, ValueKind::Length)).unwrap_or_default();
        (c.radial, c.axial, c.fit, c.run_on) = (mm("radial"), mm("axial"), mm("bearing_fit"), mm("shaft_extension"));
        match v.get("bearing") {
            Some(Value::String(n)) => c.bearing = solidcraft_gear::hub::BEARINGS.iter().position(|b| b.0 == n.as_str()).unwrap_or(c.bearing),
            Some(b @ Value::Object(_)) => {
                c.bearing = solidcraft_gear::hub::BEARINGS.len();
                let b_mm = |k: &str| b.get(k).and_then(Value::as_f64).map(|x| shown(x, ValueKind::Length)).unwrap_or_default();
                (c.od, c.width, c.bore) = (b_mm("od"), b_mm("width"), b_mm("bore"));
            }
            _ => {}
        }
        c
    }
}

/// The bearing a new cutout starts with: a 608.
fn default_bearing() -> usize {
    solidcraft_gear::hub::BEARINGS.iter().position(|b| b.0 == "608").unwrap_or(0)
}

/// One gear as the engine plans it (`gear.calculate`'s `gears[]`): its side A (unit, world) and
/// its bore and hub diameters (mm).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    pub item: usize,
    pub gear: usize,
    pub side_a: Option<[f64; 3]>,
    pub bore: Option<f64>,
    pub hub: Option<f64>,
}

/// A bore or hub circle the drawing suggests for a gear (`detail_suggestions[]`): a circle
/// concentric with its pitch circle, inside it, that nothing picks.
#[derive(Clone, Debug, PartialEq)]
pub struct DetailSuggested {
    pub item: usize,
    pub gear: usize,
    pub hub: bool,
    pub sel: Sel,
    pub diameter: f64,
}

/// `gear.calculate`'s per-gear side A, bore and hub.
pub(crate) fn built_of(r: &Value) -> Vec<Built> {
    let p3 = |v: Option<&Value>| -> Option<[f64; 3]> {
        let a = v?.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
    };
    r.get("gears")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| {
            Some(Built {
                item: g.get("item").and_then(crate::dialogs_gear_graph::item_index)?,
                gear: g.get("gear").and_then(Value::as_u64).unwrap_or(0) as usize,
                side_a: p3(g.get("side_a")),
                bore: g.get("bore").and_then(Value::as_f64),
                hub: g.get("hub").and_then(Value::as_f64),
            })
        })
        .collect()
}

/// `gear.calculate`'s `detail_suggestions`.
pub(crate) fn details_suggested_of(r: &Value) -> Vec<DetailSuggested> {
    r.get("detail_suggestions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| {
            Some(DetailSuggested {
                item: d.get("item").and_then(crate::dialogs_gear_graph::item_index)?,
                gear: d.get("gear").and_then(Value::as_u64).unwrap_or(0) as usize,
                hub: d.get("slot").and_then(Value::as_str) == Some("hub"),
                sel: Sel::SketchCurve { id: d.get("curve")?.as_str()?.to_string(), sketch: Some(d.get("sketch")?.as_u64()?) },
                diameter: d.get("diameter").and_then(Value::as_f64)?,
            })
        })
        .collect()
}

/// The drawing's bore and hub circles taken by themselves (the sketch defines the details): every
/// circle the engine suggests for a gear made in this dialog, unless taken out by hand. True when
/// one was taken (the set is worked out again).
pub(crate) fn auto_details(app: &SolveApp, k: &mut Gear, inputs: &mut [SelInput]) -> bool {
    let mut took = false;
    for d in k.details_suggested.clone() {
        let Some(it) = k.items.get_mut(d.item) else { continue };
        if !it.auto_details || it.dropped.contains(&d.sel) {
            continue;
        }
        took |= matches!(it.take_detail(app, &d.sel), Some(Ok(())));
    }
    if took {
        k.calc = None;
        if let (Some(a), Some(input)) = (k.active.filter(|a| *a < k.items.len()), inputs.first_mut()) {
            input.items = k.items[a].all_picks();
        }
    }
    took
}

/// A bore or hub circle taken out by hand (its × on the card): kept out from then on.
pub(crate) fn drop_detail(k: &mut Gear, item: usize, sel: &Sel, inputs: &mut [SelInput]) {
    let Some(it) = k.items.get_mut(item) else { return };
    it.details.retain(|(s, _)| s != sel);
    if !it.dropped.contains(sel) {
        it.dropped.push(sel.clone());
    }
    k.calc = None;
    if k.active == Some(item)
        && let Some(input) = inputs.first_mut()
    {
        input.items = k.items[item].all_picks();
    }
}

/// A suggested bore or hub circle taken (its Use button): into its gear's item.
pub(crate) fn use_detail(app: &SolveApp, k: &mut Gear, d: &DetailSuggested, inputs: &mut [SelInput]) {
    let Some(it) = k.items.get_mut(d.item) else { return };
    if let Some(Err(e)) = it.take_detail(app, &d.sel) {
        k.refused = Some((Some(d.item), e));
    }
    k.calc = None;
    if k.active == Some(d.item)
        && let Some(input) = inputs.first_mut()
    {
        input.items = k.items[d.item].all_picks();
    }
}

impl Gear {
    pub fn title(&self) -> &'static str {
        "GEARS"
    }

    /// No kernel preview: the teeth are solved on OK only (the viewport shows the outline).
    pub fn previews(&self) -> bool {
        false
    }

    /// The type a new gear is made as.
    pub fn new_type(&self) -> ItemKind {
        TYPES.get(self.new_kind).map_or(ItemKind::Spur, |t| t.0)
    }

    /// Every gear of every item, in order: (item, gear).
    pub fn all_gears(&self) -> Vec<(usize, usize)> {
        self.gear_counts.iter().enumerate().flat_map(|(i, n)| (0..*n).map(move |g| (i, g))).collect()
    }

    /// A gear's short name: "A" for an item of one gear, "B2" for a compound's second, "C sun"
    /// for a planetary set's sun.
    pub fn gear_name(&self, (item, gear): (usize, usize)) -> String {
        let id = crate::dialogs_gear_graph::item_id(item);
        let n = self.gear_counts.get(item).copied().unwrap_or(1);
        match self.items.get(item).map(|it| it.kind).and_then(|k| crate::dialogs_gear_items::gear_part(k, gear)) {
            Some(part) => format!("{id} {part}"),
            None if n <= 1 => id,
            None => format!("{id}{}", gear + 1),
        }
    }
}

fn new(sketch: Option<u64>) -> Gear {
    Gear {
        sketch,
        group: None,
        base: Map::new(),
        legacy: None,
        items: Vec::new(),
        active: None,
        new_kind: 0,
        gear_tab: 0,
        graph: Graph::default(),
        show_graph: false,
        graph_hovered: None,
        card_hovered: None,
        gear_counts: Vec::new(),
        link_add: (0, 0, 1),
        drive_item: None,
        rpm: String::new(),
        torque: String::new(),
        ratio_pick: [None; 2],
        ratio: None,
        assembly: true,
        equations: false,
        values: BTreeMap::new(),
        operation: 0,
        cut_keep_out: false,
        keep_out_gap: String::new(),
        refused: None,
        calc: None,
        item_msgs: Vec::new(),
        set_msgs: Vec::new(),
        notes: Vec::new(),
        results: Vec::new(),
        shape: Vec::new(),
        picked: Vec::new(),
        contacts: Vec::new(),
        arrows: Vec::new(),
        from_engine: (false, false),
        action: None,
        built: Vec::new(),
        details_suggested: Vec::new(),
        cutout: Cutout { bearing: default_bearing(), ..Cutout::default() },
        cutout_rings: Vec::new(),
        scroll: Scroll::Top,
        original: None,
    }
}

/// The pick input (sketch circles, lines and points, 3D ones too, model edges, faces and axes:
/// the active gear's picks) and the bodies the set's operation and keep-out act on.
fn inputs() -> Vec<SelInput> {
    vec![SelInput::new("Geometry", CURVES | SKETCH_POINTS | EDGES | FACES | AXES, true), SelInput::new("Targets", BODIES, true)]
}

/// The dialog for `gear.create`.
pub fn start(app: &SolveApp, id: &str) -> Option<(Kind, Vec<SelInput>)> {
    if id != "gear.create" {
        return None;
    }
    // A set selected (one of its bodies, its feature, a component or occurrence of its
    // assembly): the dialog on that set.
    if let Some(set) = selected_set(app) {
        let mut inputs = inputs();
        inputs[1].items = targets_of(&set);
        return Some((Kind::Gear(for_set(&set)), inputs));
    }
    let g = match &app.gear_after_sketch {
        Some(after) => Gear { calc: None, action: None, ..after.dialog.clone() },
        None => new(None),
    };
    Some((Kind::Gear(g), inputs()))
}

/// The gear set the selection or the timeline's picked folder stands for, if any.
fn selected_set(app: &SolveApp) -> Option<solidcraft_engine::schema::gear::GearSet> {
    if let Some(set) = solidcraft_engine::gear_selected_set(&app.session) {
        return Some(set);
    }
    let sets = solidcraft_engine::gear_sets(&app.session);
    // A gear's component active (a double-click on it used to activate it) and nothing else
    // picked: that gear's set.
    if app.session.active_component != 0 && app.session.selection.is_empty() && app.tree.picked_timeline_groups.is_empty() {
        let st = crate::open_meshing::session_world(&app.session);
        if let Some(set) = sets.iter().find(|s| s.bodies.iter().any(|b| st.body(b).is_some_and(|x| x.component == app.session.active_component))) {
            return Some(set.clone());
        }
    }
    // A set's timeline folder or feature picked in the timeline (not a model selection).
    app.tree.picked_timeline_groups.iter().find_map(|id| sets.iter().find(|set| owns(set, *id)).cloned())
}

/// Whether a timeline folder or feature is a gear set (Edit Gears opens the dialog on it).
pub fn is_set(app: &SolveApp, group: u64) -> bool {
    solidcraft_engine::gear_sets(&app.session).iter().any(|set| owns(set, group))
}

/// The gear set a feature belongs to: the set's own folder or Gears feature, or a feature the set
/// made (a gear's body, inside its folder).
pub fn set_of_feature(app: &SolveApp, id: u64) -> Option<u64> {
    let sets = solidcraft_engine::gear_sets(&app.session);
    // A feature in a set's timeline folder (the assembly's body transfers, joints' features).
    let folder = |set: &solidcraft_engine::schema::gear::GearSet| {
        app.session.doc.timeline_groups.iter().any(|g| (g.id == set.group || Some(g.id) == set.feature) && g.features.contains(&id))
    };
    sets.into_iter().find(|set| owns(set, id) || set.features.contains(&id) || folder(set)).map(|set| set.group)
}

/// A double-click on a body in the view: a gear of a set opens the Gears dialog on that set (in
/// an assembly set the gear is a component, which a double-click would otherwise activate).
/// True when it did.
pub fn double_click_body(app: &mut SolveApp, body: &str) -> bool {
    let Some(set) = solidcraft_engine::gear_sets(&app.session).into_iter().find(|set| set.bodies.iter().any(|b| b == body)) else { return false };
    if let Err(why) = open_set(app, set.group) {
        app.set_status(why, true);
    }
    true
}

/// Whether `id` is the set's timeline folder or its Gears feature (the two ids differ).
fn owns(set: &solidcraft_engine::schema::gear::GearSet, id: u64) -> bool {
    set.group == id || set.feature == Some(id)
}

fn targets_of(set: &solidcraft_engine::schema::gear::GearSet) -> Vec<Sel> {
    set.request.get("targets").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(|name| Sel::Body { name: name.to_string() }).collect()
}

/// Opens the dialog on the gear set of a timeline folder, its Gears feature or a feature it made.
pub fn open_set(app: &mut SolveApp, group: u64) -> Result<(), String> {
    let sets = solidcraft_engine::gear_sets(&app.session);
    let set = sets
        .iter()
        .find(|set| owns(set, group) || set.features.contains(&group))
        .ok_or_else(|| "That is not a gear set".to_string())?;
    let mut g = for_set(set);
    // A gear whose pick is gone opens active, so the status line asks for its replacement.
    g.active = g.items.iter().position(|it| !it.missing(app).is_empty());
    let targets = targets_of(set);
    crate::dialogs::cancel(app);
    app.dialog = None;
    app.start("gear.create");
    match app.dialog.as_mut() {
        Some(d) if matches!(d.kind, Kind::Gear(_)) => {
            d.kind = Kind::Gear(g);
            if let Some(input) = d.inputs.get_mut(1) {
                input.items = targets;
            }
            if let Some(input) = d.inputs.first_mut() {
                input.items.clear();
            }
            d.active = 0;
            Ok(())
        }
        _ => Err("The Gears dialog could not open (finish what is open first)".into()),
    }
}

/// A number as a field shows it.
pub(crate) fn fmt(v: f64) -> String {
    let r = (v * 1e4).round() / 1e4;
    let s = format!("{r:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn shown(v: f64, kind: ValueKind) -> String {
    match kind {
        ValueKind::Length => format!("{} mm", fmt(v)),
        ValueKind::Angle => format!("{} deg", fmt(v)),
        _ => fmt(v),
    }
}

/// The dialog on a set, with what it was made with: its items, links, blocks, drive and output
/// settings; a set made before gear items opens as it is ([`Legacy`]).
pub fn for_set(set: &solidcraft_engine::schema::gear::GearSet) -> Gear {
    let mut r = set.request.as_object().cloned().unwrap_or_default();
    let ui = r.remove("ui");
    crate::dialogs_gear_items::positional_ids(&mut r);
    let mut g = new(set.sketch);
    g.group = Some(set.group);
    g.assembly = r.get("assembly").and_then(Value::as_bool) == Some(true);
    g.equations = r.get("tooth_curves").and_then(Value::as_str) == Some("equations");
    for (key, _, kind, _, _) in ADVANCED {
        if let Some(v) = r.get(key).and_then(Value::as_f64) {
            g.values.insert(key.to_string(), shown(v, kind));
        }
    }
    g.operation = r.get("operation").and_then(Value::as_str).and_then(|o| OPS.iter().position(|x| *x == o)).unwrap_or(0);
    g.cut_keep_out = r.get("keep_out_cuts").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
    g.cutout = Cutout::of(r.get("clearance_body"));
    g.keep_out_gap = r.get("keep_out_gap").and_then(Value::as_f64).map(|v| shown(v, ValueKind::Length)).unwrap_or_default();
    if r.get("items").is_some_and(Value::is_array) {
        g.items = crate::dialogs_gear_items::items_of(&r, ui.as_ref(), &mut g.graph);
        if let Some(d) = r.get("drive") {
            let item = d.get("item").and_then(crate::dialogs_gear_graph::item_index);
            g.drive_item = item.map(|i| (i, d.get("gear").and_then(Value::as_u64).unwrap_or(0) as usize));
            g.rpm = d.get("rpm").and_then(Value::as_f64).map(fmt).unwrap_or_default();
            g.torque = d.get("torque").and_then(Value::as_f64).map(fmt).unwrap_or_default();
        }
        // The type list starts on the first gear's type.
        g.new_kind = g.items.first().and_then(|it| TYPES.iter().position(|t| t.0 == it.kind)).unwrap_or(0);
    } else {
        g.legacy = Some(legacy_of(&r));
    }
    g.base = r;
    g
}

/// What an old set is, in words, and whether its circles can be made gear items.
fn legacy_of(r: &Map<String, Value>) -> Legacy {
    let ty = r.get("type").and_then(Value::as_str).unwrap_or("spur");
    let label = solidcraft_gear::plan::Kind::ALL.iter().find(|k| k.name() == ty).map_or_else(|| ty.replace('_', " "), |k| k.label().to_string());
    let rows = r.get("selection").and_then(Value::as_array).map_or(0, Vec::len);
    let what = if rows > 0 {
        format!("{label} set from {rows} picked rows")
    } else {
        format!("{label} set read from one sketch")
    };
    let convertible = rows > 0 || matches!(ty, "spur" | "helical" | "herringbone" | "pair" | "internal" | "planetary" | "train");
    Legacy { what, convertible }
}

/// An old set's circles made gear items (each circle a gear of the set's tooth style; a ring set
/// a Ring Gear item, a planetary set a Planetary item), so it can be edited the new way. Returns
/// how many gears were made, or why none could be.
pub fn convert_legacy(app: &SolveApp, k: &mut Gear) -> Result<usize, String> {
    let r = &k.base;
    let ty = r.get("type").and_then(Value::as_str).unwrap_or("spur");
    let helix = r.get("helix_angle").and_then(Value::as_f64).unwrap_or(0.0);
    let style = match ty {
        "helical" => ItemKind::Helical,
        "herringbone" => ItemKind::Herringbone,
        _ if r.get("herringbone").and_then(Value::as_bool) == Some(true) => ItemKind::Herringbone,
        _ if helix.abs() > 1e-9 => ItemKind::Helical,
        _ => ItemKind::Spur,
    };
    // The circles: the picked rows that are gears, else every circle of the set's sketch.
    let mut circles: Vec<(Sel, ItemKind)> = Vec::new();
    if let Some(rows) = r.get("selection").and_then(Value::as_array) {
        for row in rows {
            let (Some(sketch), Some(curve)) = (row.get("sketch").and_then(Value::as_u64), row.get("curve").and_then(Value::as_str)) else { continue };
            let kind = match row.get("role").and_then(Value::as_str).unwrap_or("auto") {
                "auto" | "spur" => style,
                "helical" => ItemKind::Helical,
                "herringbone" => ItemKind::Herringbone,
                "internal" => ItemKind::Internal,
                _ => continue,
            };
            circles.push((Sel::SketchCurve { id: curve.to_string(), sketch: Some(sketch) }, kind));
        }
    } else if let Some(sketch) = r.get("sketch").and_then(Value::as_u64).or(k.sketch) {
        let gs = solidcraft_engine::gear_sketch(&app.session, sketch).map_err(|e| e.to_string())?;
        circles = gs.circles.iter().map(|(id, _)| (Sel::SketchCurve { id: id.clone(), sketch: Some(sketch) }, style)).collect();
    }
    let circles: Vec<(Sel, ItemKind, f64)> = circles
        .into_iter()
        .filter_map(|(s, kind)| match thing_of(app, &s)? {
            gi::Thing::Circle { radius, .. } => Some((s, kind, radius)),
            _ => None,
        })
        .collect();
    if circles.is_empty() {
        return Err("No circles found to turn into gears: make the gears again with New gear".into());
    }
    let mut items: Vec<GearItem> = Vec::new();
    match ty {
        // One set round one centre: the largest circle is the ring.
        "planetary" | "internal" => {
            let mut sorted = circles.clone();
            sorted.sort_by(|a, b| b.2.total_cmp(&a.2));
            let kind = if ty == "planetary" { ItemKind::Planetary } else { ItemKind::Internal };
            let mut it = GearItem::new(kind);
            for (s, _, _) in &sorted {
                if it.refusal(app, s, &[]).is_none() {
                    it.picks.push(s.clone());
                }
            }
            items.push(it);
        }
        _ => {
            for (s, kind, _) in circles {
                items.push(GearItem { picks: vec![s], ..GearItem::new(kind) });
            }
        }
    }
    let n = items.len();
    k.graph = Graph::default();
    for i in 0..n {
        k.graph.node_of(i);
    }
    k.items = items;
    k.legacy = None;
    k.active = None;
    k.calc = None;
    Ok(n)
}

/// The gear crate's world description of `sel` (see [`thing_of`]), for a projected copy the
/// geometry it was projected from.
fn source_of(app: &SolveApp, sel: &Sel) -> Option<Sel> {
    use solidcraft_engine::sketch::LinkSource;
    let state = crate::open_meshing::session_world(&app.session);
    let mut at = sel.clone();
    let mut found = None;
    for _ in 0..4 {
        let (sketch, id, curve) = match &at {
            Sel::SketchCurve { id, sketch } => (sketch.or(app.session.active_sketch)?, id.clone(), true),
            Sel::ExternalSketchCurve { sketch, id } => (*sketch, id.clone(), true),
            Sel::ExternalSketchPoint { sketch, id } => (*sketch, id.clone(), false),
            Sel::SketchPoint { id } => (app.session.active_sketch?, id.clone(), false),
            _ => break,
        };
        let Some(ss) = state.sketch(sketch) else { break };
        let link = if curve { ss.sketch.curves.iter().find(|c| c.id == id).and_then(|c| c.link.clone()) } else { ss.sketch.points.iter().find(|p| p.id == id).and_then(|p| p.link.clone()) };
        let Some(link) = link.and_then(|l| ss.sketch.link(&l)) else { break };
        let next = match &link.source {
            LinkSource::SketchCurve { sketch, curve } => Sel::SketchCurve { id: curve.clone(), sketch: Some(*sketch) },
            LinkSource::SketchPoint { sketch, point } => Sel::ExternalSketchPoint { sketch: *sketch, id: point.clone() },
            _ => break,
        };
        found = Some(next.clone());
        at = next;
    }
    found
}

/// `sel` as a `selection` locator (for the items' components).
pub(crate) fn locate(app: &SolveApp, sel: &Sel) -> Option<Value> {
    let src = source_of(app, sel);
    let x = src.as_ref().unwrap_or(sel);
    let v = |p: solidcore_record::Vec3| json!([p.x, p.y, p.z]);
    match x {
        Sel::SketchCurve { id, sketch } => {
            let sk = sketch.or(app.session.active_sketch).or_else(|| crate::dialogs::picks_sketch(app, std::slice::from_ref(x), None))?;
            Some(json!({"sketch": sk, "curve": id}))
        }
        Sel::ExternalSketchCurve { sketch, id } => Some(json!({"sketch": sketch, "curve": id})),
        Sel::ExternalSketchPoint { sketch, id } => Some(json!({"sketch": sketch, "point": id})),
        Sel::SketchPoint { id } => Some(json!({"sketch": app.session.active_sketch?, "point": id})),
        Sel::Edge { point, .. } => Some(json!({"edge": v(*point)})),
        Sel::Face { point, .. } => Some(json!({"face": v(*point)})),
        Sel::Axis { name } => Some(json!({"axis": name})),
        _ => None,
    }
}

/// What `sel` is in the world, as the gear crate's items take it ([`gi::Thing`]): a sketch
/// circle or arc (its plane's normal as side A), line (a construction line keeps its flag: the
/// item checks refuse it where it can't be a shaft) or point; a 3D sketch's arc, line or point as
/// the engine reads it; a round model edge as its circle; a straight edge, a round face or an axis
/// as its line (`dialogs::axis_of`). A projected copy is what it was projected from. `None`:
/// nothing a gear item can take.
pub(crate) fn thing_of(app: &SolveApp, sel: &Sel) -> Option<gi::Thing> {
    use solidcraft_engine::geom::prelude::*;
    use solidcraft_engine::sketch::CurveKind;
    use solidcraft_gear::items::Thing;
    let src = source_of(app, sel);
    let sel = src.as_ref().unwrap_or(sel);
    let v = |p: solidcore_record::Vec3| [p.x, p.y, p.z];
    let in_sketch = |sketch: u64, id: &str, curve: bool| -> Option<Thing> {
        let st = crate::open_meshing::session_world(&app.session);
        let ss = st.sketch(sketch)?;
        let sk = &ss.sketch;
        let w = |i: usize| sk.point(i).map(|p| v(ss.plane.to_world(p)));
        let flat = if curve { sk.curves.iter().find(|c| c.id == id).map(Some) } else { sk.point_index(id).map(|_| None) };
        let thing = match flat {
            Some(None) => Thing::point(w(sk.point_index(id)?)?),
            Some(Some(c)) => match c.kind {
                CurveKind::Circle { c: ci, r } => Thing::circle(w(ci)?, v(ss.plane.normal()), r),
                CurveKind::Arc { c: ci, a, .. } => Thing::circle(w(ci)?, v(ss.plane.normal()), sk.point(a)?.dist(sk.point(ci)?)),
                CurveKind::Line { a, b } => Thing::Line { a: w(a)?, b: w(b)?, construction: c.construction, ids: Default::default() },
                _ => return None,
            },
            // A 3D sketch's curve or point, as the engine reads it.
            None => {
                let gs = solidcraft_engine::gear_sketch(&app.session, sketch).ok()?;
                match gs.things.iter().find(|(k, _)| k == id)?.1 {
                    solidcraft_gear::drivetrain::Thing::Circle(c) => Thing::circle(c.c, c.n, c.r),
                    solidcraft_gear::drivetrain::Thing::Line(l) => Thing::line(l.a, l.b),
                    solidcraft_gear::drivetrain::Thing::Point(p) => Thing::point(p),
                }
            }
        };
        Some(thing.with_ids(Some(sketch), id))
    };
    let line = |entity: String| -> Option<Thing> {
        let (o, d) = crate::dialogs::axis_of(app, sel)?;
        Some(Thing::line(v(o), v(o + d)).with_ids(None, &entity))
    };
    match sel {
        Sel::SketchCurve { id, sketch } => {
            let sk = sketch.or(app.session.active_sketch).or_else(|| crate::dialogs::picks_sketch(app, std::slice::from_ref(sel), None))?;
            in_sketch(sk, id, true)
        }
        Sel::ExternalSketchCurve { sketch, id } => in_sketch(*sketch, id, true),
        Sel::ExternalSketchPoint { sketch, id } => in_sketch(*sketch, id, false),
        Sel::SketchPoint { id } => in_sketch(app.session.active_sketch?, id, false),
        Sel::Edge { body, index, .. } => match crate::dialogs::edge_circle(&app.session, body, *index) {
            Some((c, n, r)) => Some(Thing::circle(v(c), v(n), r).with_ids(None, &format!("edge {body} {index}"))),
            None => line(format!("edge {body} {index}")),
        },
        Sel::Face { body, index, .. } => line(format!("face {body} {index}")),
        Sel::Axis { name } => line(format!("axis {name}")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Picks: they go to the active gear, or start a gear of the chosen type.
// ---------------------------------------------------------------------------------------------

/// A new gear of the chosen type, made the active one: picks go to it (the Geometry input is
/// emptied for its picks).
pub(crate) fn new_gear(k: &mut Gear, inputs: &mut [SelInput]) {
    k.items.push(GearItem::new(k.new_type()));
    let i = k.items.len() - 1;
    k.graph.node_of(i);
    k.active = Some(i);
    k.gear_tab = 0;
    k.refused = None;
    k.scroll = Scroll::Settings;
    if let Some(input) = inputs.first_mut() {
        input.items.clear();
    }
}

/// Item `i` made the active one: its picks are the Geometry input's.
pub(crate) fn make_active(k: &mut Gear, i: usize, inputs: &mut [SelInput]) {
    if i >= k.items.len() {
        return;
    }
    if k.active != Some(i) {
        k.gear_tab = 0;
        k.scroll = Scroll::Settings;
    }
    k.active = Some(i);
    k.refused = None;
    if let Some(input) = inputs.first_mut() {
        input.items = k.items[i].all_picks();
    }
}

/// Item `i` taken out, with its links; the others renumbered.
pub(crate) fn remove_item(k: &mut Gear, i: usize, inputs: &mut [SelInput]) {
    if i >= k.items.len() {
        return;
    }
    k.items.remove(i);
    k.graph.remove_item(i);
    let shift = |a: usize| if a > i { a - 1 } else { a };
    k.active = k.active.filter(|a| *a != i).map(shift);
    k.drive_item = k.drive_item.filter(|(d, _)| *d != i).map(|(d, g)| (shift(d), g));
    k.ratio_pick = k.ratio_pick.map(|p| p.filter(|(d, _)| *d != i).map(|(d, g)| (shift(d), g)));
    k.refused = None;
    if let Some(input) = inputs.first_mut() {
        input.items = k.active.map(|a| k.items[a].all_picks()).unwrap_or_default();
    }
}

/// The Geometry input's picks laid onto the gears (once a frame, while picks go to it): one taken
/// out again leaves its gear; a new one fills the active gear's next slot, or, when the active
/// gear is complete (or there is none) and it doesn't fit there, starts a new gear of the chosen
/// type. A pick that fits nowhere is refused with the reason ([`gi::validate_slot`]). Afterwards
/// the input holds the active gear's picks.
pub(crate) fn sync_picks(app: &SolveApp, k: &mut Gear, inputs: &mut [SelInput]) {
    let Some(input) = inputs.first_mut() else { return };
    let current: Vec<Sel> = k.active.and_then(|a| k.items.get(a)).map(GearItem::all_picks).unwrap_or_default();
    if let Some(a) = k.active.filter(|a| *a < k.items.len()) {
        // (A pick that no longer reads stays: the card names it until it is re-picked.)
        k.items[a].picks.retain(|p| input.items.contains(p) || thing_of(app, p).is_none());
        k.items[a].details.retain(|(p, _)| input.items.contains(p) || thing_of(app, p).is_none());
    }
    let fresh: Vec<Sel> = input.items.iter().filter(|s| !current.contains(s)).cloned().collect();
    for sel in fresh {
        take_pick(app, k, sel);
    }
    input.items = k.active.and_then(|a| k.items.get(a)).map(GearItem::all_picks).unwrap_or_default();
}

fn take_pick(app: &SolveApp, k: &mut Gear, sel: Sel) {
    let others = |k: &Gear, skip: Option<usize>| -> Vec<(usize, GearItem)> { k.items.iter().enumerate().filter(|(i, _)| Some(*i) != skip).map(|(i, it)| (i, it.clone())).collect() };
    let mut why_active = None;
    // A small circle drawn inside one of the active gear's pitch circles, on its centre: its
    // bore (the smaller) or hub circle.
    if let Some(a) = k.active.filter(|a| *a < k.items.len())
        && !k.items.iter().any(|it| it.all_picks().contains(&sel))
        && let Some(done) = k.items[a].take_detail(app, &sel)
    {
        k.refused = done.err().map(|e| (Some(a), e));
        return;
    }
    if let Some(a) = k.active.filter(|a| *a < k.items.len()) {
        let list = others(k, Some(a));
        let refs: Vec<(usize, &GearItem)> = list.iter().map(|(i, it)| (*i, it)).collect();
        match k.items[a].refusal(app, &sel, &refs) {
            None => {
                k.items[a].take(app, sel);
                k.refused = None;
                return;
            }
            Some(why) if !k.items[a].complete(app) => {
                k.refused = Some((Some(a), why));
                return;
            }
            Some(why) => why_active = Some(why),
        }
    }
    // The active gear is complete (or there is none): the pick starts a new gear of the type
    // chosen in the list.
    let fresh = GearItem::new(k.new_type());
    let list = others(k, None);
    let refs: Vec<(usize, &GearItem)> = list.iter().map(|(i, it)| (*i, it)).collect();
    match fresh.refusal(app, &sel, &refs) {
        None => {
            k.items.push(GearItem { picks: vec![sel], ..fresh });
            let i = k.items.len() - 1;
            k.graph.node_of(i);
            k.active = Some(i);
            k.gear_tab = 0;
            k.refused = None;
        }
        Some(why) => {
            let text = match why_active {
                // Used elsewhere: say so once.
                Some(w) if w.starts_with("already used") => w,
                Some(w) => format!("{w}; it doesn't start a new {} gear either: {why}", k.new_type().label()),
                None => format!("A {} gear can't start with that: {why}", k.new_type().label()),
            };
            k.refused = Some((k.active, text));
        }
    }
}

/// What the status line says to do next.
pub(crate) fn next_step(app: &SolveApp, k: &Gear) -> (String, bool) {
    if let Some((_, why)) = &k.refused {
        return (format!("That pick doesn't fit: {why}"), true);
    }
    if k.legacy.is_some() {
        return ("This set was made with the earlier Gears dialog. OK keeps it; Convert turns its circles into gears.".into(), false);
    }
    let Some(a) = k.active.filter(|a| *a < k.items.len()) else {
        return match k.items.len() {
            0 => (format!("Press New gear (or click a circle in the view) to start a {} gear", k.new_type().label()), false),
            _ => ("Click a gear's card to change it, or New gear for another".into(), false),
        };
    };
    let it = &k.items[a];
    let name = format!("Gear {} · {}", crate::dialogs_gear_graph::item_id(a), it.label());
    if let Some(gone) = it.missing(app).first() {
        return (format!("{name} — its {gone} is missing: click its replacement in the view"), true);
    }
    match it.next_slot(app) {
        Some(next) => (format!("{name} — click: {next}"), false),
        None => match it.optional_slot(app) {
            // A bevel side view without its contact line: sized by its gears' settings.
            Some(more) if it.kind == ItemKind::Bevel && more == "contact line" => (
                format!("{name} is complete — sized by its teeth and module (Gear {} settings below); or click its contact line to size it from the drawing", crate::dialogs_gear_graph::item_id(a)),
                false,
            ),
            Some(more) => (format!("{name} is complete — optional: {more}; or New gear for the next one"), false),
            None => (format!("{name} is complete — New gear for the next one"), false),
        },
    }
}

// ---------------------------------------------------------------------------------------------
// The request
// ---------------------------------------------------------------------------------------------

/// The keys the dialog owns: on an update, one it leaves out is taken away (back to auto), and so
/// are the old single-set keys once the set is made of gear items.
fn owned() -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = ADVANCED.iter().map(|a| a.0).collect();
    keys.extend([
        "assembly",
        "tooth_curves",
        "clearance_body",
        "operation",
        "targets",
        "keep_out_cuts",
        "keep_out_gap",
        "assembly_driver",
        "items",
        "links",
        "module_blocks",
        "drive",
        "ui",
        // The old single-set keys (a set made before gear items, converted).
        "type",
        "sketch",
        "selection",
        "picks",
        "module",
        "teeth",
        "teeth2",
        "helix_angle",
        "herringbone",
        "face_width",
        "profile_shift",
        "profile_shift2",
        "bore",
        "bore2",
        "hub_diameter",
        "hub_length",
        "spiral_angle",
        "shaft_angle",
        "starts",
        "planets",
        "shafts",
        "differential",
    ]);
    keys
}

/// A typed value as a number (`None` when empty).
fn number(app: &SolveApp, text: &str, kind: ValueKind, what: &str) -> Result<Option<f64>, String> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let v = app.session.doc.eval(t, kind).map_err(|e| format!("{what}: {e}: type a number, or leave it empty"))?;
    Ok(Some(if matches!(kind, ValueKind::Angle) { v.to_degrees() } else { v }))
}

/// The output settings (operation, targets, keep-out, assembly, the Advanced defaults).
fn output(app: &SolveApp, k: &Gear, inputs: &[SelInput], p: &mut Map<String, Value>) -> Result<(), String> {
    for (key, label, kind, _, _) in ADVANCED {
        if let Some(v) = number(app, k.values.get(key).map(String::as_str).unwrap_or(""), kind, label)? {
            p.insert(key.into(), json!(v));
        }
    }
    p.insert("assembly".into(), json!(k.assembly));
    if k.cutout.on {
        p.insert("clearance_body".into(), k.cutout.params(app)?);
    }
    if k.equations {
        p.insert("tooth_curves".into(), json!("equations"));
    }
    let targets: Vec<String> = inputs
        .get(1)
        .map(|i| i.items.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter_map(|x| match x {
            Sel::Body { name } => Some(name.clone()),
            _ => None,
        })
        .collect();
    p.insert("operation".into(), json!(OPS.get(k.operation).copied().unwrap_or("new")));
    if targets.is_empty() {
        if k.operation != 0 {
            return Err("Join, Cut and Intersect act on the Targets: pick the bodies (Output › Targets)".into());
        }
        if k.cut_keep_out {
            return Err("Cut Keep-Out cuts the gears' room out of the Targets: pick the housing bodies (Output › Targets)".into());
        }
    } else {
        p.insert("targets".into(), json!(targets));
        if k.cut_keep_out {
            p.insert("keep_out_cuts".into(), json!(targets));
            if let Some(gap) = number(app, &k.keep_out_gap, ValueKind::Length, "Keep-out gap")? {
                p.insert("keep_out_gap".into(), json!(gap));
            }
        }
    }
    Ok(())
}

/// The parameters that make the set as the dialog holds it (the whole of it, when editing: the
/// set's request with the dialog's keys laid over it).
pub(crate) fn request(app: &SolveApp, k: &Gear, inputs: &[SelInput]) -> Result<Value, String> {
    let mut p = Map::new();
    if k.legacy.is_some() {
        // An old set: kept as it was, with its output settings as the dialog has them.
        let mut whole = k.base.clone();
        for key in ["operation", "targets", "keep_out_cuts", "keep_out_gap", "assembly", "tooth_curves"] {
            whole.remove(key);
        }
        output(app, k, inputs, &mut p)?;
        whole.extend(p);
        return Ok(Value::Object(whole));
    }
    if k.items.is_empty() {
        return Err("Add a gear: pick a type, press New gear and click the sketch pieces it lists".into());
    }
    p.extend(crate::dialogs_gear_graph::items_request(app, &k.items, &k.graph)?);
    if let Some((item, gear)) = k.drive_item.filter(|(i, _)| *i < k.items.len()) {
        let mut drive = json!({"item": crate::dialogs_gear_graph::item_id(item), "gear": gear});
        if let Some(rpm) = number(app, &k.rpm, ValueKind::Unitless, "Input rpm")? {
            drive["rpm"] = json!(rpm);
        }
        if let Some(t) = number(app, &k.torque, ValueKind::Unitless, "Input torque")? {
            drive["torque"] = json!(t);
        }
        p.insert("drive".into(), drive);
    }
    output(app, k, inputs, &mut p)?;
    if k.group.is_none() {
        p.retain(|_, v| !v.is_null());
        return Ok(Value::Object(p));
    }
    let mut whole = k.base.clone();
    for key in owned() {
        whole.remove(key);
    }
    for (key, v) in p {
        if !v.is_null() {
            whole.insert(key, v);
        }
    }
    Ok(Value::Object(whole))
}

pub fn commands(app: &SolveApp, k: &Gear, inputs: &[SelInput], _extra: &Map<String, Value>) -> Result<Vec<(String, Value)>, String> {
    let mut p = request(app, k, inputs)?;
    // Edit, OK with nothing changed: nothing to run (the design stays as it was, unchanged).
    if k.group.is_some() && k.original.as_ref() == Some(&comparable(&p)) {
        return Ok(Vec::new());
    }
    Ok(vec![match k.group {
        Some(group) => {
            p["group"] = json!(group);
            p["reset"] = json!(true);
            ("gear.update".to_string(), p)
        }
        None => ("gear.create".to_string(), p),
    }])
}

// ---------------------------------------------------------------------------------------------
// What the engine says
// ---------------------------------------------------------------------------------------------

/// A ratio as a link shows it ("2.00").
pub(crate) fn fmt_ratio(r: f64) -> String {
    format!("{r:.2}")
}

/// A length in mm as the design shows it (its units and length precision, no unit name).
pub(crate) fn len_of(app: &SolveApp) -> impl Fn(f64) -> String + use<> {
    let (units, decimals) = (app.session.doc.units.clone(), app.preferences.length_decimals);
    move |v| crate::prefs::length_number(v, &units, decimals)
}

/// A gear clicked for the ratio calculator: the second of the two when one is picked, else a
/// new first one (clicking a picked one again takes it out).
pub(crate) fn pick_for_ratio(k: &mut Gear, gear: (usize, usize)) {
    k.ratio_pick = match k.ratio_pick {
        [Some(a), None] if a == gear => [None, None],
        [Some(a), None] => [Some(a), Some(gear)],
        [Some(a), Some(b)] if a == gear => [Some(b), None],
        [Some(a), Some(b)] if b == gear => [Some(a), None],
        _ => [Some(gear), None],
    };
}

/// A gear as `gear.ratio` names it: {item, gear}.
pub(crate) fn gear_ref(g: (usize, usize)) -> Value {
    json!({"item": crate::dialogs_gear_graph::item_id(g.0), "gear": g.1})
}

/// `gear.ratio`'s reply as the readout's lines: the ratio (b's speed over a's) and the
/// direction, the path of meshes and shared shafts, the efficiency, and a rack's travel per
/// turn (`len` shows a length in the design's units). Read: `ratio`, `direction` ("same" |
/// "opposite"), `path` (steps {from, to, from_gear?, to_gear?}), `efficiency` (0–1), `mm_per_rev`.
pub(crate) fn ratio_lines(r: &Value, len: &dyn Fn(f64) -> String) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let tr = solidcraft_ui_text::tr_label;
    if let Some(x) = r.get("ratio").and_then(Value::as_f64) {
        let dir = match r.get("direction").and_then(Value::as_str) {
            Some("same") => format!(", {}", tr("same direction")),
            Some("opposite") => format!(", {}", tr("opposite direction")),
            _ => String::new(),
        };
        out.push((tr("Ratio").to_string(), format!("{} : 1{dir}", fmt_ratio(if dir.is_empty() { x } else { x.abs() }))));
    }
    // A gear {item, gear} as "2·1", else the body's name.
    let gear_or = |g: Option<&Value>, body: Option<&Value>| -> Option<String> {
        match g.and_then(|g| Some((g.get("item").and_then(crate::dialogs_gear_graph::item_index)?, g.get("gear").and_then(Value::as_u64).unwrap_or(0)))) {
            Some((item, gear)) => Some(format!("{}·{}", item + 1, gear + 1)),
            None => body.and_then(Value::as_str).map(str::to_string),
        }
    };
    let steps = r.get("path").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let mut path: Vec<String> = steps.first().and_then(|s| gear_or(s.get("from_gear"), s.get("from"))).into_iter().collect();
    path.extend(steps.iter().filter_map(|s| gear_or(s.get("to_gear"), s.get("to"))));
    if !path.is_empty() {
        out.push((tr("Path").to_string(), path.join(" → ")));
    }
    if let Some(e) = r.get("efficiency").and_then(Value::as_f64) {
        out.push((tr("Efficiency").to_string(), format!("{} %", fmt((e * 1000.0).round() / 10.0))));
    }
    if let Some(mm) = r.get("mm_per_rev").and_then(Value::as_f64) {
        out.push((tr("Rack travel").to_string(), format!("{} / rev", len(mm))));
    }
    out
}

/// `gear.ratio`'s parameters for two bodies selected in the model, when both are gears of one
/// Gears feature: {group, a, b} with the two bodies' names (the quick readout shows the ratio).
pub(crate) fn ratio_bodies(session: &solidcraft_engine::Session, selection: &[Sel]) -> Option<Value> {
    let [Sel::Body { name: a }, Sel::Body { name: b }] = selection else { return None };
    let set = solidcraft_engine::gear_sets(session).into_iter().find(|s| s.bodies.contains(a) && s.bodies.contains(b))?;
    Some(json!({"group": set.feature.unwrap_or(set.group), "a": a, "b": b}))
}

/// The results table's rows from `gear.calculate`'s `gears` (§6): gear, z, m, Ø pitch, ratio
/// from the driver, rpm, torque, contact ratio, warnings.
pub(crate) fn results_of(r: &Value, len: &dyn Fn(f64) -> String) -> Vec<[String; 9]> {
    let f = |v: Option<f64>| v.map(fmt).unwrap_or_default();
    crate::dialogs_gear_graph::gear_replies(r)
        .into_iter()
        .map(|g| {
            [
                format!("{}·{}", g.item + 1, g.gear + 1),
                if g.teeth > 0 { g.teeth.to_string() } else { String::new() },
                f((g.module > 0.0).then_some(g.module)),
                if g.pitch_d > 0.0 { len(g.pitch_d) } else { String::new() },
                g.ratio.map(fmt_ratio).unwrap_or_default(),
                f(g.rpm),
                f(g.torque),
                f(g.contact_ratio),
                g.warnings.join("; "),
            ]
        })
        .collect()
}

/// The facts worth showing: what the set came to.
fn summary(r: &Value) -> Vec<String> {
    let mut out: Vec<String> = r["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|w| format!("⚠ {w}")).collect();
    out.dedup();
    out
}

/// What the status says of a reply: the set's warnings, or what is wrong with it (the engine's
/// refusal, or its `error` when it answers without building).
pub(crate) fn status_of(r: Result<Value, impl std::fmt::Display>) -> Result<Vec<String>, String> {
    let r = r.map_err(|e| e.to_string())?;
    match r.get("error").and_then(Value::as_str) {
        Some(e) => Err(e.to_string()),
        None => Ok(summary(&r)),
    }
}

/// The reply's `notes` (plain sentences about the set).
pub(crate) fn notes_of(r: &Value) -> Vec<String> {
    r.get("notes").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
}

/// A message as the dialog says it: the engine's "Item B" is "Gear B" here.
pub(crate) fn worded(text: &str) -> String {
    text.replace("Item ", "Gear ").replace("item ", "gear ")
}

/// The item a message is about ("Gear B: …", "Item B · Compound: …", "item B …"), and the
/// message without that name in front.
pub(crate) fn about_item(text: &str) -> Option<(usize, String)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let at = words.iter().position(|w| matches!(*w, "Gear" | "Item" | "gear" | "item"))?;
    if at > 2 {
        return None;
    }
    let id = words.get(at + 1)?.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let i = gi::item_number(id)?;
    // The rest, after "Gear B:" or "Gear B · Compound:".
    let rest = match text.find(':') {
        Some(c) if c < 40 => text[c + 1..].trim().to_string(),
        _ => text.to_string(),
    };
    Some((i, worded(&rest)))
}

/// Works out the set (`gear.calculate`, once per change) and what is said of each gear: the
/// engine's per-gear warnings, its refusal (on the gear it names), a setting that doesn't read;
/// then the preview (outline, picked geometry, contacts, side-A arrows), the suggested links and
/// the ratio readout.
pub(crate) fn refresh(app: &mut SolveApp, k: &mut Gear, inputs: &[SelInput]) {
    let len = len_of(app);
    k.gear_counts = k.items.iter().map(|it| it.gear_count(app)).collect();
    let asked = request(app, k, inputs);
    if let (Some(_), None, Ok(p)) = (k.group, &k.original, &asked) {
        k.original = Some(comparable(p));
    }
    match &asked {
        Ok(p) => {
            let key = format!("{}@{}", p, app.session.revision);
            if k.calc.as_ref().is_none_or(|(c, _)| *c != key) {
                let r = app.session.execute("gear.calculate", p);
                k.from_engine = r.as_ref().map_or((false, false), |r| (r.get("outlines").is_some_and(|o| o.as_array().is_some_and(|a| !a.is_empty())), r.get("suggestions").is_some()));
                k.shape = r.as_ref().map(outlines_of).unwrap_or_default();
                if let Ok(r) = r.as_ref() {
                    k.graph.note(r);
                }
                k.notes = r.as_ref().map(notes_of).unwrap_or_default();
                k.results = r.as_ref().map(|r| results_of(r, &len)).unwrap_or_default();
                k.contacts = r.as_ref().map(contacts_of).unwrap_or_default();
                k.built = r.as_ref().map(built_of).unwrap_or_default();
                k.details_suggested = r.as_ref().map(details_suggested_of).unwrap_or_default();
                k.calc = Some((key, status_of(r)));
            }
        }
        Err(_) => {
            k.calc = None;
            k.from_engine = (false, false);
        }
    }
    // What is said of each gear, and of the set.
    let mut per: Vec<Vec<Msg>> = vec![Vec::new(); k.items.len()];
    let mut set_msgs = Vec::new();
    let mut put = |text: &str, error: bool| {
        let msg = |t: String| Msg { text: t, error };
        match about_item(text) {
            Some((i, rest)) if i < per.len() => {
                if !per[i].iter().any(|m| m.text == rest) {
                    per[i].push(msg(rest));
                }
            }
            _ => {
                let t = worded(text);
                if !set_msgs.iter().any(|m: &Msg| m.text == t) {
                    set_msgs.push(msg(t));
                }
            }
        }
    };
    if let Err(e) = &asked
        && !k.items.is_empty()
        && !e.contains("still needs")
        && !e.contains("is missing (")
    {
        put(e, true);
    }
    match k.calc.as_ref().map(|c| &c.1) {
        Some(Err(e)) => put(e, true),
        Some(Ok(lines)) => {
            for l in lines {
                put(l.trim_start_matches("⚠ "), false);
            }
        }
        None => {}
    }
    for r in k.graph.replies() {
        for w in &r.warnings {
            if r.item < per.len() && !per[r.item].iter().any(|m| m.text == *w) {
                per[r.item].push(Msg { text: worded(w), error: false });
            }
        }
    }
    // A message several gears share (one sketch that isn't fully defined): said once, above the
    // cards, naming the gears.
    let mut shared: Vec<(String, bool, Vec<usize>)> = Vec::new();
    for (i, msgs) in per.iter().enumerate() {
        for m in msgs {
            match shared.iter_mut().find(|s| s.0 == m.text) {
                Some(s) => s.2.push(i),
                None => shared.push((m.text.clone(), m.error, vec![i])),
            }
        }
    }
    for (text, error, gears) in shared.into_iter().filter(|s| s.2.len() > 1) {
        for i in &gears {
            per[*i].retain(|m| m.text != text);
        }
        let names: Vec<String> = gears.iter().map(|i| crate::dialogs_gear_graph::item_id(*i)).collect();
        set_msgs.push(Msg { text: format!("Gears {}: {text}", names.join(", ")), error });
    }
    k.item_msgs = per;
    k.set_msgs = set_msgs;
    // The preview: the outline from the picked geometry where the engine drew none, each item's
    // picked geometry, each gear's side-A arrow, the links the drawing suggests.
    // (Always the items' own outline, coloured by item: the engine's outlines are numbered by
    // body, which a compound shares, so their colours wouldn't match the cards.)
    let (lines, contacts, arrows) = items_preview(app, k);
    k.shape = lines;
    if k.contacts.is_empty() || asked.is_err() {
        k.contacts = contacts;
    }
    k.arrows = arrows;
    k.picked = picked_geometry(app, k);
    k.cutout_rings = if k.cutout.on { cutout_preview(app, k) } else { Vec::new() };
    if !k.from_engine.1 {
        let local = local_suggestions(app, k);
        k.graph.suggest(local);
    }
    // A collision between two gears the drawing suggests linking: the suggestion says what to do,
    // so the collision isn't said as well (until the suggestion is dismissed by a link).
    let suggested: Vec<(usize, usize)> = k.graph.suggested().iter().map(|s| (s.a.min(s.b), s.a.max(s.b))).collect();
    let resolved = |text: &str| -> bool {
        if !text.contains("collide") {
            return false;
        }
        let ids = gears_named(text);
        ids.iter().any(|a| ids.iter().any(|b| a < b && suggested.contains(&(*a, *b))))
    };
    for msgs in &mut k.item_msgs {
        msgs.retain(|m| !resolved(&m.text));
    }
    k.set_msgs.retain(|m| !resolved(&m.text));
    // Two pitch circles in one plane a little apart with no link: they won't mesh; say so.
    for (i, j, gap) in near_misses(app, k) {
        let text = format!(
            "{} and {} are {} mm apart, so they won't mesh. Make them tangent in the sketch to mesh them.",
            crate::dialogs_gear_graph::item_id(i),
            crate::dialogs_gear_graph::item_id(j),
            fmt((gap * 10.0).round() / 10.0)
        );
        if let Some(m) = k.item_msgs.get_mut(j) {
            m.push(Msg { text, error: false });
        }
    }
    // The ratio between the two gears picked (`gear.ratio`), once per change.
    match (&asked, k.ratio_pick) {
        (Ok(p), [Some(a), Some(b)]) if a != b => {
            let q = json!({"request": p, "a": gear_ref(a), "b": gear_ref(b)});
            let key = format!("{q}@{}", app.session.revision);
            if k.ratio.as_ref().is_none_or(|(c, _)| *c != key) {
                let r = app.session.execute("gear.ratio", &q);
                let lines = r.map_err(|e| e.to_string()).and_then(|r| match r.get("error").and_then(Value::as_str) {
                    Some(e) => Err(worded(e)),
                    None => Ok(ratio_lines(&r, &len).into_iter().map(|(l, v)| format!("{l}\t{v}")).collect()),
                });
                k.ratio = Some((key, lines));
            }
        }
        (Err(e), [Some(_), Some(_)]) => k.ratio = Some((String::new(), Err(worded(e)))),
        _ => k.ratio = None,
    }
}

/// The items a message names ("Gear D gear 1 and Gear E gear 1 collide", "Gear B1"): their indices.
fn gears_named(text: &str) -> Vec<usize> {
    let words: Vec<&str> = text.split_whitespace().collect();
    words
        .windows(2)
        .filter(|w| matches!(w[0], "Gear" | "Item" | "gear" | "item"))
        .filter_map(|w| {
            let id: String = w[1].chars().take_while(char::is_ascii_alphanumeric).collect();
            (id.chars().next().is_some_and(|c| c.is_ascii_uppercase())).then(|| gi::item_number(&id)).flatten()
        })
        .collect()
}

/// Pairs of single gears whose pitch circles lie in one plane a little apart (within two modules
/// of touching, or a fifth of their radii) and aren't linked: (first, second, the gap in mm).
fn near_misses(app: &SolveApp, k: &Gear) -> Vec<(usize, usize, f64)> {
    let geoms: Vec<Vec<gi::GearGeom>> = k.items.iter().enumerate().map(|(i, it)| gi::gear_geoms(&it.crate_item(app, &crate::dialogs_gear_graph::item_id(i)))).collect();
    let links = k.graph.links();
    let linked = |a: usize, b: usize| links.iter().any(|l| (l.0 == a && l.2 == b) || (l.0 == b && l.2 == a)) || k.graph.suggested().iter().any(|s| (s.a == a && s.b == b) || (s.a == b && s.b == a));
    let module = |i: usize| k.graph.replies().iter().find(|r| r.item == i).map(|r| r.module).filter(|m| *m > 0.0).unwrap_or(2.0);
    let mut out = Vec::new();
    for i in 0..geoms.len() {
        for j in i + 1..geoms.len() {
            let (Some(a), Some(b)) = (geoms[i].first(), geoms[j].first()) else { continue };
            let (Some(ra), Some(rb), Some(n)) = (a.radius, b.radius, unit(a.axis)) else { continue };
            if a.ring || b.ring || unit(b.axis).is_none_or(|m| (n[0] * m[0] + n[1] * m[1] + n[2] * m[2]).abs() < 1.0 - 1e-6) || linked(i, j) {
                continue;
            }
            let d = [0, 1, 2].map(|q| b.centre[q] - a.centre[q]);
            let off_plane = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
            let gap = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - ra - rb;
            if off_plane.abs() < 1e-6 && gap > 0.05 && gap <= (2.0 * module(i).max(module(j))).max(0.2 * (ra + rb)) {
                out.push((i, j, gap));
            }
        }
    }
    out
}

/// The links the drawing suggests between the items (the gear crate's `suggest_links`, decision
/// 3), for a reply that names none.
fn local_suggestions(app: &SolveApp, k: &Gear) -> Vec<crate::dialogs_gear_graph::Suggested> {
    let items: Vec<gi::GearItem> = k.items.iter().enumerate().map(|(i, it)| it.crate_item(app, &crate::dialogs_gear_graph::item_id(i))).collect();
    gi::suggest_links(&items)
        .into_iter()
        .map(|s| crate::dialogs_gear_graph::Suggested { a: s.a.item, a_gear: s.a.gear, b: s.b.item, b_gear: s.b.gear, kind: s.kind.name().to_string() })
        .collect()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (l > 1e-12).then(|| v.map(|x| x / l))
}

/// A circle in the world: 49 points round `centre` square to `axis`.
fn ring_points(centre: [f64; 3], axis: [f64; 3], r: f64, h: f64) -> Vec<[f64; 3]> {
    let Some(ax) = unit(axis) else { return Vec::new() };
    let x = unit(cross(ax, if ax[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] })).unwrap_or([1.0, 0.0, 0.0]);
    let y = cross(ax, x);
    (0..=48)
        .map(|q| {
            let a = std::f64::consts::TAU * f64::from(q) / 48.0;
            [0, 1, 2].map(|j| centre[j] + r * (a.cos() * x[j] + a.sin() * y[j]) + h * ax[j])
        })
        .collect()
}

/// Each item's picked geometry in the world, to draw in its colour: a circle as its ring, a line
/// as itself, a point as a single point.
fn picked_geometry(app: &SolveApp, k: &Gear) -> Vec<(usize, Vec<[f64; 3]>, String)> {
    let mut out = Vec::new();
    for (i, it) in k.items.iter().enumerate() {
        let things = it.things(app);
        let slots = gi::assign(it.kind, &things).unwrap_or_default();
        let layouts = gi::slots(it.kind);
        for (n, t) in things.into_iter().enumerate() {
            let what = slots.get(n).and_then(|s| layouts.get(s.layout)?.get(s.slot)).map_or("", |s| s.what);
            let label = format!("{} · {what}", crate::dialogs_gear_graph::item_id(i));
            match t {
                gi::Thing::Circle { centre, normal, radius, .. } => out.push((i, ring_points(centre, normal, radius, 0.0), label)),
                gi::Thing::Line { a, b, .. } => out.push((i, vec![a, b], label)),
                gi::Thing::Point { p, .. } => out.push((i, vec![p], label)),
            }
        }
        // Its bore and hub circles ("A · bore").
        for (_, hub, t, _) in it.detail_things(app) {
            if let gi::Thing::Circle { centre, normal, radius, .. } = t {
                out.push((i, ring_points(centre, normal, radius, 0.0), format!("{} · {}", crate::dialogs_gear_graph::item_id(i), if hub { "hub" } else { "bore" })));
            }
        }
    }
    out
}

/// The housing cutout's preview: each gear's swept envelope (its tip circle plus the radial gap,
/// past its faces by the axial gap) as two rings to fill translucent, and each bearing point's
/// seat (the bearing's OD plus the fit, its width across) as rings round its shaft line in its
/// gear's colour.
fn cutout_preview(app: &SolveApp, k: &Gear) -> Vec<(Option<usize>, Vec<[f64; 3]>)> {
    let gap = |t: &str, d: f64| number(app, t, ValueKind::Length, "").ok().flatten().unwrap_or(d);
    let (radial, axial, fit) = (gap(&k.cutout.radial, 0.5), gap(&k.cutout.axial, 0.5), gap(&k.cutout.fit, 0.0));
    let replies = k.graph.replies();
    let mut out = Vec::new();
    for (i, it) in k.items.iter().enumerate() {
        let ci = it.crate_item(app, &crate::dialogs_gear_graph::item_id(i));
        for (g, geo) in gi::gear_geoms(&ci).iter().enumerate() {
            let (Some(r), Some(ax)) = (geo.radius, unit(geo.axis)) else { continue };
            let m = replies.iter().find(|x| (x.item, x.gear) == (i, g)).map(|x| x.module).filter(|m| *m > 0.0).unwrap_or(1.0);
            let (fa, fb) = gi::face_extent(&ci.settings(g), m);
            let reach = if geo.ring { r + 2.5 * m } else { r + m } + radial;
            out.push((None, ring_points(geo.centre, ax, reach, fa + axial)));
            out.push((None, ring_points(geo.centre, ax, reach, -fb - axial)));
        }
        // Its bearing points' seats, round its shaft line.
        let Some((_, od, width)) = k.cutout.bearing_size(app).map(|b| (b.1, b.2, b.3)) else { continue };
        let things = it.things(app);
        let Some(dir) = things.iter().find_map(|t| match t {
            gi::Thing::Line { a, b, .. } => unit([b[0] - a[0], b[1] - a[1], b[2] - a[2]]),
            _ => None,
        }) else {
            continue;
        };
        for t in &things {
            if let gi::Thing::Point { p, .. } = t {
                for h in [-width / 2.0, 0.0, width / 2.0] {
                    out.push((Some(i), ring_points(*p, dir, (od + fit) / 2.0, h)));
                }
            }
        }
    }
    out
}

/// The items' outline drawn while the dialog is open, from their picked geometry and what the
/// engine worked out (the plan gives no 2D preview): each gear's pitch circle at both faces
/// (decision 4: side A along its axis, side B against it) with four lines between, its tip and
/// root circles once its module is known; each mesh link's contact; each gear's side-A arrow.
#[allow(clippy::type_complexity)]
fn items_preview(app: &SolveApp, k: &Gear) -> (Vec<(usize, Vec<[f64; 3]>)>, Vec<[[f64; 3]; 2]>, Vec<(usize, [[f64; 3]; 2])>) {
    let at = |c: [f64; 3], d: [f64; 3], s: f64| [0, 1, 2].map(|i| c[i] + d[i] * s);
    let replies = k.graph.replies();
    let geoms: Vec<(gi::GearItem, Vec<gi::GearGeom>)> = k
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let ci = it.crate_item(app, &crate::dialogs_gear_graph::item_id(i));
            let g = gi::gear_geoms(&ci);
            (ci, g)
        })
        .collect();
    let blocks: Vec<gi::ModuleBlock> = Vec::new();
    let (mut lines, mut contacts, mut arrows) = (Vec::new(), Vec::new(), Vec::new());
    let mut faces = BTreeMap::new();
    for (i, (ci, gs)) in geoms.iter().enumerate() {
        for (g, geo) in gs.iter().enumerate() {
            let s = ci.settings(g);
            let reply = replies.iter().find(|r| (r.item, r.gear) == (i, g));
            let m = reply.map(|r| r.module).filter(|m| *m > 0.0).or(gi::resolve_module(&blocks, i, &s).module).unwrap_or(1.0);
            let (fa, fb) = gi::face_extent(&s, m);
            faces.insert((i, g), (fa, fb));
            let Some(ax) = unit(geo.axis) else { continue };
            let reach = geo.radius.unwrap_or(5.0 * m);
            // Side A as the engine says it (its side_a), else the axis read from the picks.
            let side = k.built.iter().find(|b| (b.item, b.gear) == (i, g)).and_then(|b| b.side_a).and_then(unit).unwrap_or(ax);
            arrows.push((i, [geo.centre, at(geo.centre, side, (0.6 * reach).max(fa + 2.0 * m))]));
            let ax = side;
            let Some(r) = geo.radius else {
                // A rack or a worm: its line.
                if let Some((a, b)) = geo.line {
                    lines.push((i, vec![a, b]));
                }
                continue;
            };
            lines.push((i, ring_points(geo.centre, ax, r, fa)));
            lines.push((i, ring_points(geo.centre, ax, r, -fb)));
            if reply.is_some() {
                let shift = reply.and_then(|r| r.profile_shift).unwrap_or(0.0) * m;
                let (tip, root) = if geo.ring { (r - m + shift, r + 1.25 * m + shift) } else { (r + m + shift, r - 1.25 * m + shift) };
                for d in [tip, root] {
                    if d > 0.0 {
                        lines.push((i, ring_points(geo.centre, ax, d, fa)));
                        lines.push((i, ring_points(geo.centre, ax, d, -fb)));
                    }
                }
            }
            let x = unit(cross(ax, if ax[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] })).unwrap_or([1.0, 0.0, 0.0]);
            let y = cross(ax, x);
            for q in 0..4 {
                let a = std::f64::consts::FRAC_PI_2 * f64::from(q);
                let p = [0, 1, 2].map(|j| geo.centre[j] + r * (a.cos() * x[j] + a.sin() * y[j]));
                lines.push((i, vec![at(p, ax, fa), at(p, ax, -fb)]));
            }
        }
    }
    // Each mesh link's contact: where the two pitch circles touch, across the faces.
    for (a, ag, b, bg, kind) in k.graph.links() {
        if kind != "mesh" {
            continue;
        }
        let (Some(ga), Some(gb)) = (geoms.get(a).and_then(|g| g.1.get(ag)), geoms.get(b).and_then(|g| g.1.get(bg))) else { continue };
        let Some(ax) = unit(ga.axis) else { continue };
        let (base, other) = if gb.ring { (gb, ga) } else { (ga, gb) };
        let (Some(r), Some(_)) = (base.radius, other.radius) else { continue };
        let Some(dir) = unit([0, 1, 2].map(|j| other.centre[j] - base.centre[j])) else { continue };
        let p = at(base.centre, dir, r);
        let (fa, fb) = faces.get(&(a, ag)).copied().unwrap_or((0.0, 0.0));
        contacts.push([at(p, ax, fa), at(p, ax, -fb)]);
    }
    (lines, contacts, arrows)
}

/// Each gear's outline `gear.calculate` gives (`outlines`): pitch, tip and root circles at its
/// back and front faces with four lines between the pitch circles; a bevel's pitch cone from
/// its heel to its toe (towards its apex). By the outline's `item` (else its `index`).
fn outlines_of(r: &Value) -> Vec<(usize, Vec<[f64; 3]>)> {
    let p3 = |v: Option<&Value>| -> Option<[f64; 3]> {
        let a = v?.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
    };
    let mut out = Vec::new();
    for o in r.get("outlines").and_then(Value::as_array).into_iter().flatten() {
        let (Some(c), Some(ax)) = (p3(o.get("center")), p3(o.get("axis"))) else { continue };
        let num = |k: &str| o.get(k).and_then(Value::as_f64);
        let member = o.get("item").and_then(crate::dialogs_gear_graph::item_index).or_else(|| o.get("index").and_then(Value::as_u64).map(|i| i as usize)).unwrap_or(0);
        let Some(z) = unit(ax) else { continue };
        let x0 = if z[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let Some(x) = unit(cross(cross(z, x0), z)) else { continue };
        let y = cross(z, x);
        let at = |r: f64, a: f64, h: f64| [0, 1, 2].map(|i| c[i] + r * (a.cos() * x[i] + a.sin() * y[i]) + h * z[i]);
        let ring = |r: f64, h: f64| (0..=48).map(|i| at(r, std::f64::consts::TAU * f64::from(i) / 48.0, h)).collect::<Vec<_>>();
        let fw = num("face_width").unwrap_or(0.0);
        let pitch = num("pitch_diameter").unwrap_or(0.0) / 2.0;
        let toe = num("toe_pitch_diameter").map(|d| d / 2.0);
        let apex = p3(o.get("apex"));
        let (h1, r1) = match (toe, apex) {
            (Some(t), Some(a)) => {
                let along = [0, 1, 2].map(|i| a[i] - c[i]).iter().zip(z).map(|(d, zz)| d * zz).sum::<f64>();
                (along * (1.0 - t / pitch.max(1e-9)), t)
            }
            _ => (fw, pitch),
        };
        for key in ["tip_diameter", "root_diameter"] {
            if let Some(d) = num(key) {
                out.push((member, ring(d / 2.0, 0.0)));
                if toe.is_none() {
                    out.push((member, ring(d / 2.0, fw)));
                }
            }
        }
        out.push((member, ring(pitch, 0.0)));
        out.push((member, ring(r1, h1)));
        for q in 0..4 {
            let a = std::f64::consts::FRAC_PI_2 * f64::from(q);
            out.push((member, vec![at(pitch, a, 0.0), at(r1, a, h1)]));
        }
    }
    out
}

/// Each mesh's contact `gear.calculate` gives (`contacts`: {a, b} in the world; a point when they
/// are the same).
fn contacts_of(r: &Value) -> Vec<[[f64; 3]; 2]> {
    let p3 = |v: &Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
    };
    r.get("contacts").and_then(Value::as_array).into_iter().flatten().filter_map(|c| Some([p3(c.get("a")?)?, p3(c.get("b").unwrap_or(c.get("a")?))?])).collect()
}

/// What is drawn over the viewport while the dialog is open: the outline in each gear's colour,
/// each gear's picked geometry (thicker; the hovered card's thickest), each mesh's contact, and
/// the active gear's side-A arrows.
pub fn overlay(painter: &egui::Painter, proj: &crate::viewport::Proj, k: &Gear) {
    let to = |p: &[f64; 3]| proj.to_screen(solidcore_record::Vec3::new(p[0], p[1], p[2]));
    let lit = k.card_hovered.or(k.graph_hovered);
    for (item, line) in &k.shape {
        let pts: Vec<egui::Pos2> = line.iter().filter_map(to).collect();
        if pts.len() == line.len() && pts.len() >= 2 {
            let color = if lit.is_some_and(|l| l != *item) { item_color(*item).gamma_multiply(0.35) } else { item_color(*item) };
            painter.add(egui::Shape::line(pts, egui::Stroke::new(if line.len() == 2 { 1.0 } else { 1.4 }, color)));
        }
    }
    let mut placed: Vec<egui::Rect> = Vec::new();
    for (item, line, label) in &k.picked {
        let width = if lit == Some(*item) { 5.0 } else if k.active == Some(*item) { 3.0 } else { 2.2 };
        let color = item_color(*item);
        let pts: Vec<egui::Pos2> = line.iter().filter_map(to).collect();
        if pts.len() != line.len() || pts.is_empty() {
            continue;
        }
        match pts.as_slice() {
            [p] => {
                painter.circle_filled(*p, width + 2.0, color);
            }
            _ => {
                painter.add(egui::Shape::line(pts.clone(), egui::Stroke::new(width, color)));
            }
        }
        // Which gear and which of its slots this sketch piece is ("B · shaft line"), by its
        // highest point on screen (a line: its middle). With many gears, only the active or
        // hovered one's, so the labels don't pile up.
        if k.items.len() > 4 && k.active != Some(*item) && lit != Some(*item) {
            continue;
        }
        let at =if pts.len() == 2 { pts[0] + (pts[1] - pts[0]) * 0.5 } else { pts.iter().copied().fold(pts[0], |a, p| if p.y < a.y { p } else { a }) };
        let font = egui::FontId::proportional(11.0);
        let g = painter.layout_no_wrap(label.clone(), font, color);
        let mut r = egui::Align2::CENTER_BOTTOM.anchor_size(at - egui::vec2(0.0, 4.0), g.size()).expand2(egui::vec2(4.0, 1.0));
        // (Labels that would overlap are stacked upwards.)
        for _ in 0..8 {
            if !placed.iter().any(|p: &egui::Rect| p.intersects(r)) {
                break;
            }
            r = r.translate(egui::vec2(0.0, -(r.height() + 2.0)));
        }
        placed.push(r);
        painter.rect_filled(r, 3.0, egui::Color32::from_black_alpha(170));
        painter.galley(r.min + egui::vec2(4.0, 1.0), g, color);
    }
    // The housing cutout: the swept envelopes filled translucent, the seats in their gears' colours.
    for (item, line) in &k.cutout_rings {
        let pts: Vec<egui::Pos2> = line.iter().filter_map(to).collect();
        if pts.len() != line.len() || pts.len() < 3 {
            continue;
        }
        match item {
            None => {
                let grey = egui::Color32::from_rgba_unmultiplied(170, 180, 195, 38);
                painter.add(egui::Shape::convex_polygon(pts.clone(), grey, egui::Stroke::NONE));
                painter.add(egui::Shape::line(pts, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(170, 180, 195, 120))));
            }
            Some(i) => {
                painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0, item_color(*i))));
            }
        }
    }
    // Each mesh's contact: highlighted.
    let contact = egui::Color32::from_rgb(255, 210, 40);
    for [a, b] in &k.contacts {
        if let (Some(pa), Some(pb)) = (to(a), to(b)) {
            if pa.distance(pb) < 1.0 {
                painter.circle_filled(pa, 4.0, contact);
            } else {
                painter.line_segment([pa, pb], egui::Stroke::new(3.0, contact));
            }
        }
    }
    // The active gear's side A (decision 4): an arrow from its centre along its axis, marked "A".
    let side_a = egui::Color32::from_rgb(60, 200, 120);
    for (item, [a, b]) in &k.arrows {
        if k.active != Some(*item) && lit != Some(*item) {
            continue;
        }
        if let (Some(pa), Some(pb)) = (to(a), to(b))
            && pa.distance(pb) > 2.0
        {
            painter.arrow(pa, pb - pa, egui::Stroke::new(2.0, side_a));
            painter.text(pb + (pb - pa).normalized() * 8.0, egui::Align2::CENTER_CENTER, "A", egui::FontId::proportional(12.0), side_a);
        }
    }
}

/// Once a frame: what a Gear dialog asked for, and the dialog back on a sketch opened from it
/// once its editing ends.
pub fn tick(app: &mut SolveApp) {
    let asked = match app.dialog.as_mut() {
        Some(d) => match &mut d.kind {
            Kind::Gear(g) => g.action.take().map(|a| (a, g.clone())),
            _ => None,
        },
        None => None,
    };
    if let Some((Action::EditSketch(sketch), g)) = asked {
        crate::dialogs::cancel(app);
        app.dialog = None;
        app.gear_after_sketch = Some(AfterSketch { sketch, opened: false, dialog: g });
        app.edit_sketch(sketch);
        return;
    }
    // The dialog back once the sketch opened from it is finished.
    let Some(after) = app.gear_after_sketch.as_mut() else { return };
    if matches!(app.dialog.as_ref().map(|d| &d.kind), Some(Kind::Gear(_))) {
        app.gear_after_sketch = None;
        return;
    }
    if app.session.active_sketch == Some(after.sketch) {
        after.opened = true;
        return;
    }
    if after.opened && app.session.active_sketch.is_none() && app.dialog.is_none() && app.tool.is_none() {
        app.start("gear.create");
        app.gear_after_sketch = None;
    }
}

#[cfg(test)]
#[path = "dialogs_gear_tests.rs"]
mod tests;
