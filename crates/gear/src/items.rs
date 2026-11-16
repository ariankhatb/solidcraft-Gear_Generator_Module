//! Gear sets as items and links (the Gears dialog's node graph): each item is one gear, a
//! compound of gears on one shaft, a pair that is one set (bevel, hypoid, worm and wheel,
//! crossed helicals) or a shaft, made from the components picked for its slots; links join
//! items' gears by a mesh or a shared shaft; a module block keeps one module for every gear in
//! it.
//!
//! | Type | Slots (● required, ○ optional) | Gears |
//! |---|---|---|
//! | spur, helical, herringbone | ● `pitch` circle; ○ `axis` line (its shaft) | 1 |
//! | internal (ring gear) | ● `pitch` circle | 1 |
//! | compound | ● `axis` line; ● `pitch`, `pitch2`, `pitch3`… circles on it | one per circle, one body |
//! | bevel | ● `pitch`, `pitch2` circles on meeting axes | 2 (meshed inside the item) |
//! | hypoid | ● `pitch` ring gear circle; ● `axis2` pinion axis line (skew) | 2 |
//! | worm | ● `pitch` wheel circle; ● `axis2` worm axis line | 2 (wheel, worm) |
//! | crossed | ● `pitch`, `pitch2` circles on skew axes | 2 |
//! | shaft | ● `axis` line; ○ `bearing`, `bearing2`… points on it | 0 |
//! | rack | ● `pitch` circle (its pinion), ● `line` (pitch line) | refused: not in a drivetrain yet |
//!
//! Everything is read through one reader ([`crate::drivetrain::read_linked`]): the items'
//! circles, lines and points with what each gear asks, and the links as which members mesh and
//! share a shaft. A pair item is read on its own (it is a set of its own: linking it to others
//! is refused by name for now).

use crate::axes::Solved;
use crate::drivetrain::{Ask, Item, Links, Member, Role, Thing, read_linked};
use crate::plan::{Kind, Plan, Spec};
use crate::simple::Simple;

/// An item's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemType {
    Spur,
    Helical,
    Herringbone,
    Internal,
    Rack,
    Compound,
    Bevel,
    Hypoid,
    Worm,
    Crossed,
    Shaft,
}

/// What a slot takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Circle,
    Line,
    Point,
}

impl ItemType {
    pub const ALL: [ItemType; 11] = [
        ItemType::Spur,
        ItemType::Helical,
        ItemType::Herringbone,
        ItemType::Internal,
        ItemType::Rack,
        ItemType::Compound,
        ItemType::Bevel,
        ItemType::Hypoid,
        ItemType::Worm,
        ItemType::Crossed,
        ItemType::Shaft,
    ];
    pub fn name(self) -> &'static str {
        match self {
            ItemType::Spur => "spur",
            ItemType::Helical => "helical",
            ItemType::Herringbone => "herringbone",
            ItemType::Internal => "internal",
            ItemType::Rack => "rack",
            ItemType::Compound => "compound",
            ItemType::Bevel => "bevel",
            ItemType::Hypoid => "hypoid",
            ItemType::Worm => "worm",
            ItemType::Crossed => "crossed",
            ItemType::Shaft => "shaft",
        }
    }
    pub fn parse(s: &str) -> Option<ItemType> {
        ItemType::ALL.iter().copied().find(|t| t.name() == s)
    }
    /// Its slots: name, what it takes, required. A compound takes `pitch`, `pitch2`, … and a
    /// shaft `bearing`, `bearing2`, … (any number; the first listed here).
    pub fn slots(self) -> &'static [(&'static str, Need, bool)] {
        match self {
            ItemType::Spur | ItemType::Helical | ItemType::Herringbone => &[("pitch", Need::Circle, true), ("axis", Need::Line, false)],
            ItemType::Internal => &[("pitch", Need::Circle, true)],
            ItemType::Rack => &[("pitch", Need::Circle, true), ("line", Need::Line, true)],
            ItemType::Compound => &[("axis", Need::Line, true), ("pitch", Need::Circle, true), ("pitch2", Need::Circle, true)],
            ItemType::Bevel | ItemType::Crossed => &[("pitch", Need::Circle, true), ("pitch2", Need::Circle, true)],
            ItemType::Hypoid | ItemType::Worm => &[("pitch", Need::Circle, true), ("axis2", Need::Line, true)],
            ItemType::Shaft => &[("axis", Need::Line, true), ("bearing", Need::Point, false)],
        }
    }
    /// A pair that is a set of its own.
    pub fn is_pair(self) -> bool {
        matches!(self, ItemType::Bevel | ItemType::Hypoid | ItemType::Worm | ItemType::Crossed)
    }
    /// The slot `name` takes this (a numbered `pitch3`, `bearing4` like its first).
    fn slot(self, name: &str) -> Option<(Need, bool)> {
        let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
        let numbered = base != name && name[base.len()..].parse::<u32>().is_ok_and(|k| k >= 2);
        self.slots().iter().find(|(s, _, _)| *s == name || (numbered && *s == base && matches!(self, ItemType::Compound | ItemType::Shaft))).map(|(_, n, r)| (*n, *r))
    }
}

