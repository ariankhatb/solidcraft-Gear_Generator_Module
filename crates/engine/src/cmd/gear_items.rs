//! `gear.create` / `gear.calculate` / `gear.update` with `items` (GEAR_UI_SPEC.md §3, §6): a
//! whole drivetrain of gear items, planned by `solidcraft_gear::plan::system::plan_items` and
//! built as one Gears feature (the assembly made of its plan with `assembly`).
//!
//! The request is read into the gear crate's items contract ([`solidcraft_gear::items`]): each
//! item's picked components are read where they are now through the sketch reader the other
//! requests use ([`super::sketch_items`] on [`Reads::gear_sketch`], so flat and 3D sketches, any
//! number of them; model edges, faces and axes as `axis_of` reads them), each keeping its sketch
//! and entity id, so evaluating the feature again after a sketch edit reads the new geometry.
//!
//! Bodies are named by item and gear ("Gear A·1", "Gear E·1+2" for a compound's one body,
//! "Carrier P", "Planet P 2"), not by their place in the plan: the feature keeps these names
//! (`body_keys`) beside its body names, so a re-plan that orders the bodies otherwise keeps every
//! body's name, component and joint, and an update that adds or takes away an item keeps the
//! others' ([`carry_names`]).

use serde_json::{Value, json};
use solidcore_record::Vec3;
use solidcraft_geom::Vec3Geom;
use solidcraft_gear::analysis::{self, MeshKind, StepKind};
use solidcraft_gear::drivetrain;
use solidcraft_gear::items::{self as gi, GearRef};
use solidcraft_gear::plan::system::{SystemPlan, plan_items};
use solidcraft_gear::plan::{Plan, Spec, solid};

use super::{GearAxis, Reads, axis_of, bad, sketch_items, spec_of};
use crate::schema::gear::{
    GearComponent, GearDetailSuggestion, GearEndReply, GearItemGear, GearItemParams, GearItemReply, GearLinkReply, GearParams, GearRatioParams, GearRatioResult, GearRatioStep, GearResult, GearSuggestionReply, GearType,
};
use crate::{Result, Session};

/// At most this many items, components per item and gears per item.
const MAX_ITEMS: usize = 64;

/// A request's items, planned: their ids and the plan of the whole drivetrain (its `plan` moved
/// out: the request's [`Plan`]).
#[derive(Clone, Debug)]
pub(super) struct Items {
    pub(super) ids: Vec<String>,
    pub(super) system: SystemPlan,
    /// What the sketch gives beyond the gear crate's plan: side A, bores and hubs.
    pub(super) extra: Extra,
}

/// The sketch-driven details of a set of items ([`gear_details`], [`apply_details`]).
#[derive(Clone, Debug, Default)]
pub(super) struct Extra {
    /// Each gear's side A (its axis direction: the pitch circle's sketch normal, or its shaft
    /// line's direction from its first point).
    side_a: Vec<(GearRef, [f64; 3])>,
    /// The bore and hub diameters built on each gear.
    built: Vec<(GearRef, Option<f64>, Option<f64>)>,
    /// Concentric circles the drawing suggests as bores and hubs.
    suggested: Vec<GearDetailSuggestion>,
    /// Each differential on a ring gear, as planned (`GearResult::differentials`).
    diffs: Vec<solidcraft_gear::plan::system::DiffReply>,
}

fn ids_of(items: &[GearItemParams]) -> Vec<String> {
    items.iter().enumerate().map(|(i, it)| it.id.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| gi::item_id(i))).collect()
}

/// The item `v` names: its id, or its index.
fn item_index(ids: &[String], v: &Value) -> Option<usize> {
    match v {
        Value::String(s) => ids.iter().position(|id| id == s),
        Value::Number(n) => n.as_u64().and_then(|n| usize::try_from(n).ok()).filter(|n| *n < ids.len()),
        _ => None,
    }
}

fn dt_thing(t: drivetrain::Thing) -> gi::Thing {
    match t {
        drivetrain::Thing::Circle(c) => gi::Thing::circle(c.c, c.n, c.r),
        drivetrain::Thing::Line(l) => gi::Thing::line(l.a, l.b),
        drivetrain::Thing::Point(p) => gi::Thing::point(p),
    }
}

/// Who a picked component is, for the errors that name it: its item, its slot (as the dialog
/// labels it, else its place) and where it is in the request.
struct Pick<'a> {
    item: &'a str,
    slot: String,
    what: String,
}

impl Pick<'_> {
    fn new<'a>(item: &'a str, k: usize, c: &GearComponent, what: String) -> Pick<'a> {
        let slot = c.slot.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| format!("pick {}", k + 1));
        Pick { item, slot, what }
    }
    fn said(&self, why: impl std::fmt::Display) -> String {
        format!("Gear {}, {}: {why}", self.item, slot_label(&self.slot))
    }
    /// What to do about a pick that is gone.
    fn repick(&self) -> String {
        format!("click Gear {}'s card, then its replacement in the view", self.item)
    }
}

/// A slot as the dialog labels it ("pitch": "pitch circle", "axis2": "shaft line 2").
fn slot_label(slot: &str) -> String {
    match slot {
        "bore" => return "bore circle".into(),
        "hub" => return "hub circle".into(),
        _ => {}
    }
    gi::ItemKind::ALL
        .iter()
        .flat_map(|k| gi::slots(*k).iter().flat_map(|l| l.iter()))
        .find(|s| s.name == slot)
        .map_or_else(|| slot.to_string(), |s| s.what.to_string())
}

/// Whether a sketch has an entity (a point, curve or 3D wire) with id `entity`, whatever it is.
fn has_entity(ss: &solidcraft_doc::SolvedSketch, entity: &str) -> bool {
    let sk = &ss.sketch;
    sk.points.iter().any(|p| p.id == entity) || sk.curves.iter().any(|c| c.id == entity) || sk.wires.iter().any(|w| w.id == entity)
}

/// One picked component where it is now. A sketch entity is found by its sketch's id and its own
/// id only: one that is gone (deleted, its sketch deleted or after the feature) is an error that
/// names the item, the slot, the sketch and the entity, never another entity read instead.
fn thing_of(s: &dyn Reads, cmd: &str, who: &Pick, c: &GearComponent) -> Result<gi::Thing> {
    let what = who.what.as_str();
    let given = [("sketch point", c.point.is_some()), ("sketch curve", c.curve.is_some()), ("edge", c.edge.is_some()), ("face", c.face.is_some()), ("axis", c.axis.is_some())];
    let named: Vec<&str> = given.iter().filter(|(_, g)| *g).map(|(n, _)| *n).collect();
    if named.len() != 1 {
        let extra = if named.is_empty() { String::new() } else { format!(", not {}", named.join(" and ")) };
        return Err(bad(cmd, format!("`{what}`: give exactly one of {{sketch, curve}}, {{sketch, point}}, {{edge: [x,y,z]}}, {{face: [x,y,z]}} or {{axis}}{extra}")));
    }
    if (c.point.is_some() || c.curve.is_some()) != c.sketch.is_some() {
        return Err(bad(cmd, format!("`{what}`: a sketch point or curve needs `sketch` (and `sketch` a point or curve)")));
    }
    let line = |a: &solidcraft_gear::axes::Axis| gi::Thing::line(a.origin, solidcraft_gear::select::bearing_point(a, 1.0));
    if let (Some(sk), Some(entity)) = (&c.sketch, c.curve.as_ref().or(c.point.as_ref())) {
        let label = c.sketch_name.clone().map_or_else(|| super::super::features::sketch_key(Some(sk)).unwrap_or_default(), |n| format!("'{n}'"));
        let id = s.sketch_id(Some(sk), cmd, what).map_err(|_| bad(cmd, who.said(format!("sketch {label} is gone (deleted): {}", who.repick()))))?;
        let Some(ss) = s.solved(id) else {
            let name = s.sketch_name(id).map_or(label, |n| format!("'{n}'"));
            return Err(bad(cmd, who.said(format!("sketch {name} isn't there before the Gears feature (it comes after it in the timeline, or it is suppressed or failed): move it before the Gears feature"))));
        };
        let name = ss.name.clone();
        let kind = if c.point.is_some() { "point" } else { "curve" };
        if !has_entity(&ss, entity) {
            return Err(bad(cmd, who.said(format!("{kind} {entity} is gone from sketch '{name}': {} (or draw it again with id {entity})", who.repick()))));
        }
        let gs = s.gear_sketch(id).map_err(|e| bad(cmd, who.said(e)))?;
        if let Some((_, it)) = sketch_items(&gs).into_iter().find(|(k, _)| k == entity) {
            return Ok(dt_thing(it.thing).with_ids(Some(id), entity));
        }
        // A line from one circle's centre to another's (read as a centre line by the sketch
        // reader) is a line all the same here: a shaft or an axis picked on purpose.
        if let Some((_, a, b)) = gs.centre_lines.iter().find(|(k, _, _)| k == entity)
            && let (Some((_, ca)), Some((_, cb))) = (gs.circles.get(*a), gs.circles.get(*b))
        {
            let f = &gs.frame;
            return Ok(gi::Thing::line(f.world([ca.c.x, ca.c.y, 0.0]), f.world([cb.c.x, cb.c.y, 0.0])).with_ids(Some(id), entity));
        }
        // The sketch's origin, a construction line: as the sketch has them.
        let construction = c.curve.as_ref().is_some_and(|k| ss.sketch.curves.iter().any(|cv| cv.id == *k && cv.construction));
        let a = axis_of(s, &GearAxis(json!({"sketch": id, "point": c.point, "curve": c.curve})), cmd, what).map_err(|_| {
            let circle = ss.sketch.curves.iter().any(|cv| cv.id == *entity && matches!(cv.kind, crate::sketch::CurveKind::Circle { .. } | crate::sketch::CurveKind::Arc { .. }));
            let why = if circle && construction { "is construction geometry now: a pitch circle must be drawn geometry" } else { "isn't a circle, line or point the set can read" };
            bad(cmd, who.said(format!("{kind} `{entity}` in sketch '{name}' {why}")))
        })?;
        let t = if c.point.is_some() {
            gi::Thing::point(a.origin)
        } else {
            gi::Thing::Line { a: a.origin, b: solidcraft_gear::select::bearing_point(&a, 1.0), construction, ids: gi::ThingIds::default() }
        };
        return Ok(t.with_ids(Some(id), entity));
    }
    let v = if let Some(e) = &c.edge {
        json!({"edge": e})
    } else if let Some(f) = &c.face {
        json!({"face": f})
    } else {
        c.axis.clone().unwrap_or(Value::Null)
    };
    Ok(line(&axis_of(s, &GearAxis(v), cmd, what)?))
}

