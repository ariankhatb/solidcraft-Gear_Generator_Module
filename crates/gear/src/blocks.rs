//! Blocks: gears put under one block share its settings. A block ([`ModuleBlock`]) holds a
//! module (decision 2: [`crate::items::resolve_module`]) and, optionally, the rest of
//! [`BlockSettings`]: tooth chamfer, pressure angle, helix angle, face width, backlash and shaft
//! Ø. A member gear's own value stays as its override; where it gives none (Auto), the block's
//! applies.
//!
//! - [`with_blocks`]: the items with their blocks' helix, face and bore filled in;
//!   [`block_spec`]: a unit's spec with its block's pressure angle and backlash;
//!   [`chamfer_groups`]: which items each tooth chamfer goes on.
//! - [`block_shafts`]: a block with a shaft Ø gets one shaft per distinct axis its gears are on,
//!   over the gears' faces and a margin past them (or out to the bearing points on that axis);
//!   [`expand_blocks`] adds each as a Shaft item joined to the gears on its axis, so the shaft
//!   and its gears are one body (the planners' own Shaft machinery,
//!   [`crate::plan::system::plan_items`]).

use crate::axes::{Axis, add, cross, dot, len, mul, sub, unit};
use crate::items::{
    ANGLE_TOL, BlockSettings, Bore, ShaftMode, FacePreset, FaceSides, GearGeom, GearItem, GearRef, GearSettings, ItemKind, ItemLink, LinkKind, ModuleBlock,
    Thing, face_extent, gear_geoms, resolve_module, tol,
};
use crate::plan::Spec;
use crate::sketch_rules::auto_module;

type V = [f64; 3];

fn positive(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x > 0.0)
}

/// The block item `item` is in (an index into the blocks): the first that lists it.
pub fn block_of(blocks: &[ModuleBlock], item: usize) -> Option<usize> {
    blocks.iter().position(|b| b.members.contains(&item))
}

/// Whether a helix angle means something for an item of `kind` (gears on parallel shafts).
fn takes_helix(kind: ItemKind) -> bool {
    matches!(
        kind,
        ItemKind::Spur | ItemKind::Helical | ItemKind::Herringbone | ItemKind::Compound | ItemKind::Internal | ItemKind::Rack | ItemKind::Planetary
    )
}

/// Gear settings `s` (of an item of `kind`) with block `b`'s filled in where `s` gives none: its
/// helix angle, its face width (half each side; an A only or B only face all on that side), and
/// (Bore only) its Ø as the bore. A gear on a made shaft gets no bore: it is one body with it.
pub fn block_settings(b: &BlockSettings, kind: ItemKind, s: &GearSettings) -> GearSettings {
    let mut out = *s;
    if let Some(h) = b.helix.filter(|h| h.is_finite())
        && s.helix.is_none()
        && takes_helix(kind)
    {
        out.helix = Some(h);
    }
    if let Some(w) = positive(b.face_width)
        && s.face.a.is_none()
        && s.face.b.is_none()
    {
        out.face = match s.face.preset {
            FacePreset::AOnly => FaceSides { preset: FacePreset::AOnly, a: Some(w), b: None },
            FacePreset::BOnly => FaceSides { preset: FacePreset::BOnly, a: None, b: Some(w) },
            _ => FaceSides { preset: FacePreset::Symmetric, a: Some(w / 2.0), b: None },
        };
    }
    if let Some(d) = positive(b.shaft_diameter)
        && b.shaft == ShaftMode::BoreOnly
        && s.bore == Bore::Auto
    {
        out.bore = Bore::Diameter(d);
    }
    out
}

/// The items with each member's gears given its block's settings ([`block_settings`]) and its
/// shaft Ø (a Shaft item's, a drawn shaft line's) where it has none.
pub fn with_blocks(items: &[GearItem], blocks: &[ModuleBlock]) -> Vec<GearItem> {
    items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let Some(k) = block_of(blocks, i) else { return it.clone() };
            let b = &blocks[k].settings;
            let mut out = it.clone();
            let n = it.gear_count().max(it.gears.len()).max(1);
            out.gears = (0..n).map(|g| block_settings(b, it.kind, &it.settings(g))).collect();
            if out.shaft_diameter.is_none() && b.shaft == ShaftMode::Make {
                out.shaft_diameter = positive(b.shaft_diameter);
            }
            out
        })
        .collect()
}

/// `base` with the pressure angle and backlash of the block holding `gears` (the first of them
/// in a block), where it sets them.
pub fn block_spec(blocks: &[ModuleBlock], gears: &[GearRef], base: &Spec) -> Spec {
    let mut s = base.clone();
    if let Some(b) = gears.iter().find_map(|r| block_of(blocks, r.item)).map(|k| &blocks[k].settings) {
        if let Some(pa) = positive(b.pressure_angle) {
            s.pressure_angle = pa;
        }
        if let Some(bl) = b.backlash.filter(|v| v.is_finite() && *v >= 0.0) {
            s.backlash = bl;
        }
    }
    s
}

