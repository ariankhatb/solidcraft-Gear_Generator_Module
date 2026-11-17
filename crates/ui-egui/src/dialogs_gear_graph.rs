//! The Gears dialog's node graph (GEAR_UI_SPEC.md §4); the items it links are in
//! `dialogs_gear_items`.
//!
//! Each item is a node of the graph (drawn with egui-snarl, vendored), one row per gear, each
//! with a ● Mesh pin and a ■ Shaft pin: mesh to mesh says two gears mesh, shaft to shaft that they
//! share a shaft. A pair (bevel, hypoid, worm, crossed helical) has its two gears' mesh made
//! inside it, drawn as a solid inner link. A Module Block is a group frame round the items in it
//! (its header holds the module, or Auto): dragging an item into the frame puts it in the block,
//! dragging it out takes it out. What `gear.calculate` says of each link (its ratio, or its error
//! in red) is drawn on the link ([`Graph::note`]); the links the drawing suggests are drawn
//! dashed, accepted with one button or the right-click menu. Hovering a node lights its geometry,
//! hovering a link both of its contacts ([`GraphOut`]). The items, links, blocks and the driver
//! are sent as `gear.create`'s `items`, `links`, `module_blocks` and `drive` (§6, by
//! `dialogs_gear_items::items_request`).

use egui_snarl::ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget};
use egui_snarl::{InPin, InPinId, NodeId, OutPin, OutPinId, Snarl};
use serde_json::{Value, json};

pub use crate::dialogs_gear_items::{GearItem, GearSettings, item_id, item_index, items_request};
use solidcraft_gear::items::{GearReply, ItemKind};

const MESH_TIP: &str = "Drag to another gear's Mesh pin: they mesh. The distance between them is checked.";
const SHAFT_TIP: &str = "Drag to another gear's Shaft pin: they share a shaft and turn together.";
const BLOCK_TIP: &str = "Gears in this block share one module, so they can mesh with each other.";
const DRIVER_TIP: &str = "The gear you turn. Every other gear's speed and direction follow from it.";
const ACCEPT_TIP: &str = "Make every dashed link real. Dashed = circles the drawing shows as touching or on one line.";
const RATIO_TIP: &str = "Click two gear rows to see the ratio between them (Ratio, under the graph).";
const EMPTY: &str = "Items appear here. Drag from a ● Mesh pin to another to make them mesh.";

/// A ratio as a link shows it: "2.00" (of "2.00 : 1").
fn ratio_text(r: f64) -> String {
    format!("{r:.2}")
}

/// A module as a gear row shows it: "2", "1.25".
fn module_text(m: f64) -> String {
    let t = format!("{m:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A node of the graph: an item (by its index), or a module block (its module as typed, empty:
/// Auto, and the items in its frame).
#[derive(Clone, Debug, PartialEq)]
pub enum GNode {
    Item(usize),
    Block { module: String, members: Vec<usize> },
}

/// What the engine said of one link (`gear.calculate`'s `links`): its ends, and its ratio or its
/// error with what to do.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub a: usize,
    pub a_gear: usize,
    pub b: usize,
    pub b_gear: usize,
    pub kind: String,
    pub ratio: Option<f64>,
    pub error: Option<String>,
}

impl Note {
    /// Whether the note is about the link between these two gears (either way round).
    fn is(&self, a: (usize, usize), b: (usize, usize)) -> bool {
        ((self.a, self.a_gear) == a && (self.b, self.b_gear) == b) || ((self.a, self.a_gear) == b && (self.b, self.b_gear) == a)
    }
    /// The label drawn on the link ("2.00 : 1", or the error), and whether it is an error.
    pub fn label(&self) -> Option<(String, bool)> {
        match (&self.error, self.ratio) {
            (Some(e), _) => Some((e.clone(), true)),
            (None, Some(r)) => Some((format!("{} : 1", ratio_text(r)), false)),
            _ => None,
        }
    }
}

/// The graph, kept with the dialog (compared and printed by its nodes and wires only), with what
/// the engine last said of its links and where its pins and nodes were last drawn.
#[derive(Clone, Default)]
pub struct Graph {
    snarl: Snarl<GNode>,
    notes: Vec<Note>,
    suggested: Vec<Suggested>,
    /// What the engine worked out for each gear (`gear.calculate`'s `gears`).
    replies: Vec<GearReply>,
    /// Where each node and pin was drawn last (screen): nodes by id, pins by (node, output?, pin).
    rects: Vec<(NodeId, egui::Rect)>,
    pins: Vec<((NodeId, bool, usize), egui::Pos2)>,
    /// The nodes' rects when a drag began: a node that moved has been dragged.
    pressed: Option<Vec<(NodeId, egui::Rect)>>,
}

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph").field("nodes", &self.snarl.nodes().count()).field("wires", &self.snarl.wires().count()).finish()
    }
}