fn positive(cmd: &str, what: &str, v: Option<f64>) -> Result<Option<f64>> {
    match v {
        Some(x) if !(x.is_finite() && x > 0.0 && x < 1e6) => Err(bad(cmd, format!("`{what}` must be a number greater than 0"))),
        _ => Ok(v),
    }
}

/// An item's differential as the gear crate takes it (`what`: where it is in the request).
fn diff_of(cmd: &str, what: &str, kind: gi::ItemKind, d: &crate::schema::gear::GearItemDiffParams) -> Result<gi::DiffItem> {
    use solidcraft_gear::diff::Style;
    if !matches!(kind, gi::ItemKind::Hypoid | gi::ItemKind::Bevel) {
        return Err(bad(cmd, format!("`{what}.differential`: a differential rides on a ring gear: give it to a hypoid or bevel pair")));
    }
    let style = match d.style.as_deref() {
        None => Style::Open,
        Some(s) => Style::parse(s).ok_or_else(|| {
            let names: Vec<&str> = Style::ALL.iter().map(|k| k.name()).collect();
            bad(cmd, format!("`{what}.differential.style` must be one of {}, not `{s}`", names.join(", ")))
        })?,
    };
    let mut out = gi::DiffItem::of(style);
    let s = &mut out.settings;
    macro_rules! put {
        ($field:ident, $src:expr) => {
            if let Some(v) = $src.clone() {
                s.$field = v;
            }
        };
    }
    put!(side_teeth, d.side_teeth);
    put!(side_teeth2, d.side_teeth2);
    put!(pinion_teeth, d.pinion_teeth);
    put!(pinions, d.pinions);
    put!(face_width, d.face_width);
    put!(spiral, d.spiral_angle);
    put!(helix, d.helix_angle);
    put!(pressure_angle, d.pressure_angle);
    put!(backlash, d.backlash);
    put!(friction, d.friction);
    put!(pin_diameter, d.pin_diameter);
    put!(wall, d.wall);
    put!(washer, d.washer);
    put!(clearance, d.clearance);
    put!(bearing, d.bearing);
    put!(windows, d.windows);
    put!(ring_bolts, d.ring_bolts);
    put!(bolts, d.bolts);
    put!(bolt_diameter, d.bolt_diameter);
    let axle = d.axle.clone().unwrap_or_default();
    out.module = positive(cmd, &format!("{what}.differential.module"), d.module)?;
    out.axle_length = positive(cmd, &format!("{what}.differential.axle.length"), axle.length)?;
    out.axle_diameter = positive(cmd, &format!("{what}.differential.axle.diameter"), axle.diameter)?;
    // The settings checked as the differential generator checks them (at a stand-in module and
    // axle where those are Auto: the planner sizes them to the ring).
    let probe = solidcraft_gear::plan::Spec {
        diff: solidcraft_gear::diff::Diff {
            module: out.module.unwrap_or(1.0),
            axle_diameter: out.axle_diameter.unwrap_or(1.0),
            drive: solidcraft_gear::diff::DriveKind::None,
            ..out.settings.clone()
        },
        ..solidcraft_gear::plan::Spec::defaults_for(solidcraft_gear::plan::Kind::Differential)
    };
    solidcraft_gear::diff::check(&probe).map_err(|e| bad(cmd, format!("`{what}.differential`: {e}")))?;
    Ok(out)
}

fn bevel_style(cmd: &str, what: &str, s: Option<&str>) -> Result<Option<gi::BevelStyle>> {
    Ok(match s {
        None => None,
        Some("straight") => Some(gi::BevelStyle::Straight),
        Some("spiral") => Some(gi::BevelStyle::Spiral),
        Some("chevron") => Some(gi::BevelStyle::Chevron),
        Some(other) => return Err(bad(cmd, format!("`{what}.bevel_style` must be straight, spiral or chevron, not `{other}`"))),
    })
}

/// One gear's settings as the gear crate takes them.
fn settings_of(cmd: &str, what: &str, it: &GearItemParams, g: &GearItemGear) -> Result<gi::GearSettings> {
    let size_by = match g.size_by.as_deref() {
        Some("module") => gi::SizeBy::Module,
        Some("teeth") => gi::SizeBy::Teeth,
        None if g.teeth.is_some() && g.module.is_none() => gi::SizeBy::Teeth,
        None => gi::SizeBy::Module,
        Some(other) => return Err(bad(cmd, format!("`{what}.size_by` must be module or teeth, not `{other}`"))),
    };
    let module = positive(cmd, &format!("{what}.module"), g.module)?;
    if g.teeth.is_some_and(|z| z == 0 || z > 10_000) {
        return Err(bad(cmd, format!("`{what}.teeth` must be 1 to 10000")));
    }
    let mut face = gi::FaceSides::default();
    if let Some(f) = &g.face {
        for (side, v) in [("a", f.a), ("b", f.b)] {
            if v.is_some_and(|v| !(v.is_finite() && (0.0..1e6).contains(&v))) {
                return Err(bad(cmd, format!("`{what}.face.{side}` must be 0 or more (mm), or null for Auto")));
            }
        }
        face.a = f.a;
        face.b = f.b;
        face.preset = match f.preset.as_deref() {
            Some("symmetric") => gi::FacePreset::Symmetric,
            Some("a_only" | "a") => gi::FacePreset::AOnly,
            Some("b_only" | "b") => gi::FacePreset::BOnly,
            Some("custom") => gi::FacePreset::Custom,
            Some(other) => return Err(bad(cmd, format!("`{what}.face.preset` must be symmetric, a_only, b_only or custom, not `{other}`"))),
            None => match (f.a, f.b) {
                (None, None) => gi::FacePreset::Symmetric,
                (Some(a), Some(b)) if (a - b).abs() < 1e-12 => gi::FacePreset::Symmetric,
                (_, Some(b)) if b <= 0.0 => gi::FacePreset::AOnly,
                (Some(a), _) if a <= 0.0 => gi::FacePreset::BOnly,
                _ => gi::FacePreset::Custom,
            },
        };
    }
    if g.helix_angle.is_some_and(|h| !(h.is_finite() && h.abs() < 89.0)) {
        return Err(bad(cmd, format!("`{what}.helix_angle` must be between -89 and 89 degrees")));
    }
    let profile_shift = match &g.profile_shift {
        Value::Null => None,
        Value::String(t) if t == "auto" => None,
        Value::Number(n) => match n.as_f64().filter(|x| x.is_finite() && x.abs() <= 5.0) {
            Some(x) => Some(x),
            None => return Err(bad(cmd, format!("`{what}.profile_shift` must be between -5 and 5, or \"auto\""))),
        },
        _ => return Err(bad(cmd, format!("`{what}.profile_shift` must be a number or \"auto\""))),
    };
    let bore = match &g.bore {
        Value::String(t) if t == "auto" => gi::Bore::Auto,
        Value::Null => gi::Bore::Solid,
        Value::String(t) if t == "none" => gi::Bore::Solid,
        Value::Number(n) => match n.as_f64().filter(|d| d.is_finite() && *d > 0.0 && *d < 1e6) {
            Some(d) => gi::Bore::Diameter(d),
            None => return Err(bad(cmd, format!("`{what}.bore` must be a diameter greater than 0"))),
        },
        _ => return Err(bad(cmd, format!("`{what}.bore` must be \"auto\", null (no hole) or a diameter"))),
    };
    let hub = match &g.hub {
        None => None,
        Some(h) => {
            let len = |side: &str, v: Option<f64>| -> Result<f64> {
                match v {
                    None => Ok(0.0),
                    Some(x) if x.is_finite() && (0.0..1e6).contains(&x) => Ok(x),
                    Some(_) => Err(bad(cmd, format!("`{what}.hub.length_{side}` must be 0 or more (mm)"))),
                }
            };
            Some(gi::Hub { diameter: positive(cmd, &format!("{what}.hub.diameter"), h.diameter)?, length_a: len("a", h.length_a)?, length_b: len("b", h.length_b)?, auto: h.auto == Some(true) })
        }
    };
    let style = match bevel_style(cmd, what, g.bevel_style.as_deref().or(it.bevel_style.as_deref()))? {
        Some(s) => s,
        None if it.kind == "spiral_bevel" => gi::BevelStyle::Spiral,
        None if it.kind == "chevron_bevel" => gi::BevelStyle::Chevron,
        None => gi::BevelStyle::Straight,
    };
    let spiral = g.spiral_angle.or(it.spiral_angle);
    if spiral.is_some_and(|a| !(a.is_finite() && a.abs() < 89.0)) {
        return Err(bad(cmd, format!("`{what}.spiral_angle` must be between -89 and 89 degrees")));
    }
    let starts = g.starts.or(it.starts);
    if starts.is_some_and(|n| n == 0 || n > 16) {
        return Err(bad(cmd, format!("`{what}.starts` must be 1 to 16")));
    }
    Ok(gi::GearSettings {
        size_by,
        module,
        teeth: g.teeth,
        face,
        helix: g.helix_angle,
        hand: g.hand.map(|h| if h == crate::schema::gear::SpiralHand::Left { gi::Hand::Left } else { gi::Hand::Right }),
        v: g.herringbone == Some(true),
        profile_shift,
        bore,
        hub,
        rim: positive(cmd, &format!("{what}.rim"), g.rim.or(it.rim))?,
        starts,
        throated: g.throated.or(it.throated) == Some(true),
        spiral_angle: spiral,
        bevel_style: style,
    })
}