/// A bevel's teeth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BevelStyle {
    Straight,
    Spiral,
    Chevron,
}

/// What one gear of an item asks (all optional: worked out where left out).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GearAsk {
    pub module: Option<f64>,
    pub teeth: Option<u32>,
    /// Its face each side of its circle's plane (`a` along the axis, `b` back; `None`: auto).
    pub face: Option<(Option<f64>, Option<f64>)>,
    pub helix: Option<f64>,
    /// Left hand (`Some(true)`) or right; its mates get the other.
    pub left_hand: Option<bool>,
    pub herringbone: Option<bool>,
    pub spiral_angle: Option<f64>,
    pub bevel_style: Option<BevelStyle>,
    pub starts: Option<u32>,
    pub rim: Option<f64>,
}

/// One picked component: its slot and what it is (world coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Component {
    pub slot: &'static str,
    pub thing: Thing,
}

/// One item: its id, type, components and what each of its gears asks (`gears[k]` the k-th:
/// the item's circles in slot order; one entry for all where there is one).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemIn {
    pub id: String,
    pub kind: ItemType,
    pub components: Vec<(String, Thing)>,
    pub gears: Vec<GearAsk>,
    pub module_block: Option<String>,
}

/// One end of a link: an item's gear (0 its first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    pub item: String,
    pub gear: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkType {
    Mesh,
    Shaft,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkIn {
    pub a: Port,
    pub b: Port,
    pub kind: LinkType,
}

/// A module block: every gear of its items keeps one module (`None`: worked out, the same).
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub id: String,
    pub module: Option<f64>,
}

/// The gear that drives the set, and its speed and torque.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Drive {
    pub item: String,
    pub gear: usize,
    pub rpm: Option<f64>,
    pub torque: Option<f64>,
}

/// What one gear became.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GearOut {
    pub item: String,
    pub gear: usize,
    pub teeth: Option<u32>,
    pub module: Option<f64>,
    pub pitch_d: Option<f64>,
    pub face_width: Option<f64>,
    /// Its turn per turn of the driver (signed: negative the other way).
    pub ratio: Option<f64>,
    pub rpm: Option<f64>,
    pub torque: Option<f64>,
    /// The least contact ratio of its meshes.
    pub contact_ratio: Option<f64>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

/// What one link became.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkOut {
    pub a: Port,
    pub b: Port,
    pub kind: LinkType,
    /// A mesh's tooth ratio (b's teeth over a's).
    pub ratio: Option<f64>,
    pub error: Option<String>,
    pub hint: Option<String>,
}

/// What [`resolve`] made of the items.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Resolved {
    pub solved: Option<Solved>,
    /// One per item: what is wrong with it (a missing component per slot, a pair linked).
    pub item_errors: Vec<(String, Option<String>)>,
    pub gears: Vec<GearOut>,
    pub links: Vec<LinkOut>,
    /// One per module block: what is wrong with it.
    pub block_errors: Vec<(String, Option<String>)>,
    /// Members that touch or share an axis without a link.
    pub suggestions: Vec<String>,
    pub notes: Vec<String>,
    /// The bearing seats (world points), for the keep-out body.
    pub seats: Vec<[f64; 3]>,
    /// The reader's members (one per circle, line, point of the items, in their order) and
    /// each of the set's gears' (train order) port.
    pub members: Vec<Member>,
    pub gear_ports: Vec<Port>,
    /// Each of the set's gears' member (index into `members`).
    pub gear_items: Vec<usize>,
    pub error: Option<String>,
}

fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Reads the items and links as one set (see the module docs). `style` the teeth of gears
/// whose item says nothing else; `settings` and `base` as [`crate::drivetrain::read`]'s.
pub fn resolve(items: &[ItemIn], links: &[LinkIn], blocks: &[Block], style: Option<Kind>, settings: &Simple, base: &Spec, shafts: bool) -> Resolved {
    let mut out = Resolved::default();
    // Each item's components against its slots.
    let mut errs: Vec<Option<String>> = vec![None; items.len()];
    for (k, it) in items.iter().enumerate() {
        let name = format!("item {} ({})", it.id, it.kind.name());
        for (slot, thing) in &it.components {
            match it.kind.slot(slot) {
                None => {
                    let slots: Vec<&str> = it.kind.slots().iter().map(|s| s.0).collect();
                    errs[k].get_or_insert(format!("{name}: no slot `{slot}` (it takes {})", slots.join(", ")));
                }
                Some((need, _)) => {
                    let ok = matches!((need, thing), (Need::Circle, Thing::Circle(_)) | (Need::Line, Thing::Line(_)) | (Need::Point, Thing::Point(_)));
                    if !ok {
                        let want = match need {
                            Need::Circle => "a circle",
                            Need::Line => "a line",
                            Need::Point => "a point",
                        };
                        errs[k].get_or_insert(format!("{name}: slot `{slot}` takes {want}"));
                    }
                }
            }
        }
        for (slot, need, required) in it.kind.slots() {
            if *required && !it.components.iter().any(|(s, _)| s == slot) {
                let what = match need {
                    Need::Circle => "a circle",
                    Need::Line => "a line",
                    Need::Point => "a point",
                };
                errs[k].get_or_insert(format!("{name}: pick {what} for slot `{slot}`"));
            }
        }
        if it.kind == ItemType::Rack {
            errs[k].get_or_insert(format!("{name}: a rack is not part of a drivetrain yet: make it with type rack from a sketch (a circle and its tangent pitch line)"));
        }
        if it.items_ids_repeat(items) {
            errs[k].get_or_insert(format!("{name}: another item has the same id"));
        }
    }
    let pairs: Vec<usize> = (0..items.len()).filter(|k| items[*k].kind.is_pair()).collect();
    for &k in &pairs {
        if items.len() > 1 || !links.is_empty() {
            errs[k].get_or_insert(format!(
                "item {} ({}): a {} pair is a set of its own for now: make it alone, not with other items or links",
                items[k].id,
                items[k].kind.name(),
                items[k].kind.name()
            ));
        }
    }
    out.item_errors = items.iter().zip(&errs).map(|(it, e)| (it.id.clone(), e.clone())).collect();
    if errs.iter().any(Option::is_some) {
        out.error = Some(errs.iter().flatten().cloned().collect::<Vec<_>>().join("; "));
        return out;
    }
    // The reader's members: each item's components, with what its gears ask; each gear's port.
    let mut read: Vec<Item> = Vec::new();
    let mut port_of: Vec<(Port, usize)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut inner_shafts: Vec<(usize, usize)> = Vec::new();
    for it in items {
        let mut gear = 0usize;
        let mut first_circle: Option<usize> = None;
        for (slot, thing) in &it.components {
            let ask_k = it.gears.get(gear).or(it.gears.first()).copied().unwrap_or_default();
            let role = match (it.kind, thing) {
                (_, Thing::Point(_)) => Some(Role::Bearing),
                (ItemType::Compound | ItemType::Shaft, Thing::Line(_)) => Some(Role::Shaft),
                (ItemType::Spur | ItemType::Helical | ItemType::Herringbone, Thing::Line(_)) => Some(Role::Shaft),
                (ItemType::Hypoid, Thing::Line(_)) | (ItemType::Worm, Thing::Line(_)) => None,
                (ItemType::Internal, Thing::Circle(_)) => Some(Role::Internal),
                (ItemType::Helical, Thing::Circle(_)) => Some(Role::Gear(Kind::Helical)),
                (ItemType::Herringbone, Thing::Circle(_)) => Some(Role::Gear(Kind::Herringbone)),
                (ItemType::Spur, Thing::Circle(_)) => Some(Role::Gear(Kind::Spur)),
                (ItemType::Bevel, Thing::Circle(_)) => Some(Role::Gear(match ask_k.bevel_style {
                    Some(BevelStyle::Spiral | BevelStyle::Chevron) => Kind::SpiralBevel,
                    _ if ask_k.spiral_angle.is_some_and(|a| a != 0.0) => Kind::SpiralBevel,
                    _ => Kind::Bevel,
                })),
                (ItemType::Hypoid, Thing::Circle(_)) => Some(Role::Gear(Kind::Hypoid)),
                (ItemType::Worm, Thing::Circle(_)) => Some(Role::Wheel),
                (ItemType::Crossed, Thing::Circle(_)) => Some(Role::Gear(Kind::CrossedHelical)),
                _ => None,
            };
            let index = read.len();
            let mut ask = Ask { role, row: Some(index), ..Ask::default() };
            if let Thing::Circle(_) = thing {
                ask.module = ask_k.module;
                ask.teeth = ask_k.teeth;
                ask.sides = ask_k.face;
                ask.helix = ask_k.helix;
                ask.herringbone = ask_k.herringbone;
                port_of.push((Port { item: it.id.clone(), gear }, index));
                names.push(format!("item {} gear {}", it.id, gear + 1));
                if it.kind == ItemType::Compound {
                    if let Some(f) = first_circle {
                        inner_shafts.push((f, index));
                    }
                    first_circle.get_or_insert(index);
                }
                gear += 1;
            } else {
                names.push(format!("item {} {slot}", it.id));
            }
            let _ = slot;
            read.push(Item { thing: *thing, ask });
        }
    }
    // The links, as the reader's members.
    let find = |p: &Port| port_of.iter().find(|(q, _)| q == p).map(|(_, i)| *i);
    let mut given = Links::default();
    let mut link_index: Vec<Option<(bool, usize)>> = Vec::new();
    for l in links {
        let (Some(a), Some(b)) = (find(&l.a), find(&l.b)) else {
            link_index.push(None);
            continue;
        };
        match l.kind {
            LinkType::Mesh => {
                link_index.push(Some((true, given.mesh.len())));
                given.mesh.push((a, b));
            }
            LinkType::Shaft => {
                link_index.push(Some((false, given.shaft.len())));
                given.shaft.push((a, b));
            }
        }
    }
    let inner = inner_shafts.len();
    given.shaft.extend(inner_shafts);
    // Module blocks: their module on every gear of their items.
    let mut block_errs: Vec<Option<String>> = vec![None; blocks.len()];
    for (b, block) in blocks.iter().enumerate() {
        if let Some(m) = block.module {
            for (k, it) in items.iter().enumerate() {
                if it.module_block.as_deref() != Some(block.id.as_str()) {
                    continue;
                }
                for (p, i) in &port_of {
                    if p.item != it.id {
                        continue;
                    }
                    if let Some(own) = read[*i].ask.module.filter(|own| (own - m).abs() > 1e-9) {
                        block_errs[b].get_or_insert(format!("module block {}: item {} gear {} asks module {}, the block module {}", block.id, it.id, p.gear + 1, num(own), num(m)));
                    }
                    read[*i].ask.module = Some(m);
                }
                let _ = k;
            }
        }
    }
    for it in items {
        if let Some(b) = &it.module_block
            && !blocks.iter().any(|x| &x.id == b)
        {
            out.item_errors.iter_mut().find(|(id, _)| id == &it.id).map(|(_, e)| e.get_or_insert(format!("item {}: no module block {b}", it.id)));
        }
    }
    // The pair item's settings (spiral, chevron, starts, hand) are the set's.
    let mut base = base.clone();
    if let Some(it) = pairs.first().map(|k| &items[*k]) {
        let g = it.gears.first().copied().unwrap_or_default();
        // The pair's family's own numbers first (its spiral, starts, helix), then what it asks.
        let kind = match it.kind {
            ItemType::Bevel if matches!(g.bevel_style, Some(BevelStyle::Spiral | BevelStyle::Chevron)) || g.spiral_angle.is_some_and(|a| a != 0.0) => Kind::SpiralBevel,
            ItemType::Bevel => Kind::Bevel,
            ItemType::Hypoid => Kind::Hypoid,
            ItemType::Worm => Kind::Worm,
            _ => Kind::CrossedHelical,
        };
        let d = Spec::defaults_for(kind);
        base = Spec { kind, helix: d.helix, spiral_angle: d.spiral_angle, starts: d.starts, offset: d.offset, ..base };
        if let Some(a) = g.spiral_angle {
            base.spiral_angle = a;
        }
        if g.bevel_style == Some(BevelStyle::Chevron) {
            base.herringbone = true;
        }
        if let Some(st) = g.starts {
            base.starts = st;
        }
        if let Some(l) = g.left_hand {
            base.left_hand = l;
        }
    }
    if let Some(r) = items.iter().flat_map(|it| it.gears.iter()).find_map(|g| g.rim) {
        base.rim = r;
    }
    let mut settings = settings.clone();
    if let Some(l) = items.iter().flat_map(|it| it.gears.first()).find_map(|g| g.left_hand) {
        settings.left_hand = Some(l);
    }
    let run = |read: &[Item]| read_linked(read, if pairs.is_empty() { Some(&given) } else { None }, style, &settings, &base, shafts);
    let mut o = run(&read);
    // A block left to work out: one module for all its gears (the finest of theirs, which fits
    // them all), or its error.
    for (b, block) in blocks.iter().enumerate() {
        if block.module.is_some() {
            continue;
        }
        let in_block: Vec<usize> = port_of.iter().filter(|(p, _)| items.iter().any(|it| it.id == p.item && it.module_block.as_deref() == Some(block.id.as_str()))).map(|(_, i)| *i).collect();
        let ms: Vec<f64> = in_block.iter().filter_map(|i| o.members.get(*i).and_then(|m| m.module)).collect();
        if let Some(least) = ms.iter().copied().reduce(f64::min)
            && ms.iter().any(|m| (m - least).abs() > 1e-9)
        {
            for &i in &in_block {
                read[i].ask.module = Some(least);
            }
            o = run(&read);
            let again: Vec<f64> = in_block.iter().filter_map(|i| o.members.get(*i).and_then(|m| m.module)).collect();
            if again.iter().any(|m| (m - least).abs() > 1e-9) || in_block.iter().any(|i| o.members.get(*i).is_some_and(|m| m.error.is_some())) {
                block_errs[b].get_or_insert(format!("module block {}: its gears can't share one module (module {} doesn't give every drawn diameter whole teeth); take the gear that won't fit out of the block, or redraw it", block.id, num(least)));
            }
        }
    }
    out.block_errors = blocks.iter().zip(&block_errs).map(|(b, e)| (b.id.clone(), e.clone())).collect();
    // Names: the reader's "member N" as "item <id> gear k".
    let rename = |t: &str| -> String {
        let mut s = t.to_string();
        for (i, n) in names.iter().enumerate().rev() {
            s = s.replace(&format!("members {} and", i + 1), &format!("{n} and")).replace(&format!("member {}", i + 1), n);
        }
        s
    };
    out.suggestions = o.suggestions.iter().map(|s| rename(s)).collect();
    out.notes = o.notes.iter().map(|s| rename(s)).collect();
    for (l, li) in links.iter().zip(&link_index) {
        let Some((mesh, k)) = li else {
            out.links.push(LinkOut { a: l.a.clone(), b: l.b.clone(), kind: l.kind, ratio: None, error: Some("one of its ends is no gear of an item here".into()), hint: Some("link two gears' pins".into()) });
            continue;
        };
        let e = o.link_errors.get(if *mesh { *k } else { given.mesh.len() + k }).cloned().flatten().map(|e| rename(&e));
        let hint = e.as_ref().map(|e| {
            if e.contains("don't touch") {
                "make the two circles tangent (Tangent constraint), or move one".to_string()
            } else if e.contains("one axis") {
                "put both circles on one axis: a shaft line through both centres, or concentric centres".to_string()
            } else if e.contains("one plane") {
                "a mesh: both circles in one plane; a shaft: at different places along it".to_string()
            } else {
                "link two gears' pins".to_string()
            }
        });
        let teeth = |p: &Port| find(p).and_then(|i| o.members.get(i)).and_then(|m| m.teeth);
        let ratio = match (l.kind, teeth(&l.a), teeth(&l.b)) {
            (LinkType::Mesh, Some(za), Some(zb)) if e.is_none() => Some(f64::from(zb) / f64::from(za)),
            _ => None,
        };
        out.links.push(LinkOut { a: l.a.clone(), b: l.b.clone(), kind: l.kind, ratio, error: e, hint });
    }
    let _ = inner;
    for (p, i) in &port_of {
        let m = o.members.get(*i).cloned().unwrap_or_default();
        out.gears.push(GearOut {
            item: p.item.clone(),
            gear: p.gear,
            teeth: m.teeth,
            module: m.module,
            pitch_d: m.pitch_diameter,
            face_width: m.face_width,
            error: m.error.as_deref().map(|e| rename(e)),
            ..GearOut::default()
        });
    }
    out.gear_ports = o.gears.iter().filter_map(|i| port_of.iter().find(|(_, j)| j == i).map(|(p, _)| p.clone())).collect();
    out.gear_items = o.gears.clone();
    out.seats = o.seats.clone();
    out.members = o.members.clone();
    let blocked = block_errs.iter().any(Option::is_some);
    let link_bad = out.links.iter().any(|l| l.error.is_some());
    out.error = o.error.as_deref().map(|e| rename(e)).or_else(|| blocked.then(|| block_errs.iter().flatten().cloned().collect::<Vec<_>>().join("; "))).or_else(|| link_bad.then(|| "a link can't be made: see its error".into()));
    out.solved = if out.error.is_none() { o.solved } else { None };
    out
}

