//! The Gear Generator: `gear.create` builds a gear set (from `solidcraft_gear`'s plan) out of
//! ordinary sketches and features, grouped in the timeline as one undo step; `gear.calculate`
//! only works out its numbers and warnings.

use serde_json::{Value, json};
use solidcore_record::Vec2;
use solidcraft_gear::geom::{Outline, Region, Seg};
use solidcraft_gear::hub::{Bore, BoreKind};
use solidcraft_geom::PlaneGeom as _;
use solidcraft_gear::involute::BasicRack;
use solidcraft_gear::axes::{self, Axis, Drive, Relation, Solved};
use solidcraft_gear::plan::{Body, Fact, Frame, Kind, Op, Plan, Spec, Step, Unit};
use solidcraft_io::Geom2;
use solidcraft_sketch::Analysis;

use super::CommandSpec;
use super::sketch::{Left, edit_solved, ids_of};
use super::sketch_import::add_geometry;
use crate::params::bad;
use crate::schema::gear::{
    BoreParams, BoreType, GearAxesResult, GearAxis, GearAxisLine, GearCandidate, GearFact, GearFrame, GearParams, GearRelation, GearResult, GearType, PlacementParams, RackStandard,
};
use crate::schema::{TypedContract, output_schema_for};
use crate::{EngineError, Result, Session};

const GEAR_CONTRACT: TypedContract<GearParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("gear.create", "Gear Generator", crate::checked!("gear.create", create))
        .at("SOLID", "CREATE")
        .icon("gear")
        .contract(&GEAR_CONTRACT)
        .output(output_schema_for::<GearResult>)
        .params(
            "type?: spur|helical|herringbone|internal|rack|pair|planetary|bevel|spiral_bevel|hypoid|crossed_helical|worm|cycloidal|cycloidal_drive; \
             module? (mm) | diametral_pitch?; teeth?, teeth2? (mate or planet), ring_teeth?, planets?; pressure_angle?, helix_angle? (deg, sign = hand); \
             profile_shift?, profile_shift2?; rack?: iso53_a|iso53_b|iso53_c|iso53_d|agma_full_depth|stub, addendum?, dedendum?, root_radius? (coefficients); \
             backlash? (mm); face_width?; center_distance? (pair: gear 2's shift reaches it); shaft_angle?, spiral_angle? (deg); offset?, face_start? (hypoid, mm); \
             starts?, worm_diameter?, worm_length? (worm); rim?; rack_teeth?, rack_height?; pins?, pin_circle?, pin_diameter?, eccentricity? (cycloidal drive); \
             bore?, bore2?: {type?: round|keyway|d_flat|hex|square|none, diameter, key_width?, key_depth?, flat?}; hub_diameter?, hub_length?; holes?, hole_diameter?; \
             only?: 1|2|3 (one member); origin?: [x,y,z]; name?. Axis-first: axis1?, axis2?: X|Y|Z | construction axis name | {origin?, dir} | {edge|face: [x,y,z]} | \
             {sketch, curve} (the shafts: the set is sized and placed on them, type? defaults to what fits, module/teeth/teeth2/starts not given are chosen), \
             ratio? (teeth2 ÷ teeth), cone_distance? (bevels, mm), flip1?, flip2?, swap_axes?, along? (mm along axis1); placement?: {origin, x, z}. Returns bodies, group, facts (diameters, ratio, centre distance, contact ratio, span and over-pins \
             measurements) and warnings (undercut, pointed teeth, low contact ratio, interference)",
        ),
    CommandSpec::new("gear.calculate", "Gear Calculator", crate::checked!("gear.calculate", calculate))
        .noundo()
        .read_only()
        .contract(&GEAR_CONTRACT)
        .output(output_schema_for::<GearResult>)
        .params("the same as gear.create; builds nothing: returns the facts and warnings gear.create would"),
    CommandSpec::new("gear.from_axes", "Gears From Axes", crate::checked!("gear.from_axes", from_axes))
        .noundo()
        .read_only()
        .contract(&GEAR_CONTRACT)
        .output(output_schema_for::<GearAxesResult>)
        .params(
            "axis1, axis2? and the rest as gear.create; builds nothing: returns how the axes lie (relation: parallel with distance, intersecting with angle and apex, \
             skew with angle and offset), candidates (every type: fits, natural, reason), the type solved, params (the solved set as explicit gear.create \
             parameters with its placement), derived (which the axes decided), frames (where gear 1 and gear 2 turn), facts, warnings, notes; error when the type \
             asked for can't be solved on these axes",
        ),
];

