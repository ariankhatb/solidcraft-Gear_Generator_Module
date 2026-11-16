//! The Gears dialog's items and their node graph.
//!
//! A gear item is one thing the generator makes, defined by the geometry picked for it: a spur
//! gear by one circle, a compound gear by several circles and the line they share, a bevel,
//! hypoid or worm pair by both its circles (or shaft lines) at once. Pick a type, press Create
//! New, then pick the item's geometry: its row lists what the type still needs.
//!
//! Each item is a node of the graph (drawn with egui-snarl, vendored): its Mesh pins meshed to
//! another item's say the two mesh, its Shaft pins joined say they share a shaft, and its Module
//! pin into a Module Block (a group node) says the gears in it share one module. Links the
//! drawing suggests (tangent circles) are listed to accept; what `gear.calculate` says of each
//! link (can't mesh at that distance or angle) is shown under the graph. The items, links and
//! blocks are sent as `gear.create`'s `items`, `links` and `module_blocks`.

use egui_snarl::ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget};
use egui_snarl::{InPin, NodeId, OutPin, Snarl};
use serde_json::{Value, json};
use solidcraft_engine::Sel;

use crate::SolveApp;
use crate::dialogs::Kind;

/// What an item can be: `gear.create`'s item types, with their labels.
pub const ITEM_TYPES: [(&str, &str); 10] = [
    ("spur", "Spur"),
    ("helical", "Helical"),
    ("herringbone", "Herringbone"),
    ("internal", "Ring Gear"),
    ("rack", "Rack"),
    ("compound", "Compound"),
    ("bevel", "Bevel Pair"),
    ("spiral_bevel", "Spiral Bevel Pair"),
    ("hypoid", "Hypoid Pair"),
    ("worm", "Worm and Wheel"),
];

/// What an item of type `t` is made from: at least this many circles and lines.
fn needs(t: &str) -> (usize, usize) {
    match t {
        "compound" => (2, 1),
        "bevel" | "spiral_bevel" | "hypoid" | "worm" => (2, 0),
        _ => (1, 0),
    }
}

/// One gear item: its type (an index into [`ITEM_TYPES`]) and the geometry picked for it.
#[derive(Clone, Debug, PartialEq)]
pub struct GearItem {
    pub kind: usize,
    pub picks: Vec<Sel>,
}

impl GearItem {
    pub fn type_name(&self) -> &'static str {
        ITEM_TYPES.get(self.kind).map_or("spur", |t| t.0)
    }

    /// What the item still needs, as a checklist: (what, done).
    pub fn checklist(&self, app: &SolveApp) -> Vec<(String, bool)> {
        let (circles, lines) = self.picks.iter().fold((0, 0), |(c, l), s| match crate::dialogs_gear::geo_kind(app, s) {
            Some(true) => (c + 1, l),
            Some(false) => (c, l + 1),
            None => (c, l),
        });
        let (nc, nl) = needs(self.type_name());
        let mut out = vec![(if nc == 1 { "a pitch circle".to_string() } else { format!("{nc} pitch circles (or shaft lines)") }, circles + lines >= nc && circles >= nc.min(1))];
        if nl > 0 {
            out.push(("the shaft line they share".to_string(), lines >= nl));
        }
        out
    }
}

/// A node of the graph: an item (by its index), or a module block (its module as typed; empty:
/// worked out).
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

/// An item node's pins: 0 Mesh, 1 Shaft (in and out), 2 Module (out only).
const MESH: usize = 0;
const SHAFT: usize = 1;
const MODULE: usize = 2;

impl Graph {
    /// The node of item `i`, made where there is room when it is new.
    pub fn node_of(&mut self, i: usize) -> NodeId {
        if let Some((id, _)) = self.0.node_ids().find(|(_, n)| **n == GNode::Item(i)) {
            return id;
        }
        let n = self.0.nodes().count() as f32;
        self.0.insert_node(egui::pos2(40.0 + 180.0 * (n % 4.0), 40.0 + 140.0 * (n / 4.0).floor()), GNode::Item(i))
    }

