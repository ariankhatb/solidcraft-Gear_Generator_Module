//! The Gears dialog's links and blocks (GEAR_UI_SPEC.md §4); the items they join are in
//! `dialogs_gear_items`.
//!
//! A link joins two gears: Mesh (their teeth engage) or Shaft (they share a shaft and turn
//! together). The Links & Motion tab lists them as rows (hovering a row lights the link in the
//! view, where every link is drawn between its gears with its ratio); the links the drawing
//! suggests are listed dim and accepted with one button. A **block** ([`Block`]) holds gears
//! that share its settings: its module, and optionally its shaft Ø (every member bored to it and
//! a shaft along each of its axes, one body with its gears), face width, helix angle, pressure
//! angle, backlash and tooth chamfer; a member's own value wins where it gives one. Blocks are
//! made and filled on the Gears tab. What `gear.calculate` says of each link (its ratio, or its
//! error) and of each gear is kept here ([`Links::note`]). The items, links, blocks and the
//! driver are sent as `gear.create`'s `items`, `links`, `module_blocks` and `drive` (§6, by
//! `dialogs_gear_items::items_request`).

use serde_json::{Value, json};

pub use crate::dialogs_gear_items::{GearItem, GearSettings, item_id, item_index, items_request};
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_gear::items::{self as gi, GearReply};

use crate::SolveApp;

/// A ratio as a link shows it: "2.00" (of "2.00 : 1").
fn ratio_text(r: f64) -> String {
    format!("{r:.2}")
}