/// A detail picked for an item's gear (`slot: bore` or `hub`): a circle concentric with one of
/// its pitch circles; kept by its sketch and id like any pick, read where it is now.
#[derive(Clone, Debug)]
struct DetailPick {
    item: usize,
    hub: bool,
    thing: gi::Thing,
    /// Who it is, for its errors (item and slot).
    said: String,
}

/// Whether a component is a detail (`Some(true)` a hub circle, `Some(false)` a bore circle), not
/// a pick for the gear crate's slots.
fn detail_slot(c: &GearComponent) -> Option<bool> {
    match c.slot.as_deref().map(str::trim) {
        Some(s) if s.eq_ignore_ascii_case("bore") => Some(false),
        Some(s) if s.eq_ignore_ascii_case("hub") => Some(true),
        _ => None,
    }
}

/// The request read: the items' ids, the items, links, module blocks, drive and detail picks.
struct ReadItems {
    ids: Vec<String>,
    items: Vec<gi::GearItem>,
    links: Vec<gi::ItemLink>,
    blocks: Vec<gi::ModuleBlock>,
    drive: Option<gi::Drive>,
    details: Vec<DetailPick>,
}

/// What the request's items, links, module blocks and drive say, as the gear crate takes them.
fn read_items(s: &dyn Reads, cmd: &str, p: &GearParams, list: &[GearItemParams]) -> Result<ReadItems> {
    if p.sketch.is_some() || p.selection.is_some() || p.axis1.is_some() || p.axis2.is_some() || p.placement.is_some() {
        return Err(bad(cmd, "give `items`, or `sketch`, `selection`, `axis1` or `placement`: one of them"));
    }
    if list.is_empty() {
        return Err(bad(cmd, "`items` is empty: add a gear item (pick a pitch circle)"));
    }
    if list.len() > MAX_ITEMS {
        return Err(bad(cmd, format!("`items` takes {MAX_ITEMS} items at most")));
    }
    let ids = ids_of(list);
    for (i, id) in ids.iter().enumerate() {
        if ids[..i].contains(id) {
            return Err(bad(cmd, format!("`items`: two items are called {id}: give each its own id")));
        }
    }
    let mut items = Vec::with_capacity(list.len());
    let mut details = Vec::new();
    for (i, it) in list.iter().enumerate() {
        let what = format!("items[{i}]");
        let kind = gi::ItemKind::parse(&it.kind).ok_or_else(|| {
            let names: Vec<&str> = gi::ItemKind::ALL.iter().map(|k| k.name()).collect();
            bad(cmd, format!("`{what}.type` must be one of {}, not `{}`", names.join(", "), it.kind))
        })?;
        if it.components.len() > MAX_ITEMS || it.gears.len() > MAX_ITEMS {
            return Err(bad(cmd, format!("`{what}`: {MAX_ITEMS} components and gears at most")));
        }
        let mut item = gi::GearItem::new(&ids[i], kind);
        for (k, c) in it.components.iter().enumerate() {
            let who = Pick::new(&ids[i], k, c, format!("{what}.components[{k}]"));
            let thing = thing_of(s, cmd, &who, c)?;
            match detail_slot(c) {
                Some(hub) => {
                    if !matches!(thing, gi::Thing::Circle { .. }) {
                        return Err(bad(cmd, who.said("a bore or hub is a circle drawn concentric with the gear's pitch circle")));
                    }
                    details.push(DetailPick { item: i, hub, thing, said: format!("Gear {}, {}", ids[i], slot_label(&who.slot)) });
                }
                None => item.things.push(thing),
            }
        }
        let mut gears: Vec<Option<gi::GearSettings>> = Vec::new();
        for (k, g) in it.gears.iter().enumerate() {
            let at = g.gear.unwrap_or(k);
            if at >= MAX_ITEMS {
                return Err(bad(cmd, format!("`{what}.gears[{k}].gear` is out of range")));
            }
            if gears.len() <= at {
                gears.resize(at + 1, None);
            }
            gears[at] = Some(settings_of(cmd, &format!("{what}.gears[{k}]"), it, g)?);
        }
        // No gears given: the item's own settings (a worm's starts...) on default gears.
        if gears.is_empty() {
            gears.push(Some(settings_of(cmd, &what, it, &GearItemGear::default())?));
        }
        let first = gears.iter().flatten().next().copied().unwrap_or_default();
        item.gears = gears.into_iter().map(|g| g.unwrap_or(gi::GearSettings { teeth: None, ..first })).collect();
        item.planets = it.planets;
        item.carrier = it.carrier != Some(false);
        item.back_height = positive(cmd, &format!("{what}.back_height"), it.back_height)?;
        item.shaft_diameter = positive(cmd, &format!("{what}.diameter"), it.diameter)?;
        item.keyway = it.keyway == Some(true);
        if let Some(d) = &it.differential {
            item.differential = Some(diff_of(cmd, &what, kind, d)?);
        }
        items.push(item);
    }
    let end = |what: &str, e: &crate::schema::gear::GearEnd| -> Result<GearRef> {
        let item = item_index(&ids, &e.item).ok_or_else(|| bad(cmd, format!("`{what}.item`: no item {}", e.item)))?;
        Ok(GearRef::new(item, e.gear))
    };
    let mut links = Vec::new();
    for (k, l) in p.links.iter().flatten().enumerate() {
        let what = format!("links[{k}]");
        let kind = match l.kind.as_deref() {
            None => gi::LinkKind::Mesh,
            Some(t) => gi::LinkKind::parse(t).ok_or_else(|| bad(cmd, format!("`{what}.kind` must be mesh or shaft, not `{t}`")))?,
        };
        links.push(gi::ItemLink { a: end(&format!("{what}.a"), &l.a)?, b: end(&format!("{what}.b"), &l.b)?, kind });
    }
    let mut blocks = Vec::new();
    for (k, b) in p.module_blocks.iter().flatten().enumerate() {
        let what = format!("module_blocks[{k}]");
        let id = b.id.clone().unwrap_or_else(|| format!("M{}", k + 1));
        let mut members = Vec::new();
        for m in b.members.iter().flatten() {
            members.push(item_index(&ids, m).ok_or_else(|| bad(cmd, format!("`{what}.members`: no item {m}")))?);
        }
        members.extend(list.iter().enumerate().filter(|(_, it)| it.module_block.as_deref() == Some(id.as_str())).map(|(i, _)| i));
        members.sort_unstable();
        members.dedup();
        blocks.push(gi::ModuleBlock { id, module: positive(cmd, &format!("{what}.module"), b.module)?, members });
    }
    for (i, it) in list.iter().enumerate() {
        if let Some(b) = &it.module_block
            && !blocks.iter().any(|x| x.id == *b)
        {
            return Err(bad(cmd, format!("`items[{i}].module_block`: no module block {b} in `module_blocks`")));
        }
    }
    let drive = match &p.drive {
        None => None,
        Some(d) => {
            for (name, v) in [("rpm", d.rpm), ("torque", d.torque)] {
                if v.is_some_and(|v| !v.is_finite() || v.abs() > 1e9) {
                    return Err(bad(cmd, format!("`drive.{name}` must be a finite number")));
                }
            }
            let item = item_index(&ids, &d.item).ok_or_else(|| bad(cmd, format!("`drive.item`: no item {}", d.item)))?;
            Some(gi::Drive { item, gear: d.gear, rpm: d.rpm, torque: d.torque })
        }
    };
    Ok(ReadItems { ids, items, links, blocks, drive, details })
}