const KINDS: [(GearType, Kind); 14] = [
    (GearType::Spur, Kind::Spur),
    (GearType::Helical, Kind::Helical),
    (GearType::Herringbone, Kind::Herringbone),
    (GearType::Internal, Kind::Internal),
    (GearType::Rack, Kind::Rack),
    (GearType::Pair, Kind::Pair),
    (GearType::Planetary, Kind::Planetary),
    (GearType::Bevel, Kind::Bevel),
    (GearType::SpiralBevel, Kind::SpiralBevel),
    (GearType::Hypoid, Kind::Hypoid),
    (GearType::CrossedHelical, Kind::CrossedHelical),
    (GearType::Worm, Kind::Worm),
    (GearType::Cycloidal, Kind::Cycloidal),
    (GearType::CycloidalDrive, Kind::CycloidalDrive),
];

const RACKS: [(RackStandard, &str); 6] = [
    (RackStandard::Iso53A, "iso53_a"),
    (RackStandard::Iso53B, "iso53_b"),
    (RackStandard::Iso53C, "iso53_c"),
    (RackStandard::Iso53D, "iso53_d"),
    (RackStandard::AgmaFullDepth, "agma_full_depth"),
    (RackStandard::Stub, "stub"),
];

const BORES: [(BoreType, BoreKind); 6] = [
    (BoreType::None, BoreKind::None),
    (BoreType::Round, BoreKind::Round),
    (BoreType::Keyway, BoreKind::Keyway),
    (BoreType::DFlat, BoreKind::DFlat),
    (BoreType::Hex, BoreKind::Hex),
    (BoreType::Square, BoreKind::Square),
];

fn bore_of(b: &Option<BoreParams>) -> Bore {
    let Some(b) = b else { return Bore::default() };
    let kind = b.kind.unwrap_or(BoreType::Round);
    Bore {
        kind: BORES.iter().find(|(t, _)| *t == kind).map(|(_, k)| *k).unwrap_or(BoreKind::Round),
        diameter: b.diameter,
        key_width: b.key_width.unwrap_or(0.0),
        key_depth: b.key_depth.unwrap_or(0.0),
        flat: b.flat.unwrap_or(0.0),
    }
}

fn bore_params(b: &Bore) -> Option<BoreParams> {
    if b.kind == BoreKind::None || b.diameter <= 0.0 {
        return None;
    }
    let kind = BORES.iter().find(|(_, k)| *k == b.kind).map(|(t, _)| *t);
    let some = |v: f64| (v > 0.0).then_some(v);
    Some(BoreParams { kind, diameter: b.diameter, key_width: some(b.key_width), key_depth: some(b.key_depth), flat: some(b.flat) })
}