    fn item_of(&self, id: NodeId) -> Option<usize> {
        match self.0.get_node(id)? {
            GNode::Item(i) => Some(*i),
            GNode::Block { .. } => None,
        }
    }

    /// The links the wires make: (item, item, "mesh" | "shaft").
    pub fn links(&self) -> Vec<(usize, usize, &'static str)> {
        self.0
            .wires()
            .filter_map(|(o, i)| {
                let (a, b) = (self.item_of(o.node)?, self.item_of(i.node)?);
                let kind = match (o.output, i.input) {
                    (MESH, MESH) => "mesh",
                    (SHAFT, SHAFT) => "shaft",
                    _ => return None,
                };
                Some((a, b, kind))
            })
            .collect()
    }

    /// The module blocks: each its module as typed and the items wired into it.
    pub fn blocks(&self) -> Vec<(String, Vec<usize>)> {
        self.0
            .node_ids()
            .filter_map(|(id, n)| match n {
                GNode::Block { module } => {
                    let mut items: Vec<usize> = self.0.wires().filter(|(o, i)| i.node == id && o.output == MODULE).filter_map(|(o, _)| self.item_of(o.node)).collect();
                    items.sort_unstable();
                    Some((module.clone(), items))
                }
                GNode::Item(_) => None,
            })
            .collect()
    }

    /// A suggested link accepted: the wire it stands for.
    pub fn accept(&mut self, a: usize, b: usize, kind: &str) {
        let (na, nb) = (self.node_of(a), self.node_of(b));
        let pin = if kind == "shaft" { SHAFT } else { MESH };
        self.0.connect(egui_snarl::OutPinId { node: na, output: pin }, egui_snarl::InPinId { node: nb, input: pin });
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

/// What the dialog sends for its items: `items` (type and components, each a `selection`
/// locator), `links` from the wires, `module_blocks` from the blocks.
pub fn items_request(app: &SolveApp, items: &[GearItem], graph: &Graph) -> Result<serde_json::Map<String, Value>, String> {
    let mut list = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let components = it
            .picks
            .iter()
            .map(|s| crate::dialogs_gear::locate(app, s).ok_or_else(|| format!("item {}: a pick that can't be part of a gear", i + 1)))
            .collect::<Result<Vec<_>, _>>()?;
        list.push(json!({"index": i, "type": it.type_name(), "components": components}));
    }
    let links: Vec<Value> = graph.links().into_iter().map(|(a, b, kind)| json!({"a": a, "b": b, "kind": kind})).collect();
    let mut blocks = Vec::new();
    for (module, members) in graph.blocks() {
        let mut b = json!({"items": members});
        if !module.trim().is_empty() {
            b["module"] = json!(app.session.doc.eval(module.trim(), solidcraft_engine::doc::expr::Kind::Length).map_err(|e| format!("module block: {e}"))?);
        }
        blocks.push(b);
    }
    let mut p = serde_json::Map::new();
    p.insert("items".into(), Value::Array(list));
    p.insert("links".into(), Value::Array(links));
    p.insert("module_blocks".into(), Value::Array(blocks));
    Ok(p)
}

/// What `gear.calculate` says of each link and suggests (`links[]`: {a, b, kind, error?};
/// `suggested_links[]`: {a, b, kind}).
pub fn link_notes(r: &Value) -> (Vec<String>, Vec<(usize, usize, String)>) {
    let errors = r
        .get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|l| Some(format!("item {} – item {}: {}", l.get("a")?.as_u64()? + 1, l.get("b")?.as_u64()? + 1, l.get("error")?.as_str()?)))
        .collect();
    let suggested = r
        .get("suggested_links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|l| Some((l.get("a")?.as_u64()? as usize, l.get("b")?.as_u64()? as usize, l.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string())))
        .collect();
    (errors, suggested)
}

struct Viewer<'a> {
    items: &'a [GearItem],
}