impl PartialEq for Graph {
    fn eq(&self, other: &Self) -> bool {
        let wires = |g: &Graph| g.snarl.wires().map(|(o, i)| (o.node.0, o.output, i.node.0, i.input)).collect::<Vec<_>>();
        self.snarl.nodes().collect::<Vec<_>>() == other.snarl.nodes().collect::<Vec<_>>() && wires(self) == wires(other)
    }
}

/// A gear row's pins: 2 × gear (Mesh), 2 × gear + 1 (Shaft), the same on both sides of a node.
pub fn pin(gear: usize, shaft: bool) -> usize {
    2 * gear + usize::from(shaft)
}

/// The meshes inside an item of `kind` making `gears` gears (drawn as solid inner links): a
/// pair's two gears, a ring gear or rack with its pinion, a planetary set's sun–planet and
/// planet–ring.
fn inner_meshes(kind: ItemKind, gears: usize) -> &'static [(usize, usize)] {
    match kind {
        ItemKind::Bevel | ItemKind::Hypoid | ItemKind::Worm | ItemKind::CrossedHelical => &[(0, 1)],
        ItemKind::Internal | ItemKind::Rack if gears >= 2 => &[(0, 1)],
        ItemKind::Planetary => &[(0, 2), (2, 1)],
        _ => &[],
    }
}

/// `gear.calculate`'s `gears` (GEAR_UI_SPEC §6): per gear its `item` (an id or index), `gear`,
/// `teeth`, `module`, `pitch_d`, `ratio`, `rpm`, `torque`, `contact_ratio`, `profile_shift` and
/// `warnings`, as the gear crate's [`GearReply`].
pub fn gear_replies(r: &Value) -> Vec<GearReply> {
    r.get("gears")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let (item, gear) = end(Some(g));
            let num = |k: &str| g.get(k).and_then(Value::as_f64);
            Some(GearReply {
                item,
                gear,
                teeth: num("teeth").map_or(0, |z| z.round().max(0.0) as u32),
                module: num("module").unwrap_or(0.0),
                pitch_d: num("pitch_d").or_else(|| num("pitch_diameter")).unwrap_or(0.0),
                ratio: num("ratio"),
                rpm: num("rpm"),
                torque: num("torque"),
                contact_ratio: num("contact_ratio"),
                profile_shift: num("profile_shift"),
                warnings: g.get("warnings").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
            })
            .filter(|_| g.is_object())
        })
        .collect()
}

/// A gear row's text once worked out: "z20 m2 Ø40 · 0.50 : 1 · 500 rpm" (`len` shows a length in
/// the design's units).
pub fn reply_text(r: &GearReply, len: &dyn Fn(f64) -> String) -> String {
    let mut t = Vec::new();
    if r.teeth > 0 {
        t.push(format!("z{} m{}", r.teeth, module_text(r.module)));
    }
    if r.pitch_d > 0.0 {
        t.push(format!("Ø{}", len(r.pitch_d)));
    }
    if let Some(x) = r.ratio {
        t.push(format!("{} : 1", ratio_text(x)));
    }
    if let Some(n) = r.rpm {
        t.push(format!("{} rpm", crate::dialogs_gear::fmt((n * 10.0).round() / 10.0)));
    }
    t.join(" · ")
}

/// A gear's hover text: torque, contact ratio, profile shift and warnings.
pub fn reply_tip(r: &GearReply) -> String {
    let mut t = Vec::new();
    if let Some(v) = r.torque {
        t.push(format!("Torque {} N·m", crate::dialogs_gear::fmt(v)));
    }
    if let Some(v) = r.contact_ratio {
        t.push(format!("Contact ratio {}", crate::dialogs_gear::fmt(v)));
    }
    if let Some(v) = r.profile_shift {
        t.push(format!("Profile shift {}", crate::dialogs_gear::fmt(v)));
    }
    t.extend(r.warnings.iter().map(|w| format!("⚠ {w}")));
    t.join("\n")
}

/// Where the next new node goes: a grid, by how many nodes there are.
fn next_place(n: usize) -> egui::Pos2 {
    let n = n as f32;
    egui::pos2(40.0 + 200.0 * (n % 3.0), 30.0 + 160.0 * (n / 3.0).floor())
}

impl Graph {
    /// The node of item `i`, made where there is room when it is new.
    pub fn node_of(&mut self, i: usize) -> NodeId {
        if let Some(id) = self.item_node(i) {
            return id;
        }
        let at = next_place(self.snarl.nodes().count());
        self.snarl.insert_node(at, GNode::Item(i))
    }

    fn item_of(&self, id: NodeId) -> Option<usize> {
        match self.snarl.get_node(id)? {
            GNode::Item(i) => Some(*i),
            GNode::Block { .. } => None,
        }
    }

    fn item_node(&self, i: usize) -> Option<NodeId> {
        self.snarl.node_ids().find(|(_, n)| **n == GNode::Item(i)).map(|(id, _)| id)
    }