/// The gear set the parameters describe, starting from the type's defaults.
pub fn spec_of(p: &GearParams) -> std::result::Result<Spec, String> {
    let kind = p.kind.map(|k| KINDS.iter().find(|(t, _)| *t == k).map(|(_, k)| *k).unwrap_or(Kind::Spur)).unwrap_or(Kind::Spur);
    let mut s = Spec::defaults_for(kind);
    if let Some(dp) = p.diametral_pitch {
        if !(dp.is_finite() && dp > 0.0) {
            return Err("`diametral_pitch` must be greater than 0".into());
        }
        s.module = 25.4 / dp;
    }
    macro_rules! set {
        ($field:ident, $src:expr) => {
            if let Some(v) = $src {
                s.$field = v;
            }
        };
    }
    set!(module, p.module);
    set!(teeth, p.teeth);
    set!(teeth2, p.teeth2);
    set!(teeth3, p.ring_teeth);
    set!(planets, p.planets);
    set!(pressure_angle, p.pressure_angle);
    set!(helix, p.helix_angle);
    set!(shift, p.profile_shift);
    set!(shift2, p.profile_shift2);
    if let Some(r) = p.rack {
        let name = RACKS.iter().find(|(k, _)| *k == r).map(|(_, n)| *n).unwrap_or("iso53_a");
        s.rack = BasicRack::standard(name).unwrap_or_default();
    }
    set!(backlash, p.backlash);
    if let Some(v) = p.addendum {
        s.rack.addendum = v;
    }
    if let Some(v) = p.dedendum {
        s.rack.dedendum = v;
    }
    if let Some(v) = p.root_radius {
        s.rack.root_radius = v;
    }
    set!(face_width, p.face_width);
    set!(center_distance, p.center_distance);
    set!(shaft_angle, p.shaft_angle);
    set!(spiral_angle, p.spiral_angle);
    set!(offset, p.offset);
    set!(face_start, p.face_start);
    set!(starts, p.starts);
    set!(worm_diameter, p.worm_diameter);
    set!(worm_length, p.worm_length);
    set!(rim, p.rim);
    set!(rack_teeth, p.rack_teeth);
    set!(rack_height, p.rack_height);
    set!(pins, p.pins);
    set!(pin_circle, p.pin_circle);
    set!(pin_diameter, p.pin_diameter);
    set!(eccentricity, p.eccentricity);
    set!(hub_diameter, p.hub_diameter);
    set!(hub_length, p.hub_length);
    set!(holes, p.holes);
    set!(hole_diameter, p.hole_diameter);
    set!(only, p.only);
    set!(origin, p.origin);
    if let Some(n) = &p.name {
        s.name = n.clone();
    }
    s.bore = bore_of(&p.bore);
    s.bore2 = bore_of(&p.bore2);
    if let Some(f) = &p.placement {
        s.placement = Some(frame_of(f)?);
    }
    let finite = [
        s.module,
        s.pressure_angle,
        s.helix,
        s.shift,
        s.shift2,
        s.backlash,
        s.face_width,
        s.center_distance,
        s.shaft_angle,
        s.spiral_angle,
        s.offset,
        s.face_start,
        s.worm_diameter,
        s.worm_length,
        s.rim,
        s.rack_height,
        s.pin_circle,
        s.pin_diameter,
        s.eccentricity,
        s.hub_diameter,
        s.hub_length,
        s.hole_diameter,
        s.bore.diameter,
        s.bore2.diameter,
        s.rack.addendum,
        s.rack.dedendum,
        s.rack.root_radius,
    ];
    if finite.iter().chain(s.origin.iter()).any(|v| !v.is_finite()) {
        return Err("every number must be finite".into());
    }
    if [s.backlash, s.center_distance, s.offset, s.worm_diameter, s.worm_length, s.rim, s.rack_height, s.hub_diameter, s.hub_length, s.hole_diameter, s.bore.diameter, s.bore2.diameter]
        .iter()
        .any(|v| *v < 0.0)
    {
        return Err("lengths must not be negative".into());
    }
    if s.only > 3 {
        return Err("`only` must be 1, 2 or 3".into());
    }
    if s.name.len() > 120 {
        return Err("`name` is too long".into());
    }
    let size = s.module * f64::from(s.teeth.max(s.teeth2).max(s.teeth3).max(1));
    if size > 1e5 || s.face_width > 1e5 || s.module > 1e4 {
        return Err("the gear would be too large".into());
    }
    if s.module < 0.01 {
        return Err("`module` must be at least 0.01 mm".into());
    }
    Ok(s)
}

/// `s` as parameters with every value explicit.
fn explicit_params(s: &Spec) -> GearParams {
    let kind = KINDS.iter().find(|(_, k)| *k == s.kind).map(|(t, _)| *t);
    let rack = RACKS.iter().find(|(_, n)| BasicRack::standard(n) == Some(s.rack)).map(|(r, _)| *r);
    let custom = rack.is_none();
    GearParams {
        kind,
        module: Some(s.module),
        diametral_pitch: None,
        teeth: Some(s.teeth),
        teeth2: Some(s.teeth2),
        ring_teeth: (s.teeth3 > 0).then_some(s.teeth3),
        planets: Some(s.planets),
        pressure_angle: Some(s.pressure_angle),
        helix_angle: Some(s.helix),
        profile_shift: Some(s.shift),
        profile_shift2: Some(s.shift2),
        rack,
        addendum: custom.then_some(s.rack.addendum),
        dedendum: custom.then_some(s.rack.dedendum),
        root_radius: custom.then_some(s.rack.root_radius),
        backlash: Some(s.backlash),
        face_width: Some(s.face_width),
        center_distance: (s.center_distance > 0.0).then_some(s.center_distance),
        shaft_angle: Some(s.shaft_angle),
        spiral_angle: Some(s.spiral_angle),
        offset: (s.offset > 0.0).then_some(s.offset),
        face_start: Some(s.face_start),
        starts: Some(s.starts),
        worm_diameter: (s.worm_diameter > 0.0).then_some(s.worm_diameter),
        worm_length: (s.worm_length > 0.0).then_some(s.worm_length),
        rim: (s.rim > 0.0).then_some(s.rim),
        rack_teeth: Some(s.rack_teeth),
        rack_height: (s.rack_height > 0.0).then_some(s.rack_height),
        pins: Some(s.pins),
        pin_circle: Some(s.pin_circle),
        pin_diameter: Some(s.pin_diameter),
        eccentricity: Some(s.eccentricity),
        bore: bore_params(&s.bore),
        bore2: bore_params(&s.bore2),
        hub_diameter: (s.hub_diameter > 0.0).then_some(s.hub_diameter),
        hub_length: (s.hub_length > 0.0).then_some(s.hub_length),
        holes: (s.holes > 0).then_some(s.holes),
        hole_diameter: (s.hole_diameter > 0.0).then_some(s.hole_diameter),
        only: (s.only > 0).then_some(s.only),
        origin: (s.origin != [0.0; 3]).then_some(s.origin),
        name: (!s.name.is_empty()).then(|| s.name.clone()),
        placement: s.placement.map(|f| PlacementParams { origin: f.origin, x: f.x, z: f.z }),
        ..GearParams::default()
    }
}

