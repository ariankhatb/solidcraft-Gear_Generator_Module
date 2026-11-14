//! The Gear Generator: `gear.create` builds a gear set (from `solidcraft_gear`'s plan) out of
//! ordinary sketches and features, grouped in the timeline as one undo step; `gear.calculate`
//! only works out its numbers and warnings.

use serde_json::{Value, json};
use solidcore_record::Vec2;
use solidcraft_gear::geom::{Outline, Region, Seg};
use solidcraft_gear::hub::{Bore, BoreKind};
use solidcraft_gear::involute::BasicRack;
use solidcraft_gear::plan::{Body, Frame, Kind, Op, Plan, Spec, Step, Unit};
use solidcraft_io::Geom2;
use solidcraft_sketch::Analysis;

use super::CommandSpec;
use super::sketch::{Left, edit_solved, ids_of};
use super::sketch_import::add_geometry;
use crate::params::bad;
use crate::schema::gear::{BeltProfile, BoreParams, BoreType, ChainSize, GearFact, GearMaterial, GearParams, GearResult, GearType, RackStandard, SplineStandardParam};
use solidcraft_gear::spline::SplineStandard;
use solidcraft_gear::strength::Material;
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
            "type?: spur|helical|herringbone|internal|rack|pair|planetary|bevel|spiral_bevel|hypoid|crossed_helical|worm|cycloidal|cycloidal_drive|sprocket|pulley|spline|face_gear|elliptical; \
             module? (mm) | diametral_pitch?; teeth?, teeth2? (mate or planet), ring_teeth?, planets?; pressure_angle?, helix_angle? (deg, sign = hand); \
             profile_shift?, profile_shift2?; rack?: iso53_a|iso53_b|iso53_c|iso53_d|agma_full_depth|stub|print_fdm, addendum?, dedendum?, root_radius? (coefficients); \
             backlash? (mm); face_width?; center_distance? (pair: gear 2's shift reaches it); shaft_angle?, spiral_angle? (deg); offset?, face_start? (hypoid, mm); \
             starts?, worm_diameter?, worm_length? (worm); rim?; rack_teeth?, rack_height?; pins?, pin_circle?, pin_diameter?, eccentricity? (cycloidal drive); \
             bore?, bore2?: {type?: round|keyway|d_flat|hex|square|none, diameter, key_width?, key_depth?, flat?}; hub_diameter?, hub_length?; holes?, hole_diameter?; \
             sprocket: chain?: 04b|05b|06b|08b|10b|12b|16b|20b|24b|ansi25|ansi35|ansi40|ansi50|ansi60|ansi80|ansi100|ansi120|custom (chain_pitch?, roller_diameter?, roller_width?, strand_pitch?), \
             strands? (1…4), side_chamfer?; teeth2 > 0 and center_distance? for a pair (even links); pulley: belt?: gt2_2|gt2_3|gt2_5|htd_3m|htd_5m|htd_8m|t2_5|t5|t10|mxl|xl, \
             flanges?, face_width = belt width; spline: spline_standard?: din5480|ansi_b92, spline_diameter? (DIN reference Ø; default module × (teeth + 1.1)), fillet_root?, backlash = flank clearance, face_width = length; \
             face_gear: teeth (pinion), teeth2 (face gear), inner_diameter? (default: the undercut limit), face_width = tooth length; elliptical: ellipse_eccentricity? (0…0.7), teeth on each gear; \
             only?: 1|2|3 (one member); origin?: [x,y,z]; name?; strength rating: torque? (N·m) | power? (kW), speed? (rpm) on the driver, \
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
];

const KINDS: [(GearType, Kind); 19] = [
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
    (GearType::Sprocket, Kind::Sprocket),
    (GearType::Pulley, Kind::Pulley),
    (GearType::Spline, Kind::Spline),
    (GearType::FaceGear, Kind::FaceGear),
    (GearType::Elliptical, Kind::Elliptical),
];