/// Which items each tooth chamfer goes on: each block's own (its members), and the set's
/// (`set`, mm) on every item in no block with a chamfer of its own. Sizes of 0 are left out.
pub fn chamfer_groups(count: usize, blocks: &[ModuleBlock], set: Option<f64>) -> Vec<(f64, Vec<usize>)> {
    let own = |i: usize| block_of(blocks, i).and_then(|k| positive(blocks[k].settings.tooth_chamfer));
    let mut out = Vec::new();
    if let Some(c) = positive(set) {
        let rest: Vec<usize> = (0..count).filter(|i| own(*i).is_none()).collect();
        if !rest.is_empty() {
            out.push((c, rest));
        }
    }
    for (k, b) in blocks.iter().enumerate() {
        let Some(c) = positive(b.settings.tooth_chamfer) else { continue };
        let members: Vec<usize> = (0..count).filter(|i| block_of(blocks, *i) == Some(k)).collect();
        if !members.is_empty() {
            out.push((c, members));
        }
    }
    out
}

/// The items with only those `keep` names left as they are, the others emptied (no geometry, so
/// no gears) with their places kept: a pass over every gear of a set (the tooth chamfer) then
/// reaches only those.
pub fn only_items(items: &[GearItem], keep: &[usize]) -> Vec<GearItem> {
    items.iter().enumerate().map(|(i, it)| if keep.contains(&i) { it.clone() } else { GearItem::new(&it.id, ItemKind::Spur) }).collect()
}

/// A shaft a block with a shaft Ø makes along one axis its gears are on.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockShaft {
    /// The block (an index into the blocks).
    pub block: usize,
    /// The block's gears on this axis.
    pub gears: Vec<GearRef>,
    /// Its ends (world): over the gears' faces and a margin past them (the shaft Ø, at least 3
    /// mm), or out to the bearing points picked on the axis.
    pub a: V,
    pub b: V,
    pub diameter: f64,
    /// The item whose own shaft line is on this axis already (a Shaft item, or a gear's drawn
    /// shaft line): that shaft takes the block's Ø and no other is made.
    pub existing: Option<usize>,
}

impl BlockShaft {
    pub fn length(&self) -> f64 {
        len(sub(self.b, self.a))
    }
}

/// Whether a gear can sit on a block's shaft: not a ring, a rack's bar, a planetary set's (its
/// carrier holds its planets) or a Shaft item.
fn on_a_shaft(it: &GearItem, geo: &GearGeom) -> bool {
    !geo.ring && !geo.rack && !matches!(it.kind, ItemKind::Planetary | ItemKind::Shaft)
}

/// Whether the axis through `p` along `w` is the one through `o` along unit `u`.
fn coaxial(o: V, u: V, p: V, w: V, t: f64) -> bool {
    len(cross(u, w)) <= ANGLE_TOL && Axis { origin: o, dir: u }.distance(p) <= t
}

/// The item (other than a block's own) whose shaft line lies on the axis through `o` along `u`:
/// a Shaft item's line, or a parallel-axis gear's drawn shaft line.
fn line_on(items: &[GearItem], o: V, u: V, t: f64) -> Option<usize> {
    items.iter().position(|it| {
        matches!(it.kind, ItemKind::Shaft | ItemKind::Spur | ItemKind::Helical | ItemKind::Herringbone | ItemKind::Compound)
            && it.things.iter().any(|x| match x {
                Thing::Line { a, b, .. } => unit(sub(*b, *a)).is_some_and(|w| coaxial(o, u, *a, w, t)),
                _ => false,
            })
    })
}

