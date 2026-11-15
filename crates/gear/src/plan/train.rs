//! Gear trains: any number of parallel-axis gears in one plane, meshing outside, inside a ring,
//! or sharing a shaft (compound gears), as one set.
//!
//! A [`Train`] lists the gears (where each sits, its teeth and module, profile shift, signed helix
//! angle, whether it is a ring gear, and its axial place) and the links between them: an external
//! mesh, an internal mesh (a gear inside a ring) or a shared shaft. It is sized from a sketch by
//! [`crate::sketch_rules::train_spec`] (circles are pitch circles) and travels in
//! [`Spec::train`]; the spec's kind is the tooth style (spur, helical, herringbone).
//!
//! The planner checks the train (rings only mesh inside, loops close and agree, gears that don't
//! mesh stay clear of each other), turns each gear to mesh with the one that drives it along a
//! spanning tree from gear 1 (the pair and planetary planners' phasing: [`mesh_phase`] and
//! [`mesh_phase_internal`]), and builds a body, a preview member and a 2D mesh pair per mesh.
//! Pairs and planetary sets keep their own planners (coupons, parametric teeth); a train of two
//! external gears is a pair with gear 1 at the origin.

use std::f64::consts::{PI, TAU};

use super::{Body, Frame, Kind, Plan, PreviewGear, Spec, Unit, hub_steps, mesh_warnings, mm, name, ring_warnings, tooth_warnings, twist_for, wanted, with_bore};
use crate::geom::{Outline, Region, V2, wrap_period};
use crate::involute::{Internal, Involute, ToothFlank, contact_ratio_external, mesh_external, mesh_internal, mesh_phase, mesh_phase_internal, shift_sum_for_center, teeth_outline};

/// The most gears a train takes.
pub const MAX_GEARS: usize = 64;

/// One gear of a [`Train`].
#[derive(Clone, Debug, PartialEq)]
pub struct TrainGear {
    /// Its centre (its shaft) in the set's plane, about `Spec::origin` (mm).
    pub center: V2,
    pub teeth: u32,
    /// Normal module (each meshing group has one; compound gears may differ).
    pub module: f64,
    pub shift: f64,
    /// Signed helix angle (degrees; + right hand): opposite across an external mesh, the same
    /// across an internal one and on a shared shaft.
    pub helix: f64,
    /// A ring gear (teeth inside, a plain rim outside).
    pub ring: bool,
    /// Where its back face is along the shafts (mm): compound gears sit on different layers.
    pub z: f64,
    /// Its face width (mm; 0: the spec's `face_width`).
    pub face: f64,
    /// V (herringbone) teeth: two opposite helical halves (needs a helix).
    pub herringbone: bool,
    /// Where it came from: its index in what was read (the drawing's circles, a selection).
    pub source: usize,
}

impl TrainGear {
    /// Its face width: its own, or `spec`'s.
    pub fn face_of(&self, spec: &Spec) -> f64 {
        if self.face > 0.0 { self.face } else { spec.face_width }
    }
}

/// How two gears of a [`Train`] are tied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// They mesh outside each other.
    External,
    /// `a` meshes inside ring gear `b`.
    Internal,
    /// They share a shaft (concentric circles): joined rigidly, on different layers.
    Compound,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainLink {
    pub a: usize,
    pub b: usize,
    pub kind: LinkKind,
}

/// A gear train: its gears and how they are tied. Gear 1 (index 0) is the driver.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Train {
    pub gears: Vec<TrainGear>,
    pub links: Vec<TrainLink>,
    /// Shafts built with the train: each a round bar through the gears it carries (their bores
    /// made to fit), joined to them as one part.
    pub shafts: Vec<TrainShaft>,
}

/// A shaft built with a [`Train`]: a round bar on the axis of the gears it carries.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainShaft {
    /// Its axis in the set's plane (the gears' common centre), and where it runs along the
    /// shafts (mm, as the gears' `z`).
    pub center: V2,
    pub from: f64,
    pub to: f64,
    pub diameter: f64,
    /// The gears it carries (indices into `Train::gears`; the first is the one it is joined to
    /// in the assembly).
    pub gears: Vec<usize>,
}

/// How the train turns, worked out from its links: a spanning tree from gear 1 (and from the
/// first gear of each group not tied to it).
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// Each gear's turn per radian of its group's first gear (gear 1 for its own group).
    pub speed: Vec<f64>,
    /// The link that drives each gear (an index into `Train::links`; `None` for a group's first).
    pub driver: Vec<Option<usize>>,
    /// Each gear's group: the index of its first gear.
    pub group: Vec<usize>,
    /// Links off the tree (closing a loop), checked rather than followed.
    pub closing: Vec<usize>,
    /// Gears in the order they are reached (each after its driver).
    pub order: Vec<usize>,
}

impl Train {
    /// The links of gear `i`, with the gear at the other end.
    fn links_of(&self, i: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.links.iter().enumerate().filter_map(move |(k, l)| if l.a == i { Some((k, l.b)) } else if l.b == i { Some((k, l.a)) } else { None })
    }