const CHAINS: [(ChainSize, &str); 18] = [
    (ChainSize::B04, "04b"),
    (ChainSize::B05, "05b"),
    (ChainSize::B06, "06b"),
    (ChainSize::B08, "08b"),
    (ChainSize::B10, "10b"),
    (ChainSize::B12, "12b"),
    (ChainSize::B16, "16b"),
    (ChainSize::B20, "20b"),
    (ChainSize::B24, "24b"),
    (ChainSize::Ansi25, "ansi25"),
    (ChainSize::Ansi35, "ansi35"),
    (ChainSize::Ansi40, "ansi40"),
    (ChainSize::Ansi50, "ansi50"),
    (ChainSize::Ansi60, "ansi60"),
    (ChainSize::Ansi80, "ansi80"),
    (ChainSize::Ansi100, "ansi100"),
    (ChainSize::Ansi120, "ansi120"),
    (ChainSize::Custom, solidcraft_gear::sprocket::CUSTOM),
];

const BELTS: [(BeltProfile, &str); 11] = [
    (BeltProfile::Gt2_2, "gt2_2"),
    (BeltProfile::Gt2_3, "gt2_3"),
    (BeltProfile::Gt2_5, "gt2_5"),
    (BeltProfile::Htd3m, "htd_3m"),
    (BeltProfile::Htd5m, "htd_5m"),
    (BeltProfile::Htd8m, "htd_8m"),
    (BeltProfile::T2_5, "t2_5"),
    (BeltProfile::T5, "t5"),
    (BeltProfile::T10, "t10"),
    (BeltProfile::Mxl, "mxl"),
    (BeltProfile::Xl, "xl"),
];

const SPLINES: [(SplineStandardParam, SplineStandard); 2] = [(SplineStandardParam::Din5480, SplineStandard::Din5480), (SplineStandardParam::AnsiB92, SplineStandard::AnsiB92)];

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
    if let Some(c) = p.chain {
        s.chain = CHAINS.iter().find(|(k, _)| *k == c).map(|(_, id)| *id).unwrap_or("08b");
    }
    set!(chain_pitch, p.chain_pitch);
    set!(roller_diameter, p.roller_diameter);
    set!(roller_width, p.roller_width);
    set!(strand_pitch, p.strand_pitch);
    set!(strands, p.strands);
    set!(side_chamfer, p.side_chamfer);
    if let Some(b) = p.belt {
        s.belt = BELTS.iter().find(|(k, _)| *k == b).map(|(_, id)| *id).unwrap_or("gt2_2");
    }
    set!(flanges, p.flanges);
    if let Some(st) = p.spline_standard {
        s.spline_standard = SPLINES.iter().find(|(k, _)| *k == st).map(|(_, v)| *v).unwrap_or_default();
    }
    set!(spline_diameter, p.spline_diameter);
    set!(fillet_root, p.fillet_root);
    set!(inner_diameter, p.inner_diameter);
    set!(ellipse_eccentricity, p.ellipse_eccentricity);
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
        s.chain_pitch,
        s.roller_diameter,
        s.roller_width,
        s.strand_pitch,
        s.spline_diameter,
        s.inner_diameter,
        s.ellipse_eccentricity,
    ];
    if finite.iter().chain(s.origin.iter()).any(|v| !v.is_finite()) {
        return Err("every number must be finite".into());
    }
    if [
        s.backlash,
        s.center_distance,
        s.offset,
        s.worm_diameter,
        s.worm_length,
        s.rim,
        s.rack_height,
        s.hub_diameter,
        s.hub_length,
        s.hole_diameter,
        s.bore.diameter,
        s.bore2.diameter,
        s.chain_pitch,
        s.roller_diameter,
        s.roller_width,
        s.strand_pitch,
        s.spline_diameter,
        s.inner_diameter,
    ]
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

