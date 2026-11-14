//! The Gear Generator: `gear.create` builds a gear set (from `solidcraft_gear`'s plan) out of
//! ordinary sketches and features, grouped in the timeline as one undo step; `gear.calculate`
//! only works out its numbers and warnings.
//!
//! A set stays editable: `gear.create` keeps what it was asked for and which sketches and
//! features it made with the design (as plugin data, `gear-generator/sets/<folder id>.json`:
//! saved with the file and undone with the command), `gear.inspect` gives that back, and
//! `gear.update` regenerates the set from changed parameters. Where the new set has the same
//! bodies and steps, the update happens in place: each sketch's outline is redrawn (its curves
//! keeping their ids as far as there are as many), each sketch's plane is moved, and each
//! feature is redefined, so feature ids and names, body names and the faces named after the
//! sketch curves all stay, and what was built on the gear keeps its references. Where the
//! structure changed (another type, a hub or bore added, another planet count) the set's
//! features are deleted and made again under the same body names, in the same folder and at the
//! same place in the history; the reply says which features outside the set fail afterwards.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use solidcraft_doc::{Document, FeatureKind};
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
use super::sketch::{Left, edit_solved};
use super::sketch_import::add_geometry;
use crate::params::bad;
use crate::schema::gear::{
    BoreParams, BoreType, GearAxesResult, GearAxis, GearAxisLine, GearBroken, GearCandidate, GearFact, GearFrame, GearInspectParams, GearInspectResult, GearMaterial, GearParams,
    GearRelation, GearResult, GearSet, GearType, PlacementParams, RackStandard,
};
use solidcraft_gear::strength::Material;
use crate::schema::{TypedContract, output_schema_for};
use crate::{EngineError, Result, Session};