    /// The spanning tree and speeds (see [`Motion`]).
    pub fn motion(&self) -> Motion {
        let n = self.gears.len();
        let mut speed = vec![0.0; n];
        let mut driver = vec![None; n];
        let mut group = vec![usize::MAX; n];
        let mut on_tree = vec![false; self.links.len()];
        let mut order = Vec::new();
        for root in 0..n {
            if group[root] != usize::MAX {
                continue;
            }
            group[root] = root;
            speed[root] = 1.0;
            let mut queue = std::collections::VecDeque::from([root]);
            while let Some(i) = queue.pop_front() {
                order.push(i);
                for (k, j) in self.links_of(i) {
                    if group[j] != usize::MAX {
                        continue;
                    }
                    group[j] = root;
                    driver[j] = Some(k);
                    on_tree[k] = true;
                    speed[j] = speed[i] * self.ratio_across(k, i);
                    queue.push_back(j);
                }
            }
        }
        let closing = (0..self.links.len()).filter(|k| !on_tree[*k]).collect();
        Motion { speed, driver, group, closing, order }
    }

    /// The turn of the gear at the far end of link `k` per radian of gear `from`.
    pub fn ratio_across(&self, k: usize, from: usize) -> f64 {
        let l = self.links[k];
        let to = if l.a == from { l.b } else { l.a };
        let (zf, zt) = (f64::from(self.gears[from].teeth.max(1)), f64::from(self.gears[to].teeth.max(1)));
        match l.kind {
            LinkKind::External => -zf / zt,
            LinkKind::Internal => zf / zt,
            LinkKind::Compound => 1.0,
        }
    }
}

/// A member's name for people ("gear 3").
pub fn who(i: usize) -> String {
    format!("gear {}", i + 1)
}