/// The parameters that describe `s` (the inverse of [`spec_of`]), for front ends.
pub fn params_of(s: &Spec) -> Value {
    let kind = KINDS.iter().find(|(_, k)| *k == s.kind).map(|(t, _)| *t);
    let rack = RACKS.iter().find(|(_, n)| BasicRack::standard(n) == Some(s.rack)).map(|(r, _)| *r);
    let custom = rack.is_none();
    let p = GearParams {
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
        chain: None,
        chain_pitch: None,
        roller_diameter: None,
        roller_width: None,
        strand_pitch: None,
        strands: None,
        side_chamfer: None,
        belt: None,
        flanges: None,
        spline_standard: None,
        spline_diameter: None,
        fillet_root: None,
        inner_diameter: None,
        ellipse_eccentricity: None,
        torque: (s.load.torque > 0.0).then_some(s.load.torque),
        power: (s.load.power > 0.0).then_some(s.load.power),
        speed: (s.load.speed > 0.0).then_some(s.load.speed),
        material: material_param(s.load.material),
        material2: material_param(s.load.material2),
        material3: s.load.material3.and_then(material_param),
        application_factor: (s.load.application_factor != 1.0).then_some(s.load.application_factor),
        life_hours: (s.load.life_hours > 0.0).then_some(s.load.life_hours),
        accuracy_grade: (s.load.quality != 7).then_some(s.load.quality),
    };
    let p = with_extras(s, p);
    serde_json::to_value(p).map(strip_nulls).unwrap_or(Value::Null)
}

/// The fields only sprockets, pulleys, splines and face gears use, for those types.
fn with_extras(s: &Spec, mut p: GearParams) -> GearParams {
    match s.kind {
        Kind::Sprocket => {
            p.chain = CHAINS.iter().find(|(_, id)| *id == s.chain).map(|(k, _)| *k);
            p.chain_pitch = Some(s.chain_pitch);
            p.roller_diameter = Some(s.roller_diameter);
            p.roller_width = Some(s.roller_width);
            p.strand_pitch = Some(s.strand_pitch);
            p.strands = Some(s.strands);
            p.side_chamfer = Some(s.side_chamfer);
        }
        Kind::Pulley => {
            p.belt = BELTS.iter().find(|(_, id)| *id == s.belt).map(|(k, _)| *k);
            p.flanges = Some(s.flanges);
        }
        Kind::Spline => {
            p.spline_standard = SPLINES.iter().find(|(_, v)| *v == s.spline_standard).map(|(k, _)| *k);
            p.spline_diameter = Some(s.spline_diameter);
            p.fillet_root = Some(s.fillet_root);
        }
        Kind::FaceGear => p.inner_diameter = (s.inner_diameter > 0.0).then_some(s.inner_diameter),
        Kind::Elliptical => p.ellipse_eccentricity = Some(s.ellipse_eccentricity),
        _ => {}
    }
    p
}

fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, strip_nulls(v))).collect()),
        v => v,
    }
}

fn parse(cmd: &str, p: &Value) -> Result<(Spec, Plan)> {
    let params: GearParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let spec = spec_of(&params).map_err(|e| bad(cmd, e))?;
    let plan = solidcraft_gear::plan(&spec).map_err(|e| bad(cmd, e))?;
    Ok((spec, plan))
}

fn facts(plan: &Plan) -> Vec<GearFact> {
    plan.facts
        .iter()
        .map(|f| GearFact {
            label: f.label.clone(),
            value: f.value,
            unit: match f.unit {
                Unit::Mm => "mm",
                Unit::Deg => "deg",
                Unit::None => "",
            }
            .into(),
        })
        .collect()
}

fn reply(cmd: &str, r: GearResult) -> Result<Value> {
    serde_json::to_value(r).map_err(|e| bad(cmd, e.to_string()))
}

fn calculate(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.calculate";
    let (_, plan) = parse(cmd, p)?;
    reply(cmd, GearResult { bodies: Vec::new(), group: None, facts: facts(&plan), warnings: plan.warnings.clone(), param_warnings: Vec::new() })
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "gear.create";
    let (spec, plan) = parse(cmd, p)?;
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
    reply(cmd, GearResult { bodies: made, group, facts: facts(&plan), warnings, param_warnings: Vec::new() })
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