    /// The links the wires make: (item, gear, item, gear, "mesh" | "shaft").
    pub fn links(&self) -> Vec<(usize, usize, usize, usize, &'static str)> {
        let mut out: Vec<_> = self
            .snarl
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

    /// The module blocks, in the order they were made: each its module as typed and the items in
    /// its frame.
    pub fn blocks(&self) -> Vec<(String, Vec<usize>)> {
        self.snarl
            .node_ids()
            .filter_map(|(_, n)| match n {
                GNode::Block { module, members } => {
                    let mut items = members.clone();
                    items.sort_unstable();
                    items.dedup();
                    Some((module.clone(), items))
                }
                GNode::Item(_) => None,
            })
            .collect()
    }

    /// A new Module Block (its module as typed; empty: Auto): its index among the blocks.
    pub fn add_block(&mut self, module: &str) -> usize {
        let at = next_place(self.snarl.nodes().count());
        self.snarl.insert_node(at, GNode::Block { module: module.to_string(), members: Vec::new() });
        self.blocks().len() - 1
    }

    /// Item `item` put in block `block` (an index among the blocks), and out of any other.
    pub fn put_in_block(&mut self, block: usize, item: usize) {
        let mut k = 0;
        for n in self.snarl.nodes_mut() {
            if let GNode::Block { members, .. } = n {
                members.retain(|m| *m != item);
                if k == block {
                    members.push(item);
                }
                k += 1;
            }
        }
    }

    /// A suggested link accepted: the wire it stands for.
    pub fn accept(&mut self, link: &Suggested) {
        let (na, nb) = (self.node_of(link.a), self.node_of(link.b));
        let shaft = link.kind == "shaft";
        self.snarl.connect(OutPinId { node: na, output: pin(link.a_gear, shaft) }, InPinId { node: nb, input: pin(link.b_gear, shaft) });
        self.suggested.retain(|s| s != link);
    }

    /// The link between these two gears taken away (either way round).
    pub fn remove_link(&mut self, a: (usize, usize), b: (usize, usize), kind: &str) {
        let shaft = kind == "shaft";
        let ends = |o: &OutPinId, i: &InPinId| -> Option<((usize, usize), (usize, usize))> {
            Some(((self.item_of(o.node)?, o.output / 2), (self.item_of(i.node)?, i.input / 2)))
        };
        let wires: Vec<(OutPinId, InPinId)> = self
            .snarl
            .wires()
            .filter(|(o, i)| (o.output % 2 == 1) == shaft && ends(o, i).is_some_and(|(x, y)| (x == a && y == b) || (x == b && y == a)))
            .collect();
        for (o, i) in wires {
            self.snarl.disconnect(o, i);
        }
    }

    /// Every suggested link accepted.
    pub fn accept_all(&mut self) {
        for s in self.suggested.clone() {
            self.accept(&s);
        }
    }

    /// The links the drawing suggests that aren't links yet.
    pub fn suggested(&self) -> &[Suggested] {
        &self.suggested
    }

    /// Item `i` taken out: its node and wires go, it leaves its block, and later items are
    /// renumbered.
    pub fn remove_item(&mut self, i: usize) {
        if let Some(id) = self.item_node(i) {
            self.snarl.remove_node(id);
        }
        for n in self.snarl.nodes_mut() {
            match n {
                GNode::Item(j) if *j > i => *j -= 1,
                GNode::Block { members, .. } => {
                    members.retain(|m| *m != i);
                    for m in members.iter_mut().filter(|m| **m > i) {
                        *m -= 1;
                    }
                }
                GNode::Item(_) => {}
            }
        }
        self.suggested.retain(|s| s.a != i && s.b != i);
        self.notes.retain(|n| n.a != i && n.b != i);
    }

    /// What `gear.calculate` says (its reply), drawn on the graph: each link's ratio or error, the
    /// links the drawing suggests (less those already made), and each gear's teeth and module.
    pub fn note(&mut self, r: &Value) {
        self.notes = notes_of(r);
        self.suggest(suggested_of(r));
        self.replies = gear_replies(r);
    }

    /// The links the drawing suggests, less those already made (drawn dashed).
    pub fn suggest(&mut self, list: Vec<Suggested>) {
        let made = self.links();
        self.suggested = list
            .into_iter()
            .filter(|s| !made.iter().any(|(a, ag, b, bg, _)| (*a, *ag, *b, *bg) == (s.a, s.a_gear, s.b, s.b_gear) || (*b, *bg, *a, *ag) == (s.a, s.a_gear, s.b, s.b_gear)))
            .collect();
    }

    /// What the engine worked out for each gear, as last given ([`Graph::note`]).
    pub fn replies(&self) -> &[GearReply] {
        &self.replies
    }

    /// The engine's notes on the links, as last given ([`Graph::note`]).
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    fn pin_at(&self, node: NodeId, output: bool, p: usize) -> Option<egui::Pos2> {
        self.pins.iter().find(|(k, _)| *k == (node, output, p)).map(|(_, q)| *q)
    }

    fn rect_of(&self, node: NodeId) -> Option<egui::Rect> {
        self.rects.iter().find(|(n, _)| *n == node).map(|(_, r)| *r)
    }

    /// A block's frame: its header node and the items in it, with a margin (screen, as last
    /// drawn), leaving out item `but`.
    fn frame(&self, block: NodeId, but: Option<usize>) -> Option<egui::Rect> {
        let GNode::Block { members, .. } = self.snarl.get_node(block)? else { return None };
        let mut r = self.rect_of(block)?;
        for m in members.iter().filter(|m| Some(**m) != but) {
            if let Some(q) = self.item_node(*m).and_then(|n| self.rect_of(n)) {
                r = r.union(q);
            }
        }
        Some(r.expand(12.0))
    }

    /// After a drag: every item node that moved is in the block whose frame it was dropped in, or
    /// in none.
    fn settle_blocks(&mut self, before: &[(NodeId, egui::Rect)]) {
        let blocks: Vec<NodeId> = self.snarl.node_ids().filter(|(_, n)| matches!(n, GNode::Block { .. })).map(|(id, _)| id).collect();
        let moved: Vec<(usize, egui::Pos2)> = self
            .rects
            .iter()
            .filter_map(|(id, r)| {
                let i = self.item_of(*id)?;
                let was = before.iter().find(|(n, _)| n == id).map(|(_, q)| *q)?;
                ((was.center() - r.center()).length() > 2.0).then_some((i, r.center()))
            })
            .collect();
        for (item, at) in moved {
            let into = blocks.iter().position(|b| self.frame(*b, Some(item)).is_some_and(|f| f.contains(at)));
            for (k, b) in blocks.iter().enumerate() {
                if let Some(GNode::Block { members, .. }) = self.snarl.get_node_mut(*b) {
                    members.retain(|m| *m != item);
                    if Some(k) == into {
                        members.push(item);
                    }
                }
            }
        }
    }
}

/// A link the drawing suggests (`gear.calculate`'s `suggestions`).
#[derive(Clone, Debug, PartialEq)]
pub struct Suggested {
    pub a: usize,
    pub a_gear: usize,
    pub b: usize,
    pub b_gear: usize,
    pub kind: String,
}

/// A link's end: {item, gear} (or the item alone: gear 0).
fn end(v: Option<&Value>) -> (usize, usize) {
    match v {
        Some(Value::Object(o)) => (o.get("item").and_then(item_index).unwrap_or(0), o.get("gear").or(o.get("port")).and_then(Value::as_u64).unwrap_or(0) as usize),
        Some(other) => (item_index(other).unwrap_or(0), 0),
        None => (0, 0),
    }
}

/// The reply's links, each with its ratio or its error (and what to do, after a colon).
fn notes_of(r: &Value) -> Vec<Note> {
    r.get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|l| {
            let ((a, a_gear), (b, b_gear)) = (end(l.get("a")), end(l.get("b")));
            let error = l.get("error").and_then(Value::as_str).map(|e| {
                let hint = l.get("hint").and_then(Value::as_str).map(|h| format!(": {h}")).unwrap_or_default();
                format!("{e}{hint}")
            });
            Note { a, a_gear, b, b_gear, kind: l.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string(), ratio: l.get("ratio").and_then(Value::as_f64), error }
        })
        .collect()
}

