//! The Gears dialog's node graph (GEAR_UI_SPEC.md §4); the items it links are in
//! `dialogs_gear_items`.
//!
//! Each item is a node of the graph (drawn with egui-snarl, vendored), one row per gear, each
//! with a Mesh pin and a Shaft pin: mesh to mesh says two gears mesh, shaft to shaft that they
//! share a shaft; an item's Module pin into a Module Block says its gears share that block's
//! module. Links the drawing implies come back from `gear.calculate` and are accepted with one
//! button; each link shows its ratio or its error. The items, links, blocks and the driver are
//! sent as `gear.create`'s `items`, `links`, `module_blocks` and `drive` (§6).

use egui_snarl::ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget};
use egui_snarl::{InPin, InPinId, NodeId, OutPin, OutPinId, Snarl};
use serde_json::{Value, json};

use crate::SolveApp;
pub use crate::dialogs_gear_items::{GearItem, GearSettings, ITEM_TYPES, Slot, item_id, item_index, item_settings, items_request};

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