/// The parameters that describe `s` (the inverse of [`spec_of`]), for front ends.
pub fn params_of(s: &Spec) -> Value {
    let p = explicit_params(s);
    serde_json::to_value(p).map(strip_nulls).unwrap_or(Value::Null)
}

fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, strip_nulls(v))).collect()),
        v => v,
    }
}

/// A placement frame from its parameters: `z` as given, `x` made square to it.
fn frame_of(f: &PlacementParams) -> std::result::Result<Frame, String> {
    let bad = || "`placement` needs a finite `origin`, and `x` and `z` that are not zero or parallel".to_string();
    let len = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let unit = |v: [f64; 3]| {
        let l = len(v);
        (l.is_finite() && l > 1e-9).then(|| v.map(|c| c / l))
    };
    let z = unit(f.z).ok_or_else(bad)?;
    let x = unit(f.x).ok_or_else(bad)?;
    let along: f64 = (0..3).map(|i| x[i] * z[i]).sum();
    let x = unit([0, 1, 2].map(|i| x[i] - along * z[i])).ok_or_else(bad)?;
    if !f.origin.iter().all(|v| v.is_finite() && v.abs() < 1e7) {
        return Err(bad());
    }
    Ok(Frame { origin: f.origin, x, z })
}

fn gear_type(k: Kind) -> GearType {
    KINDS.iter().find(|(_, x)| *x == k).map(|(t, _)| *t).unwrap_or(GearType::Spur)
}

/// A shaft axis from its parameter, where it is now.
fn axis_of(s: &Session, v: &GearAxis, cmd: &str, what: &str) -> Result<Axis> {
    let v = &v.0;
    let (o, d) = if v.get("curve").is_some() {
        let id = super::features::sketch_id(s, v.get("sketch"), cmd, what)?;
        let curve = v.get("curve").and_then(Value::as_str).unwrap_or_default();
        let st = s.world_state()?;
        let line = st.sketch(id).and_then(|ss| {
            let c = ss.sketch.curves.get(ss.sketch.curve_index(curve)?)?;
            let crate::sketch::CurveKind::Line { a, b } = c.kind else { return None };
            Some((ss.plane.to_world(ss.sketch.point(a)?), ss.plane.to_world(ss.sketch.point(b)?)))
        });
        let (a, b) = line.ok_or_else(|| bad(cmd, format!("`{what}`: sketch {id} has no straight line `{curve}`")))?;
        (a, b - a)
    } else {
        let (o, d, _, _) = super::features::axis_line(s, &json!({"axis": v}), cmd).map_err(|e| match e {
            EngineError::BadParams { msg, .. } => bad(cmd, format!("`{what}`: {msg}")),
            e => e,
        })?;
        (o, d)
    };
    Axis::new([o.x, o.y, o.z], [d.x, d.y, d.z]).ok_or_else(|| bad(cmd, format!("`{what}` has no direction")))
}

/// The set the parameters ask for: as they say, or solved on their axes.
struct Asked {
    spec: Spec,
    solved: Option<Solved>,
}

/// The axes of an axis-first request, where they are now.
fn axes_of(s: &Session, cmd: &str, p: &GearParams) -> Result<Option<(Axis, Option<Axis>)>> {
    let Some(a1) = &p.axis1 else {
        let stray = [("axis2", p.axis2.is_some()), ("ratio", p.ratio.is_some()), ("cone_distance", p.cone_distance.is_some()), ("along", p.along.is_some())];
        return match stray.iter().find(|(_, given)| *given) {
            Some((name, _)) => Err(bad(cmd, format!("`{name}` needs `axis1`"))),
            None => Ok(None),
        };
    };
    if p.placement.is_some() {
        return Err(bad(cmd, "give `placement` or `axis1`, not both"));
    }
    let a1 = axis_of(s, a1, cmd, "axis1")?;
    let a2 = p.axis2.as_ref().map(|a| axis_of(s, a, cmd, "axis2")).transpose()?;
    Ok(Some((a1, a2)))
}

/// The type an axis-first request means: the one asked for, or what the axes suggest.
fn kind_for(p: &GearParams, a1: &Axis, a2: Option<&Axis>) -> Kind {
    let rel = a2.map(|a2| if p.swap_axes == Some(true) { axes::relate(a2, a1) } else { axes::relate(a1, a2) });
    p.kind.map(|k| KINDS.iter().find(|(t, _)| *t == k).map(|(_, k)| *k).unwrap_or(Kind::Spur)).unwrap_or_else(|| axes::natural(rel.as_ref()))
}