/// The shafts the blocks with a shaft Ø make: one per distinct axis their gears are on (gears
/// on one axis share it), over the gears' faces (at the block's module, or the Auto one its
/// gears' sizes give) and a margin past them, or out to the bearing points picked on that axis.
pub fn block_shafts(items: &[GearItem], blocks: &[ModuleBlock]) -> Vec<BlockShaft> {
    let items = with_blocks(items, blocks);
    let mut out = Vec::new();
    for (k, b) in blocks.iter().enumerate() {
        let Some(d) = positive(b.settings.shaft_diameter).filter(|_| b.settings.shaft == ShaftMode::Make) else { continue };
        let mut members: Vec<usize> = b.members.iter().copied().filter(|i| block_of(blocks, *i) == Some(k) && *i < items.len()).collect();
        members.sort_unstable();
        members.dedup();
        // The module the faces are reached at where a gear gives none: the block's, else the
        // Auto one for its gears (as the planning works it out).
        let auto = || {
            let ds: Vec<(f64, f64)> = members.iter().flat_map(|i| gear_geoms(&items[*i])).filter_map(|g| g.radius.map(|r| (2.0 * r, 1.0))).collect();
            (!ds.is_empty()).then(|| auto_module(&ds, 8).0)
        };
        let block_module = positive(b.module).or_else(auto);
        // Each axis: a point on it, its direction, its gears, how far they reach along it.
        let mut axes: Vec<(V, V, Vec<GearRef>, f64, f64)> = Vec::new();
        for &i in &members {
            let it = &items[i];
            for (g, geo) in gear_geoms(it).iter().enumerate() {
                let Some(w) = unit(geo.axis).filter(|_| on_a_shaft(it, geo)) else { continue };
                let s = it.settings(g);
                let m = resolve_module(blocks, i, &s).module.or(block_module).unwrap_or(1.0);
                let (fa, fb) = face_extent(&s, m);
                let t = tol(geo.radius.unwrap_or(1.0));
                let at = match axes.iter().position(|(o, u, ..)| coaxial(*o, *u, geo.centre, w, t)) {
                    Some(x) => x,
                    None => {
                        axes.push((geo.centre, w, Vec::new(), f64::INFINITY, f64::NEG_INFINITY));
                        axes.len() - 1
                    }
                };
                let (o, u, gears, lo, hi) = &mut axes[at];
                let c = dot(sub(geo.centre, *o), *u);
                let (ahead, behind) = if dot(w, *u) >= 0.0 { (fa, fb) } else { (fb, fa) };
                *lo = lo.min(c - behind);
                *hi = hi.max(c + ahead);
                gears.push(GearRef::new(i, g));
            }
        }
        for (o, u, gears, lo, hi) in axes {
            let line = Axis { origin: o, dir: u };
            let bearings: Vec<f64> = items
                .iter()
                .flat_map(|it| it.things.iter())
                .filter_map(|t| match t {
                    Thing::Point { p, .. } if line.distance(*p) <= tol(d) => Some(dot(sub(*p, o), u)),
                    _ => None,
                })
                .collect();
            let (lo, hi) = if bearings.is_empty() {
                let margin = d.max(3.0);
                (lo - margin, hi + margin)
            } else {
                (bearings.iter().copied().fold(lo, f64::min), bearings.iter().copied().fold(hi, f64::max))
            };
            out.push(BlockShaft {
                block: k,
                gears,
                a: add(o, mul(u, lo)),
                b: add(o, mul(u, hi)),
                diameter: d,
                existing: line_on(&items, o, u, tol(d)),
            });
        }
    }
    out
}

/// The id of the set's `n`-th block shaft (from 1, across its blocks): "S2"; its body is
/// "Shaft S2".
pub fn shaft_id(n: usize) -> String {
    format!("S{n}")
}

/// Each of `shafts`' ids ([`shaft_id`], numbered in order), `None` for one that is a shaft line
/// already there ([`BlockShaft::existing`]).
pub fn shaft_ids(shafts: &[BlockShaft]) -> Vec<Option<String>> {
    let mut n = 0;
    shafts
        .iter()
        .map(|s| {
            s.existing.is_none().then(|| {
                n += 1;
                shaft_id(n)
            })
        })
        .collect()
}

/// The items and links with the blocks applied, for planning: every member given its block's
/// settings ([`with_blocks`]), and each [`block_shafts`] shaft made: a Shaft item along it (after
/// the items, so their indices stay) joined to the gears on its axis (one body), or the shaft
/// line already there given the block's Ø.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Expanded {
    pub items: Vec<GearItem>,
    pub links: Vec<ItemLink>,
    pub shafts: Vec<BlockShaft>,
    /// The Shaft items added (indices into `items`, the same order as the shafts without an
    /// `existing` line).
    pub added: Vec<usize>,
}

/// See [`Expanded`]. A request with links of its own gets a Shaft link from each new shaft to
/// each gear on its axis; one with none infers them as for any Shaft item (the gears on its line).
pub fn expand_blocks(items: &[GearItem], links: &[ItemLink], blocks: &[ModuleBlock]) -> Expanded {
    let shafts = block_shafts(items, blocks);
    let mut out = Expanded { items: with_blocks(items, blocks), links: links.to_vec(), shafts: Vec::new(), added: Vec::new() };
    let given = !links.is_empty();
    for (s, id) in shafts.iter().zip(shaft_ids(&shafts)) {
        match s.existing {
            Some(e) => {
                let it = &mut out.items[e];
                if it.shaft_diameter.is_none() {
                    it.shaft_diameter = Some(s.diameter);
                }
            }
            None => {
                let mut it = GearItem::new(&id.unwrap_or_default(), ItemKind::Shaft).with(vec![Thing::line(s.a, s.b)]);
                it.shaft_diameter = Some(s.diameter);
                let at = out.items.len();
                out.items.push(it);
                out.added.push(at);
                if given {
                    out.links.extend(s.gears.iter().map(|r| ItemLink { a: GearRef::new(at, 0), b: *r, kind: LinkKind::Shaft }));
                }
            }
        }
    }
    out.shafts = shafts;
    out
}