impl ItemIn {
    fn items_ids_repeat(&self, all: &[ItemIn]) -> bool {
        all.iter().filter(|x| x.id == self.id).count() > 1
    }
}

/// Fills each gear's ratio, speed, torque and least contact ratio from the planned set, the
/// driver `drive` (default: the first gear) turning at its `rpm` with its `torque` (lossless).
pub fn report(r: &mut Resolved, plan: &Plan, drive: Option<&Drive>) -> Option<String> {
    let k_of = |p: &Port| r.gear_ports.iter().position(|q| q == p);
    let drive_k = match drive {
        Some(d) => match k_of(&Port { item: d.item.clone(), gear: d.gear }) {
            Some(k) => k,
            None => return Some(format!("the drive (item {} gear {}) is no gear of the set", d.item, d.gear + 1)),
        },
        None => 0,
    };
    let speed = |k: usize| plan.preview.get(k).map(|g| g.speed).filter(|v| *v != 0.0);
    let train = plan.members.len() == r.gear_ports.len();
    for g in &mut r.gears {
        let Some(k) = k_of(&Port { item: g.item.clone(), gear: g.gear }) else { continue };
        if let (true, Some(sk), Some(sd)) = (train, speed(k), speed(drive_k)) {
            let ratio = sk / sd;
            g.ratio = Some(ratio);
            g.rpm = drive.and_then(|d| d.rpm).map(|rpm| rpm * ratio);
            g.torque = drive.and_then(|d| d.torque).map(|t| t / ratio.abs());
        }
        // Its meshes' contact ratios, from the plan's facts ("gears 2–5 contact ratio": the gears
        // by their rows, here the members' places).
        let me = r.gear_items.get(k).map_or(k, |i| *i) + 1;
        let crs: Vec<f64> = plan
            .facts
            .iter()
            .filter(|f| f.label.ends_with(" contact ratio") && !f.label.contains("total"))
            .filter(|f| {
                let head = f.label.trim_start_matches("gears ").split(' ').next().unwrap_or("");
                head.split('–').any(|x| x.parse::<usize>().ok() == Some(me))
            })
            .map(|f| f.value)
            .collect();
        g.contact_ratio = crs.iter().copied().reduce(f64::min).or_else(|| plan.facts.iter().find(|f| f.label == "contact ratio").map(|f| f.value));
        if g.contact_ratio.is_some_and(|c| c < 1.2) {
            g.warnings.push(format!("contact ratio {:.2} is low (1.2 or more is usual): fewer, larger teeth make it worse, more teeth better", g.contact_ratio.unwrap_or(0.0)));
        }
    }
    None
}