/// Sizes and places the set on its axes: what the parameters give is kept, the rest is chosen.
fn solve_on(p: &GearParams, a1: &Axis, a2: Option<&Axis>) -> std::result::Result<Solved, String> {
    let kind = kind_for(p, a1, a2);
    let base = spec_of(&GearParams { kind: Some(gear_type(kind)), ..p.clone() })?;
    for (name, v) in [("ratio", p.ratio), ("cone_distance", p.cone_distance), ("along", p.along)] {
        if v.is_some_and(|v| !v.is_finite() || v.abs() > 1e6) {
            return Err(format!("`{name}` must be a finite number"));
        }
    }
    let drive = Drive {
        ratio: p.ratio,
        module_free: p.module.is_none() && p.diametral_pitch.is_none(),
        teeth_free: p.teeth.is_none(),
        teeth2_free: p.teeth2.is_none(),
        starts_free: p.starts.is_none(),
        cone_distance: p.cone_distance,
        flip1: p.flip1 == Some(true),
        flip2: p.flip2 == Some(true),
        swap: p.swap_axes == Some(true),
        along: p.along.unwrap_or(0.0),
    };
    axes::solve(a1, a2, Some(kind), &base, &drive)
}

fn ask(s: &Session, cmd: &str, p: &Value) -> Result<(Asked, Plan)> {
    let params: GearParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let asked = match axes_of(s, cmd, &params)? {
        None => Asked { spec: spec_of(&params).map_err(|e| bad(cmd, e))?, solved: None },
        Some((a1, a2)) => {
            let solved = solve_on(&params, &a1, a2.as_ref()).map_err(|e| bad(cmd, e))?;
            Asked { spec: solved.spec.clone(), solved: Some(solved) }
        }
    };
    let plan = solidcraft_gear::plan(&asked.spec).map_err(|e| bad(cmd, e))?;
    Ok((asked, plan))
}

fn fact_of(f: &Fact) -> GearFact {
    GearFact {
        label: f.label.clone(),
        value: f.value,
        unit: match f.unit {
            Unit::Mm => "mm",
            Unit::Deg => "deg",
            Unit::None => "",
        }
        .into(),
    }
}

/// The set's numbers: what its axes fixed first, then the plan's.
fn facts(asked: &Asked, plan: &Plan) -> Vec<GearFact> {
    asked.solved.iter().flat_map(|s| s.facts.iter()).chain(plan.facts.iter()).map(fact_of).collect()
}

fn notes(asked: &Asked) -> Vec<String> {
    asked.solved.as_ref().map(|s| s.notes.clone()).unwrap_or_default()
}

fn reply<T: serde::Serialize>(cmd: &str, r: T) -> Result<Value> {
    serde_json::to_value(r).map_err(|e| bad(cmd, e.to_string()))
}

fn calculate(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.calculate";
    let (asked, plan) = ask(s, cmd, p)?;
    reply(cmd, GearResult { bodies: Vec::new(), group: None, facts: facts(&asked, &plan), warnings: plan.warnings.clone(), param_warnings: Vec::new(), notes: notes(&asked) })
}

fn relation_of(rel: Option<&Relation>) -> GearRelation {
    let mut r = GearRelation {
        kind: rel.map(Relation::name).unwrap_or("single").into(),
        text: rel.map(Relation::describe).unwrap_or_else(|| "one axis".into()),
        distance: None,
        angle: None,
        apex: None,
        foot1: None,
        foot2: None,
    };
    match rel {
        Some(Relation::Parallel { distance, foot1, foot2 }) => (r.distance, r.foot1, r.foot2) = (Some(*distance), Some(*foot1), Some(*foot2)),
        Some(Relation::Intersecting { angle, apex }) => (r.angle, r.apex) = (Some(*angle), Some(*apex)),
        Some(Relation::Skew { angle, offset, foot1, foot2 }) => (r.angle, r.distance, r.foot1, r.foot2) = (Some(*angle), Some(*offset), Some(*foot1), Some(*foot2)),
        _ => {}
    }
    r
}

/// The `gear.create` names of the parameters a solve decided.
fn derived_names(d: &axes::Derived) -> Vec<String> {
    [
        (d.module, "module"),
        (d.teeth, "teeth"),
        (d.teeth2, "teeth2"),
        (d.starts, "starts"),
        (d.center_distance, "center_distance"),
        (d.shaft_angle, "shaft_angle"),
        (d.offset, "offset"),
        (d.helix, "helix_angle"),
        (d.worm_diameter, "worm_diameter"),
        (d.shift, "profile_shift"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, n)| n.to_string())
    .collect()
}