/// The reply's suggested links: objects with their ends, or sentences ("item A gear 1 and item B
/// gear 1 are tangent: link them?", gears counted from 1).
fn suggested_of(r: &Value) -> Vec<Suggested> {
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
    r.get("suggestions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| match v {
            Value::String(s) => parse(s),
            Value::Object(_) => {
                let ((a, a_gear), (b, b_gear)) = (end(v.get("a")), end(v.get("b")));
                Some(Suggested { a, a_gear, b, b_gear, kind: v.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string() })
            }
            _ => None,
        })
        .collect()
}

/// What `gear.calculate` says of each link, as lines for the dialog ("1·1 – 2·2: 2.00 : 1", or
/// the error and what to do: red), and the links it suggests.
pub fn link_notes(r: &Value) -> (Vec<(String, bool)>, Vec<Suggested>) {
    let notes = notes_of(r)
        .iter()
        .filter_map(|n| {
            let name = format!("{}·{} – {}·{}", n.a + 1, n.a_gear + 1, n.b + 1, n.b_gear + 1);
            n.label().map(|(text, bad)| (format!("{name}: {text}"), bad))
        })
        .collect();
    (notes, suggested_of(r))
}

struct Viewer<'a> {
    items: &'a [GearItem],
    gears: &'a [usize],
    replies: &'a [GearReply],
    len: &'a dyn Fn(f64) -> String,
    /// The two gears picked for the ratio, and a gear row clicked now.
    picked: [Option<(usize, usize)>; 2],
    clicked: Option<(usize, usize)>,
    suggested: usize,
    driver: Option<(usize, usize)>,
    hovered: Option<usize>,
    make_driver: Option<(usize, usize)>,
    accept_all: bool,
    rects: Vec<(NodeId, egui::Rect)>,
    pins: Vec<((NodeId, bool, usize), egui::Pos2)>,
}