/// Checks a train's numbers and structure; the error says which gear or link and why.
pub fn check(t: &Train) -> Result<(), String> {
    let n = t.gears.len();
    if n == 0 {
        return Err("a gear train needs at least one gear".into());
    }
    if n > MAX_GEARS {
        return Err(format!("a gear train takes {MAX_GEARS} gears at most, not {n}"));
    }
    for (i, g) in t.gears.iter().enumerate() {
        let w = who(i);
        if !(g.module.is_finite() && g.module > 0.0) {
            return Err(format!("{w}: module must be greater than 0"));
        }
        if !(3..=2000).contains(&g.teeth) {
            return Err(format!("{w}: teeth must be between 3 and 2000"));
        }
        if !(g.shift.is_finite() && g.shift.abs() <= 3.0) {
            return Err(format!("{w}: profile shift must be between −3 and 3"));
        }
        if !(g.helix.is_finite() && g.helix.abs() < 80.0) {
            return Err(format!("{w}: helix angle must be under 80°"));
        }
        if !(g.face.is_finite() && g.face >= 0.0) {
            return Err(format!("{w}: face width can't be negative"));
        }
        if !g.center.is_finite() || !g.z.is_finite() {
            return Err(format!("{w}: its place must be numbers"));
        }
    }
    for (k, sh) in t.shafts.iter().enumerate() {
        let w = format!("shaft {}", k + 1);
        if !(sh.diameter.is_finite() && sh.diameter > 0.0) || !(sh.from.is_finite() && sh.to.is_finite() && sh.to > sh.from) || !sh.center.is_finite() {
            return Err(format!("{w}: needs a diameter over 0 and a length"));
        }
        if sh.gears.is_empty() || sh.gears.iter().any(|g| *g >= n) {
            return Err(format!("{w}: must carry gears of the train"));
        }
        for &g in &sh.gears {
            if t.gears[g].ring {
                return Err(format!("{w}: gear {} is a ring gear, which has no shaft through it", g + 1));
            }
            if t.gears[g].center.dist(sh.center) > 1e-6 * t.gears[g].module.max(1.0) {
                return Err(format!("{w}: gear {} is not on its axis", g + 1));
            }
        }
    }
    for l in &t.links {
        if l.a >= n || l.b >= n || l.a == l.b {
            return Err(format!("a link joins gear {} to gear {}: there are {n} gears", l.a + 1, l.b + 1));
        }
        let (ga, gb) = (&t.gears[l.a], &t.gears[l.b]);
        let pair = format!("gears {} and {}", l.a + 1, l.b + 1);
        match l.kind {
            LinkKind::External | LinkKind::Internal => {
                if (ga.module - gb.module).abs() > 1e-9 * ga.module {
                    return Err(format!("{pair} mesh but have different modules ({} and {}): meshing gears share one module", mm(ga.module), mm(gb.module)));
                }
                if (ga.z - gb.z).abs() > 1e-9 {
                    return Err(format!("{pair} mesh but sit on different layers"));
                }
                let same_hand = l.kind == LinkKind::Internal;
                let want = if same_hand { ga.helix } else { -ga.helix };
                if (gb.helix - want).abs() > 1e-9 {
                    return Err(format!(
                        "{pair}: {} needs helix angles of {} hands",
                        if same_hand { "a gear inside a ring" } else { "an external mesh" },
                        if same_hand { "the same" } else { "opposite" }
                    ));
                }
            }
            LinkKind::Compound => {
                if ga.center.dist(gb.center) > 1e-6 * ga.module.max(gb.module) {
                    return Err(format!("{pair} share a shaft: their centres must coincide"));
                }
                if (ga.z - gb.z).abs() < 1e-9 {
                    return Err(format!("{pair} share a shaft on the same layer: one must sit beside the other along the shaft"));
                }
            }
        }
        match l.kind {
            LinkKind::External if ga.ring || gb.ring => {
                let r = if ga.ring { l.a } else { l.b };
                return Err(format!("gear {} is a ring gear: gears mesh only inside it, not on its outside", r + 1));
            }
            LinkKind::Internal if ga.ring || !gb.ring => return Err(format!("{pair}: gear {} must be the ring and gear {} a gear inside it", l.b + 1, l.a + 1)),
            LinkKind::Internal if gb.teeth < ga.teeth + 3 => {
                return Err(format!("{pair}: the ring needs at least 3 teeth more than the gear inside it ({} and {})", gb.teeth, ga.teeth));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Gear `b`'s profile shift that puts it `a` mm from gear `ga` in mesh, `gb` the gear unshifted
/// (an internal mesh: `ga` inside ring `gb`, or `gb` inside ring `ga` when `ring_first`).
pub fn shift_to_reach(ga: &Involute, gb: &Involute, kind: LinkKind, ring_first: bool, a: f64) -> Option<f64> {
    match kind {
        LinkKind::External => shift_sum_for_center(ga, gb, a).map(|sum| sum - ga.shift),
        LinkKind::Internal if !ring_first => super::ring_shift_for_center(ga, gb, a),
        // The gear inside: the ring's shift less the difference the distance needs.
        LinkKind::Internal => super::ring_shift_for_center(gb, ga, a).map(|x_ring_less| ga.shift - x_ring_less),
        LinkKind::Compound => Some(gb.shift),
    }
}

/// One gear's involute (its own module and helix) and the spec its body is built with.
fn gear_of(s: &Spec, g: &TrainGear) -> (Spec, Involute) {
    let sg = Spec { module: g.module, helix: g.helix.abs(), train: None, ..s.clone() };
    let inv = sg.involute(g.teeth, g.shift, g.helix);
    (sg, inv)
}

/// A gear's working centre distance to a gear it meshes with, and the mesh.
fn mesh_of(l: &TrainLink, gs: &[Involute], rings: &[Option<Internal>], face: f64) -> crate::involute::Mesh {
    match l.kind {
        LinkKind::Internal => mesh_internal(&gs[l.a], rings[l.b].as_ref().unwrap_or(&Internal { gear: gs[l.b] }), face),
        _ => mesh_external(&gs[l.a], &gs[l.b], face),
    }
}

/// Builds the train of `s` (`Spec::train`): bodies, facts, warnings and the preview.
pub(crate) fn build(s: &Spec, t: &Train, p: &mut Plan) {
    if let Err(e) = check(t) {
        p.refuse(e);
        return;
    }
    let n = t.gears.len();
    let faces: Vec<f64> = t.gears.iter().map(|g| g.face_of(s)).collect();
    // A mesh's face: where both gears' faces overlap (they share their back face's place).
    let mesh_face = |l: &TrainLink| faces[l.a].min(faces[l.b]);
    let motion = t.motion();
    let herring = s.kind == Kind::Herringbone || s.herringbone;
    if (herring || t.gears.iter().any(|g| g.herringbone)) && t.gears.iter().all(|g| g.helix == 0.0) {
        p.warn("herringbone needs a helix angle: built with straight teeth");
    }
    // Each gear's spec and involute; shifted meshes keep the basic rack's tip clearance (as a
    // pair's): every gear's tips are shortened by the most any of its meshes asks.
    let mut specs = Vec::with_capacity(n);
    let mut gs = Vec::with_capacity(n);
    for (i, g) in t.gears.iter().enumerate() {
        let (mut sg, inv) = gear_of(s, g);
        // A gear on a built shaft: its bore fits the shaft.
        if let Some(sh) = t.shafts.iter().find(|sh| sh.gears.contains(&i)) {
            let bore = crate::hub::Bore { kind: crate::hub::BoreKind::Round, diameter: sh.diameter, ..crate::hub::Bore::default() };
            if i == 0 {
                sg.bore = bore;
            } else {
                sg.bore2 = bore;
            }
        }
        specs.push(sg);
        gs.push(inv);
    }
    let mut shorten = vec![0.0f64; n];
    for l in t.links.iter().filter(|l| l.kind == LinkKind::External) {
        let (a, b) = (&gs[l.a], &gs[l.b]);
        let cut = (a.shift + b.shift) * a.module - (mesh_external(a, b, mesh_face(l)).center_distance - a.r() - b.r());
        if cut > 1e-9 {
            shorten[l.a] = shorten[l.a].max(cut);
            shorten[l.b] = shorten[l.b].max(cut);
        }
    }
    for (g, cut) in gs.iter_mut().zip(&shorten) {
        g.tip_shortening += cut;
    }
    if let Some(cut) = shorten.iter().copied().reduce(f64::max).filter(|c| *c > 1e-9) {
        p.fact("tip shortening (most)", cut, Unit::Mm);
    }
    // Ring gears: their tips kept on the involute of every gear inside them.
    let mut rings: Vec<Option<Internal>> = t.gears.iter().zip(&gs).map(|(tg, g)| tg.ring.then_some(Internal { gear: *g })).collect();
    for l in t.links.iter().filter(|l| l.kind == LinkKind::Internal) {
        let pinion = &gs[l.a];
        if let Some(ring) = rings[l.b].as_mut() {
            super::clear_ring_tips(ring, pinion, mesh_face(l));
        }
    }
    // Phases along the tree: each gear turned to mesh with the one that drives it.
    let mut phase = vec![0.0f64; n];
    for &i in &motion.order {
        let Some(k) = motion.driver[i] else { continue };
        let l = t.links[k];
        let from = if l.a == i { l.b } else { l.a };
        phase[i] = phase_from(t, &l, from, phase[from]);
    }
    // Every mesh where the gears are: at its working centre distance.
    let mut meshes = Vec::new();
    for (k, l) in t.links.iter().enumerate() {
        if l.kind == LinkKind::Compound {
            continue;
        }
        let m = mesh_of(l, &gs, &rings, mesh_face(l));
        let drawn = t.gears[l.a].center.dist(t.gears[l.b].center);
        if (drawn - m.center_distance).abs() > 1e-3 * gs[l.a].module {
            p.refuse(format!(
                "gears {} and {} mesh {} mm apart, but their centres are {} mm apart: move one, or change the teeth or profile shifts",
                l.a + 1,
                l.b + 1,
                mm(m.center_distance),
                mm(drawn)
            ));
            return;
        }
        meshes.push((k, m));
    }
    // Links that close a loop: the speeds and the teeth must agree all the way round. A loop
    // through a shared shaft (a power split: one gear driving two countershafts that drive one
    // gear) has a free turn there: the compound gear on the shaft, with all it drives, is turned
    // so the teeth close (each shared shaft frees one loop).
    let mut freed = vec![false; t.links.len()];
    for &k in &motion.closing {
        let l = t.links[k];
        if l.kind == LinkKind::Compound {
            continue;
        }
        let pitch = TAU / f64::from(t.gears[l.b].teeth);
        let miss = wrap_period(phase_from(t, &l, l.a, phase[l.a]) - phase[l.b], pitch);
        if miss.abs() <= 1e-3 * pitch {
            continue;
        }
        let up = |mut i: usize| {
            let mut chain = vec![i];
            while let Some(d) = motion.driver[i] {
                let dl = t.links[d];
                i = if dl.a == i { dl.b } else { dl.a };
                chain.push(i);
            }
            chain
        };
        let (from_a, from_b) = (up(l.a), up(l.b));
        // A compound gear on one side of the loop only (not shared by both sides' paths to gear
        // 1), whose shaft link is not yet used to close another loop.
        let free = |side: &[usize], other: &[usize]| side.iter().copied().find(|c| !other.contains(c) && motion.driver[*c].is_some_and(|d| t.links[d].kind == LinkKind::Compound && !freed[d]));
        let turned = if let Some(c) = free(&from_b, &from_a) {
            // Turning c's side turns b by speed_b / speed_c of it.
            Some((c, miss * motion.speed[c] / motion.speed[l.b]))
        } else {
            free(&from_a, &from_b).map(|c| (c, -miss * motion.speed[c] / motion.speed[l.b]))
        };
        if let Some((c, by)) = turned {
            if let Some(d) = motion.driver[c] {
                freed[d] = true;
            }
            // c and every gear below it on the tree turn with it, each by its speed ratio.
            let below = |mut i: usize| loop {
                if i == c {
                    return true;
                }
                match motion.driver[i] {
                    Some(d) => {
                        let dl = t.links[d];
                        i = if dl.a == i { dl.b } else { dl.a };
                    }
                    None => return false,
                }
            };
            for i in 0..n {
                if motion.group[i] == motion.group[c] && below(i) {
                    phase[i] += by * motion.speed[i] / motion.speed[c];
                }
            }
        }
    }
    for &k in &motion.closing {
        let l = t.links[k];
        let want = motion.speed[l.a] * t.ratio_across(k, l.a);
        if (want - motion.speed[l.b]).abs() > 1e-9 * want.abs().max(1.0) {
            p.refuse(format!(
                "gears {} and {} close a loop of the train whose turns don't agree: the train locks (an odd number of outside meshes round a loop can't turn)",
                l.a + 1,
                l.b + 1
            ));
            return;
        }
        if l.kind != LinkKind::Compound {
            let got = phase_from(t, &l, l.a, phase[l.a]);
            let pitch = TAU / f64::from(t.gears[l.b].teeth);
            if wrap_period(got - phase[l.b], pitch).abs() > 1e-3 * pitch {
                p.refuse(format!(
                    "gears {} and {} close a loop of the train whose teeth can't all mesh: the turn round the loop isn't a whole number of teeth, and no shared shaft in the loop is free to turn; change a tooth count or a place",
                    l.a + 1,
                    l.b + 1
                ));
                return;
            }
        }
    }
    // The gears, their outlines and bodies.
    let mut outlines: Vec<Region> = Vec::with_capacity(n);
    let mut flanks: Vec<Option<ToothFlank>> = vec![None; n];
    let mut rims = vec![(0.0f64, 0.0f64); n];
    let mut built = vec![None; n];
    for i in 0..n {
        let w = who(i);
        let tg = &t.gears[i];
        let sg = &specs[i];
        let bore = if i == 0 { &sg.bore } else { &sg.bore2 };
        if let Some(ring) = &rings[i] {
            let rim = if s.rim > 0.0 { s.rim } else { 3.0 * tg.module };
            let outer_r = ring.r_root() + rim;
            let sf = ring.space_flank();
            ring_warnings(p, &w, ring, &sf);
            let teeth = teeth_outline(&sf, tg.teeth, phase[i] + PI / f64::from(tg.teeth));
            let region = Region { outer: vec![Outline::circle(V2::ZERO, outer_r)], holes: vec![teeth] };
            p.teeth_source(super::ring_source(sg, &w, 1, ring, &sf));
            involute_ring_facts(p, &w, ring, sf.rf, sf.ra, outer_r);
            rims[i] = (sf.rf, outer_r);
            p.shapes.push(crate::plan::MemberShape {
                member: i,
                center: Frame::at(s.origin).world([tg.center.x, tg.center.y, tg.z]),
                axis: [0.0, 0.0, 1.0],
                face: faces[i],
                pitch_diameter: 2.0 * ring.gear.r(),
                tip_diameter: 2.0 * sf.rf,
                root_diameter: 2.0 * sf.ra,
                apex: None,
                toe_pitch_diameter: None,
            });
            if wanted(s, (i + 1).min(255) as u8) {
                let face = faces[i];
                let mut steps = super::tooth_steps(region.clone(), face, twist_for(&ring.gear, face), (herring || tg.herringbone) && tg.helix != 0.0, super::Op::New);
                crate::body::ring_chamfers(sg, sf.rf, outer_r, face, &mut steps, p, &w);
                built[i] = Some(Body { name: name(s, &format!("Ring gear {} z{}", i + 1, tg.teeth)), frame: Frame::at(s.origin).shifted([tg.center.x, tg.center.y, tg.z]), steps });
            }
            outlines.push(region);
        } else {
            let g = &gs[i];
            let (o, f) = crate::body::teeth(sg, g, phase[i], p, &w);
            tooth_warnings(p, &w, g, &f);
            super::involute_facts(p, &w, g, &f);
            if g.shift.abs() > 1e-9 {
                p.fact(format!("{w} profile shift"), g.shift, Unit::None);
            }
            rims[i] = (0.0, f.ra);
            flanks[i] = Some(f.clone());
            p.shapes.push(crate::plan::MemberShape {
                member: i,
                center: Frame::at(s.origin).world([tg.center.x, tg.center.y, tg.z]),
                axis: [0.0, 0.0, 1.0],
                face: faces[i],
                pitch_diameter: 2.0 * g.r(),
                tip_diameter: 2.0 * f.ra,
                root_diameter: 2.0 * f.rf,
                apex: None,
                toe_pitch_diameter: None,
            });
            if wanted(s, (i + 1).min(255) as u8) {
                let face = faces[i];
                let mut steps = crate::body::tooth_body(sg, g, &f, phase[i], face, (herring || tg.herringbone) && tg.helix != 0.0, p, &w);
                hub_steps(sg, if i == 0 { &sg.bore } else { &sg.bore2 }, f.rf - s.elephant_foot, face, &mut steps, p, &w);
                built[i] = Some(Body { name: name(s, &format!("Gear {} z{}", i + 1, tg.teeth)), frame: Frame::at(s.origin).shifted([tg.center.x, tg.center.y, tg.z]), steps });
            }
            outlines.push(with_bore(Region { outer: vec![o], holes: vec![] }, bore));
        }
    }
    // Gears that don't mesh must stay clear of each other on their layer.
    let meshing = |i: usize, j: usize| t.links.iter().any(|l| (l.a == i && l.b == j) || (l.a == j && l.b == i));
    for i in 0..n {
        for j in i + 1..n {
            let (zi, zj) = (t.gears[i].z, t.gears[j].z);
            if meshing(i, j) || zi + faces[i] <= zj || zj + faces[j] <= zi {
                continue;
            }
            if let Some(by) = overlap(&t.gears[i], rims[i], &t.gears[j], rims[j]) {
                p.refuse(format!(
                    "gears {} and {} overlap by {} mm: they don't mesh (their pitch circles don't touch) but their teeth or rims cross; move them apart or make them touch",
                    i + 1,
                    j + 1,
                    mm(by)
                ));
                return;
            }
        }
    }
    // Facts per mesh, then the train's ratios from gear 1.
    for (k, m) in &mut meshes {
        let l = t.links[*k];
        let pair = format!("gears {}–{}", l.a + 1, l.b + 1);
        if l.kind == LinkKind::External {
            if let (Some(fa), Some(fb)) = (&flanks[l.a], &flanks[l.b]) {
                m.contact_ratio = contact_ratio_external(&gs[l.a], fa, &gs[l.b], fb, m);
                m.total_contact_ratio = m.contact_ratio + m.overlap_ratio;
            }
        }
        p.fact(format!("{pair} ratio"), f64::from(t.gears[l.b].teeth) / f64::from(t.gears[l.a].teeth), Unit::None);
        p.fact(format!("{pair} centre distance"), m.center_distance, Unit::Mm);
        if (m.center_distance - m.center_distance_ref).abs() > 1e-6 {
            p.fact(format!("{pair} working pressure angle"), m.working_pressure_angle.to_degrees(), Unit::Deg);
        }
        p.fact(format!("{pair} contact ratio"), m.contact_ratio, Unit::None);
        if m.overlap_ratio > 0.0 {
            p.fact(format!("{pair} total contact ratio"), m.total_contact_ratio, Unit::None);
        }
        mesh_warnings(p, &pair, m);
        if l.kind == LinkKind::Internal
            && let Some(ring) = &rings[l.b]
            && crate::involute::internal_tip_interference(&gs[l.a], gs[l.a].ra(), ring, m)
        {
            p.warn(format!("{pair}: the tips hit going in and out of mesh (ring − gear = {} teeth is too few)", t.gears[l.b].teeth - t.gears[l.a].teeth));
        }
    }
    for i in 1..n {
        if motion.group[i] == 0 {
            p.fact(format!("train ratio gear 1 → gear {}", i + 1), 1.0 / motion.speed[i], Unit::None);
        }
    }
    let loose: Vec<String> = (1..n).filter(|i| motion.group[*i] != 0).map(|i| (i + 1).to_string()).collect();
    if !loose.is_empty() {
        p.warn(format!("gear{} {} not driven by gear 1: nothing ties {} to it", if loose.len() > 1 { "s" } else { "" }, loose.join(", "), if loose.len() > 1 { "them" } else { "it" }));
    }
    // Shafts must clear the gears they don't carry.
    for (k, sh) in t.shafts.iter().enumerate() {
        for (i, g) in t.gears.iter().enumerate() {
            if sh.gears.contains(&i) || g.z + faces[i] <= sh.from || sh.to <= g.z {
                continue;
            }
            let bar = TrainGear { center: sh.center, ring: false, ..g.clone() };
            if let Some(by) = overlap(&bar, (0.0, sh.diameter / 2.0), g, rims[i]) {
                p.refuse(format!("shaft {} (Ø{}) runs into gear {} by {} mm: shorten it, or move the gear", k + 1, mm(sh.diameter), i + 1, mm(by)));
                return;
            }
        }
    }
    // One body per member that turns as one: the gears on one axis (a shaft's, a cluster's, or
    // a gear on its own) with their shaft, joined. Built alone (`Spec::only`), each gear is its
    // own body.
    let mut body = vec![None; n];
    let units = rigid_units(t);
    let mut built = built;
    for unit in &units {
        let shaft = unit.shaft.map(|k| &t.shafts[k]);
        let gears: Vec<usize> = unit.gears.iter().copied().filter(|i| built[*i].is_some()).collect();
        if gears.is_empty() {
            continue;
        }
        if s.only != 0 {
            // Built alone: the gear's own body.
            for &i in &gears {
                body[i] = Some(p.bodies.len());
                p.bodies.push(built[i].take().unwrap_or_else(|| unreachable!()));
            }
            continue;
        }
        let base = shaft.map_or_else(|| gears.iter().map(|i| t.gears[*i].z).fold(f64::INFINITY, f64::min), |sh| sh.from);
        let c = t.gears[gears[0]].center;
        let mut steps = Vec::new();
        for &i in &gears {
            let b = built[i].take().unwrap_or_else(|| unreachable!());
            let dz = t.gears[i].z - base;
            for st in b.steps {
                let mut st = shifted(st, dz);
                if !steps.is_empty() {
                    set_new_to_join(&mut st);
                }
                steps.push(st);
            }
        }
        if let Some(sh) = shaft {
            let region = Region { outer: vec![Outline::circle(V2::ZERO, sh.diameter / 2.0)], holes: vec![] };
            steps.push(super::Step::Prism { region, z0: 0.0, height: sh.to - sh.from, twist: 0.0, op: super::Op::Join });
        }
        let label = if gears.len() == 1 {
            format!("Gear {} z{}", gears[0] + 1, t.gears[gears[0]].teeth)
        } else {
            format!("Gears {}", gears.iter().map(|i| format!("{} z{}", i + 1, t.gears[*i].teeth)).collect::<Vec<_>>().join(" + "))
        };
        let label = if let Some(sh) = shaft { format!("{label} on shaft Ø{}", mm(sh.diameter)) } else { label };
        let index = p.bodies.len();
        for &i in &gears {
            body[i] = Some(index);
        }
        p.bodies.push(Body { name: name(s, &label), frame: Frame::at(s.origin).shifted([c.x, c.y, base]), steps });
        if let (Some(k), Some(sh)) = (unit.shaft, shaft) {
            p.shafts.push(crate::plan::PlanShaft { body: index, gears: sh.gears.clone(), diameter: sh.diameter, length: sh.to - sh.from });
            p.fact(format!("shaft {} diameter", k + 1), sh.diameter, Unit::Mm);
            p.fact(format!("shaft {} length", k + 1), sh.to - sh.from, Unit::Mm);
        }
    }
    // Each mesh's pitch line across the faces its gears share (the set's frame; placed later).
    for l in t.links.iter().filter(|l| l.kind != LinkKind::Compound) {
        let (ga, gb) = (&t.gears[l.a], &t.gears[l.b]);
        let ra = gs[l.a].r();
        let d = gb.center - ga.center;
        let len = d.len();
        if len <= 1e-12 {
            continue;
        }
        // Outside: r_a from a towards b; inside a ring (a inside b): r_a from a, away from b.
        let u = d * (1.0 / len);
        let q = if l.kind == LinkKind::Internal { ga.center - u * ra } else { ga.center + u * ra };
        let lo = ga.z.max(gb.z);
        let hi = (ga.z + faces[l.a]).min(gb.z + faces[l.b]).max(lo);
        let at = |z: f64| Frame::at(s.origin).world([q.x, q.y, z]);
        p.contacts.push(crate::plan::Contact { a: l.a, b: l.b, from: at(lo), to: at(hi) });
    }
    let (members, clusters) = set_members(t, &faces, &body);
    p.members = members;
    p.clusters = clusters;
    for (i, region) in outlines.into_iter().enumerate() {
        let tg = &t.gears[i];
        p.preview.push(PreviewGear {
            name: who(i),
            region,
            center: tg.center,
            speed: motion.speed[i],
            slide: 0.0,
            pitch_radius: gs[i].r(),
            orbit: 0.0,
            period: 0.0,
        });
    }
}

/// Gear `to`'s tooth-0 angle (the other end of link `l` from gear `from` at `phase_from`) that
/// meshes it with `from`.
fn phase_from(t: &Train, l: &TrainLink, from: usize, phase: f64) -> f64 {
    let to = if l.a == from { l.b } else { l.a };
    let (gf, gt) = (&t.gears[from], &t.gears[to]);
    match l.kind {
        LinkKind::Compound => 0.0,
        LinkKind::External => mesh_phase(gf.teeth, phase, gt.teeth, (gt.center - gf.center).angle()),
        LinkKind::Internal if gt.ring => mesh_phase_internal(gf.teeth, phase, gt.teeth, (gf.center - gt.center).angle()),
        LinkKind::Internal => {
            // From the ring to the gear inside: mesh_phase_internal turned round.
            let dir = (gt.center - gf.center).angle();
            let (zp, zr) = (f64::from(gt.teeth), f64::from(gf.teeth));
            dir + (phase - PI / zr - dir) * zr / zp
        }
    }
}

/// How deep two gears that don't mesh cut into each other (`None`: clear). `ra` and `rb` are
/// each one's material between two radii: an external gear's from 0 to its tips, a ring's from
/// its inner tips to its rim.
fn overlap(a: &TrainGear, ra: (f64, f64), b: &TrainGear, rb: (f64, f64)) -> Option<f64> {
    let d = a.center.dist(b.center);
    // Apart, or one wholly inside the other's ring of teeth.
    if d >= ra.1 + rb.1 || d + ra.1 <= rb.0 || d + rb.1 <= ra.0 {
        return None;
    }
    // The least move that clears them.
    let mut by = ra.1 + rb.1 - d;
    if rb.0 > 0.0 {
        by = by.min(d + ra.1 - rb.0);
    }
    if ra.0 > 0.0 {
        by = by.min(d + rb.1 - ra.0);
    }
    Some(by)
}

fn involute_ring_facts(p: &mut Plan, who: &str, ring: &Internal, tips: f64, root: f64, outer: f64) {
    p.fact(format!("{who} pitch diameter"), 2.0 * ring.gear.r(), Unit::Mm);
    p.fact(format!("{who} tip diameter (inner)"), 2.0 * tips, Unit::Mm);
    p.fact(format!("{who} root diameter"), 2.0 * root, Unit::Mm);
    p.fact(format!("{who} outside diameter"), 2.0 * outer, Unit::Mm);
    p.fact(format!("{who} base diameter"), 2.0 * ring.gear.rb(), Unit::Mm);
    if ring.gear.shift.abs() > 1e-9 {
        p.fact(format!("{who} profile shift"), ring.gear.shift, Unit::None);
    }
}

/// The train's meshes for the 2D checker ([`crate::mesh2d::MeshPair`], preview indices are the
/// gears'): each mesh driven from the gear nearer gear 1, a ring always `b`.
pub(crate) fn mesh_pairs(s: &Spec, t: &Train, preview: &[crate::plan::PreviewGear]) -> Vec<crate::mesh2d::MeshPair> {
    if preview.len() != t.gears.len() {
        return Vec::new();
    }
    t.links
        .iter()
        .filter(|l| l.kind != LinkKind::Compound)
        .map(|l| {
            let speed = preview[l.a].speed.abs().max(1e-12);
            crate::mesh2d::MeshPair {
                a: l.a,
                b: l.b,
                internal: l.kind == LinkKind::Internal,
                period: TAU / (f64::from(t.gears[l.a].teeth) * speed),
                slack: s.backlash,
                scale: t.gears[l.a].module,
            }
        })
        .collect()
}

/// Every tie of the train as an assembly edge (members are the bodies, one per gear): a gear
/// link per mesh and a rigid link per shared shaft, loops and all, for
/// [`crate::plan::Assembly::from_graph`].
pub(crate) fn assembly_edges(t: &Train) -> Vec<crate::plan::AssemblyLink> {
    // Sized by teeth: the ratio is the exact tooth ratio.
    let teeth = |g: &TrainGear| f64::from(g.teeth.max(1));
    t.links
        .iter()
        .map(|l| match l.kind {
            LinkKind::Compound => crate::plan::AssemblyLink::Rigid { a: l.a, b: l.b },
            kind => crate::plan::AssemblyLink::Gear { a: l.a, b: l.b, ra: teeth(&t.gears[l.a]), rb: teeth(&t.gears[l.b]), internal: kind == LinkKind::Internal, reverse: false },
        })
        .collect()
}

/// A member that turns as one: gears on one axis (with their shaft, if one is built).
struct RigidUnit {
    gears: Vec<usize>,
    shaft: Option<usize>,
}

/// The train's members that turn as one, in the order of their first gear: each shaft's gears,
/// each cluster not on a shaft, each gear on its own.
fn rigid_units(t: &Train) -> Vec<RigidUnit> {
    let n = t.gears.len();
    let mut unit_of: Vec<Option<usize>> = vec![None; n];
    let mut units: Vec<RigidUnit> = Vec::new();
    for i in 0..n {
        if unit_of[i].is_some() {
            continue;
        }
        let shaft = t.shafts.iter().position(|sh| sh.gears.contains(&i));
        // Its cluster: the gears joined to it on shared shafts, and those its shaft carries.
        let mut list = vec![i];
        let mut k = 0;
        while k < list.len() {
            let g = list[k];
            for (l, j) in t.links_of(g).collect::<Vec<_>>() {
                if t.links[l].kind == LinkKind::Compound && !list.contains(&j) {
                    list.push(j);
                }
            }
            for sh in t.shafts.iter().filter(|sh| sh.gears.contains(&g)) {
                for &j in &sh.gears {
                    if !list.contains(&j) {
                        list.push(j);
                    }
                }
            }
            k += 1;
        }
        list.sort_unstable();
        let id = units.len();
        for &g in &list {
            unit_of[g] = Some(id);
        }
        let shaft = shaft.or_else(|| list.iter().find_map(|g| t.shafts.iter().position(|sh| sh.gears.contains(g))));
        units.push(RigidUnit { gears: list, shaft });
    }
    units
}

/// A step moved `dz` along its body's z.
fn shifted(st: super::Step, dz: f64) -> super::Step {
    use super::Step;
    if dz == 0.0 {
        return st;
    }
    match st {
        Step::Prism { region, z0, height, twist, op } => Step::Prism { region, z0: z0 + dz, height, twist, op },
        Step::Loft { sections, op } => Step::Loft { sections: sections.into_iter().map(|(o, h)| (o, h + dz)).collect(), op },
        Step::Taper { outline, z0, height, twist, scale, op } => Step::Taper { outline, z0: z0 + dz, height, twist, scale, op },
        Step::RingTaper { outline, rim, z0, height, twist, scale, op } => Step::RingTaper { outline, rim, z0: z0 + dz, height, twist, scale, op },
        Step::Revolve { profile, op } => Step::Revolve { profile: profile.into_iter().map(|q| V2::new(q.x, q.y + dz)).collect(), op },
        Step::Coil { section, pitch, z0, height, left, op } => Step::Coil { section, pitch, z0: z0 + dz, height, left, op },
        Step::Placed { frame, step } => Step::Placed { frame: frame.shifted([0.0, 0.0, dz]), step },
    }
}

/// A step that starts a body joins the one it is added to instead.
pub(crate) fn set_new_to_join(st: &mut super::Step) {
    use super::{Op, Step};
    let op = match st {
        Step::Prism { op, .. } | Step::Loft { op, .. } | Step::Taper { op, .. } | Step::RingTaper { op, .. } | Step::Revolve { op, .. } | Step::Coil { op, .. } => op,
        Step::Placed { step, .. } => return set_new_to_join(step),
    };
    if *op == Op::New {
        *op = Op::Join;
    }
}

/// The train's members for [`Plan::members`] (one per gear: its numbers, meshes and cluster)
/// and its clusters (gears on one shaft, joined as one part).
fn set_members(t: &Train, faces: &[f64], body: &[Option<usize>]) -> (Vec<crate::plan::SetMember>, Vec<Vec<usize>>) {
    let n = t.gears.len();
    let mut cluster: Vec<Option<usize>> = vec![None; n];
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if cluster[start].is_some() || !t.links.iter().any(|l| l.kind == LinkKind::Compound && (l.a == start || l.b == start)) {
            continue;
        }
        let id = clusters.len();
        let mut list = vec![start];
        cluster[start] = Some(id);
        let mut k = 0;
        while k < list.len() {
            for (_, j) in t.links_of(list[k]).filter(|(l, _)| t.links[*l].kind == LinkKind::Compound).collect::<Vec<_>>() {
                if cluster[j].is_none() {
                    cluster[j] = Some(id);
                    list.push(j);
                }
            }
            k += 1;
        }
        list.sort_unstable();
        clusters.push(list);
    }
    let members = t
        .gears
        .iter()
        .enumerate()
        .map(|(i, g)| crate::plan::SetMember {
            source: g.source,
            module: g.module,
            teeth: g.teeth,
            pitch_diameter: g.module * f64::from(g.teeth) / g.helix.to_radians().cos(),
            face_width: faces[i],
            ring: g.ring,
            meshes_with: t.links_of(i).filter(|(l, _)| t.links[*l].kind != LinkKind::Compound).map(|(_, j)| j).collect(),
            cluster: cluster[i],
            body: body[i],
        })
        .collect();
    (members, clusters)
}