fn from_axes(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.from_axes";
    let params: GearParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let Some((a1, a2)) = axes_of(s, cmd, &params)? else { return Err(bad(cmd, "`axis1` is required")) };
    let line = |a: &Axis| GearAxisLine { origin: a.origin, dir: a.dir };
    // As the solver sees them: swapped when asked.
    let rel = a2.as_ref().map(|a2| if params.swap_axes == Some(true) { axes::relate(a2, &a1) } else { axes::relate(&a1, a2) });
    let candidates = axes::candidates(rel.as_ref())
        .into_iter()
        .map(|c| GearCandidate { kind: gear_type(c.kind), label: c.kind.label().into(), fits: c.fits, natural: c.natural, reason: c.reason })
        .collect();
    let mut out = GearAxesResult {
        axis1: line(&a1),
        axis2: a2.as_ref().map(line),
        relation: relation_of(rel.as_ref()),
        candidates,
        kind: gear_type(kind_for(&params, &a1, a2.as_ref())),
        error: None,
        params: None,
        derived: Vec::new(),
        frames: Vec::new(),
        facts: Vec::new(),
        warnings: Vec::new(),
        notes: Vec::new(),
    };
    match solve_on(&params, &a1, a2.as_ref()).and_then(|solved| solidcraft_gear::plan(&solved.spec).map(|plan| (solved, plan))) {
        Err(e) => out.error = Some(e),
        Ok((solved, plan)) => {
            out.params = Some(explicit_params(&solved.spec));
            out.derived = derived_names(&solved.derived);
            out.frames = solved.frames.iter().map(|f| GearFrame { origin: f.origin, x: f.x, z: f.z }).collect();
            out.warnings = plan.warnings.clone();
            let asked = Asked { spec: solved.spec.clone(), solved: Some(solved) };
            out.facts = facts(&asked, &plan);
            out.notes = notes(&asked);
        }
    }
    reply(cmd, out)
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.create";
    let (asked, plan) = ask(s, cmd, p)?;
    let spec = &asked.spec;
    if s.active_sketch.is_some() {
        return Err(bad(cmd, "finish the sketch being edited first"));
    }
    let mut made = Vec::new();
    let mut features = Vec::new();
    let mut warnings = plan.warnings.clone();
    for body in &plan.bodies {
        made.push(build_body(s, body, &mut features, &mut warnings).map_err(|e| EngineError::Other(format!("{}: {e}", body.name)))?);
    }
    let group_name = if spec.name.trim().is_empty() { kind_title(spec.kind).to_string() } else { spec.name.trim().to_string() };
    let group = if features.is_empty() {
        None
    } else {
        let r = s.execute("timeline.group", &json!({"name": group_name, "features": features}))?;
        r.get("group").and_then(Value::as_u64)
    };
    reply(cmd, GearResult { bodies: made, group, facts: facts(&asked, &plan), warnings, param_warnings: Vec::new(), notes: notes(&asked) })
}

fn kind_title(k: Kind) -> &'static str {
    match k {
        Kind::Pair => "Gear pair",
        Kind::Planetary => "Planetary set",
        Kind::Worm => "Worm and wheel",
        _ => k.label(),
    }
}

fn v2(p: solidcraft_gear::V2) -> Vec2 {
    Vec2::new(p.x, p.y)
}

/// An outline as sketch geometry.
fn geometry(o: &Outline, out: &mut Vec<Geom2>) {
    for seg in &o.segs {
        match seg {
            Seg::Line(a, b) => out.push(Geom2::Line(v2(*a), v2(*b))),
            Seg::Arc { c, a, b, ccw } => {
                let (a, b) = if *ccw { (*a, *b) } else { (*b, *a) };
                out.push(Geom2::Arc { c: v2(*c), a: v2(a), b: v2(b) });
            }
            Seg::Spline(pts) => out.push(Geom2::Spline { pts: pts.iter().map(|q| v2(*q)).collect(), control: false, degree: 3 }),
            Seg::Circle(c, r) => out.push(Geom2::Circle(v2(*c), *r)),
        }
    }
}

/// A region's loops as sketch geometry: the outer outlines first, then the holes.
fn region_loops(r: &Region) -> Vec<Vec<Geom2>> {
    r.outer
        .iter()
        .chain(r.holes.iter())
        .map(|o| {
            let mut g = Vec::new();
            geometry(o, &mut g);
            g
        })
        .collect()
}