const GEAR_CONTRACT: TypedContract<GearParams> = TypedContract::new();
const INSPECT_CONTRACT: TypedContract<GearInspectParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("gear.create", "Gear Generator", crate::checked!("gear.create", create))
        .at("SOLID", "CREATE")
        .icon("gear")
        .contract(&GEAR_CONTRACT)
        .output(output_schema_for::<GearResult>)
        .params(
            "type?: spur|helical|herringbone|internal|rack|pair|planetary|bevel|spiral_bevel|hypoid|crossed_helical|worm|cycloidal|cycloidal_drive; \
             module? (mm) | diametral_pitch?; teeth?, teeth2? (mate or planet), ring_teeth?, planets?; pressure_angle?, helix_angle? (deg, sign = hand); \
             profile_shift?, profile_shift2?; rack?: iso53_a|iso53_b|iso53_c|iso53_d|agma_full_depth|stub|print_fdm, addendum?, dedendum?, root_radius? (coefficients); \
             backlash? (mm); face_width?; center_distance? (pair: gear 2's shift reaches it); shaft_angle?, spiral_angle? (deg); offset?, face_start? (hypoid, mm); \
             starts?, worm_diameter?, worm_length? (worm); rim?; rack_teeth?, rack_height?; pins?, pin_circle?, pin_diameter?, eccentricity? (cycloidal drive); \
             bore?, bore2?: {type?: round|keyway|d_flat|hex|square|none, diameter, key_width?, key_depth?, flat?}; hub_diameter?, hub_length?; holes?, hole_diameter?; \
             only?: 1|2|3 (one member); origin?: [x,y,z]; name?. Axis-first: axis1?, axis2?: X|Y|Z | construction axis name | {origin?, dir} | {edge|face: [x,y,z]} | \
             {sketch, curve} (the shafts: the set is sized and placed on them, type? defaults to what fits, module/teeth/teeth2/starts not given are chosen), \
             ratio? (teeth2 ÷ teeth), cone_distance? (bevels, mm), flip1?, flip2?, swap_axes?, along? (mm along axis1); placement?: {origin, x, z}; \
             strength rating: torque? (N·m) | power? (kW), speed? (rpm) on the driver, \
             material?, material2?, material3?: steel_normalized|steel_through_hardened|steel_case_hardened|steel_nitrided|cast_iron|bronze|pom|pa66|pla|petg, \
             application_factor?, life_hours?, accuracy_grade? (5..12). Returns bodies, group, facts (diameters, ratio, centre distance, contact ratio, span and over-pins \
             measurements; with a load: forces, stresses, safety factors SF and SH) and warnings (undercut, pointed teeth, low contact ratio, interference, SF < 1.4, SH < 1)",
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
    CommandSpec::new("gear.update", "Update Gears", crate::checked!("gear.update", update))
        .icon("gear")
        .contract(&GEAR_CONTRACT)
        .output(output_schema_for::<GearResult>)
        .params(
            "group: the set's timeline folder id | body: one of its bodies; then any gear.create parameters: they are laid over the ones the set was last made with \
             (null takes one away; reset?: true starts from the type's defaults instead). Regenerates the set, one undo step. In place where the new set has the same \
             bodies and steps: sketches are redrawn (curve ids kept), features redefined, so feature ids and names, body names and face names stay and features built \
             on the gears keep working; otherwise its features are deleted and made again under the same body names, in the same folder and place in the history. \
             Returns as gear.create, plus updated: in_place|rebuilt, rebuilt_because?, broken: [{feature, error}] (features outside the set that worked and now fail)",
        ),
    CommandSpec::new("gear.inspect", "Inspect Gears", crate::checked!("gear.inspect", inspect))
        .noundo()
        .read_only()
        .contract(&INSPECT_CONTRACT)
        .output(output_schema_for::<GearInspectResult>)
        .params(
            "group?: a set's timeline folder id | body?: one of its bodies (neither: every set in the design) → sets: [{group, name, bodies, features, \
             request (the parameters as given, with ui), params (the set as explicit gear.create parameters)}]",
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

const RACKS: [(RackStandard, &str); 7] = [
    (RackStandard::Iso53A, "iso53_a"),
    (RackStandard::Iso53B, "iso53_b"),
    (RackStandard::Iso53C, "iso53_c"),
    (RackStandard::Iso53D, "iso53_d"),
    (RackStandard::AgmaFullDepth, "agma_full_depth"),
    (RackStandard::Stub, "stub"),
    (RackStandard::PrintFdm, "print_fdm"),
];

const BORES: [(BoreType, BoreKind); 6] = [
    (BoreType::None, BoreKind::None),
    (BoreType::Round, BoreKind::Round),
    (BoreType::Keyway, BoreKind::Keyway),
    (BoreType::DFlat, BoreKind::DFlat),
    (BoreType::Hex, BoreKind::Hex),
    (BoreType::Square, BoreKind::Square),
];

const MATERIALS: [(GearMaterial, Material); 10] = [
    (GearMaterial::SteelNormalized, Material::SteelNormalized),
    (GearMaterial::SteelThroughHardened, Material::SteelThroughHardened),
    (GearMaterial::SteelCaseHardened, Material::SteelCaseHardened),
    (GearMaterial::SteelNitrided, Material::SteelNitrided),
    (GearMaterial::CastIron, Material::CastIron),
    (GearMaterial::Bronze, Material::Bronze),
    (GearMaterial::Pom, Material::Pom),
    (GearMaterial::Pa66, Material::Pa66),
    (GearMaterial::Pla, Material::Pla),
    (GearMaterial::Petg, Material::Petg),
];

fn material_of(m: GearMaterial) -> Material {
    MATERIALS.iter().find(|(g, _)| *g == m).map(|(_, k)| *k).unwrap_or_default()
}

fn material_param(m: Material) -> Option<GearMaterial> {
    MATERIALS.iter().find(|(_, k)| *k == m).map(|(g, _)| *g)
}

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
    // Strength rating (the gear crate checks the ranges).
    let load = &mut s.load;
    if let Some(v) = p.torque {
        load.torque = v;
    }
    if let Some(v) = p.power {
        load.power = v;
    }
    if let Some(v) = p.speed {
        load.speed = v;
    }
    if let Some(m) = p.material {
        load.material = material_of(m);
    }
    if let Some(m) = p.material2 {
        load.material2 = material_of(m);
    }
    load.material3 = p.material3.map(material_of);
    if let Some(v) = p.application_factor {
        load.application_factor = v;
    }
    if let Some(v) = p.life_hours {
        load.life_hours = v;
    }
    if let Some(v) = p.accuracy_grade {
        load.quality = v;
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
        torque: (s.load.torque > 0.0).then_some(s.load.torque),
        power: (s.load.power > 0.0).then_some(s.load.power),
        speed: (s.load.speed > 0.0).then_some(s.load.speed),
        material: material_param(s.load.material),
        material2: material_param(s.load.material2),
        material3: s.load.material3.and_then(material_param),
        application_factor: (s.load.application_factor != 1.0).then_some(s.load.application_factor),
        life_hours: (s.load.life_hours > 0.0).then_some(s.load.life_hours),
        accuracy_grade: (s.load.quality != 7).then_some(s.load.quality),
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

/// The parameters of a request for a set (not `gear.update`'s `group`, `body` and `reset`,
/// which it takes off before it asks).
fn read(cmd: &str, p: &Value) -> Result<GearParams> {
    let params: GearParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    for (name, given) in [("group", params.group.is_some()), ("body", params.body.is_some()), ("reset", params.reset.is_some())] {
        if given {
            return Err(bad(cmd, format!("`{name}` is for gear.update: this always starts a new set")));
        }
    }
    if params.ui.as_ref().is_some_and(|ui| ui.to_string().len() > 65_536) {
        return Err(bad(cmd, "`ui` is too large (64 KiB at most)"));
    }
    Ok(params)
}

fn ask(s: &Session, cmd: &str, p: &Value) -> Result<(Asked, Plan)> {
    let params = read(cmd, p)?;
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
    reply(cmd, GearResult { facts: facts(&asked, &plan), warnings: plan.warnings.clone(), notes: notes(&asked), ..GearResult::default() })
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
    let params = read(cmd, p)?;
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
    let mut warnings = plan.warnings.clone();
    let bodies = build_set(s, &plan, &[], &mut warnings)?;
    let features: Vec<u64> = bodies.iter().flat_map(BodyRecord::features).collect();
    let group_name = if spec.name.trim().is_empty() { kind_title(spec.kind).to_string() } else { spec.name.trim().to_string() };
    let group = if features.is_empty() {
        None
    } else {
        let r = s.execute("timeline.group", &json!({"name": group_name, "features": features}))?;
        r.get("group").and_then(Value::as_u64)
    };
    let made = bodies.iter().map(|b| b.name.clone()).collect();
    if let Some(group) = group {
        let request = if p.is_object() { p.clone() } else { json!({}) };
        keep(s, &Record { version: RECORD_VERSION, group, name: group_name, request, params: params_of(spec), bodies }, &mut warnings);
    }
    reply(cmd, GearResult { bodies: made, group, facts: facts(&asked, &plan), warnings, notes: notes(&asked), ..GearResult::default() })
}

// ---------------------------------------------------------------------------------------------
// The sets a design keeps: what each was made from and of, so it can be regenerated
// ---------------------------------------------------------------------------------------------

/// Where the sets are kept in the design's plugin data: `gear-generator/sets/<folder id>.json`.
const STORE: &str = "gear-generator/sets/";
const RECORD_VERSION: u32 = 1;

/// A sketch a step was built from: its id, and what its plane and outline were made from, so an
/// update leaves the ones that did not change alone.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SketchRecord {
    id: u64,
    plane: String,
    geometry: String,
}

/// One build step as made: its kind ([`StepDef::shape`]), the feature and its command and
/// parameters, and its sketches. A step the kernel refused and the build left out (a bevel's
/// cone trim) has the shape "skipped" and no feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StepRecord {
    shape: String,
    command: String,
    #[serde(default)]
    feature: Option<u64>,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    sketches: Vec<SketchRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BodyRecord {
    /// The body's name when it was made (it may have been renamed since: [`current_names`]).
    name: String,
    steps: Vec<StepRecord>,
}

impl BodyRecord {
    /// The body's sketches and features, in build order.
    fn features(&self) -> Vec<u64> {
        self.steps.iter().flat_map(|step| step.sketches.iter().map(|k| k.id).chain(step.feature)).collect()
    }
}

/// A gear set as the design keeps it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Record {
    version: u32,
    /// The timeline folder made for the set.
    group: u64,
    name: String,
    /// The parameters as given.
    request: Value,
    /// The set they came to, as explicit `gear.create` parameters.
    params: Value,
    bodies: Vec<BodyRecord>,
}

impl Record {
    fn features(&self) -> Vec<u64> {
        self.bodies.iter().flat_map(BodyRecord::features).collect()
    }
}

fn record_key(group: u64) -> String {
    format!("{STORE}{group}.json")
}

fn decode(data: &str) -> Option<Record> {
    let bytes = solidcraft_doc::canvas::base64_decode(data)?;
    serde_json::from_slice::<Record>(&bytes).ok().filter(|r| r.version == RECORD_VERSION)
}

/// The sets the design keeps whose features (some of them at least) are still there.
fn records(doc: &Document) -> Vec<Record> {
    doc.plugin_data
        .iter()
        .filter(|(key, _)| key.starts_with(STORE))
        .filter_map(|(_, data)| decode(data))
        .filter(|r| r.features().iter().any(|f| doc.feature(*f).is_some()))
        .collect()
}

/// Keeps `rec` with the design, and forgets the sets whose features are all gone.
fn keep(s: &mut Session, rec: &Record, warnings: &mut Vec<String>) {
    let key = record_key(rec.group);
    let dead: Vec<String> = s
        .doc
        .plugin_data
        .iter()
        .filter(|(k, _)| k.starts_with(STORE) && **k != key)
        // (A record this build can't read is another version's: it stays.)
        .filter(|(_, data)| decode(data).is_some_and(|r| !r.features().iter().any(|f| s.doc.feature(*f).is_some())))
        .map(|(k, _)| k.clone())
        .collect();
    let Ok(bytes) = serde_json::to_vec(rec) else { return };
    let doc = s.doc_mut();
    for k in dead {
        doc.plugin_data.remove(&k);
    }
    if doc.plugin_data.len() >= solidcraft_doc::plugin_data::MAX_FILES && !doc.plugin_data.contains_key(&key) {
        warnings.push("the design holds too much plugin data to keep this set's parameters: it can't be reopened in the Gear Generator".into());
        return;
    }
    doc.plugin_data.insert(key, solidcraft_doc::canvas::base64_encode(&bytes));
}

/// The names the set's bodies have now: a body follows the feature that made it through a
/// rename.
fn current_names(s: &Session, rec: &Record) -> Vec<String> {
    let state = s.model.state();
    rec.bodies
        .iter()
        .map(|b| {
            let made_by = b.steps.first().and_then(|step| step.feature);
            made_by.and_then(|f| state.bodies.iter().find(|body| body.feature == f)).map(|body| body.name.clone()).unwrap_or_else(|| b.name.clone())
        })
        .collect()
}

fn set_of(s: &Session, rec: &Record) -> GearSet {
    let name = s.doc.timeline_groups.iter().find(|g| g.id == rec.group).map(|g| g.name.clone()).unwrap_or_else(|| rec.name.clone());
    GearSet { group: rec.group, name, bodies: current_names(s, rec), features: rec.features(), request: rec.request.clone(), params: rec.params.clone() }
}

/// Every gear set kept in the session's design, for front ends.
pub fn sets(s: &Session) -> Vec<GearSet> {
    records(&s.doc).iter().map(|r| set_of(s, r)).collect()
}

/// The gear set something selected belongs to (a body, a face or edge of one, or one of its
/// sketches and features), for front ends: what "edit this gear" means.
pub fn selected_set(s: &Session) -> Option<GearSet> {
    use crate::Sel;
    let all = sets(s);
    s.selection.iter().find_map(|sel| match sel {
        Sel::Body { name } | Sel::Face { body: name, .. } | Sel::Edge { body: name, .. } => all.iter().find(|set| set.bodies.contains(name)).cloned(),
        Sel::Feature { id } => all.iter().find(|set| set.features.contains(id)).cloned(),
        _ => None,
    })
}

fn find(s: &Session, cmd: &str, group: Option<u64>, body: Option<&str>) -> Result<Record> {
    let all = records(&s.doc);
    let found = match (group, body) {
        (Some(group), _) => all.into_iter().find(|r| r.group == group),
        (None, Some(body)) => all.into_iter().find(|r| current_names(s, r).iter().any(|n| n == body)),
        (None, None) => return Err(bad(cmd, "`group` (the set's timeline folder) or `body` (one of its bodies) is required")),
    };
    found.ok_or_else(|| bad(cmd, "no gear set there: only a set made by gear.create and kept with the design can be inspected or updated"))
}

fn inspect(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.inspect";
    let q: GearInspectParams = serde_json::from_value(if p.is_null() { json!({}) } else { p.clone() }).map_err(|e| bad(cmd, e.to_string()))?;
    let sets = if q.group.is_none() && q.body.is_none() { sets(s) } else { vec![set_of(s, &find(s, cmd, q.group, q.body.as_deref())?)] };
    reply(cmd, GearInspectResult { sets })
}

/// Why the set can't be regenerated in place (`None`: it can): the new plan has other bodies or
/// steps than the ones that were built, or something that was built is gone.
fn must_rebuild(s: &Session, rec: &Record, plan: &Plan) -> Option<String> {
    if rec.bodies.len() != plan.bodies.len() {
        return Some(format!("the set had {} bodies and now has {}", rec.bodies.len(), plan.bodies.len()));
    }
    for (old, new) in rec.bodies.iter().zip(&plan.bodies) {
        if old.steps.len() != new.steps.len() {
            return Some(format!("{} was built in {} steps and now takes {}", old.name, old.steps.len(), new.steps.len()));
        }
        for (i, (was, step)) in old.steps.iter().zip(&new.steps).enumerate() {
            let def = step_def(new, i, step, 0.0);
            if was.shape != def.shape || was.sketches.len() != def.sketches.len() {
                return Some(format!("{}: step {} was a {} and is now a {}", old.name, i + 1, was.shape, def.shape));
            }
            let there = was.feature.is_some_and(|f| s.doc.feature(f).is_some()) && was.sketches.iter().all(|k| s.doc.sketch(k.id).is_ok());
            if !there {
                return Some(format!("{}: a sketch or the feature of step {} was deleted", old.name, i + 1));
            }
        }
    }
    None
}

/// Puts sketch `id` on the plane of `def`.
fn move_sketch(s: &mut Session, id: u64, def: &SketchDef) -> Result<()> {
    let plane = super::sketch::plane_ref(s, &json!({"plane": {"origin": def.origin, "x_dir": def.x, "y_dir": def.y}}), "gear.update")?;
    match s.doc_mut().feature_mut(id).map(|f| &mut f.kind) {
        Some(FeatureKind::Sketch { plane: slot, .. }) => {
            *slot = plane;
            Ok(())
        }
        _ => Err(EngineError::Other(format!("sketch {id} is gone"))),
    }
}

/// Draws the outline of `def` in sketch `id` in place of what it holds. The new curves take
/// the old ones' ids in order, as far as there are as many: the faces a feature makes from the
/// sketch are named after those ids, so what was built on them still finds them.
fn redraw_sketch(s: &mut Session, id: u64, def: &SketchDef) -> Result<()> {
    let tol = 1e-7;
    edit_solved(s, id, false, Analysis::Freedom, |sk, _| {
        let old: Vec<String> = sk.curves.iter().map(|c| c.id.clone()).collect();
        let all: Vec<usize> = (0..sk.curves.len()).collect();
        sk.remove_curves(&all);
        let mut new = Vec::new();
        for g in &def.loops {
            let (cs, _) = add_geometry(sk, g, &|q| q, 1.0, tol)?;
            new.extend(cs);
        }
        for (i, id) in new.into_iter().zip(old) {
            if let Some(c) = sk.curves.get_mut(i) {
                c.id = id;
            }
        }
        Ok(((), Left::Unsolved))
    })?;
    Ok(())
}

/// Regenerates the set where it stands: every sketch and feature keeps its id.
fn in_place(s: &mut Session, rec: &Record, plan: &Plan, names: &[String]) -> Result<Vec<BodyRecord>> {
    let gone = || EngineError::Other("a step of the set is gone".into());
    let mut bodies = Vec::new();
    for ((old, body), name) in rec.bodies.iter().zip(&plan.bodies).zip(names) {
        let mut steps = Vec::new();
        for (i, (was, step)) in old.steps.iter().zip(&body.steps).enumerate() {
            let feature = was.feature.ok_or_else(gone)?;
            let ids: Vec<u64> = was.sketches.iter().map(|k| k.id).collect();
            // What each sketch holds now, as the tries below leave it.
            let mut held: Vec<(String, String)> = was.sketches.iter().map(|k| (k.plane.clone(), k.geometry.clone())).collect();
            let trims = matches!(step, Step::Revolve { op, .. } if *op != Op::New);
            let nudges: &[f64] = if trims { &NUDGES } else { &NUDGES[..1] };
            let mut done = None;
            let mut last = None;
            for nudge in nudges {
                let def = step_def(body, i, step, *nudge);
                let mut changed = false;
                for ((sk, id), held) in def.sketches.iter().zip(&ids).zip(&mut held) {
                    let (plane, geometry) = (sk.plane_key(), sk.geometry_key());
                    if plane != held.0 {
                        move_sketch(s, *id, sk)?;
                        held.0 = plane;
                        changed = true;
                    }
                    if geometry != held.1 {
                        redraw_sketch(s, *id, sk)?;
                        held.1 = geometry;
                        changed = true;
                    }
                }
                if changed {
                    s.refresh();
                }
                let params = step_params(s, body, step, &ids, name, name)?;
                // The same feature over sketches that moved or were redrawn follows them by
                // itself; anything else is redefined.
                let outcome = if params == was.params {
                    match s.model.result(feature).and_then(|r| r.error.clone()) {
                        Some(e) => Err(EngineError::Other(e)),
                        None => Ok(()),
                    }
                } else {
                    s.execute("timeline.redefine", &json!({"feature": feature, "command": def.command, "params": params})).map(|_| ())
                };
                match outcome {
                    Ok(()) => {
                        done = Some((def, params));
                        break;
                    }
                    Err(e) => last = Some(e),
                }
            }
            let Some((def, params)) = done else { return Err(last.unwrap_or_else(gone)) };
            let sketches = ids.iter().zip(held).map(|(id, (plane, geometry))| SketchRecord { id: *id, plane, geometry }).collect();
            steps.push(StepRecord { shape: def.shape, command: def.command.into(), feature: Some(feature), params, sketches });
        }
        bodies.push(BodyRecord { name: name.clone(), steps });
    }
    Ok(bodies)
}

/// Regenerates the set by deleting what was built and building it again: under the same body
/// names, back in the set's folder and at its place in the history. Returns the bodies and the
/// folder.
fn rebuild(s: &mut Session, rec: &Record, plan: &Plan, names: &[String], warnings: &mut Vec<String>) -> Result<(Vec<BodyRecord>, Option<u64>)> {
    let old: Vec<u64> = rec.features().into_iter().filter(|f| s.doc.feature(*f).is_some()).collect();
    let first = old.iter().filter_map(|f| s.doc.feature_index(*f)).min();
    if !old.is_empty() {
        s.execute("timeline.delete", &json!({"features": old}))?;
    }
    let rest = s.doc.features.len();
    let bodies = build_set(s, plan, names, warnings)?;
    let features: Vec<u64> = bodies.iter().flat_map(BodyRecord::features).collect();
    if features.is_empty() {
        return Ok((bodies, None));
    }
    if let Some(first) = first.filter(|first| *first < rest)
        && let Err(e) = s.execute("timeline.reorder_many", &json!({"features": features, "position": first, "allow_errors": true}))
    {
        warnings.push(format!("the regenerated features could not go back to the set's place in the history ({e}): they are at its end"));
    }
    // Back in the set's folder when it is still there (it may hold other features); otherwise a
    // folder of the same name, under the same id when nothing has taken it.
    let folder = s.doc.timeline_groups.iter().any(|g| g.id == rec.group);
    if folder && s.execute("timeline.move_to_group", &json!({"features": features, "group": rec.group})).is_ok() {
        return Ok((bodies, Some(rec.group)));
    }
    let r = s.execute("timeline.group", &json!({"name": rec.name, "features": features}))?;
    let Some(made) = r.get("group").and_then(Value::as_u64) else { return Ok((bodies, None)) };
    let free = !folder && s.doc.feature(rec.group).is_none();
    if free {
        for g in &mut s.doc_mut().timeline_groups {
            if g.id == made {
                g.id = rec.group;
            }
        }
        return Ok((bodies, Some(rec.group)));
    }
    Ok((bodies, Some(made)))
}

fn update(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.update";
    if s.active_sketch.is_some() {
        return Err(bad(cmd, "finish the sketch being edited first"));
    }
    if s.doc.marker.is_some_and(|m| m < s.doc.features.len()) {
        return Err(bad(cmd, "roll the history marker to the end first: the set is regenerated in the whole design"));
    }
    let mut given = p.as_object().cloned().unwrap_or_default();
    let group = match given.remove("group") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().ok_or_else(|| bad(cmd, "`group` must be a timeline folder id"))?),
    };
    let body = match given.remove("body") {
        None | Some(Value::Null) => None,
        Some(Value::String(name)) => Some(name),
        Some(_) => return Err(bad(cmd, "`body` must be a body's name")),
    };
    let reset = match given.remove("reset") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(reset)) => reset,
        Some(_) => return Err(bad(cmd, "`reset` must be true or false")),
    };
    if s.model.is_stale() {
        s.refresh();
    }
    let mut rec = find(s, cmd, group, body.as_deref())?;
    // What was asked last time, with what is given now laid over it.
    let mut request = if reset { serde_json::Map::new() } else { rec.request.as_object().cloned().unwrap_or_default() };
    for (key, value) in given {
        if value.is_null() {
            request.remove(&key);
        } else {
            request.insert(key, value);
        }
    }
    let request = Value::Object(request);
    let (asked, plan) = ask(s, cmd, &request)?;
    let mut warnings = plan.warnings.clone();
    let own: std::collections::BTreeSet<u64> = rec.features().into_iter().collect();
    let worked: std::collections::BTreeSet<u64> = s.model.results.iter().filter(|r| r.error.is_none() && !r.skipped && !own.contains(&r.id)).map(|r| r.id).collect();
    let names = current_names(s, &rec);
    let before = s.doc.clone();
    let mut why = must_rebuild(s, &rec, &plan);
    if why.is_none() {
        match in_place(s, &rec, &plan, &names) {
            Ok(bodies) => rec.bodies = bodies,
            // Whatever it got done is taken back, and the set is built again instead.
            Err(e) => {
                s.doc = before;
                s.active_sketch = None;
                s.refresh();
                why = Some(format!("a step could not be changed in place ({e})"));
            }
        }
    }
    let mut group = Some(rec.group);
    if why.is_some() {
        // Bodies keep their names where the set still has as many; another set of members is
        // named afresh.
        let keep_names: &[String] = if names.len() == plan.bodies.len() { &names } else { &[] };
        let (bodies, folder) = rebuild(s, &rec, &plan, keep_names, &mut warnings)?;
        rec.bodies = bodies;
        group = folder;
    }
    let old_key = record_key(rec.group);
    rec.request = request;
    rec.params = params_of(&asked.spec);
    match group {
        Some(group) => {
            if group != rec.group {
                s.doc_mut().plugin_data.remove(&old_key);
                rec.group = group;
            }
            keep(s, &rec, &mut warnings);
        }
        None => {
            s.doc_mut().plugin_data.remove(&old_key);
        }
    }
    s.refresh();
    let broken = s
        .model
        .results
        .iter()
        .filter(|r| worked.contains(&r.id))
        .filter_map(|r| r.error.as_ref().map(|e| GearBroken { feature: r.name.clone(), error: e.clone() }))
        .collect();
    reply(
        cmd,
        GearResult {
            bodies: rec.bodies.iter().map(|b| b.name.clone()).collect(),
            group,
            facts: facts(&asked, &plan),
            warnings,
            notes: notes(&asked),
            updated: Some(if why.is_some() { "rebuilt" } else { "in_place" }.into()),
            rebuilt_because: why,
            broken,
            ..GearResult::default()
        },
    )
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