/// The bodies named by item and gear (see the module docs), each name once.
fn name_bodies(sp: &mut SystemPlan, ids: &[String], items: &[gi::GearItem]) {
    let n = sp.plan.bodies.len();
    let mut refs: Vec<Vec<GearRef>> = vec![Vec::new(); n];
    for (r, m) in &sp.member_of {
        if let Some(b) = sp.plan.members.get(*m).and_then(|m| m.body).filter(|b| *b < n)
            && !refs[b].contains(r)
        {
            refs[b].push(*r);
        }
    }
    let id = |i: usize| ids.get(i).cloned().unwrap_or_else(|| gi::item_id(i));
    let mut names: Vec<Option<String>> = refs
        .iter()
        .map(|rs| {
            let first = rs.first()?;
            // A Shaft item's bar, a body of its own: "Shaft S".
            if items.get(first.item).is_some_and(|it| it.kind == gi::ItemKind::Shaft) {
                return Some(format!("Shaft {}", id(first.item)));
            }
            if rs.iter().all(|r| r.item == first.item) {
                let gears: Vec<String> = rs.iter().map(|r| (r.gear + 1).to_string()).collect();
                Some(format!("Gear {}·{}", id(first.item), gears.join("+")))
            } else {
                Some(rs.iter().map(|r| format!("Gear {}·{}", id(r.item), r.gear + 1)).collect::<Vec<_>>().join(" + "))
            }
        })
        .collect();
    for (item, b) in &sp.carriers {
        if let Some(slot) = names.get_mut(*b) {
            *slot = Some(format!("Carrier {}", id(*item)));
        }
    }
    // The other planets of a planetary set: by their carrier's item.
    let asm = &sp.plan.assembly;
    let mut count: Vec<(usize, usize)> = Vec::new();
    for (b, name) in names.iter_mut().enumerate() {
        if name.is_some() {
            continue;
        }
        let carrier_body = asm.members.iter().find(|m| m.body == b).and_then(|m| m.carrier).and_then(|c| asm.members.get(c)).map(|c| c.body);
        if let Some(item) = carrier_body.and_then(|cb| sp.carriers.iter().find(|(_, x)| *x == cb)).map(|(i, _)| *i) {
            let k = match count.iter_mut().find(|(i, _)| *i == item) {
                Some(c) => {
                    c.1 += 1;
                    c.1
                }
                None => {
                    count.push((item, 2));
                    2
                }
            };
            *name = Some(format!("Planet {} {k}", id(item)));
        }
    }
    let mut taken: Vec<String> = Vec::new();
    for (b, body) in sp.plan.bodies.iter_mut().enumerate() {
        let base = names[b].clone().unwrap_or_else(|| body.name.clone());
        let mut name = base.clone();
        let mut k = 2;
        while taken.contains(&name) {
            name = format!("{base} ({k})");
            k += 1;
        }
        taken.push(name.clone());
        body.name = name;
    }
}

/// The set `items` asks for: the items read, planned whole. `Err` when the request can't be
/// read; a drivetrain that can't be built comes back with [`SystemPlan::error`] set.
pub(super) fn plan(s: &dyn Reads, cmd: &str, p: &GearParams, list: &[GearItemParams]) -> Result<(Items, Plan, Spec)> {
    let ReadItems { ids, items, links, blocks, drive, details } = read_items(s, cmd, p, list)?;
    let mut base = spec_of(&GearParams { kind: Some(GearType::Spur), items: None, links: None, module_blocks: None, drive: None, ..p.clone() }).map_err(|e| bad(cmd, e))?;
    if let Some(gap) = p.keep_out_gap
        && !(gap.is_finite() && (0.0..1e3).contains(&gap))
    {
        return Err(bad(cmd, "`keep_out_gap` must be 0 or more (mm)"));
    }
    let keep_out = p.keep_out_gap.is_some() || p.keep_out_cuts.as_ref().is_some_and(|c| !c.is_empty()) || p.clearance_body.is_some();
    if keep_out {
        let mut c = base.clearance_body.clone().unwrap_or_default();
        if let Some(gap) = p.keep_out_gap {
            c.radial = gap;
            c.axial = gap;
        }
        // Every bearing point picked (a Shaft item's, a gear's on its shaft line): a seat of the
        // set's bearing, where the point is now (so the seats follow the sketch).
        c.seats.extend(gi::bearing_points(&items).into_iter().map(solidcraft_gear::clearance::Seat::Point));
        base.clearance_body = Some(c);
    }
    let mut system = plan_items(&items, &links, &blocks, drive.as_ref(), &Spec { clearance_body: None, ..base.clone() }).map_err(|e| bad(cmd, e))?;
    loose_sketches(s, cmd, list, &mut system);
    let mut extra = Extra { side_a: side_a(&items), ..Extra::default() };
    let wanted = gear_details(cmd, &items, &details)?;
    extra.suggested = suggest_details(s, &ids, &items, &details, &wanted);
    if system.error.is_none() {
        extra.built = apply_details(&items, &wanted, &mut system);
        if let Some(tc) = &p.tooth_chamfer {
            use solidcraft_gear::plan::chamfer::{ChamferFaces, ToothChamfer, tooth_chamfer};
            let faces = ChamferFaces::parse(tc.faces.as_deref().unwrap_or("both")).ok_or_else(|| bad(cmd, "`tooth_chamfer.faces` must be \"both\", \"a\" or \"b\""))?;
            if !(tc.size.is_finite() && tc.size >= 0.0) {
                return Err(bad(cmd, "`tooth_chamfer.size` must be 0 or more (mm)"));
            }
            tooth_chamfer(&items, &mut system, ToothChamfer { size: tc.size, faces }).map_err(|e| bad(cmd, e))?;
        }
        name_bodies(&mut system, &ids, &items);
        if keep_out {
            let mut whole = system.plan.clone();
            match solidcraft_gear::clearance::clearance_body(&base, &mut whole) {
                Ok(mut body) => {
                    // The housing cutout: one body, no member (no component, no joint).
                    body.name = "Housing cutout".into();
                    whole.clearance = Some(body);
                    system.plan = whole;
                }
                Err(e) => system.plan.warnings.push(format!("the keep-out body can't be made: {e}")),
            }
        }
    }
    extra.diffs = solidcraft_gear::plan::system::differential_replies(&items, &system);
    let plan = std::mem::take(&mut system.plan);
    Ok((Items { ids, system, extra }, plan, base))
}

// ---------------------------------------------------------------------------------------------
// Details the sketch gives: side A, bores and hubs
// ---------------------------------------------------------------------------------------------

/// The items whose gears take bore and hub circles: gears on parallel shafts, whose bodies stand
/// on their pitch circles' planes (a ring, rack, bevel, hypoid, worm or planetary set keeps its
/// own bores).
fn takes_details(k: gi::ItemKind) -> bool {
    matches!(k, gi::ItemKind::Spur | gi::ItemKind::Helical | gi::ItemKind::Herringbone | gi::ItemKind::Compound)
}