/// The profiles to extrude for a region: with holes, each outer outline picked by its curves
/// (the default would also fill the holes, which are profiles of their own); without, all.
fn profiles_of(r: &Region) -> Option<Value> {
    if r.holes.is_empty() {
        return None;
    }
    // A point just inside each outer outline, by its farthest point from the middle (a tooth
    // tip or the rim), stepped in towards the middle: never inside a hole.
    let picks: Vec<Value> = r
        .outer
        .iter()
        .filter_map(|o| {
            let pts = o.polyline(0.01);
            let n = pts.len().max(1) as f64;
            let mid = pts.iter().fold(solidcraft_gear::V2::ZERO, |a, q| a + *q) * (1.0 / n);
            let far = pts.iter().copied().max_by(|a, b| a.dist(mid).total_cmp(&b.dist(mid)))?;
            let reach = far.dist(mid);
            let step = (reach * 0.01).min(0.2).max(1e-4);
            let at = far + (mid - far).norm() * step;
            Some(json!({"point": [at.x, at.y]}))
        })
        .collect();
    (!picks.is_empty()).then_some(Value::Array(picks))
}

/// A sketch on the plane through `origin` with axes `x`, `y` (world), holding `geom`; returns
/// its id and the reply of the line command when one was drawn.
fn sketch_with(s: &mut Session, name: &str, origin: [f64; 3], x: [f64; 3], y: [f64; 3], geom: &[Geom2], features: &mut Vec<u64>) -> Result<u64> {
    Ok(sketch_loops(s, name, origin, x, y, &[geom.to_vec()], features)?.0)
}

/// [`sketch_with`] for several loops; also returns each loop's curve ids.
fn sketch_loops(s: &mut Session, name: &str, origin: [f64; 3], x: [f64; 3], y: [f64; 3], loops: &[Vec<Geom2>], features: &mut Vec<u64>) -> Result<(u64, Vec<Vec<String>>)> {
    let r = s.execute("sketch.create", &json!({"plane": {"origin": origin, "x_dir": x, "y_dir": y}, "name": name, "project_edges": false}))?;
    let id = r.get("sketch").and_then(Value::as_u64).ok_or_else(|| EngineError::Other("no sketch".into()))?;
    let tol = 1e-7;
    let (ids, _) = edit_solved(s, id, false, Analysis::Freedom, |sk, _| {
        let mut ids = Vec::new();
        for g in loops {
            let (cs, _) = add_geometry(sk, g, &|q| q, 1.0, tol)?;
            ids.push(ids_of(sk, &cs));
        }
        Ok((ids, Left::Unsolved))
    })?;
    s.execute("sketch.finish", &json!({}))?;
    features.push(id);
    Ok((id, ids))
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::New => "new",
        Op::Join => "join",
        Op::Cut => "cut",
        Op::Intersect => "intersect",
    }
}

fn feature_params(op: Op, body: &str, name: &str) -> Value {
    let mut v = json!({"operation": op_name(op)});
    if op == Op::New {
        v["body_name"] = json!(name);
    } else {
        v["targets"] = json!([body]);
    }
    v
}