/// A module as a gear row shows it: "2", "1.25".
fn module_text(m: f64) -> String {
    let t = format!("{m:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A link: (item, gear, item, gear, "mesh" | "shaft").
pub type Link = (usize, usize, usize, usize, &'static str);

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
    pub fn is(&self, a: (usize, usize), b: (usize, usize)) -> bool {
        ((self.a, self.a_gear) == a && (self.b, self.b_gear) == b) || ((self.a, self.a_gear) == b && (self.b, self.b_gear) == a)
    }
    /// The label of the link ("2.00 : 1", or the error), and whether it is an error.
    pub fn label(&self) -> Option<(String, bool)> {
        match (&self.error, self.ratio) {
            (Some(e), _) => Some((e.clone(), true)),
            (None, Some(r)) => Some((format!("{} : 1", ratio_text(r)), false)),
            _ => None,
        }
    }
}

/// A block as typed (each setting empty: none, the gears' own or Auto), and the items in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub name: String,
    pub module: String,
    pub shaft_d: String,
    pub face: String,
    pub helix: String,
    pub pressure_angle: String,
    pub backlash: String,
    pub chamfer: String,
    /// Indices into the items, each item in one block at most.
    pub members: Vec<usize>,
}

impl Block {
    /// The name shown: its own, else "Block 1".
    pub fn title(&self, k: usize) -> String {
        if self.name.trim().is_empty() { format!("Block {}", k + 1) } else { self.name.trim().to_string() }
    }
    /// The gear crate's block (`id`: its id in the request) as typed; `Err` names the setting a
    /// value can't be read for.
    pub fn read(&self, app: &SolveApp, id: &str, who: &str) -> Result<gi::ModuleBlock, String> {
        let num = |t: &str, kind: ValueKind, f: &str| crate::dialogs_gear_items::eval(app, t, kind, &format!("{who}: {f}"));
        let mut members = self.members.clone();
        members.sort_unstable();
        members.dedup();
        Ok(gi::ModuleBlock {
            id: id.to_string(),
            module: num(&self.module, ValueKind::Length, "module")?,
            members,
            settings: gi::BlockSettings {
                tooth_chamfer: num(&self.chamfer, ValueKind::Length, "tooth chamfer")?.filter(|v| *v > 0.0),
                pressure_angle: num(&self.pressure_angle, ValueKind::Angle, "pressure angle")?,
                helix: num(&self.helix, ValueKind::Angle, "helix angle")?,
                face_width: num(&self.face, ValueKind::Length, "face width")?,
                backlash: num(&self.backlash, ValueKind::Length, "backlash")?,
                shaft_diameter: num(&self.shaft_d, ValueKind::Length, "shaft Ø")?,
            },
        })
    }
    /// As `gear.create`'s `module_blocks` entry (its members name it in their `module_block`).
    pub fn json(&self, app: &SolveApp, k: usize) -> Result<Value, String> {
        let b = self.read(app, &block_id(k), &self.title(k))?;
        let mut v = json!({"id": b.id});
        if !self.name.trim().is_empty() {
            v["name"] = json!(self.name.trim());
        }
        let s = b.settings;
        for (key, x) in [
            ("module", b.module),
            ("shaft_diameter", s.shaft_diameter),
            ("face_width", s.face_width),
            ("helix_angle", s.helix),
            ("pressure_angle", s.pressure_angle),
            ("backlash", s.backlash),
            ("tooth_chamfer", s.tooth_chamfer),
        ] {
            if let Some(x) = x {
                v[key] = json!(x);
            }
        }
        Ok(v)
    }
    /// Read back from a `module_blocks` entry (Edit); its members come from the items.
    pub fn of(v: &Value) -> Block {
        let s = |key: &str, kind: ValueKind| crate::dialogs_gear_items::shown(v.get(key).and_then(Value::as_f64), kind);
        Block {
            name: v.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
            module: s("module", ValueKind::Length),
            shaft_d: s("shaft_diameter", ValueKind::Length),
            face: s("face_width", ValueKind::Length),
            helix: s("helix_angle", ValueKind::Angle),
            pressure_angle: s("pressure_angle", ValueKind::Angle),
            backlash: s("backlash", ValueKind::Length),
            chamfer: s("tooth_chamfer", ValueKind::Length),
            members: Vec::new(),
        }
    }
    /// Whether a setting of its More group is given (the group opens).
    pub fn has_more(&self) -> bool {
        [&self.helix, &self.pressure_angle, &self.backlash, &self.chamfer].iter().any(|t| !t.trim().is_empty())
    }
}

/// Block `k`'s id in the request: "M1", "M2", …
pub fn block_id(k: usize) -> String {
    format!("M{}", k + 1)
}

/// The links between the items and the blocks, kept with the dialog (compared by its links and
/// blocks only), with what the engine last said of them.
#[derive(Clone, Debug, Default)]
pub struct Links {
    links: Vec<Link>,
    blocks: Vec<Block>,
    notes: Vec<Note>,
    suggested: Vec<Suggested>,
    /// What the engine worked out for each gear (`gear.calculate`'s `gears`).
    replies: Vec<GearReply>,
}

impl PartialEq for Links {
    fn eq(&self, other: &Self) -> bool {
        self.links == other.links && self.blocks == other.blocks
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
            let (item, gear) = end(Some(g))?;
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

impl Links {
    /// The links, sorted: (item, gear, item, gear, "mesh" | "shaft").
    pub fn all(&self) -> Vec<Link> {
        self.links.clone()
    }

    /// The blocks, in the order they were made.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Block `k`, to edit in place.
    pub fn block_mut(&mut self, k: usize) -> Option<&mut Block> {
        self.blocks.get_mut(k)
    }

    /// The block item `item` is in.
    pub fn block_of(&self, item: usize) -> Option<usize> {
        self.blocks.iter().position(|b| b.members.contains(&item))
    }

    /// A new block holding `members` (each taken out of any other): its index.
    pub fn add_block(&mut self, members: &[usize]) -> usize {
        self.blocks.push(Block::default());
        let k = self.blocks.len() - 1;
        for m in members {
            self.put_in_block(k, *m);
        }
        k
    }

    /// Block `k` taken away (its gears in no block).
    pub fn remove_block(&mut self, k: usize) {
        if k < self.blocks.len() {
            self.blocks.remove(k);
        }
    }

    /// Item `item` put in block `block`, and out of any other.
    pub fn put_in_block(&mut self, block: usize, item: usize) {
        for (k, b) in self.blocks.iter_mut().enumerate() {
            b.members.retain(|m| *m != item);
            if k == block {
                b.members.push(item);
                b.members.sort_unstable();
            }
        }
    }

    /// Item `item` taken out of its block.
    pub fn take_out(&mut self, item: usize) {
        for b in &mut self.blocks {
            b.members.retain(|m| *m != item);
        }
    }

    /// The gear crate's blocks as typed, each with its request id (a block with a value that
    /// can't be read keeps only its members).
    pub fn crate_blocks(&self, app: &SolveApp) -> Vec<gi::ModuleBlock> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(k, b)| b.read(app, &block_id(k), "").unwrap_or_else(|_| gi::ModuleBlock { id: block_id(k), members: b.members.clone(), ..Default::default() }))
            .collect()
    }

    /// A link made (a suggested one accepted, or one by hand): the same link isn't made twice.
    pub fn accept(&mut self, link: &Suggested) {
        let kind = if link.kind == "shaft" { "shaft" } else { "mesh" };
        let (x, y) = ((link.a, link.a_gear), (link.b, link.b_gear));
        if x != y && !self.links.iter().any(|l| l.4 == kind && (((l.0, l.1) == x && (l.2, l.3) == y) || ((l.0, l.1) == y && (l.2, l.3) == x))) {
            self.links.push((link.a, link.a_gear, link.b, link.b_gear, kind));
            self.links.sort_unstable();
        }
        self.suggested.retain(|s| s != link);
    }

    /// The link between these two gears taken away (either way round).
    pub fn remove_link(&mut self, a: (usize, usize), b: (usize, usize), kind: &str) {
        self.links.retain(|l| !(l.4 == kind && (((l.0, l.1) == a && (l.2, l.3) == b) || ((l.0, l.1) == b && (l.2, l.3) == a))));
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

    /// Item `i` taken out: its links go, it leaves its block, and later items are renumbered.
    pub fn remove_item(&mut self, i: usize) {
        let down = |j: usize| if j > i { j - 1 } else { j };
        self.links.retain(|l| l.0 != i && l.2 != i);
        for l in &mut self.links {
            (l.0, l.2) = (down(l.0), down(l.2));
        }
        for b in &mut self.blocks {
            b.members.retain(|m| *m != i);
            for m in &mut b.members {
                *m = down(*m);
            }
        }
        self.suggested.retain(|s| s.a != i && s.b != i);
        self.notes.retain(|n| n.a != i && n.b != i);
    }

    /// What `gear.calculate` says (its reply): each link's ratio or error, the links the drawing
    /// suggests (less those already made), and each gear's teeth and module.
    pub fn note(&mut self, r: &Value) {
        self.notes = notes_of(r);
        self.suggest(suggested_of(r));
        self.replies = gear_replies(r);
    }

    /// The links the drawing suggests, less those already made.
    pub fn suggest(&mut self, list: Vec<Suggested>) {
        let made = self.all();
        self.suggested = list
            .into_iter()
            .filter(|s| !made.iter().any(|(a, ag, b, bg, _)| (*a, *ag, *b, *bg) == (s.a, s.a_gear, s.b, s.b_gear) || (*b, *bg, *a, *ag) == (s.a, s.a_gear, s.b, s.b_gear)))
            .collect();
    }

    /// What the engine worked out for each gear, as last given ([`Links::note`]).
    pub fn replies(&self) -> &[GearReply] {
        &self.replies
    }

    /// The engine's notes on the links, as last given ([`Links::note`]).
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// The engine's note on link `l`.
    pub fn note_of(&self, l: &Link) -> Option<&Note> {
        self.notes.iter().find(|n| n.kind == l.4 && n.is((l.0, l.1), (l.2, l.3)))
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
fn end(v: Option<&Value>) -> Option<(usize, usize)> {
    match v {
        Some(Value::Object(o)) => Some((o.get("item").and_then(item_index)?, o.get("gear").or(o.get("port")).and_then(Value::as_u64).unwrap_or(0) as usize)),
        Some(other) => Some((item_index(other)?, 0)),
        None => None,
    }
}

/// The reply's links, each with its ratio or its error (and what to do, after a colon).
fn notes_of(r: &Value) -> Vec<Note> {
    r.get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|l| {
            let ((a, a_gear), (b, b_gear)) = (end(l.get("a"))?, end(l.get("b"))?);
            let error = l.get("error").and_then(Value::as_str).map(|e| {
                let hint = l.get("hint").and_then(Value::as_str).map(|h| format!(": {h}")).unwrap_or_default();
                format!("{e}{hint}")
            });
            Some(Note { a, a_gear, b, b_gear, kind: l.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string(), ratio: l.get("ratio").and_then(Value::as_f64), error })
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
                let ((a, a_gear), (b, b_gear)) = (end(v.get("a"))?, end(v.get("b"))?);
                Some(Suggested { a, a_gear, b, b_gear, kind: v.get("kind").and_then(Value::as_str).unwrap_or("mesh").to_string() })
            }
            _ => None,
        })
        .collect()
}

/// A gear as the dialog's messages name it: "A" (an item's first gear), "B2" (its second).
pub fn short_name(item: usize, gear: usize) -> String {
    if gear == 0 { item_id(item) } else { format!("{}{}", item_id(item), gear + 1) }
}

/// What `gear.calculate` says of each link, as lines for the dialog ("A – B2: 2.00 : 1", or
/// the error and what to do: red), and the links it suggests.
pub fn link_notes(r: &Value) -> (Vec<(String, bool)>, Vec<Suggested>) {
    let notes = notes_of(r)
        .iter()
        .filter_map(|n| {
            let name = format!("{} – {}", short_name(n.a, n.a_gear), short_name(n.b, n.b_gear));
            n.label().map(|(text, bad)| (format!("{name}: {text}"), bad))
        })
        .collect();
    (notes, suggested_of(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_suggestions_and_sizes_come_from_the_reply() {
        let mut g = Links::default();
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
        assert_eq!(g.note_of(&(1, 0, 0, 0, "mesh")).and_then(|n| n.ratio), Some(2.0));
        // The one already made isn't suggested again.
        assert_eq!(g.suggested(), &[Suggested { a: 1, a_gear: 0, b: 2, b_gear: 0, kind: "mesh".into() }]);
        g.accept_all();
        assert!(g.suggested().is_empty());
        assert_eq!(g.all(), vec![(0, 0, 1, 0, "mesh"), (1, 0, 2, 0, "mesh")]);
        assert_eq!((g.replies()[0].item, g.replies()[0].gear, g.replies()[0].teeth, g.replies()[0].module), (0, 0, 20, 2.0));
        assert_eq!(reply_text(&g.replies()[0], &|v: f64| crate::dialogs_gear::fmt(v)), "z20 m2");
        assert_eq!(module_text(1.25), "1.25");
        assert_eq!(module_text(2.0), "2");
        // A link made twice is one link; taken away either way round.
        g.accept(&Suggested { a: 1, a_gear: 0, b: 0, b_gear: 0, kind: "mesh".into() });
        assert_eq!(g.all().len(), 2);
        g.remove_link((1, 0), (0, 0), "mesh");
        assert_eq!(g.all(), vec![(1, 0, 2, 0, "mesh")]);
    }

    #[test]
    fn blocks_hold_their_items_and_follow_removals() {
        let mut g = Links::default();
        let b = g.add_block(&[0, 2]);
        assert_eq!(b, 0);
        g.block_mut(b).unwrap().module = "2 mm".into();
        let auto = g.add_block(&[1]);
        let members = |g: &Links| g.blocks().iter().map(|b| (b.module.clone(), b.members.clone())).collect::<Vec<_>>();
        assert_eq!(members(&g), vec![("2 mm".to_string(), vec![0, 2]), (String::new(), vec![1])]);
        // In one block at a time.
        g.put_in_block(auto, 2);
        assert_eq!(members(&g), vec![("2 mm".to_string(), vec![0]), (String::new(), vec![1, 2])]);
        assert_eq!((g.block_of(2), g.block_of(5)), (Some(1), None));
        // Item 1 taken out: item 2 becomes item 1, in its block still; its links renumbered.
        g.accept(&Suggested { a: 0, a_gear: 0, b: 2, b_gear: 0, kind: "shaft".into() });
        g.remove_item(1);
        assert_eq!(members(&g), vec![("2 mm".to_string(), vec![0]), (String::new(), vec![1])]);
        assert_eq!(g.all(), vec![(0, 0, 1, 0, "shaft")]);
        g.take_out(0);
        g.remove_block(1);
        assert_eq!(members(&g), vec![("2 mm".to_string(), vec![])]);
        assert_eq!(Block::default().title(1), "Block 2");
    }

    #[test]
    fn the_dialogs_lines_read_the_notes() {
        let (lines, suggested) = link_notes(&json!({"links": [{"a": {"item": "A", "gear": 0}, "b": {"item": "B", "gear": 1}, "kind": "mesh", "ratio": 0.5}], "suggestions": ["item A gear 1 and item C gear 1 are tangent: link them?"]}));
        assert_eq!(lines, vec![("A – B2: 0.50 : 1".to_string(), false)]);
        assert_eq!(suggested, vec![Suggested { a: 0, a_gear: 0, b: 2, b_gear: 0, kind: "mesh".into() }]);
    }
}