impl Viewer<'_> {
    fn gears_of(&self, node: &GNode) -> usize {
        match node {
            GNode::Item(i) => self.gears.get(*i).copied().unwrap_or(1).max(1),
            GNode::Block { .. } => 0,
        }
    }

    /// A gear row's label (its number, and what it came to once worked out) and its hover text.
    fn row(&self, item: usize, gear: usize) -> (String, String) {
        match self.replies.iter().find(|r| (r.item, r.gear) == (item, gear)) {
            Some(r) => {
                let warn = if r.warnings.is_empty() { "" } else { " ⚠" };
                (format!("{} · {}{warn}", gear + 1, reply_text(r, self.len)), reply_tip(r))
            }
            None => (format!("{}", gear + 1), String::new()),
        }
    }
}

impl SnarlViewer<GNode> for Viewer<'_> {
    fn title(&mut self, node: &GNode) -> String {
        match node {
            GNode::Item(i) => {
                let badge = if self.driver.is_some_and(|(d, _)| d == *i) { "▶ " } else { "" };
                format!("{badge}{} · {}", item_id(*i), self.items.get(*i).map_or("", GearItem::label))
            }
            GNode::Block { .. } => "Module Block".into(),
        }
    }

    fn inputs(&mut self, node: &GNode) -> usize {
        2 * self.gears_of(node)
    }

    fn outputs(&mut self, node: &GNode) -> usize {
        2 * self.gears_of(node)
    }

    fn show_input(&mut self, pin: &InPin, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) -> impl SnarlPin + 'static {
        let shaft = pin.id.input % 2 == 1;
        if let GNode::Item(i) = &snarl[pin.id.node] {
            let gear = (*i, pin.id.input / 2);
            let (row, tip) = self.row(gear.0, gear.1);
            let r = if shaft {
                ui.label("■ Shaft").on_hover_text(SHAFT_TIP)
            } else {
                // Click a gear row to pick it for the ratio (two gears).
                let picked = self.picked.contains(&Some(gear));
                let text = egui::RichText::new(format!("● Mesh  {row}"));
                let tip = if tip.is_empty() { format!("{MESH_TIP}\n{RATIO_TIP}") } else { format!("{tip}\n\n{MESH_TIP}\n{RATIO_TIP}") };
                let r = ui.add(egui::Label::new(if picked { text.strong().underline() } else { text }).sense(egui::Sense::click())).on_hover_text(tip);
                if r.clicked() {
                    self.clicked = Some(gear);
                }
                r
            };
            self.pins.push(((pin.id.node, false, pin.id.input), egui::pos2(r.rect.left() - 10.0, r.rect.center().y)));
        }
        pin_info(shaft)
    }

    fn show_output(&mut self, pin: &OutPin, ui: &mut egui::Ui, _snarl: &mut Snarl<GNode>) -> impl SnarlPin + 'static {
        let shaft = pin.id.output % 2 == 1;
        let r = if shaft { ui.label("Shaft ■").on_hover_text(SHAFT_TIP) } else { ui.label("Mesh ●").on_hover_text(MESH_TIP) };
        self.pins.push(((pin.id.node, true, pin.id.output), egui::pos2(r.rect.right() + 10.0, r.rect.center().y)));
        pin_info(shaft)
    }

    fn has_body(&mut self, node: &GNode) -> bool {
        matches!(node, GNode::Block { .. })
    }

    fn show_body(&mut self, node: NodeId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) {
        if let Some(GNode::Block { module, .. }) = snarl.get_node_mut(node) {
            ui.horizontal(|ui| {
                ui.label("Module");
                ui.add(egui::TextEdit::singleline(module).desired_width(70.0).hint_text("Auto")).on_hover_text(BLOCK_TIP);
            });
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<GNode>) {
        // Mesh to mesh, shaft to shaft, between two items: nothing else joins.
        let items = matches!(snarl.get_node(from.id.node), Some(GNode::Item(_))) && matches!(snarl.get_node(to.id.node), Some(GNode::Item(_)));
        if items && from.id.output % 2 == to.id.input % 2 && from.id.node != to.id.node {
            snarl.connect(from.id, to.id);
        }
    }

    fn final_node_rect(&mut self, node: NodeId, rect: egui::Rect, ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) {
        self.rects.push((node, rect));
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
        if ui.button(solidcraft_ui_text::tr_label("Add Module Block")).on_hover_text(BLOCK_TIP).clicked() {
            snarl.insert_node(pos, GNode::Block { module: String::new(), members: Vec::new() });
            ui.close();
        }
        if self.suggested > 0 && ui.button(format!("Accept suggested ({})", self.suggested)).on_hover_text(ACCEPT_TIP).clicked() {
            self.accept_all = true;
            ui.close();
        }
    }

    fn has_node_menu(&mut self, _node: &GNode) -> bool {
        true
    }

    fn show_node_menu(&mut self, node: NodeId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut egui::Ui, snarl: &mut Snarl<GNode>) {
        let Some(this) = snarl.get_node(node).cloned() else { return };
        match this {
            GNode::Item(i) => {
                let n = self.gears_of(&this);
                for g in 0..n {
                    let make = solidcraft_ui_text::tr_label("Make Driver");
                    let label = if n > 1 { format!("{make} (gear {})", g + 1) } else { make.to_string() };
                    if ui.button(label).on_hover_text(DRIVER_TIP).clicked() {
                        self.make_driver = Some((i, g));
                        ui.close();
                    }
                }
                // Each link of the node, to remove on its own.
                let mine: Vec<(OutPinId, InPinId)> = snarl.wires().filter(|(o, to)| o.node == node || to.node == node).collect();
                for (o, to) in mine {
                    let (Some(GNode::Item(a)), Some(GNode::Item(b))) = (snarl.get_node(o.node).cloned(), snarl.get_node(to.node).cloned()) else { continue };
                    let what = if o.output % 2 == 0 { "mesh" } else { "shaft" };
                    let remove = solidcraft_ui_text::tr_label("Remove link");
                    if ui.button(format!("{remove} {}·{} – {}·{} ({what})", a + 1, o.output / 2 + 1, b + 1, to.input / 2 + 1)).clicked() {
                        snarl.disconnect(o, to);
                        ui.close();
                    }
                }
            }
            GNode::Block { .. } => {
                if ui.button(solidcraft_ui_text::tr_label("Remove Module Block")).clicked() {
                    snarl.remove_node(node);
                    ui.close();
                }
            }
        }
        if self.suggested > 0 && ui.button(format!("Accept suggested ({})", self.suggested)).on_hover_text(ACCEPT_TIP).clicked() {
            self.accept_all = true;
            ui.close();
        }
    }
}