fn v3(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// Whether circle (`c`, `n`) is concentric with gear `g`: the same centre and plane direction.
fn concentric(g: &gi::GearGeom, c: [f64; 3], n: [f64; 3]) -> bool {
    let r = g.radius.unwrap_or(1.0);
    let parallel = match (v3(g.axis).normalized(), v3(n).normalized()) {
        (Some(a), Some(b)) => a.dot(b).abs() >= 1.0 - 1e-6,
        _ => false,
    };
    v3(g.centre).dist(v3(c)) <= gi::tol(r) && parallel
}

/// Each gear's side A: the direction of its axis its face's side A reaches (decision 4).
fn side_a(items: &[gi::GearItem]) -> Vec<(GearRef, [f64; 3])> {
    let mut out = Vec::new();
    for (i, it) in items.iter().enumerate() {
        for (g, geo) in gi::gear_geoms(it).into_iter().enumerate() {
            if let Some(u) = v3(geo.axis).normalized() {
                out.push((GearRef::new(i, g), [u.x, u.y, u.z]));
            }
        }
    }
    out
}

/// A gear's bore and hub as asked: from its settings, then from the circles picked for it.
#[derive(Clone, Copy, Debug)]
struct Wanted {
    gear: GearRef,
    bore: Option<f64>,
    /// A hub asked with its Ø on auto: 1.6 × the bore, or half the root Ø (within the roots).
    hub_auto: bool,
    hub: Option<f64>,
    /// How far the hub reaches past the face on side A and side B (mm; both 0: half the face
    /// width on side A).
    len_a: f64,
    len_b: f64,
}

/// What each gear's bore and hub are: a `bore` circle picked concentric with its pitch circle
/// gives its bore Ø, a `hub` circle its hub Ø (the hub's length from its settings), over what its
/// settings say.
fn gear_details(cmd: &str, items: &[gi::GearItem], details: &[DetailPick]) -> Result<Vec<Wanted>> {
    let mut out: Vec<Wanted> = Vec::new();
    for (i, it) in items.iter().enumerate().filter(|(_, it)| takes_details(it.kind)) {
        for g in 0..gi::gear_geoms(it).len() {
            let st = it.settings(g);
            let bore = match st.bore {
                gi::Bore::Diameter(d) => Some(d),
                _ => None,
            };
            let (hub, len_a, len_b) = st.hub.map_or((None, 0.0, 0.0), |h| (h.diameter, h.length_a, h.length_b));
            let hub_auto = st.hub.is_some_and(|h| h.auto && h.diameter.is_none());
            if bore.is_some() || hub.is_some() || hub_auto {
                out.push(Wanted { gear: GearRef::new(i, g), bore, hub, hub_auto, len_a, len_b });
            }
        }
    }
    for d in details {
        let it = &items[d.item];
        let gi::Thing::Circle { centre, normal, radius, .. } = d.thing else { continue };
        if !takes_details(it.kind) {
            return Err(bad(cmd, format!("{}: bore and hub circles are taken by spur, helical, herringbone and compound gears", d.said)));
        }
        let geoms = gi::gear_geoms(it);
        let g = geoms
            .iter()
            .position(|geo| !geo.ring && geo.radius.is_some() && concentric(geo, centre, normal))
            .ok_or_else(|| bad(cmd, format!("{}: the circle isn't concentric with a pitch circle of Gear {} (the same centre, in a parallel plane)", d.said, it.id)))?;
        let pitch = geoms[g].radius.unwrap_or(0.0);
        if radius >= pitch - 1e-9 {
            return Err(bad(cmd, format!("{}: Ø{} isn't inside the pitch circle (Ø{})", d.said, num(2.0 * radius), num(2.0 * pitch))));
        }
        let at = match out.iter().position(|w| w.gear == GearRef::new(d.item, g)) {
            Some(k) => k,
            None => {
                let h = it.settings(g).hub;
                out.push(Wanted { gear: GearRef::new(d.item, g), bore: None, hub: None, hub_auto: false, len_a: h.map_or(0.0, |h| h.length_a), len_b: h.map_or(0.0, |h| h.length_b) });
                out.len() - 1
            }
        };
        if d.hub {
            out[at].hub = Some(2.0 * radius);
            out[at].hub_auto = false;
        } else {
            out[at].bore = Some(2.0 * radius);
        }
    }
    for w in &out {
        if let (Some(b), Some(h)) = (w.bore, w.hub)
            && b >= h
        {
            return Err(bad(cmd, format!("{}: the bore (Ø{}) must be smaller than the hub (Ø{})", gear_who(items, w.gear), num(b), num(h))));
        }
    }
    Ok(out)
}

/// A gear as the dialog names it: "Gear A", "Gear W worm", "Gear C2".
fn gear_who(items: &[gi::GearItem], r: GearRef) -> String {
    let it = &items[r.item];
    match gi::part_name(it.kind, r.gear) {
        _ if it.gear_count() <= 1 => format!("Gear {}", it.id),
        Some(p) => format!("Gear {} {p}", it.id),
        None => format!("Gear {}{}", it.id, r.gear + 1),
    }
}

fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Puts each gear's bore and hub on its body: a hub a ring prism on its face's ends (side A, and
/// side B when asked), then the bore cut through; returns what each gear got. A bore at or past
/// the teeth's root is left out with a warning on the gear.
fn apply_details(items: &[gi::GearItem], wanted: &[Wanted], sp: &mut SystemPlan) -> Vec<(GearRef, Option<f64>, Option<f64>)> {
    use solidcraft_gear::plan::{Frame, Op, Step};
    use solidcraft_gear::{Outline, Region, V2};
    let mut out = Vec::new();
    // Each bored gear's body and bore Ø: a shaft stub through it is taken off below.
    let mut bored: Vec<(usize, f64)> = Vec::new();
    for w in wanted {
        let Some(m) = sp.member_of.iter().find(|(r, _)| *r == w.gear).map(|(_, m)| *m) else { continue };
        let Some(b) = sp.plan.members.get(m).and_then(|x| x.body).filter(|b| *b < sp.plan.bodies.len()) else { continue };
        let Some(geo) = items.get(w.gear.item).and_then(|it| gi::gear_geoms(it).get(w.gear.gear).copied()) else { continue };
        let module = sp.gears.iter().find(|g| g.item == w.gear.item && g.gear == w.gear.gear).map_or(1.0, |g| g.module);
        let (mut a, mut bb) = gi::face_extent(&items[w.gear.item].settings(w.gear.gear), module);
        let shape = sp.plan.shapes.iter().find(|x| x.member == m);
        // The face as planned: its sides in the asked proportion.
        if let Some(f) = shape.map(|x| x.face).filter(|f| *f > 0.0 && a + bb > 0.0) {
            let k = f / (a + bb);
            a *= k;
            bb *= k;
        }
        let root = shape.map(|x| x.root_diameter).filter(|d| *d > 0.0);
        let mut warn = Vec::new();
        let who = gear_who(items, w.gear);
        let bore = match (w.bore, root) {
            (Some(d), Some(r)) if d >= r - 1e-9 => {
                warn.push(format!("{who}: the bore (Ø{}) reaches the tooth roots (Ø{}), so no bore is cut. Draw a smaller bore circle.", num(d), num(r)));
                None
            }
            (d, _) => d,
        };
        if let (Some(h), Some(r)) = (w.hub, root)
            && h > r
        {
            warn.push(format!("{who}: the hub (Ø{}) is wider than the tooth roots (Ø{})", num(h), num(r)));
        }
        let hub_d = w.hub.or_else(|| {
            let r = root?;
            w.hub_auto.then(|| bore.map_or(0.5 * r, |b| 1.6 * b).min(0.85 * r)).filter(|h| bore.is_none_or(|b| *h > b + 1.0))
        });
        let w = &Wanted { hub: hub_d, ..*w };
        let (len_a, len_b) = if w.len_a + w.len_b > 0.0 { (w.len_a, w.len_b) } else { (0.5 * (a + bb), 0.0) };
        // The gear's axis in its body's frame: the pitch circle's centre, side A along +z.
        let body = &mut sp.plan.bodies[b];
        let f = body.frame;
        let (fx, fy, fz) = (v3(f.x), v3(f.y()), v3(f.z));
        let local = |p: Vec3| Vec3::new(p.dot(fx), p.dot(fy), p.dot(fz));
        let Some(z) = v3(geo.axis).normalized().map(local) else { continue };
        let e = if z.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        let Some(x) = (e - z * e.dot(z)).normalized() else { continue };
        let o = local(v3(geo.centre) - v3(f.origin));
        let frame = Frame { origin: [o.x, o.y, o.z], x: [x.x, x.y, x.z], z: [z.x, z.y, z.z] };
        let prism = |r: f64, z0: f64, height: f64, op: Op| Step::Placed {
            frame,
            step: Box::new(Step::Prism { region: Region { outer: vec![Outline::circle(V2::ZERO, r)], holes: Vec::new() }, z0, height, twist: 0.0, op }),
        };
        // (The hub reaches half a millimetre into the gear, so the two are one solid.)
        let into = 0.5f64.min(0.5 * (a + bb));
        if let Some(h) = w.hub {
            if len_a > 0.0 {
                body.steps.push(prism(h / 2.0, a - into, len_a + into, Op::Join));
            }
            if len_b > 0.0 {
                body.steps.push(prism(h / 2.0, -bb - len_b, len_b + into, Op::Join));
            }
        }
        if let Some(d) = bore {
            // Through the gear and its hub, and on through any shaft stub the gear was built with
            // (a bore is for a shaft through it), but not into another layer of its compound.
            let others: Vec<f64> = gi::gear_geoms(&items[w.gear.item])
                .iter()
                .enumerate()
                .filter(|(g, _)| *g != w.gear.gear)
                .map(|(_, o)| (v3(o.centre) - v3(geo.centre)).dot(v3(geo.axis).normalized().unwrap_or(Vec3::Z)))
                .collect();
            let far = 1000.0;
            let lo = if others.iter().any(|t| *t < -1e-6) { -bb - w.hub.map_or(0.0, |_| len_b) - 1.0 } else { -bb - far };
            let hi = if others.iter().any(|t| *t > 1e-6) { a + w.hub.map_or(0.0, |_| len_a) + 1.0 } else { a + far };
            body.steps.push(prism(d / 2.0, lo, hi - lo, Op::Cut));
            bored.push((b, d));
        }
        if let Some(g) = sp.gears.iter_mut().find(|g| g.item == w.gear.item && g.gear == w.gear.gear) {
            g.warnings.extend(warn);
        }
        out.push((w.gear, bore, w.hub));
    }
    drop_bored_stubs(items, sp, &bored);
    out
}

/// A bore is for a shaft of its own through the gear, so a shaft stub built from the gear's
/// shaft line would be left as a tube round it: the stub of each body with a bored gear is not
/// built. A body of several gears (a compound's layers) keeps the stub between its outermost
/// gears as the sleeve joining them, bored through with its smallest bore.
fn drop_bored_stubs(items: &[gi::GearItem], sp: &mut SystemPlan, bored: &[(usize, f64)]) {
    use solidcraft_gear::plan::{Op, Step};
    use solidcraft_gear::{Outline, Region, Seg, V2};
    let mut bodies: Vec<usize> = bored.iter().map(|(b, _)| *b).collect();
    bodies.sort_unstable();
    bodies.dedup();
    for b in bodies {
        let Some(k) = sp.plan.shafts.iter().position(|sh| sh.body == b) else { continue };
        let (d, length) = (sp.plan.shafts[k].diameter, sp.plan.shafts[k].length);
        let is_stub = |st: &Step| match st {
            Step::Prism { region, z0, height, op: Op::Join, .. } => {
                region.holes.is_empty()
                    && z0.abs() < 1e-6
                    && (height - length).abs() < 1e-6
                    && matches!(region.outer.as_slice(), [o] if matches!(o.segs.as_slice(), [Seg::Circle(c, r)] if c.x.abs() < 1e-6 && c.y.abs() < 1e-6 && (r - d / 2.0).abs() < 1e-6))
            }
            _ => false,
        };
        let Some(at) = sp.plan.bodies[b].steps.iter().rposition(is_stub) else { continue };
        // The body's gears: where each reaches along the body's z, and its root Ø.
        let frame = sp.plan.bodies[b].frame;
        let fz = v3(frame.z);
        let mut spans: Vec<(f64, f64, f64)> = Vec::new();
        for (r, m) in &sp.member_of {
            if sp.plan.members.get(*m).and_then(|x| x.body) != Some(b) {
                continue;
            }
            let Some(geo) = items.get(r.item).and_then(|it| gi::gear_geoms(it).get(r.gear).copied()) else { continue };
            let Some(shape) = sp.plan.shapes.iter().find(|x| x.member == *m) else { continue };
            let module = sp.gears.iter().find(|g| g.item == r.item && g.gear == r.gear).map_or(1.0, |g| g.module);
            let (mut a, mut bb) = gi::face_extent(&items[r.item].settings(r.gear), module);
            if shape.face > 0.0 && a + bb > 0.0 {
                let f = shape.face / (a + bb);
                a *= f;
                bb *= f;
            }
            let Some(u) = v3(geo.axis).normalized() else { continue };
            let o = (v3(geo.centre) - v3(frame.origin)).dot(fz);
            let s = u.dot(fz);
            let (e0, e1) = (o - s * bb, o + s * a);
            spans.push((e0.min(e1), e0.max(e1), shape.root_diameter));
        }
        let bore = bored.iter().filter(|(x, _)| *x == b).map(|(_, d)| *d).fold(f64::INFINITY, f64::min);
        let body = &mut sp.plan.bodies[b];
        let lo = spans.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
        let hi = spans.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);
        if spans.len() > 1 && hi - lo > 1e-6 {
            let root = spans.iter().map(|s| s.2).filter(|r| *r > 0.0).fold(f64::INFINITY, f64::min);
            // The sleeve: the stub's Ø, or wider to keep a wall round the bore, within the roots;
            // in the stub's place (before the bores are cut), then bored through.
            let sleeve = d.max(bore + 3.0).min(0.85 * root).max(d.min(0.85 * root));
            let disc = |r: f64| Region { outer: vec![Outline::circle(V2::ZERO, r)], holes: Vec::new() };
            body.steps[at] = Step::Prism { region: disc(sleeve / 2.0), z0: lo, height: hi - lo, twist: 0.0, op: Op::Join };
            if bore < sleeve {
                body.steps.push(Step::Prism { region: disc(bore / 2.0), z0: lo - 1.0, height: hi - lo + 2.0, twist: 0.0, op: Op::Cut });
            }
        } else {
            body.steps.remove(at);
        }
        sp.plan.shafts.remove(k);
    }
}

/// Circles the drawing suggests as a gear's bore and hub: drawn in its pitch circle's sketch,
/// concentric with it and inside it, picked by nothing; the smallest a bore, the next a hub
/// (only what the gear doesn't have yet).
fn suggest_details(s: &dyn Reads, ids: &[String], items: &[gi::GearItem], details: &[DetailPick], wanted: &[Wanted]) -> Vec<GearDetailSuggestion> {
    let picked: Vec<(u64, String)> = items
        .iter()
        .flat_map(|it| it.things.iter())
        .chain(details.iter().map(|d| &d.thing))
        .filter_map(|t| Some((t.ids().sketch?, t.ids().entity.clone())))
        .collect();
    let mut out = Vec::new();
    for (i, it) in items.iter().enumerate().filter(|(_, it)| takes_details(it.kind)) {
        for (g, geo) in gi::gear_geoms(it).into_iter().enumerate() {
            let Some(r) = geo.radius.filter(|_| !geo.ring) else { continue };
            let pitch = it.things.iter().find(|t| matches!(t, gi::Thing::Circle { centre, normal, radius, .. } if (radius - r).abs() <= gi::tol(r) && concentric(&geo, *centre, *normal)));
            let Some(sketch) = pitch.and_then(|t| t.ids().sketch) else { continue };
            let Ok(gs) = s.gear_sketch(sketch) else { continue };
            let mut inside: Vec<(String, f64)> = gs
                .circles
                .iter()
                .filter(|(id, c)| c.r < r - gi::tol(r) && !picked.iter().any(|(sk, e)| *sk == sketch && e == id) && concentric(&geo, gs.frame.world([c.c.x, c.c.y, 0.0]), gs.frame.z))
                .map(|(id, c)| (id.clone(), 2.0 * c.r))
                .collect();
            inside.sort_by(|x, y| x.1.total_cmp(&y.1));
            let has = wanted.iter().find(|w| w.gear == GearRef::new(i, g));
            let mut slots = Vec::new();
            if has.is_none_or(|w| w.bore.is_none()) {
                slots.push("bore");
            }
            if has.is_none_or(|w| w.hub.is_none()) {
                slots.push("hub");
            }
            for ((curve, diameter), slot) in inside.into_iter().zip(slots) {
                out.push(GearDetailSuggestion { item: ids[i].clone(), gear: g, slot: slot.into(), sketch, curve, diameter });
            }
        }
    }
    out
}

/// Each item that picks geometry from a sketch that isn't fully defined gets a warning on its
/// first gear's reply: the gears follow the sketch, so a sketch free to move lets them drift.
fn loose_sketches(s: &dyn Reads, cmd: &str, list: &[GearItemParams], system: &mut SystemPlan) {
    for (i, it) in list.iter().enumerate() {
        let mut seen: Vec<u64> = Vec::new();
        for (k, c) in it.components.iter().enumerate() {
            let Some(id) = c.sketch.as_ref().and_then(|sk| s.sketch_id(Some(sk), cmd, &format!("items[{i}].components[{k}]")).ok()) else { continue };
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            let Some(ss) = s.solved(id) else { continue };
            let dof = ss.report.dof;
            if dof == 0 {
                continue;
            }
            let w = format!(
                "sketch '{}' isn't fully defined ({dof} degree{} of freedom): dimension it so the gears can't drift",
                ss.name,
                if dof == 1 { "" } else { "s" }
            );
            if let Some(g) = system.gears.iter_mut().find(|g| g.item == i && g.gear == 0) {
                g.warnings.push(w);
            }
        }
    }
}

fn end_reply(ids: &[String], r: GearRef) -> GearEndReply {
    GearEndReply { item: ids.get(r.item).cloned().unwrap_or_else(|| gi::item_id(r.item)), gear: r.gear }
}

/// The feature's body name holding plan body `b` (`names` in the order of the rigid members).
fn body_name(plan: &Plan, names: &[String], b: usize) -> Option<String> {
    let rigid = solid::members(plan);
    rigid.iter().position(|r| r.bodies.contains(&b)).and_then(|k| names.get(k).cloned())
}

/// The replies per gear, per link and the suggestions (GEAR_UI_SPEC §6) into `r`; `names`: the
/// feature's body names (in the order of the rigid members) when it was built.
pub(super) fn fill(r: &mut GearResult, it: &Items, plan: &Plan, names: Option<&[String]>) {
    let sp = &it.system;
    let body_of = |g: GearRef| -> Option<String> {
        let names = names?;
        let m = sp.member_of.iter().find(|(x, _)| *x == g)?.1;
        let b = plan.members.get(m)?.body?;
        body_name(plan, names, b)
    };
    r.gears = sp
        .gears
        .iter()
        .map(|g| GearItemReply {
            item: end_reply(&it.ids, GearRef::new(g.item, g.gear)).item,
            gear: g.gear,
            teeth: g.teeth,
            module: g.module,
            pitch_d: g.pitch_d,
            ratio: g.ratio,
            rpm: g.rpm,
            torque: g.torque,
            contact_ratio: g.contact_ratio,
            profile_shift: g.profile_shift,
            warnings: g.warnings.clone(),
            body: body_of(GearRef::new(g.item, g.gear)),
            side_a: it.extra.side_a.iter().find(|(r, _)| *r == GearRef::new(g.item, g.gear)).map(|(_, v)| *v),
            bore: it.extra.built.iter().find(|(r, _, _)| *r == GearRef::new(g.item, g.gear)).and_then(|x| x.1),
            hub: it.extra.built.iter().find(|(r, _, _)| *r == GearRef::new(g.item, g.gear)).and_then(|x| x.2),
        })
        .collect();
    r.links = sp
        .links
        .iter()
        .map(|l| GearLinkReply { a: end_reply(&it.ids, l.a), b: end_reply(&it.ids, l.b), kind: l.kind.name().into(), ratio: l.ratio, error: l.error.clone(), hint: l.hint.clone() })
        .collect();
    r.suggestions = sp.suggestions.iter().map(|x| GearSuggestionReply { a: end_reply(&it.ids, x.a), b: end_reply(&it.ids, x.b), kind: x.kind.name().into(), why: x.why.clone() }).collect();
    r.detail_suggestions = it.extra.suggested.clone();
    r.differentials = it
        .extra
        .diffs
        .iter()
        .map(|d| crate::schema::gear::GearDiffReply { ring: end_reply(&it.ids, d.ring), facts: d.facts.iter().map(super::fact_of).collect(), warnings: d.warnings.clone(), note: d.note.clone() })
        .collect();
    r.error = sp.error.clone();
    for n in &sp.notes {
        if !r.notes.contains(n) {
            r.notes.push(n.clone());
        }
    }
}

/// The rigid member (an index into the feature's bodies) the drive turns, for the assembly.
pub(super) fn driver(it: &Items, plan: &Plan) -> Option<usize> {
    let b = it.system.driver?;
    solid::members(plan).iter().position(|r| r.bodies.contains(&b))
}

// ---------------------------------------------------------------------------------------------
// Names kept across an update that adds, takes away or renumbers items
// ---------------------------------------------------------------------------------------------

/// Where a component was picked (its sketch and entity), when it is a sketch entity.
fn locators(it: &GearItemParams) -> Vec<String> {
    it.components.iter().filter_map(|c| Some(format!("{}/{}", c.sketch.as_ref()?, c.curve.as_ref().or(c.point.as_ref())?))).collect()
}

fn items_of(request: &Value) -> Vec<GearItemParams> {
    request.get("items").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

/// Each old item's id as the new request names the same item: the new item sharing a picked
/// sketch entity with it, else the one with its id and type; none when it is gone.
fn item_map(old: &Value, new: &Value) -> Vec<(String, Option<String>)> {
    let (old, new) = (items_of(old), items_of(new));
    let (old_ids, new_ids) = (ids_of(&old), ids_of(&new));
    let new_locs: Vec<Vec<String>> = new.iter().map(locators).collect();
    let mut claimed = vec![false; new.len()];
    let mut out: Vec<(String, Option<String>)> = old_ids.iter().map(|id| (id.clone(), None)).collect();
    for (i, it) in old.iter().enumerate() {
        let mine = locators(it);
        if let Some(j) = (0..new.len()).find(|j| !claimed[*j] && new_locs[*j].iter().any(|l| mine.contains(l))) {
            claimed[j] = true;
            out[i].1 = Some(new_ids[j].clone());
        }
    }
    for (i, it) in old.iter().enumerate() {
        if out[i].1.is_some() {
            continue;
        }
        if let Some(j) = (0..new.len()).find(|j| !claimed[*j] && new_ids[*j] == old_ids[i] && new[*j].kind == it.kind) {
            claimed[j] = true;
            out[i].1 = Some(new_ids[j].clone());
        }
    }
    out
}

/// A body key ("Gear A·1", "Carrier P", "Gear B·1 + Gear C·1") with its items' ids as `map`
/// gives them; `None` when one of its items is gone.
fn translate(key: &str, map: &[(String, Option<String>)]) -> Option<String> {
    let parts: Vec<Option<String>> = key
        .split(" + ")
        .map(|part| {
            let Some((word, rest)) = part.split_once(' ') else { return Some(part.to_string()) };
            let (id, tail) = match rest.find(['·', ' ', '(']) {
                Some(k) => rest.split_at(k),
                None => (rest, ""),
            };
            match map.iter().find(|(old, _)| old == id) {
                Some((_, new)) => new.as_ref().map(|n| format!("{word} {n}{tail}")),
                None => Some(part.to_string()),
            }
        })
        .collect();
    parts.into_iter().collect::<Option<Vec<String>>>().map(|p| p.join(" + "))
}

/// The bodies' names for an update of a Gears feature made of items: a body whose item and gear
/// are still there keeps its name (`old_names`, keyed by the old request's `body_keys`, its items
/// matched to the new request's by what was picked); the others are named afresh from `bases`
/// (names no other body has).
pub(super) fn carry_names(s: &Session, own: u64, old_request: &Value, old_names: &[String], new_request: &Value, bases: &[String]) -> Vec<String> {
    let old_keys: Vec<String> = old_request.get("body_keys").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let map = item_map(old_request, new_request);
    let moved: Vec<Option<String>> = old_keys.iter().map(|k| translate(k, &map)).collect();
    let mut used = vec![false; old_names.len()];
    let mut names: Vec<Option<String>> = bases
        .iter()
        .map(|base| {
            let j = (0..old_names.len().min(moved.len())).find(|j| !used[*j] && moved[*j].as_deref() == Some(base.as_str()))?;
            used[j] = true;
            Some(old_names[j].clone())
        })
        .collect();
    let mut taken: std::collections::BTreeSet<String> = s.model.state().bodies.iter().filter(|b| b.feature != own).map(|b| b.name.clone()).collect();
    taken.extend(names.iter().flatten().cloned());
    for (k, slot) in names.iter_mut().enumerate() {
        if slot.is_none() {
            let base = &bases[k];
            let mut n = base.clone();
            let mut i = 2;
            while taken.contains(&n) {
                n = format!("{base} ({i})");
                i += 1;
            }
            taken.insert(n.clone());
            *slot = Some(n);
        }
    }
    names.into_iter().map(Option::unwrap_or_default).collect()
}

// ---------------------------------------------------------------------------------------------
// gear.ratio
// ---------------------------------------------------------------------------------------------

fn mesh_name(k: MeshKind) -> &'static str {
    match k {
        MeshKind::Spur => "spur",
        MeshKind::Helical => "helical",
        MeshKind::Herringbone => "herringbone",
        MeshKind::Internal => "internal",
        MeshKind::Rack => "rack",
        MeshKind::Bevel => "bevel",
        MeshKind::SpiralBevel => "spiral_bevel",
        MeshKind::Hypoid => "hypoid",
        MeshKind::CrossedHelical => "crossed_helical",
        MeshKind::Worm => "worm",
        MeshKind::Wrap => "wrap",
    }
}

/// `gear.ratio`: the speed ratio between two gears of one set, the path of meshes and shared
/// shafts between them, its efficiency (and a rack's travel per turn).
pub(super) fn ratio(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.ratio";
    let q: GearRatioParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    if s.model.is_stale() {
        s.refresh();
    }
    let mut all = super::records(&s.doc);
    let holds = |r: &super::Record, v: &Value| match v {
        Value::String(name) => super::current_names(s, r).iter().any(|n| n == name),
        Value::Number(c) => c.as_u64().is_some_and(|c| r.assembly.as_ref().is_some_and(|a| a.holds(c))),
        _ => false,
    };
    // A set not made yet (the dialog's `request`) has no record: its bodies go by plan name.
    let rec = match q.request {
        Some(_) => None,
        None => Some(
            match q.group {
                Some(g) => all.into_iter().find(|r| r.group == g || r.feature == Some(g)),
                None => match all.iter().position(|r| holds(r, &q.a) || holds(r, &q.b)) {
                    Some(k) => Some(all.swap_remove(k)),
                    // Both {item, gear}: the design's one set made of items.
                    None => {
                        let mut of_items: Vec<super::Record> = all.into_iter().filter(|r| r.request.get("items").is_some_and(Value::is_array)).collect();
                        if of_items.len() == 1 { of_items.pop() } else { None }
                    }
                },
            }
            .ok_or_else(|| bad(cmd, "no gear set there: give `group` (the set's folder or Gears feature), or a body or component of one as `a`"))?,
        ),
    };
    let names = rec.as_ref().map(|r| super::current_names(s, r)).unwrap_or_default();
    let feature = rec.as_ref().is_some_and(|r| r.feature.is_some());
    let (asked, plan) = super::ask(s, cmd, rec.as_ref().map_or(q.request.as_ref().unwrap_or(&Value::Null), |r| &r.request))?;
    let rigid = solid::members(&plan);
    // Body names by plan body: a Gears feature names its rigid members, a folder set its bodies.
    let name_of = |b: usize| -> String {
        let k = if feature { rigid.iter().position(|r| r.bodies.contains(&b)) } else { Some(b) };
        k.and_then(|k| names.get(k).cloned()).unwrap_or_else(|| plan.bodies.get(b).map_or_else(String::new, |x| x.name.clone()))
    };
    // The gears (and their members) on plan body `b`, for a set made of items: a compound's
    // one body holds two or more.
    let gears_on = |b: usize| -> Vec<(GearRef, usize)> {
        asked.items.as_ref().map_or_else(Vec::new, |it| it.system.member_of.iter().copied().filter(|(_, m)| analysis::member_of_gear(&plan, *m) == Some(b)).collect())
    };
    // The gears at a step's two ends: across a mesh the two that mesh; else the first on each.
    let ends_of = |from: usize, to: usize, mesh: bool| -> (Option<GearRef>, Option<GearRef>) {
        let (fa, fb) = (gears_on(from), gears_on(to));
        let meshing = mesh
            .then(|| fa.iter().find_map(|(ra, ma)| fb.iter().find(|(_, mb)| plan.members.get(*ma).is_some_and(|x| x.meshes_with.contains(mb))).map(|(rb, _)| (*ra, *rb))))
            .flatten();
        meshing.map_or_else(|| (fa.first().map(|g| g.0), fb.first().map(|g| g.0)), |(x, y)| (Some(x), Some(y)))
    };
    let reply_of = |r: Option<GearRef>| -> Option<GearEndReply> { Some(end_reply(&asked.items.as_ref()?.ids, r?)) };
    // The gear asked at an end ({item, gear}).
    let asked_gear = |v: &Value| -> Option<GearRef> {
        let (o, it) = (v.as_object()?, asked.items.as_ref()?);
        let item = o.get("item").and_then(|v| item_index(&it.ids, v))?;
        Some(GearRef::new(item, o.get("gear").or(o.get("port")).and_then(Value::as_u64).unwrap_or(0) as usize))
    };
    let resolve = |which: &str, v: &Value| -> Result<usize> {
        let by_name = |name: &str| -> Option<usize> {
            let k = names.iter().position(|n| n == name)?;
            if feature { rigid.get(k).and_then(|r| r.bodies.first().copied()) } else { Some(k).filter(|k| *k < plan.bodies.len()) }
        };
        let b = match v {
            Value::String(name) => by_name(name).ok_or_else(|| bad(cmd, format!("`{which}`: no body `{name}` in the set")))?,
            Value::Number(c) => {
                let c = c.as_u64().unwrap_or(0);
                let body = rec.as_ref().and_then(|r| r.assembly.as_ref()).and_then(|a| a.members.iter().find(|m| m.component == c)).map(|m| m.body.clone());
                body.as_deref().and_then(by_name).ok_or_else(|| bad(cmd, format!("`{which}`: component {c} is none of the set's gears")))?
            }
            Value::Object(o) => {
                let it = asked.items.as_ref().ok_or_else(|| bad(cmd, format!("`{which}`: {{item, gear}} is for a set made of items: give a body's name")))?;
                let item = o.get("item").and_then(|v| item_index(&it.ids, v)).ok_or_else(|| bad(cmd, format!("`{which}.item`: no such item")))?;
                let gear = o.get("gear").or(o.get("port")).and_then(Value::as_u64).unwrap_or(0) as usize;
                let r = GearRef::new(item, gear);
                let m = it.system.member_of.iter().find(|(g, _)| *g == r).map(|(_, m)| *m);
                m.and_then(|m| plan.members.get(m)).and_then(|m| m.body).ok_or_else(|| bad(cmd, format!("`{which}`: item {} has no gear {}", it.ids[item], gear + 1)))?
            }
            _ => return Err(bad(cmd, format!("`{which}` must be a body's name, a component's id or {{item, gear}}"))),
        };
        plan.assembly.members.iter().position(|m| m.body == b).ok_or_else(|| bad(cmd, format!("`{which}` doesn't turn in the set's assembly")))
    };
    let (a, b) = (resolve("a", &q.a)?, resolve("b", &q.b)?);
    // A set made of items: each link analysed as its stage's own kind (a worm as a worm).
    let e = match asked.items.as_ref() {
        Some(it) => it.system.efficiencies(&plan.assembly),
        None => analysis::Efficiencies::for_spec(&asked.spec),
    };
    let report = analysis::plan_ratio_between_with(&plan, a, b, &e).map_err(|e| bad(cmd, e))?;
    let body = |m: usize| plan.assembly.members.get(m).map_or(0, |x| x.body);
    // From one gear of a compound's body to another: a step along its shaft, so the path
    // names both gears.
    let along = |b: usize, from: GearRef, to: GearRef| GearRatioStep {
        from: name_of(b),
        to: name_of(b),
        from_gear: reply_of(Some(from)),
        to_gear: reply_of(Some(to)),
        kind: "shaft".into(),
        teeth: None,
        ratio: 1.0,
        efficiency: 1.0,
        self_locking: false,
    };
    let mut path = Vec::new();
    let mut at = asked_gear(&q.a);
    for st in &report.path {
        let (fb, tb) = (body(st.from), body(st.to));
        let (fg, tg) = ends_of(fb, tb, st.kind != StepKind::Shaft);
        if let (Some(c), Some(f)) = (at, fg)
            && c != f
            && gears_on(fb).iter().any(|(g, _)| *g == c)
        {
            path.push(along(fb, c, f));
        }
        path.push(GearRatioStep {
            from: name_of(fb),
            to: name_of(tb),
            from_gear: reply_of(fg),
            to_gear: reply_of(tg),
            kind: match st.kind {
                StepKind::Shaft => "shaft".into(),
                StepKind::Mesh(k) => mesh_name(k).into(),
            },
            teeth: st.teeth.map(|(x, y)| [x, y]),
            ratio: st.ratio,
            efficiency: st.efficiency,
            self_locking: st.self_locking,
        });
        at = tg;
    }
    if let (Some(c), Some(want)) = (at, asked_gear(&q.b))
        && c != want
        && gears_on(body(b)).iter().any(|(g, _)| *g == c)
        && gears_on(body(b)).iter().any(|(g, _)| *g == want)
    {
        path.push(along(body(b), c, want));
    }
    super::reply(
        cmd,
        GearRatioResult {
            group: rec.as_ref().map_or(0, |r| r.group),
            a: name_of(body(a)),
            b: name_of(body(b)),
            ratio: report.ratio,
            direction: if report.direction == analysis::Direction::Same { "same" } else { "opposite" }.into(),
            reduction: report.reduction,
            mm_per_rev: report.mm_per_rev,
            rev_per_mm: report.rev_per_mm,
            path,
            efficiency: report.efficiency,
            notes: report.notes,
        },
    )
}