/// A new sketch on the plane through `origin` with axes `x`, `y` (world), holding the loops;
/// returns its id.
fn new_sketch(s: &mut Session, def: &SketchDef) -> Result<u64> {
    let r = s.execute("sketch.create", &json!({"plane": {"origin": def.origin, "x_dir": def.x, "y_dir": def.y}, "name": def.name, "project_edges": false}))?;
    let id = r.get("sketch").and_then(Value::as_u64).ok_or_else(|| EngineError::Other("no sketch".into()))?;
    let tol = 1e-7;
    edit_solved(s, id, false, Analysis::Freedom, |sk, _| {
        for g in &def.loops {
            add_geometry(sk, g, &|q| q, 1.0, tol)?;
        }
        Ok(((), Left::Unsolved))
    })?;
    s.execute("sketch.finish", &json!({}))?;
    Ok(id)
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

/// One sketch of a build step: its plane (origin and axes, world) and its closed loops.
struct SketchDef {
    name: String,
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    loops: Vec<Vec<Geom2>>,
}

impl SketchDef {
    /// Tells this plane from another.
    fn plane_key(&self) -> String {
        digest(&format!("{:?}{:?}{:?}", self.origin, self.x, self.y))
    }
    /// Tells this outline from another.
    fn geometry_key(&self) -> String {
        digest(&format!("{:?}", self.loops))
    }
}

/// FNV-1a of a text, in hex: the same in every build, which a set kept in a file needs.
fn digest(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// What one build step is made of: its sketches, and the command that makes its feature from
/// them ([`step_params`]).
struct StepDef {
    sketches: Vec<SketchDef>,
    command: &'static str,
    /// The step's kind and operation: two steps of one shape differ only in numbers and
    /// outlines, so one can be turned into the other in place.
    shape: String,
    /// The feature picks profiles of its sketch by a point inside each (a region with holes).
    picks: bool,
}

/// How far along its axis a trimming blank (a bevel's back and front cones) is tried: the
/// kernel sometimes refuses the intersection where the cones graze a root fillet, and takes it
/// a hair further along.
const NUDGES: [f64; 4] = [0.0, 0.0137, -0.0213, 0.0391];

/// The sketches and the command of step `i` of `body` (a revolved blank moved `nudge` along
/// the axis).
fn step_def(body: &Body, i: usize, step: &Step, nudge: f64) -> StepDef {
    let f: &Frame = &body.frame;
    let (x, y, z) = (f.x, f.y(), f.z);
    let label = format!("{} {}", body.name, i + 1);
    let sketch = |name: String, origin: [f64; 3], x: [f64; 3], y: [f64; 3], loops: Vec<Vec<Geom2>>| SketchDef { name: format!("{label} {name}"), origin, x, y, loops };
    // A sweep's path: a line up the axis, on the plane through it and x.
    let path = |z0: f64, height: f64| sketch("path".into(), f.world([0.0, 0.0, z0]), z, x, vec![vec![Geom2::Line(Vec2::new(0.0, 0.0), Vec2::new(height, 0.0))]]);
    let outline = |o: &Outline| {
        let mut g = Vec::new();
        geometry(o, &mut g);
        vec![g]
    };
    match step {
        Step::Prism { region, z0, height, twist, op } => {
            let profile = sketch("profile".into(), f.world([0.0, 0.0, *z0]), x, y, region_loops(region));
            let picks = !region.holes.is_empty();
            if twist.abs() < 1e-12 {
                StepDef { sketches: vec![profile], command: "solid.extrude", shape: format!("prism ({})", op_name(*op)), picks }
            } else {
                StepDef { sketches: vec![profile, path(*z0, *height)], command: "solid.sweep", shape: format!("twisted prism ({})", op_name(*op)), picks }
            }
        }
        // Straight: a loft between the two ends builds faster than a scaled sweep.
        Step::Taper { outline: o, z0, height, twist, scale, op } if twist.abs() <= 1e-12 => {
            let ends = [(o.clone(), *z0), (o.scaled(*scale), z0 + height)];
            let sketches = ends.iter().enumerate().map(|(k, (o, h))| sketch(format!("section {}", k + 1), f.world([0.0, 0.0, *h]), x, y, outline(o))).collect();
            StepDef { sketches, command: "solid.loft", shape: format!("taper ({})", op_name(*op)), picks: false }
        }
        Step::Taper { outline: o, z0, height, op, .. } => {
            let profile = sketch("profile".into(), f.world([0.0, 0.0, *z0]), x, y, outline(o));
            StepDef { sketches: vec![profile, path(*z0, *height)], command: "solid.sweep", shape: format!("twisted taper ({})", op_name(*op)), picks: false }
        }
        Step::Loft { sections, op } => {
            let sketches: Vec<SketchDef> = sections.iter().enumerate().map(|(k, (o, h))| sketch(format!("section {}", k + 1), f.world([0.0, 0.0, *h]), x, y, outline(o))).collect();
            let shape = format!("loft of {} ({})", sketches.len(), op_name(*op));
            StepDef { sketches, command: "solid.loft", shape, picks: false }
        }
        Step::Revolve { profile, op } => {
            let moved: Vec<solidcraft_gear::V2> = profile.iter().map(|q| solidcraft_gear::V2::new(q.x, q.y + nudge)).collect();
            StepDef { sketches: vec![sketch("blank".into(), f.origin, x, z, vec![polygon(&moved)])], command: "solid.revolve", shape: format!("revolve ({})", op_name(*op)), picks: false }
        }
        Step::Coil { section, op, .. } => {
            StepDef { sketches: vec![sketch("thread".into(), f.origin, x, z, vec![polygon(section)])], command: "solid.coil", shape: format!("coil ({})", op_name(*op)), picks: false }
        }
    }
}

/// The parameters of the feature of a step over its sketches (in [`step_def`]'s order). It
/// works on the body `target` (which the first step makes, as `body_name`).
fn step_params(s: &Session, body: &Body, step: &Step, sketches: &[u64], target: &str, body_name: &str) -> Result<Value> {
    let f: &Frame = &body.frame;
    let id = |k: usize| sketches.get(k).copied().ok_or_else(|| EngineError::Other("a gear step lost a sketch".into()));
    let own = |op: Op| feature_params(op, target, body_name);
    let turn = |twist: f64| format!("{} deg", twist.to_degrees());
    Ok(match step {
        Step::Prism { region, height, twist, op, .. } => {
            let mut pick = json!({"sketch": id(0)?});
            if let Some(p) = profiles_of(region) {
                pick["profiles"] = p;
            }
            if twist.abs() < 1e-12 {
                merge(merge(pick, json!({"distance": height})), own(*op))
            } else {
                let path = id(1)?;
                merge(merge(pick, json!({"path_sketch": path, "path": [first_curve(s, path)?], "twist": turn(*twist)})), own(*op))
            }
        }
        Step::Taper { twist, op, .. } if twist.abs() <= 1e-12 => merge(json!({"sections": [{"sketch": id(0)?}, {"sketch": id(1)?}]}), own(*op)),
        Step::Taper { twist, scale, op, .. } => {
            let path = id(1)?;
            merge(json!({"sketch": id(0)?, "path_sketch": path, "path": [first_curve(s, path)?], "scale": scale, "twist": turn(*twist)}), own(*op))
        }
        Step::Loft { sections, op } => {
            let ids = (0..sections.len()).map(|k| id(k).map(|i| json!({"sketch": i}))).collect::<Result<Vec<Value>>>()?;
            merge(json!({"sections": ids}), own(*op))
        }
        Step::Revolve { op, .. } => merge(json!({"sketch": id(0)?, "axis": "y"}), own(*op)),
        Step::Coil { pitch, height, left, op, .. } => {
            merge(json!({"sketch": id(0)?, "pitch": pitch, "height": height, "base": f.origin, "axis": f.z, "clockwise": left}), own(*op))
        }
    })
}

/// Builds every body of the plan, each under its name in `names` where there is one (an
/// update keeps the names the bodies had).
fn build_set(s: &mut Session, plan: &Plan, names: &[String], warnings: &mut Vec<String>) -> Result<Vec<BodyRecord>> {
    let mut made = Vec::new();
    for (i, body) in plan.bodies.iter().enumerate() {
        let named = Body { name: names.get(i).cloned().unwrap_or_else(|| body.name.clone()), ..body.clone() };
        made.push(build_body(s, &named, warnings).map_err(|e| EngineError::Other(format!("{}: {e}", named.name)))?);
    }
    Ok(made)
}

/// Builds one body; returns what it was made of, under the name the design gave it.
fn build_body(s: &mut Session, body: &Body, warnings: &mut Vec<String>) -> Result<BodyRecord> {
    let mut name = body.name.clone();
    let mut steps = Vec::new();
    for (i, step) in body.steps.iter().enumerate() {
        // A blank that trims (a bevel's back and front cones) is tried again a hair further
        // along when the kernel refuses it, and left out with a warning if it still fails.
        let trims = matches!(step, Step::Revolve { op, .. } if *op != Op::New);
        let nudges: &[f64] = if trims { &NUDGES } else { &NUDGES[..1] };
        let mut done = None;
        for (k, nudge) in nudges.iter().enumerate() {
            let def = step_def(body, i, step, *nudge);
            let mut ids = Vec::new();
            for sketch in &def.sketches {
                ids.push(new_sketch(s, sketch)?);
            }
            if def.picks {
                // Picks are checked against the sketch's profiles, which a command run inside
                // another has not worked out yet.
                s.refresh();
            }
            let params = step_params(s, body, step, &ids, &name, &body.name)?;
            match s.execute(def.command, &params) {
                Ok(reply) => {
                    done = Some((def, ids, params, reply));
                    break;
                }
                Err(e) if trims => {
                    s.execute("timeline.delete", &json!({"features": ids})).ok();
                    if k + 1 == nudges.len() {
                        warnings.push(format!("{}: the back and front cones could not be trimmed ({e}); the teeth ends are left flat", body.name));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        let Some((def, ids, params, reply)) = done else {
            steps.push(StepRecord { shape: "skipped".into(), command: String::new(), feature: None, params: Value::Null, sketches: Vec::new() });
            continue;
        };
        if i == 0
            && let Some(made) = reply.get("created_bodies").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str)
        {
            name = made.to_string();
        }
        let sketches = def.sketches.iter().zip(&ids).map(|(d, id)| SketchRecord { id: *id, plane: d.plane_key(), geometry: d.geometry_key() }).collect();
        steps.push(StepRecord { shape: def.shape, command: def.command.into(), feature: feature_id(&reply), params, sketches });
    }
    Ok(BodyRecord { name, steps })
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