impl SnarlViewer<GNode> for Viewer<'_> {
    fn title(&mut self, node: &GNode) -> String {
        match node {
            GNode::Item(i) => format!("{} · {}", i + 1, self.items.get(*i).map_or("", |it| ITEM_TYPES.get(it.kind).map_or("", |t| t.1))),
            GNode::Block { .. } => "Module Block".into(),
        }
    }

    fn inputs(&mut self, node: &GNode) -> usize {
        match node {
            GNode::Item(_) => 2,
            GNode::Block { .. } => 1,
        }
    }

    fn outputs(&mut self, node: &GNode) -> usize {
        match node {
            GNode::Item(_) => 3,
            GNode::Block { .. } => 0,
        }
    }

    fn show_input(&mut self, pin: &InPin, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) -> impl SnarlPin + 'static {
        match &mut snarl[pin.id.node] {
            GNode::Item(_) => {
                ui.label(if pin.id.input == MESH { "Mesh" } else { "Shaft" });
            }
            GNode::Block { module } => {
                ui.add(egui::TextEdit::singleline(module).desired_width(70.0).hint_text("module"));
            }
        }
        pin_info(pin.id.input)
    }

    fn show_output(&mut self, pin: &OutPin, ui: &mut egui::Ui, _snarl: &mut Snarl<GNode>) -> impl SnarlPin + 'static {
        ui.label(match pin.id.output {
            MESH => "Mesh",
            SHAFT => "Shaft",
            _ => "Module",
        });
        pin_info(pin.id.output)
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<GNode>) {
        // Mesh to mesh, shaft to shaft, module into a block: nothing else joins.
        let block = matches!(snarl.get_node(to.id.node), Some(GNode::Block { .. }));
        let fits = if block { from.id.output == MODULE } else { from.id.output == to.id.input && from.id.output != MODULE };
        if fits && from.id.node != to.id.node {
            snarl.connect(from.id, to.id);
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<GNode>) -> bool {
        true
    }

    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) {
        if ui.button("Module Block").clicked() {
            snarl.insert_node(pos, GNode::Block { module: String::new() });
            ui.close();
        }
    }
}

fn pin_info(pin: usize) -> PinInfo {
    let mut p = match pin {
        MESH => PinInfo::circle(),
        SHAFT => PinInfo::square(),
        _ => PinInfo::triangle(),
    };
    p.fill = Some(match pin {
        MESH => egui::Color32::from_rgb(70, 140, 220),
        SHAFT => egui::Color32::from_rgb(160, 160, 160),
        _ => egui::Color32::from_rgb(230, 140, 50),
    });
    p
}

/// The graph's window, beside the Gears dialog while it is open.
pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(d) = app.dialog.as_mut() else { return };
    let Kind::Gear(g) = &mut d.kind else { return };
    if g.items.is_empty() {
        return;
    }
    for i in 0..g.items.len() {
        g.graph.node_of(i);
    }
    let mut accept = None;
    egui::Window::new(solidcraft_ui_text::tr_label("Gear Graph")).id(egui::Id::unique("sc_gear_graph")).default_size(egui::vec2(520.0, 360.0)).resizable(true).show(ctx, |ui| {
        ui.label(egui::RichText::new("Drag Mesh to Mesh to mesh two items, Shaft to Shaft to put them on one shaft, Module into a Module Block (right-click: add one) to share a module").small().weak());
        let mut viewer = Viewer { items: &g.items };
        SnarlWidget::new().id_salt("sc_gear_graph_canvas").style(SnarlStyle::new()).show(&mut g.graph.0, &mut viewer, ui);
        for e in &g.link_errors {
            ui.label(egui::RichText::new(e).small().color(ui.visuals().warn_fg_color));
        }
        for (a, b, kind) in &g.suggested {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Suggested: item {} – item {} ({kind})", a + 1, b + 1)).small().italics());
                if ui.small_button("Accept").clicked() {
                    accept = Some((*a, *b, kind.clone()));
                }
            });
        }
    });
    if let Some((a, b, kind)) = accept {
        g.graph.accept(a, b, &kind);
    }
}