fn pin_info(shaft: bool) -> PinInfo {
    let mut p = if shaft { PinInfo::square() } else { PinInfo::circle() };
    p.fill = Some(if shaft { egui::Color32::from_rgb(160, 160, 160) } else { egui::Color32::from_rgb(70, 140, 220) });
    p
}

/// What drawing the graph found: the item a hovered node stands for, the link the pointer is on
/// (item, gear, item, gear: light both its contacts in the view), and a driver chosen there
/// (item, gear).
#[derive(Default)]
pub struct GraphOut {
    pub hovered: Option<usize>,
    pub link: Option<(usize, usize, usize, usize)>,
    pub driver: Option<(usize, usize)>,
    /// A gear row clicked (item, gear): picked for the ratio.
    pub clicked: Option<(usize, usize)>,
}

/// What the graph marks: the driving gear (▶), the two gears picked for the ratio, and how a length
/// is shown (the design's units).
pub struct Marks<'a> {
    pub driver: Option<(usize, usize)>,
    pub picked: [Option<(usize, usize)>; 2],
    pub len: &'a dyn Fn(f64) -> String,
}

/// The distance from `p` to the segment `a`–`b`.
fn to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).length()
}

/// The graph, `gears` the gears of each item (docked in the dialog, or in its window).
pub fn graph_ui(ui: &mut egui::Ui, items: &[GearItem], gears: &[usize], graph: &mut Graph, marks: Marks<'_>, size: egui::Vec2, salt: &str) -> GraphOut {
    let driver = marks.driver;
    for i in 0..items.len() {
        graph.node_of(i);
    }
    if items.is_empty() {
        ui.label(egui::RichText::new(EMPTY).small().weak());
        return GraphOut::default();
    }
    // A drag begins: where the nodes are, to tell a dragged one afterwards.
    let (pressed, released) = ui.input(|i| (i.pointer.any_pressed(), i.pointer.any_released()));
    if pressed {
        graph.pressed = Some(graph.rects.clone());
    }
    let replies = graph.replies.clone();
    let mut viewer = Viewer {
        items,
        gears,
        replies: &replies,
        len: marks.len,
        picked: marks.picked,
        clicked: None,
        suggested: graph.suggested.len(),
        driver,
        hovered: None,
        make_driver: None,
        accept_all: false,
        rects: Vec::new(),
        pins: Vec::new(),
    };
    SnarlWidget::new().id_salt(salt).style(SnarlStyle::new()).min_size(size).max_size(size).show(&mut graph.snarl, &mut viewer, ui);
    let (hovered, make_driver, accept_all, clicked) = (viewer.hovered, viewer.make_driver, viewer.accept_all, viewer.clicked);
    graph.rects = std::mem::take(&mut viewer.rects);
    graph.pins = std::mem::take(&mut viewer.pins);
    if released && let Some(before) = graph.pressed.take() {
        graph.settle_blocks(&before);
    }
    if accept_all {
        graph.accept_all();
    }

    let painter = ui.painter().clone();
    let (text, weak, error, accent, back) = (ui.visuals().text_color(), ui.visuals().weak_text_color(), ui.visuals().error_fg_color, ui.visuals().selection.stroke.color, ui.visuals().extreme_bg_color);
    let font = egui::FontId::proportional(11.0);
    // Module Blocks: a frame round each block and the items in it.
    let blocks: Vec<NodeId> = graph.snarl.node_ids().filter(|(_, n)| matches!(n, GNode::Block { .. })).map(|(id, _)| id).collect();
    for b in blocks {
        if let Some(f) = graph.frame(b, None) {
            painter.rect_stroke(f, 6.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(230, 140, 50)), egui::StrokeKind::Outside);
        }
    }
    // The meshes inside an item (a pair, a ring gear or rack with its pinion, a planetary set): a
    // solid inner link between its rows.
    for (id, n) in graph.snarl.node_ids() {
        let GNode::Item(i) = n else { continue };
        let Some(it) = items.get(*i) else { continue };
        for (k, (g0, g1)) in inner_meshes(it.kind, gears.get(*i).copied().unwrap_or(0)).iter().enumerate() {
            if let (Some(p0), Some(p1)) = (graph.pin_at(id, true, pin(*g0, false)), graph.pin_at(id, true, pin(*g1, false))) {
                let x = p0.x.max(p1.x) + 8.0 + 6.0 * k as f32;
                painter.line(vec![p0, egui::pos2(x, p0.y), egui::pos2(x, p1.y), p1], egui::Stroke::new(2.0, text));
            }
        }
    }
    // The links: a shaft link thick and grey; each one's ratio, or its error in red, at its
    // middle; the one under the pointer lit, and given back so its contacts light in the view.
    let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| ui.max_rect().contains(*p));
    let mut lit = None;
    let wires: Vec<(OutPinId, InPinId)> = graph.snarl.wires().collect();
    for (o, i) in wires {
        let (Some(a), Some(b)) = (graph.item_of(o.node), graph.item_of(i.node)) else { continue };
        let (Some(pa), Some(pb)) = (graph.pin_at(o.node, true, o.output), graph.pin_at(i.node, false, i.input)) else { continue };
        let (ga, gb) = (o.output / 2, i.input / 2);
        let shaft = o.output % 2 == 1;
        if shaft {
            painter.line_segment([pa, pb], egui::Stroke::new(3.0, egui::Color32::from_rgb(150, 150, 150)));
        }
        if lit.is_none() && pointer.is_some_and(|p| to_segment(p, pa, pb) < 6.0) {
            lit = Some((a, ga, b, gb));
            painter.line_segment([pa, pb], egui::Stroke::new(4.0, accent.gamma_multiply(0.6)));
        }
        let Some((label, bad)) = graph.notes.iter().find(|n| n.is((a, ga), (b, gb))).and_then(Note::label) else { continue };
        if shaft && !bad {
            continue;
        }
        let at = pa + (pb - pa) * 0.5;
        let color = if bad { error } else { text };
        let g = painter.layout_no_wrap(label, font.clone(), color);
        let r = egui::Align2::CENTER_CENTER.anchor_size(at, g.size()).expand(2.0);
        painter.rect_filled(r, 3.0, back);
        painter.galley(r.min + egui::vec2(2.0, 2.0), g, color);
    }
    // The links the drawing suggests: dashed, until accepted.
    for s in graph.suggested.clone() {
        let (Some(na), Some(nb)) = (graph.item_node(s.a), graph.item_node(s.b)) else { continue };
        let shaft = s.kind == "shaft";
        if let (Some(pa), Some(pb)) = (graph.pin_at(na, true, pin(s.a_gear, shaft)), graph.pin_at(nb, false, pin(s.b_gear, shaft))) {
            painter.extend(egui::Shape::dashed_line(&[pa, pb], egui::Stroke::new(1.5, weak), 6.0, 4.0));
        }
    }
    GraphOut { hovered, link: lit, driver: make_driver, clicked }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: &str) -> GearItem {
        GearItem::new(ItemKind::parse(kind).unwrap())
    }

    #[test]
    fn notes_suggestions_and_sizes_come_from_the_reply() {
        let mut g = Graph::default();
        for i in 0..3 {
            g.node_of(i);
        }
        g.accept(&Suggested { a: 0, a_gear: 0, b: 1, b_gear: 0, kind: "mesh".into() });
        g.note(&json!({
            "links": [
                {"a": {"item": "A", "gear": 0}, "b": {"item": "B", "gear": 0}, "kind": "mesh", "ratio": 2.0},
                {"a": {"item": "B", "gear": 0}, "b": {"item": "C", "gear": 0}, "kind": "mesh", "error": "centre distance 31.2, needs 30.0 ±0.05", "hint": "move the circles"}
            ],
            "suggestions": [
                {"a": {"item": "A", "gear": 0}, "b": {"item": "B", "gear": 0}, "kind": "mesh"},
                {"a": {"item": "B", "gear": 0}, "b": {"item": "C", "gear": 0}, "kind": "mesh"}
            ],
            "gears": [{"item": "A", "gear": 0, "teeth": 20, "module": 2.0}]
        }));
        assert_eq!(g.notes()[0].label(), Some(("2.00 : 1".into(), false)));
        assert_eq!(g.notes()[1].label(), Some(("centre distance 31.2, needs 30.0 ±0.05: move the circles".into(), true)));
        // The one already made isn't suggested again.
        assert_eq!(g.suggested(), &[Suggested { a: 1, a_gear: 0, b: 2, b_gear: 0, kind: "mesh".into() }]);
        g.accept_all();
        assert!(g.suggested().is_empty());
        assert_eq!(g.links(), vec![(0, 0, 1, 0, "mesh"), (1, 0, 2, 0, "mesh")]);
        assert_eq!((g.replies()[0].item, g.replies()[0].gear, g.replies()[0].teeth, g.replies()[0].module), (0, 0, 20, 2.0));
        assert_eq!(reply_text(&g.replies()[0], &|v: f64| crate::dialogs_gear::fmt(v)), "z20 m2");
        assert_eq!(module_text(1.25), "1.25");
        assert_eq!(module_text(2.0), "2");
    }

    #[test]
    fn blocks_hold_their_items_and_follow_removals() {
        let mut g = Graph::default();
        for i in 0..3 {
            g.node_of(i);
        }
        let b = g.add_block("2 mm");
        assert_eq!(b, 0);
        g.put_in_block(b, 0);
        g.put_in_block(b, 2);
        let auto = g.add_block("");
        g.put_in_block(auto, 1);
        assert_eq!(g.blocks(), vec![("2 mm".to_string(), vec![0, 2]), (String::new(), vec![1])]);
        // In one block at a time.
        g.put_in_block(auto, 2);
        assert_eq!(g.blocks(), vec![("2 mm".to_string(), vec![0]), (String::new(), vec![1, 2])]);
        // Item 1 taken out: item 2 becomes item 1, in its block still.
        g.remove_item(1);
        assert_eq!(g.blocks(), vec![("2 mm".to_string(), vec![0]), (String::new(), vec![1])]);
    }

    #[test]
    fn an_item_dragged_into_a_frame_joins_its_block_and_out_of_it_leaves() {
        let mut g = Graph::default();
        let (n0, n1) = (g.node_of(0), g.node_of(1));
        let b = g.add_block("");
        let bn = g.snarl.node_ids().find(|(_, n)| matches!(n, GNode::Block { .. })).map(|(id, _)| id).unwrap();
        let r = |x: f32, y: f32| egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(100.0, 60.0));
        g.rects = vec![(n0, r(0.0, 0.0)), (n1, r(400.0, 0.0)), (bn, r(0.0, 200.0))];
        // Item 1 dropped onto the block's header: in.
        let before = g.rects.clone();
        g.rects[1].1 = r(10.0, 210.0);
        g.settle_blocks(&before);
        assert_eq!(g.blocks()[b].1, vec![1]);
        // Dragged far away again: out.
        let before = g.rects.clone();
        g.rects[1].1 = r(600.0, 600.0);
        g.settle_blocks(&before);
        assert!(g.blocks()[b].1.is_empty());
        // Item 0 that didn't move stays where it was (in no block).
        assert!(!g.blocks()[b].1.contains(&0));
        let _ = item("spur");
    }

    #[test]
    fn pairs_have_an_inner_mesh_and_the_dialogs_lines_read_the_notes() {
        assert_eq!(inner_meshes(item("bevel").kind, 2), &[(0, 1)]);
        assert_eq!(inner_meshes(item("worm").kind, 2), &[(0, 1)]);
        assert!(inner_meshes(item("compound").kind, 2).is_empty());
        assert_eq!(inner_meshes(item("planetary").kind, 3), &[(0, 2), (2, 1)]);
        assert!(inner_meshes(item("internal").kind, 1).is_empty());
        let (lines, suggested) = link_notes(&json!({"links": [{"a": {"item": "A", "gear": 0}, "b": {"item": "B", "gear": 1}, "kind": "mesh", "ratio": 0.5}], "suggestions": ["item A gear 1 and item C gear 1 are tangent: link them?"]}));
        assert_eq!(lines, vec![("1·1 – 2·2: 0.50 : 1".to_string(), false)]);
        assert_eq!(suggested, vec![Suggested { a: 0, a_gear: 0, b: 2, b_gear: 0, kind: "mesh".into() }]);
        assert!((to_segment(egui::pos2(5.0, 3.0), egui::pos2(0.0, 0.0), egui::pos2(10.0, 0.0)) - 3.0).abs() < 1e-6);
    }
}