fn merge(mut a: Value, b: Value) -> Value {
    if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

fn feature_id(r: &Value) -> Option<u64> {
    r.get("feature").and_then(Value::as_u64)
}

/// Builds one body; returns its name.
fn build_body(s: &mut Session, body: &Body, features: &mut Vec<u64>, warnings: &mut Vec<String>) -> Result<String> {
    let f: &Frame = &body.frame;
    let (x, y, z) = (f.x, f.y(), f.z);
    let mut name = body.name.clone();
    for (i, step) in body.steps.iter().enumerate() {
        let label = format!("{} {}", body.name, i + 1);
        let reply = match step {
            Step::Prism { region, z0, height, twist, op } => {
                let (id, _loops) = sketch_loops(s, &format!("{label} profile"), f.world([0.0, 0.0, *z0]), x, y, &region_loops(region), features)?;
                let mut pick = json!({"sketch": id});
                if let Some(p) = profiles_of(region) {
                    // Picks are checked against the sketch's profiles, which a command run inside
                    // another has not worked out yet.
                    s.refresh();
                    pick["profiles"] = p;
                }
                if twist.abs() < 1e-12 {
                    s.execute("solid.extrude", &merge(merge(pick, json!({"distance": height})), feature_params(*op, &name, &body.name)))?
                } else {
                    // The path: a line up the axis, on the plane through it and x.
                    let path = sketch_with(s, &format!("{label} path"), f.world([0.0, 0.0, *z0]), z, x, &[Geom2::Line(Vec2::new(0.0, 0.0), Vec2::new(*height, 0.0))], features)?;
                    let curve = first_curve(s, path)?;
                    s.execute(
                        "solid.sweep",
                        &merge(merge(pick, json!({"path_sketch": path, "path": [curve], "twist": format!("{} deg", twist.to_degrees())})), feature_params(*op, &name, &body.name)),
                    )?
                }
            }
            Step::Taper { outline, z0, height, twist, scale, op } if twist.abs() <= 1e-12 => {
                // Straight: a loft between the two ends builds faster than a scaled sweep.
                let mut ids = Vec::new();
                for (k, (o, h)) in [(outline.clone(), *z0), (outline.scaled(*scale), z0 + height)].iter().enumerate() {
                    let mut g = Vec::new();
                    geometry(o, &mut g);
                    ids.push(json!({"sketch": sketch_with(s, &format!("{label} section {}", k + 1), f.world([0.0, 0.0, *h]), x, y, &g, features)?}));
                }
                s.execute("solid.loft", &merge(json!({"sections": ids}), feature_params(*op, &name, &body.name)))?
            }
            Step::Taper { outline, z0, height, twist, scale, op } => {
                let mut g = Vec::new();
                geometry(outline, &mut g);
                let id = sketch_with(s, &format!("{label} profile"), f.world([0.0, 0.0, *z0]), x, y, &g, features)?;
                let path = sketch_with(s, &format!("{label} path"), f.world([0.0, 0.0, *z0]), z, x, &[Geom2::Line(Vec2::new(0.0, 0.0), Vec2::new(*height, 0.0))], features)?;
                let curve = first_curve(s, path)?;
                let mut p = json!({"sketch": id, "path_sketch": path, "path": [curve], "scale": scale});
                if twist.abs() > 1e-12 {
                    p["twist"] = json!(format!("{} deg", twist.to_degrees()));
                }
                s.execute("solid.sweep", &merge(p, feature_params(*op, &name, &body.name)))?
            }
            Step::Loft { sections, op } => {
                let mut ids = Vec::new();
                for (k, (o, h)) in sections.iter().enumerate() {
                    let mut g = Vec::new();
                    geometry(o, &mut g);
                    ids.push(json!({"sketch": sketch_with(s, &format!("{label} section {}", k + 1), f.world([0.0, 0.0, *h]), x, y, &g, features)?}));
                }
                s.execute("solid.loft", &merge(json!({"sections": ids}), feature_params(*op, &name, &body.name)))?
            }
            Step::Revolve { profile, op } => {
                // A blank that trims (a bevel's back and front cones): the kernel sometimes
                // refuses the intersection where the cones graze a root fillet, so it is tried
                // again a hair further along, and left out with a warning if it still fails.
                let mut done = None;
                for nudge in [0.0, 0.0137, -0.0213, 0.0391] {
                    let moved: Vec<solidcraft_gear::V2> = profile.iter().map(|q| solidcraft_gear::V2::new(q.x, q.y + nudge)).collect();
                    let mut made = Vec::new();
                    let id = sketch_with(s, &format!("{label} blank"), f.origin, x, z, &polygon(&moved), &mut made)?;
                    match s.execute("solid.revolve", &merge(json!({"sketch": id, "axis": "y"}), feature_params(*op, &name, &body.name))) {
                        Ok(r) => {
                            features.extend(made);
                            done = Some(r);
                            break;
                        }
                        Err(e) if *op != Op::New => {
                            s.execute("timeline.delete", &json!({"features": [id]})).ok();
                            if nudge == 0.0391 {
                                warnings.push(format!("{}: the back and front cones could not be trimmed ({e}); the teeth ends are left flat", body.name));
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
                match done {
                    Some(r) => r,
                    None => continue,
                }
            }
            Step::Coil { section, pitch, z0, height, left, op } => {
                let g = polygon(section);
                let id = sketch_with(s, &format!("{label} thread"), f.origin, x, z, &g, features)?;
                let _ = z0;
                s.execute(
                    "solid.coil",
                    &merge(json!({"sketch": id, "pitch": pitch, "height": height, "base": f.origin, "axis": z, "clockwise": left}), feature_params(*op, &name, &body.name)),
                )?
            }
        };
        if let Some(id) = feature_id(&reply) {
            features.push(id);
        }
        if i == 0
            && let Some(made) = reply.get("created_bodies").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str)
        {
            name = made.to_string();
        }
    }
    Ok(name)
}

fn polygon(pts: &[solidcraft_gear::V2]) -> Vec<Geom2> {
    (0..pts.len()).map(|i| Geom2::Line(v2(pts[i]), v2(pts[(i + 1) % pts.len()]))).collect()
}

/// The first curve's id in a sketch.
fn first_curve(s: &Session, sketch: u64) -> Result<String> {
    let sk = super::sketch_store::read(&s.doc, sketch)?;
    sk.curves.first().map(|c| c.id.clone()).ok_or_else(|| EngineError::Other("the path sketch is empty".into()))
}

#[cfg(test)]
#[path = "gear_tests.rs"]
mod tests;
