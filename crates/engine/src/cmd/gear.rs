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
use solidcraft_gear::hub::{Bore, BoreKind, Chamfer, Hardware, Shaft, ShaftEnd};
use solidcraft_geom::PlaneGeom as _;
use solidcraft_gear::involute::BasicRack;
use solidcraft_gear::axes::{self, Axis, Drive, Relation, Solved};
use solidcraft_gear::clearance::{BearingSize, Clearance, Seat};
use solidcraft_gear::plan::{Body, Fact, Frame, Kind, Op, Plan, Spec, Step, Unit};
use solidcraft_gear::sketch_rules;
use solidcraft_io::Geom2;
use solidcraft_sketch::Analysis;

use super::CommandSpec;
use super::sketch::{Left, edit_solved};
use super::sketch_import::add_geometry;
use crate::params::bad;
use crate::schema::gear::{
    BearingSizeParams, BeltProfile, BoreParams, BoreType, ChainSize, ChamferParams, ClearanceBearing, ClearanceParams, ClearanceSeat, SeatAlongParams, SeatPointParams, GearAxesResult, GearAxis, GearAxisLine, GearBevelPlanetMode, GearBroken, GearCandidate, GearFact, GearFrame, GearInspectParams, GearInspectResult,
    GearMaterial, GearParams, GearRelation, GearResult, GearSet, GearType, HardwareParams, PlacementParams, RackStandard, ShaftEndType, ShaftParams, SpiralHand, SplineStandardParam,
};
use solidcraft_gear::spline::SplineStandard;
use solidcraft_gear::strength::Material;
use crate::schema::{TypedContract, output_schema_for};
use crate::{EngineError, Result, Session};

const GEAR_CONTRACT: TypedContract<GearParams> = TypedContract::new();
const INSPECT_CONTRACT: TypedContract<GearInspectParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("gear.create", "Gear Generator", crate::checked!("gear.create", create))
        .at("SOLID", "CREATE")
        .icon("gear_generator")
        .contract(&GEAR_CONTRACT)
        .output(output_schema_for::<GearResult>)
        .params(
            "type?: spur|helical|herringbone|internal|rack|pair|planetary|bevel|spiral_bevel|hypoid|crossed_helical|worm|cycloidal|cycloidal_drive|sprocket|pulley|spline|face_gear|elliptical; \
             module? (mm) | diametral_pitch?; teeth?, teeth2? (mate or planet), ring_teeth?, planets?; pressure_angle?, helix_angle? (deg, sign = hand); \
             profile_shift?, profile_shift2?; rack?: iso53_a|iso53_b|iso53_c|iso53_d|agma_full_depth|stub|print_fdm, addendum?, dedendum?, root_radius? (coefficients); \
             backlash? (mm); face_width?; center_distance? (pair: gear 2's shift reaches it); shaft_angle?, spiral_angle? (deg); offset?, ring_diameter? (hypoid, mm), hand?: left|right (hypoid pinion); \
             starts?, worm_diameter?, worm_length? (worm); rim?; rack_teeth?, rack_height?; pins?, pin_circle?, pin_diameter?, eccentricity? (cycloidal drive); \
             bore?, bore2?: {type?: round|keyway|d_flat|hex|square|none, diameter, key_width?, key_depth?, flat?}; hub_diameter?, hub_length?; holes?, hole_diameter?; \
             sprocket: chain?: 04b|05b|06b|08b|10b|12b|16b|20b|24b|ansi25|ansi35|ansi40|ansi50|ansi60|ansi80|ansi100|ansi120|custom (chain_pitch?, roller_diameter?, roller_width?, strand_pitch?), \
             strands? (1…4), side_chamfer?; teeth2 > 0 and center_distance? for a pair (even links); pulley: belt?: gt2_2|gt2_3|gt2_5|htd_3m|htd_5m|htd_8m|t2_5|t5|t10|mxl|xl, \
             flanges?, face_width = belt width; spline: spline_standard?: din5480|ansi_b92, spline_diameter? (DIN reference Ø; default module × (teeth + 1.1)), fillet_root?, backlash = flank clearance, face_width = length; \
             face_gear: teeth (pinion), teeth2 (face gear), inner_diameter? (default: the undercut limit), face_width = tooth length; elliptical: ellipse_eccentricity? (0…0.7), teeth on each gear; \
             only?: 1|2|3 (one member); origin?: [x,y,z]; coupon?: bool, coupon_teeth? (pair, planetary: a print test coupon instead of the set); name?. Axis-first: axis1?, axis2?: X|Y|Z | construction axis name | {origin?, dir} | {edge|face: [x,y,z]} | \
             {sketch, curve} | {sketch, point} (a sketch point: the shaft through it normal to the sketch) (the shafts: the set is sized and placed on them, type? defaults to what fits, module/teeth/teeth2/starts not given are chosen), \
             Drawn in one sketch: sketch?: id|name (circles = pitch circles, lines = shafts; type = the mode or what the drawing looks like), picks?: [point/curve ids per slot, origin point first], sketch_turn?: [x°, y°, normal°] about the origin point, \
             ratio? (teeth2 ÷ teeth), cone_distance? (bevels, mm), flip1?, flip2?, swap_axes?, along? (mm along axis1); placement?: {origin, x, z}; \
             strength rating: torque? (N·m) | power? (kW), speed? (rpm) on the driver, \
             material?, material2?, material3?: steel_normalized|steel_through_hardened|steel_case_hardened|steel_nitrided|cast_iron|bronze|pom|pa66|pla|petg, \
             application_factor?, life_hours?, accuracy_grade? (5..12); tooth tuning: tip_round? (modules), xy_compensation?, elephant_foot?, elephant_foot_height?, nozzle? (mm), \
             clearance? (c*), tip_shortening?, tip_diameter?, tip_diameter2?, tip_relief?, tip_relief_height?, crowning?, backlash2?, face_width2?, face_offset2? (pair); \
             tooth_chamfer_front?, tooth_chamfer_back?: {size, angle?}; bore_chamfer?, hub_chamfer?; shaft_front?, shaft_back?, shaft2_front?, shaft2_back?: \
             {diameter, length, step_diameter?, step_length?, end?: plain|d_flat|keyway|hex|cross|centre_hole, end_length?, end_size?, chamfer?}; \
             hardware?, hardware2?: {set_screw?, set_screws?, nut_width?, nut_thickness?, insert_diameter?, insert_depth?, clamp_slit?, clamp_screw?, bearing?: name (608, 625, 6001, MR105, ...), \
             bearing_od?, bearing_width?, bearing_fit?}; tooth_curves?: fitted (default) | equations (spur, helical, herringbone, internal, pair, planetary, rack: \
             one tooth of equation curves from the gear formulas, involute flank and trochoid root fillet, with tip and root arcs, repeated by a circular \
             pattern; tooth tuning that has no formula (tip relief, tip rounding, elephant foot, crowning) stays fitted), parametric?: bool (with equations: \
             design parameters <prefix>_module, _teeth, _teeth2, _teeth3, _pressure_angle, _helix, _shift, _backlash, _face_width and derived radii drive the \
             formulas, pattern counts and tooth extrusions: editing one regenerates the teeth; positions, hubs and bores keep their numbers until gear.update, \
             which reads the parameters back), parameter_prefix? (default from the name); clearance_body?: {radial? (mm, default 0.5), axial? (0.5), shaft_extension? (past each gear, default 2 × face_width), \
             ring_as_housing? (default true: an internal ring or pin ring is part of the housing and left out), bearing?: name (608, 6001, 6902, MR105, ...) | {od, width, bore?}, \
             bearing_fit? (on the diameter), seats?: [{member: 1|2|..., along: mm from the middle of its teeth} | {point: [x,y,z]} (onto the nearest member's axis)], \
             travel? (rack: its slide, default its length)} (one more body, \"<name> clearance\": every member's swept tooth envelope, shafts and bearing seats \
             in one solid, to cut a housing with; facts: envelope sizes, the bearing; warnings: seats past a shaft or on the teeth, bearing bore ≠ shaft). Returns bodies (and clearance: the clearance body's name), group, facts (diameters, ratio, centre distance, contact ratio, span and over-pins \
             measurements; with a load: forces, stresses, safety factors SF and SH) and warnings (undercut, pointed teeth, low contact ratio, interference, SF < 1.4, SH < 1)",
        ),
    CommandSpec::new("gear.calculate", "Gear Calculator", crate::checked!("gear.calculate", calculate))
        .icon("gear_generator")
        .noundo()
        .read_only()
        .contract(&GEAR_CONTRACT)
        .output(output_schema_for::<GearResult>)
        .params("the same as gear.create; builds nothing: returns the facts and warnings gear.create would"),
    CommandSpec::new("gear.from_axes", "Gears From Axes", crate::checked!("gear.from_axes", from_axes))
        .icon("gear_generator")
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
        .icon("gear_generator")
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

const KINDS: [(GearType, Kind); 20] = [
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
    (GearType::BevelPlanetary, Kind::BevelPlanetary),
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

const SHAFT_ENDS: [(ShaftEndType, ShaftEnd); 6] = [
    (ShaftEndType::Plain, ShaftEnd::Plain),
    (ShaftEndType::DFlat, ShaftEnd::DFlat),
    (ShaftEndType::Keyway, ShaftEnd::Keyway),
    (ShaftEndType::Hex, ShaftEnd::Hex),
    (ShaftEndType::Cross, ShaftEnd::Cross),
    (ShaftEndType::CentreHole, ShaftEnd::CentreHole),
];

fn shaft_of(p: &Option<ShaftParams>) -> Shaft {
    let Some(p) = p else { return Shaft::default() };
    let end = p.end.unwrap_or(ShaftEndType::Plain);
    Shaft {
        diameter: p.diameter,
        length: p.length,
        step_diameter: p.step_diameter.unwrap_or(0.0),
        step_length: p.step_length.unwrap_or(0.0),
        end: SHAFT_ENDS.iter().find(|(t, _)| *t == end).map(|(_, k)| *k).unwrap_or(ShaftEnd::Plain),
        end_length: p.end_length.unwrap_or(0.0),
        end_size: p.end_size.unwrap_or(0.0),
        chamfer: p.chamfer.unwrap_or(0.0),
    }
}

fn shaft_params(s: &Shaft) -> Option<ShaftParams> {
    if *s == Shaft::default() {
        return None;
    }
    Some(ShaftParams {
        diameter: s.diameter,
        length: s.length,
        step_diameter: Some(s.step_diameter),
        step_length: Some(s.step_length),
        end: SHAFT_ENDS.iter().find(|(_, k)| *k == s.end).map(|(t, _)| *t),
        end_length: Some(s.end_length),
        end_size: Some(s.end_size),
        chamfer: Some(s.chamfer),
    })
}

fn chamfer_of(p: &Option<ChamferParams>) -> Chamfer {
    let Some(p) = p else { return Chamfer::default() };
    Chamfer { size: p.size, angle: p.angle.unwrap_or(45.0) }
}

fn chamfer_params(c: &Chamfer) -> Option<ChamferParams> {
    if *c == Chamfer::default() {
        return None;
    }
    Some(ChamferParams { size: c.size, angle: Some(c.angle) })
}

fn hardware_of(p: &Option<HardwareParams>) -> std::result::Result<Hardware, String> {
    let mut h = Hardware::default();
    let Some(p) = p else { return Ok(h) };
    if let Some(name) = &p.bearing
        && !h.set_bearing(name)
    {
        return Err(format!("`bearing`: no standard bearing is called '{name}'"));
    }
    macro_rules! set {
        ($field:ident) => {
            if let Some(v) = p.$field {
                h.$field = v;
            }
        };
    }
    set!(set_screw);
    set!(set_screws);
    set!(nut_width);
    set!(nut_thickness);
    set!(insert_diameter);
    set!(insert_depth);
    set!(clamp_slit);
    set!(clamp_screw);
    set!(bearing_od);
    set!(bearing_width);
    set!(bearing_fit);
    Ok(h)
}

fn hardware_params(h: &Hardware) -> Option<HardwareParams> {
    if *h == Hardware::default() {
        return None;
    }
    Some(HardwareParams {
        set_screw: Some(h.set_screw),
        set_screws: Some(h.set_screws),
        nut_width: Some(h.nut_width),
        nut_thickness: Some(h.nut_thickness),
        insert_diameter: Some(h.insert_diameter),
        insert_depth: Some(h.insert_depth),
        clamp_slit: Some(h.clamp_slit),
        clamp_screw: Some(h.clamp_screw),
        bearing: None,
        bearing_od: Some(h.bearing_od),
        bearing_width: Some(h.bearing_width),
        bearing_fit: Some(h.bearing_fit),
    })
}

/// The clearance body's settings from their parameters (the gear crate checks the ranges and
/// the bearing's name).
fn clearance_of(p: &ClearanceParams) -> Clearance {
    let mut c = Clearance::default();
    if let Some(v) = p.radial {
        c.radial = v;
    }
    if let Some(v) = p.axial {
        c.axial = v;
    }
    if let Some(v) = p.shaft_extension {
        c.shaft_extension = v;
    }
    if let Some(v) = p.ring_as_housing {
        c.ring_as_housing = v;
    }
    match &p.bearing {
        Some(ClearanceBearing::Named(name)) => c.bearing = Some(name.clone()),
        Some(ClearanceBearing::Size(b)) => c.bearing_size = Some(BearingSize { bore: b.bore.unwrap_or(0.0), od: b.od, width: b.width }),
        None => {}
    }
    if let Some(v) = p.bearing_fit {
        c.bearing_fit = v;
    }
    if let Some(v) = p.travel {
        c.travel = v;
    }
    c.seats = p
        .seats
        .iter()
        .flatten()
        .map(|seat| match seat {
            ClearanceSeat::Along(a) => Seat::Along { member: a.member, along: a.along },
            ClearanceSeat::Point(q) => Seat::Point(q.point),
        })
        .collect();
    c
}

fn clearance_params(c: &Clearance) -> ClearanceParams {
    let bearing = match (&c.bearing, &c.bearing_size) {
        (Some(name), _) => Some(ClearanceBearing::Named(name.clone())),
        (None, Some(b)) => Some(ClearanceBearing::Size(BearingSizeParams { od: b.od, width: b.width, bore: (b.bore > 0.0).then_some(b.bore) })),
        (None, None) => None,
    };
    ClearanceParams {
        radial: Some(c.radial),
        axial: Some(c.axial),
        shaft_extension: (c.shaft_extension > 0.0).then_some(c.shaft_extension),
        ring_as_housing: Some(c.ring_as_housing),
        bearing,
        bearing_fit: (c.bearing_fit != 0.0).then_some(c.bearing_fit),
        seats: (!c.seats.is_empty()).then(|| {
            c.seats
                .iter()
                .map(|seat| match seat {
                    Seat::Along { member, along } => ClearanceSeat::Along(SeatAlongParams { member: *member, along: *along }),
                    Seat::Point(p) => ClearanceSeat::Point(SeatPointParams { point: *p }),
                })
                .collect()
        }),
        travel: (c.travel > 0.0).then_some(c.travel),
    }
}

/// The bodies a plan builds, in order: the gears, then its clearance body when it has one.
fn built_bodies(plan: &Plan) -> Vec<Body> {
    plan.bodies.iter().chain(plan.clearance.iter()).cloned().collect()
}

/// The clearance body's name among what was built, when it was.
fn clearance_name(plan: &Plan, bodies: &[BodyRecord]) -> Option<String> {
    (plan.clearance.is_some() && bodies.len() > plan.bodies.len()).then(|| bodies[bodies.len() - 1].name.clone())
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
    if let Some(h) = p.hand {
        s.left_hand = h == SpiralHand::Left;
    }
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
    set!(herringbone, p.herringbone);
    if let Some(a) = p.first_azimuth {
        if !a.is_finite() {
            return Err("`first_azimuth` must be a finite angle in degrees".into());
        }
        s.first_azimuth = a;
    }
    if let Some(m) = p.bevel_planet_mode {
        s.bevel_planet_mode = match m {
            GearBevelPlanetMode::Single => solidcraft_gear::plan::BevelPlanetMode::Single,
            GearBevelPlanetMode::Differential => solidcraft_gear::plan::BevelPlanetMode::Differential,
            GearBevelPlanetMode::Ring => solidcraft_gear::plan::BevelPlanetMode::Ring,
        };
    }
    set!(coupon, p.coupon);
    set!(coupon_teeth, p.coupon_teeth);
    set!(origin, p.origin);
    if let Some(n) = &p.name {
        s.name = n.clone();
    }
    if let Some(d) = p.ring_diameter {
        if !(d.is_finite() && d > 0.0) {
            return Err("`ring_diameter` must be greater than 0".into());
        }
        s.module = d / f64::from(s.teeth2.max(1));
    }
    s.bore = bore_of(&p.bore);
    s.bore2 = bore_of(&p.bore2);
    if let Some(f) = &p.placement {
        s.placement = Some(frame_of(f)?);
    }
    // Tooth tuning, chamfers, shafts and hub hardware (the gear crate checks the ranges).
    set!(tip_round, p.tip_round);
    set!(xy_compensation, p.xy_compensation);
    set!(elephant_foot, p.elephant_foot);
    set!(elephant_foot_height, p.elephant_foot_height);
    set!(nozzle, p.nozzle);
    set!(clearance, p.clearance);
    set!(tip_shortening, p.tip_shortening);
    set!(tip_diameter, p.tip_diameter);
    set!(tip_diameter2, p.tip_diameter2);
    set!(tip_relief, p.tip_relief);
    set!(tip_relief_height, p.tip_relief_height);
    set!(crowning, p.crowning);
    s.backlash2 = p.backlash2;
    set!(face_width2, p.face_width2);
    set!(face_offset2, p.face_offset2);
    s.tooth_chamfer_front = chamfer_of(&p.tooth_chamfer_front);
    s.tooth_chamfer_back = chamfer_of(&p.tooth_chamfer_back);
    set!(bore_chamfer, p.bore_chamfer);
    set!(hub_chamfer, p.hub_chamfer);
    s.shaft_front = shaft_of(&p.shaft_front);
    s.shaft_back = shaft_of(&p.shaft_back);
    s.shaft2_front = shaft_of(&p.shaft2_front);
    s.shaft2_back = shaft_of(&p.shaft2_back);
    s.hardware = hardware_of(&p.hardware)?;
    s.hardware2 = hardware_of(&p.hardware2)?;
    s.clearance_body = p.clearance_body.as_ref().map(clearance_of);
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

/// `s` as parameters with every value explicit.
fn explicit_params(s: &Spec) -> GearParams {
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
        offset: (s.offset > 0.0 || s.kind == Kind::Hypoid).then_some(s.offset),
        face_start: (s.face_start != 0.0).then_some(s.face_start),
        hand: (s.kind == Kind::Hypoid).then_some(if s.left_hand { SpiralHand::Left } else { SpiralHand::Right }),
        ring_diameter: None,
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
        herringbone: s.herringbone.then_some(true),
        first_azimuth: None,
        bevel_planet_mode: None,
        origin: (s.origin != [0.0; 3]).then_some(s.origin),
        name: (!s.name.is_empty()).then(|| s.name.clone()),
        placement: s.placement.map(|f| PlacementParams { origin: f.origin, x: f.x, z: f.z }),
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
        tip_round: (s.tip_round != 0.0).then_some(s.tip_round),
        xy_compensation: (s.xy_compensation != 0.0).then_some(s.xy_compensation),
        elephant_foot: (s.elephant_foot != 0.0).then_some(s.elephant_foot),
        elephant_foot_height: (s.elephant_foot_height != 0.0).then_some(s.elephant_foot_height),
        nozzle: (s.nozzle != 0.0).then_some(s.nozzle),
        clearance: (s.clearance != 0.0).then_some(s.clearance),
        tip_shortening: (s.tip_shortening != 0.0).then_some(s.tip_shortening),
        tip_diameter: (s.tip_diameter != 0.0).then_some(s.tip_diameter),
        tip_diameter2: (s.tip_diameter2 != 0.0).then_some(s.tip_diameter2),
        tip_relief: (s.tip_relief != 0.0).then_some(s.tip_relief),
        tip_relief_height: (s.tip_relief_height != 0.0).then_some(s.tip_relief_height),
        crowning: (s.crowning != 0.0).then_some(s.crowning),
        backlash2: s.backlash2,
        face_width2: (s.face_width2 != 0.0).then_some(s.face_width2),
        face_offset2: (s.face_offset2 != 0.0).then_some(s.face_offset2),
        tooth_chamfer_front: chamfer_params(&s.tooth_chamfer_front),
        tooth_chamfer_back: chamfer_params(&s.tooth_chamfer_back),
        bore_chamfer: (s.bore_chamfer != 0.0).then_some(s.bore_chamfer),
        hub_chamfer: (s.hub_chamfer != 0.0).then_some(s.hub_chamfer),
        shaft_front: shaft_params(&s.shaft_front),
        shaft_back: shaft_params(&s.shaft_back),
        shaft2_front: shaft_params(&s.shaft2_front),
        shaft2_back: shaft_params(&s.shaft2_back),
        hardware: hardware_params(&s.hardware),
        hardware2: hardware_params(&s.hardware2),
        coupon: s.coupon.then_some(true),
        coupon_teeth: (s.coupon && s.coupon_teeth != 4).then_some(s.coupon_teeth),
        clearance_body: s.clearance_body.as_ref().map(clearance_params),
        ..GearParams::default()
    };
    with_extras(s, p)
}

/// The parameters that describe `s` (the inverse of [`spec_of`]), for front ends.
pub fn params_of(s: &Spec) -> Value {
    let p = explicit_params(s);
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
        Kind::BevelPlanetary => {
            p.first_azimuth = Some(s.first_azimuth);
            p.bevel_planet_mode = Some(match s.bevel_planet_mode {
                solidcraft_gear::plan::BevelPlanetMode::Single => GearBevelPlanetMode::Single,
                solidcraft_gear::plan::BevelPlanetMode::Differential => GearBevelPlanetMode::Differential,
                solidcraft_gear::plan::BevelPlanetMode::Ring => GearBevelPlanetMode::Ring,
            });
        }
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
    let (o, d) = if let Some(point) = v.get("point").filter(|_| v.get("sketch").is_some()) {
        // A sketch point: the shaft through it, normal to its sketch.
        let id = super::features::sketch_id(s, v.get("sketch"), cmd, what)?;
        let point = point.as_str().unwrap_or_default();
        let st = s.world_state()?;
        let found = st.sketch(id).and_then(|ss| Some((ss.plane.to_world(ss.sketch.point(ss.sketch.point_index(point)?)?), ss.plane.normal())));
        found.ok_or_else(|| bad(cmd, format!("`{what}`: sketch {id} has no point `{point}`")))?
    } else if v.get("curve").is_some() {
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

/// A sketch's standalone points (not a curve's ends or centre), circles and lines, in its plane:
/// what a gear set drawn in one sketch is read from (`gear.create {sketch}` and the Gear
/// Generator window's picks). `frame` is the sketch's plane in the world (its z the normal), in
/// which the 2D coordinates are given.
#[derive(Clone, Debug, PartialEq)]
pub struct GearSketch {
    pub sketch: u64,
    pub frame: Frame,
    pub points: Vec<(String, solidcraft_gear::V2)>,
    pub circles: Vec<(String, sketch_rules::Circle)>,
    pub lines: Vec<(String, sketch_rules::Line)>,
    /// The last point is the sketch's own origin (not one the user drew).
    pub has_origin: bool,
}

impl GearSketch {
    pub fn drawn(&self) -> sketch_rules::Drawn {
        sketch_rules::Drawn {
            points: self.points.iter().map(|(_, p)| *p).collect(),
            circles: self.circles.iter().map(|(_, c)| *c).collect(),
            lines: self.lines.iter().map(|(_, l)| *l).collect(),
        }
    }
    /// What the user drew: [`Self::drawn`] without the sketch's own origin point (last), for
    /// recognising the set and picking its slots (the origin is not a dot the user placed; a
    /// bevel's apex away from it would otherwise be missed). Indices match [`Self::drawn`]'s.
    pub fn user_drawn(&self) -> sketch_rules::Drawn {
        let mut d = self.drawn();
        if self.has_origin {
            d.points.pop();
        }
        d
    }
    /// Where the entity `id` is among the slots' role (points, circles or lines).
    pub fn index(&self, role: sketch_rules::Role, id: &str) -> Option<usize> {
        match role {
            sketch_rules::Role::Point => self.points.iter().position(|(k, _)| k == id),
            sketch_rules::Role::Circle => self.circles.iter().position(|(k, _)| k == id),
            sketch_rules::Role::Line => self.lines.iter().position(|(k, _)| k == id),
        }
    }
}

/// Reads sketch `sketch`'s standalone points, circles and lines (see [`GearSketch`]).
pub fn sketch_entities(s: &Session, sketch: u64) -> std::result::Result<GearSketch, String> {
    let st = s.world_state().map_err(|e| e.to_string())?;
    let ss = st.sketch(sketch).ok_or_else(|| format!("no sketch {sketch}"))?;
    let o = ss.plane.to_world(Vec2::new(0.0, 0.0));
    let ex = ss.plane.to_world(Vec2::new(1.0, 0.0)) - o;
    let n = ss.plane.normal();
    let unit = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l > 1e-12 { v.map(|c| c / l) } else { [0.0, 0.0, 1.0] }
    };
    let z = unit([n.x, n.y, n.z]);
    let x = [ex.x, ex.y, ex.z];
    let along: f64 = (0..3).map(|i| x[i] * z[i]).sum();
    let x = unit([0, 1, 2].map(|i| x[i] - along * z[i]));
    let frame = Frame { origin: [o.x, o.y, o.z], x, z };
    let sk = &ss.sketch;
    let at = |i: usize| sk.point(i).map(|p| solidcraft_gear::V2::new(p.x, p.y));
    let mut used = vec![false; sk.points.len()];
    let (mut circles, mut lines) = (Vec::new(), Vec::new());
    for c in &sk.curves {
        match c.kind {
            crate::sketch::CurveKind::Line { a, b } => {
                if let (Some(pa), Some(pb)) = (at(a), at(b)) {
                    lines.push((c.id.clone(), sketch_rules::Line { a: pa, b: pb }));
                }
                for i in [a, b] {
                    if let Some(u) = used.get_mut(i) {
                        *u = true;
                    }
                }
            }
            crate::sketch::CurveKind::Circle { c: ci, r } => {
                if let Some(pc) = at(ci) {
                    circles.push((c.id.clone(), sketch_rules::Circle { c: pc, r }));
                }
                if let Some(u) = used.get_mut(ci) {
                    *u = true;
                }
            }
            crate::sketch::CurveKind::Arc { c: ci, a, b } => {
                for i in [ci, a, b] {
                    if let Some(u) = used.get_mut(i) {
                        *u = true;
                    }
                }
            }
            _ => {}
        }
    }
    // The sketch's own origin point (every sketch has one) goes last: a dot the user drew comes
    // first, and the origin is only ever picked by name (`picks`), never by auto picking.
    let mut points: Vec<(String, solidcraft_gear::V2)> = Vec::new();
    let mut origin = None;
    for (i, p) in sk.points.iter().enumerate().filter(|(i, _)| !used[*i]) {
        let entry = (p.id.clone(), solidcraft_gear::V2::new(p.pos.x, p.pos.y));
        if i == 0 && p.id == "origin" { origin = Some(entry) } else { points.push(entry) }
    }
    let has_origin = origin.is_some();
    points.extend(origin);
    Ok(GearSketch { sketch, frame, points, circles, lines, has_origin })
}

/// The bevel planetary mode asked for, as the gear crate names it.
fn base_mode(p: &GearParams) -> Option<solidcraft_gear::plan::BevelPlanetMode> {
    p.bevel_planet_mode.map(|m| match m {
        GearBevelPlanetMode::Single => solidcraft_gear::plan::BevelPlanetMode::Single,
        GearBevelPlanetMode::Differential => solidcraft_gear::plan::BevelPlanetMode::Differential,
        GearBevelPlanetMode::Ring => solidcraft_gear::plan::BevelPlanetMode::Ring,
    })
}

/// A set drawn in one sketch (`sketch`, with `type` the mode and `picks` the slots' entity
/// ids, or what the drawing looks like and its entities in order): measured by the sketch rules,
/// sized, and solved on the shafts they give.
fn from_sketch(s: &Session, cmd: &str, p: &GearParams) -> Result<Solved> {
    if p.axis1.is_some() || p.axis2.is_some() || p.placement.is_some() {
        return Err(bad(cmd, "give `sketch`, or `axis1` (`axis2`) or `placement`: not both"));
    }
    let id = super::features::sketch_id(s, p.sketch.as_ref(), cmd, "sketch")?;
    let sk = sketch_entities(s, id).map_err(|e| bad(cmd, e))?;
    let drawn = sk.drawn();
    let user = sk.user_drawn();
    let kind = match p.kind {
        Some(t) => KINDS.iter().find(|(x, _)| *x == t).map(|(_, k)| *k).unwrap_or(Kind::Spur),
        None => sketch_rules::recognize(&user).ok_or_else(|| {
            bad(cmd, "the sketch's circles and lines don't make a set: draw a pitch circle (and a second touching it, or a shaft line), two shaft lines meeting at a dot for bevels, or give `type`")
        })?,
    };
    if !sketch_rules::sketched(kind) {
        return Err(bad(cmd, format!("a {} is not drawn from a sketch: give its numbers instead", kind.label())));
    }
    let slots = sketch_rules::slots(kind);
    let picks = match &p.picks {
        None => sketch_rules::auto_picks(kind, &user),
        Some(ids) => {
            if ids.len() > slots.len() {
                return Err(bad(cmd, format!("`picks`: a {} takes {} ({}), not {}", kind.label(), slots.len(), slots.iter().map(|s| s.label).collect::<Vec<_>>().join(", "), ids.len())));
            }
            let mut out = vec![None; slots.len()];
            for (i, want) in ids.iter().enumerate() {
                let Some(want) = want.as_deref().filter(|w| !w.is_empty()) else { continue };
                let role = slots[i].role;
                out[i] = Some(sk.index(role, want).ok_or_else(|| {
                    let what = match role {
                        sketch_rules::Role::Point => "point",
                        sketch_rules::Role::Circle => "circle",
                        sketch_rules::Role::Line => "line",
                    };
                    bad(cmd, format!("`picks`: {} must be a {what} of sketch {id}: there is none `{want}` (picks come from the one sketch)", slots[i].label))
                })?);
            }
            out
        }
    };
    let m = sketch_rules::measure(kind, &drawn, &picks).map_err(|e| bad(cmd, e))?;
    let base = spec_of(&GearParams { kind: Some(gear_type(kind)), ..p.clone() }).map_err(|e| bad(cmd, e))?;
    // Sized as the window's Simple mode sizes it: what the drawing gives, and what the
    // parameters say besides (the shaft angle a single sketch can't show, a ratio).
    // Settings the drawing can't show are taken as given: face width, module, helix and
    // spiral, planets and starts (sizes the drawing shows win over teeth asked).
    let settings = solidcraft_gear::simple::Simple {
        shaft_angle: p.shaft_angle,
        ratio: p.ratio,
        face_width: p.face_width,
        face_width2: p.face_width2,
        module: p.module,
        helix: p.helix_angle,
        spiral_angle: p.spiral_angle,
        planets: p.planets,
        starts: p.starts,
        bevel_planet_mode: base_mode(p),
        ..Default::default()
    };
    let sized = solidcraft_gear::simple::solve(&sketch_rules::simple_of(&m, &settings), &base);
    if let Some(e) = sized.error {
        return Err(bad(cmd, e));
    }
    let (spec, notes) = (sized.spec, sized.notes);
    let turn = p.sketch_turn.unwrap_or([0.0; 3]);
    if turn.iter().any(|t| !t.is_finite()) {
        return Err(bad(cmd, "`sketch_turn` must be three finite angles"));
    }
    let (a1, a2) = sketch_rules::axes_for(&m, &spec, &sk.frame, turn);
    let mut solved = axes::solve(&a1, a2.as_ref(), Some(kind), &spec, &Drive::default()).map_err(|e| bad(cmd, e))?;
    // A rack lies on its drawn pitch line, not across the pinion's centre.
    if let Some(f) = sketch_rules::rack_frame(&m, &sk.frame, turn) {
        solved.frames = [f; 2];
        solved.spec.placement = Some(f);
    }
    solved.notes.extend(notes);
    Ok(solved)
}

fn ask(s: &Session, cmd: &str, p: &Value) -> Result<(Asked, Plan)> {
    let params = read(cmd, p)?;
    if params.sketch.is_some() {
        let solved = from_sketch(s, cmd, &params)?;
        let asked = Asked { spec: solved.spec.clone(), solved: Some(solved) };
        let plan = solidcraft_gear::plan(&asked.spec).map_err(|e| bad(cmd, e))?;
        return Ok((asked, plan));
    }
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
    let group_name = if spec.name.trim().is_empty() { kind_title(spec.kind).to_string() } else { spec.name.trim().to_string() };
    let mut request = if p.is_object() { p.clone() } else { json!({}) };
    let (mode, notes_eq) = equations::Mode::of(s, cmd, &request, spec, &plan, &group_name, true)?;
    warnings.extend(notes_eq);
    if let Some(prefix) = &mode.prefix {
        // Kept with the set, so an update names the same parameters.
        request["parameter_prefix"] = json!(prefix);
    }
    let bodies = build_set(s, &plan, &[], &mut warnings, &mode)?;
    let features: Vec<u64> = bodies.iter().flat_map(BodyRecord::features).collect();
    let group = if features.is_empty() {
        None
    } else {
        let r = s.execute("timeline.group", &json!({"name": group_name, "features": features}))?;
        r.get("group").and_then(Value::as_u64)
    };
    let made = bodies.iter().map(|b| b.name.clone()).collect();
    let clearance = clearance_name(&plan, &bodies);
    if let Some(group) = group {
        keep(s, &Record { version: RECORD_VERSION, group, name: group_name, request, params: params_of(spec), bodies }, &mut warnings);
    }
    reply(cmd, GearResult { bodies: made, group, clearance, facts: facts(&asked, &plan), warnings, notes: notes(&asked), ..GearResult::default() })
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
fn must_rebuild(s: &Session, rec: &Record, plan: &Plan, mode: &equations::Mode) -> Option<String> {
    let all = built_bodies(plan);
    if rec.bodies.len() != all.len() {
        let n = |k: usize| if k == 1 { "1 body".to_string() } else { format!("{k} bodies") };
        return Some(format!("the set had {} and now has {}", n(rec.bodies.len()), all.len()));
    }
    for (old, new) in rec.bodies.iter().zip(&all) {
        if old.steps.len() != new.steps.len() {
            return Some(format!("{} was built in {} steps and now takes {}", old.name, old.steps.len(), new.steps.len()));
        }
        for (i, (was, step)) in old.steps.iter().zip(&new.steps).enumerate() {
            let def = step_def(new, i, step, 0.0, mode);
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
    // Teeth from formulas (now or before): the sketch is drawn again from nothing.
    if def.eq.is_some() || equations::holds_formulas(s, id) {
        return equations::redraw(s, id, def);
    }
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
fn in_place(s: &mut Session, rec: &Record, plan: &Plan, names: &[String], mode: &equations::Mode) -> Result<Vec<BodyRecord>> {
    // As a new set is built: without the kernel's full check of every face, which made an
    // update in place three times slower than building the set again (a 2-start worm 55 s
    // against 17 s). It runs when the features are next computed.
    let checked = s.validation();
    s.set_validation(solidcraft_kernel::Validation::Off);
    let r = in_place_steps(s, rec, plan, names, mode);
    s.set_validation(checked);
    r
}

fn in_place_steps(s: &mut Session, rec: &Record, plan: &Plan, names: &[String], mode: &equations::Mode) -> Result<Vec<BodyRecord>> {
    let gone = || EngineError::Other("a step of the set is gone".into());
    let mut bodies = Vec::new();
    for ((old, body), name) in rec.bodies.iter().zip(&built_bodies(plan)).zip(names) {
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
                let def = step_def(body, i, step, *nudge, mode);
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
                let params = def_params(s, &def, body, step, &ids, name, name)?;
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
fn rebuild(s: &mut Session, rec: &Record, plan: &Plan, names: &[String], warnings: &mut Vec<String>, mode: &equations::Mode) -> Result<(Vec<BodyRecord>, Option<u64>)> {
    let old: Vec<u64> = rec.features().into_iter().filter(|f| s.doc.feature(*f).is_some()).collect();
    let first = old.iter().filter_map(|f| s.doc.feature_index(*f)).min();
    if !old.is_empty() {
        s.execute("timeline.delete", &json!({"features": old}))?;
    }
    let rest = s.doc.features.len();
    let bodies = build_set(s, plan, names, warnings, mode)?;
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
    let given_keys: Vec<String> = given.keys().cloned().collect();
    for (key, value) in given {
        if value.is_null() {
            request.remove(&key);
        } else {
            request.insert(key, value);
        }
    }
    // Parametric teeth: the same parameters as before, and what was changed through them.
    if !request.contains_key("parameter_prefix")
        && let Some(prefix) = rec.request.get("parameter_prefix").filter(|_| !given_keys.iter().any(|k| k == "parameter_prefix"))
    {
        request.insert("parameter_prefix".into(), prefix.clone());
    }
    equations::read_back(s, &mut request, &rec.params, &given_keys);
    let request = Value::Object(request);
    let (asked, plan) = ask(s, cmd, &request)?;
    let mut warnings = plan.warnings.clone();
    let (mode, notes_eq) = equations::Mode::of(s, cmd, &request, &asked.spec, &plan, &rec.name, false)?;
    warnings.extend(notes_eq);
    let own: std::collections::BTreeSet<u64> = rec.features().into_iter().collect();
    let worked: std::collections::BTreeSet<u64> = s.model.results.iter().filter(|r| r.error.is_none() && !r.skipped && !own.contains(&r.id)).map(|r| r.id).collect();
    let names = current_names(s, &rec);
    let before = s.doc.clone();
    let mut why = must_rebuild(s, &rec, &plan, &mode);
    if why.is_none() {
        match in_place(s, &rec, &plan, &names, &mode) {
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
        let keep_names: &[String] = if names.len() == built_bodies(&plan).len() { &names } else { &[] };
        let (bodies, folder) = rebuild(s, &rec, &plan, keep_names, &mut warnings, &mode)?;
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
    let clearance = clearance_name(&plan, &rec.bodies);
    reply(
        cmd,
        GearResult {
            bodies: rec.bodies.iter().map(|b| b.name.clone()).collect(),
            group,
            clearance,
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
    if def.eq.is_some() {
        return equations::new_sketch(s, def);
    }
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

/// Sections a spiral taper is lofted through.

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
    /// Teeth drawn from their formulas instead (`tooth_curves: equations`).
    eq: Option<equations::EqSketch>,
}

impl SketchDef {
    /// Tells this plane from another.
    fn plane_key(&self) -> String {
        digest(&format!("{:?}{:?}{:?}", self.origin, self.x, self.y))
    }
    /// Tells this outline from another.
    fn geometry_key(&self) -> String {
        match &self.eq {
            None => digest(&format!("{:?}", self.loops)),
            Some(eq) => digest(&format!("{:?}{:?}", self.loops, eq)),
        }
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
    /// Parametric teeth: the extrusion's distance and the sweep's twist as expressions of the
    /// design parameters, in place of the numbers.
    distance: Option<String>,
    twist: Option<String>,
    /// A mirrored half ([`mirrored_half`]): the mirror plane's point and normal (world).
    mirror: Option<([f64; 3], [f64; 3])>,
}

impl StepDef {
    fn new(sketches: Vec<SketchDef>, command: &'static str, shape: String, picks: bool) -> StepDef {
        StepDef { sketches, command, shape, picks, distance: None, twist: None, mirror: None }
    }
    /// A step's feature parameters with the expressions put in.
    fn with_exprs(&self, mut params: Value) -> Value {
        if let Some(d) = &self.distance
            && params.get("distance").is_some()
        {
            params["distance"] = json!(d);
        }
        if let Some(t) = &self.twist
            && params.get("twist").is_some()
        {
            params["twist"] = json!(t);
        }
        params
    }
}

/// A herringbone's second half, when step `i` of `body` is it: the first step's twisted prism
/// turned back the other way above it, which is that prism mirrored in the plane between them
/// (each point of the upper half at height `d` above it lies where the lower half's point `d`
/// below it does, turned the same). Mirrored and joined, it is the same solid for one sweep and
/// a mirror in place of two sweeps (a 20-tooth gear's halves 2.7 s and 5.4 s swept and joined,
/// 3.1 s and 2.8 s swept and mirrored, the volume the same to 1e-13). The plane's height, in the
/// body's frame. Teeth drawn from their formulas keep their two sweeps: their heights follow
/// the design's parameters, which the mirror's plane does not.
fn mirrored_half(body: &Body, i: usize, step: &Step, mode: &equations::Mode) -> Option<f64> {
    let (Some(Step::Prism { region: r0, z0: a, height: h, twist: t, op: Op::New }), Some(Step::Prism { region: r1, z0: b, height: h1, twist: t1, op: Op::Join })) =
        (body.steps.first(), body.steps.get(1))
    else {
        return None;
    };
    let turned = Region { outer: r0.outer.iter().map(|o| o.rotated(*t)).collect(), holes: r0.holes.iter().map(|o| o.rotated(*t)).collect() };
    let same = i == 1
        && matches!(step, Step::Prism { .. })
        && t.abs() > 1e-12
        && *t1 == -*t
        && h1 == h
        && (b - (a + h)).abs() <= 1e-9 * (1.0 + h.abs())
        && *r1 == turned
        && mode.sketch_for(r0).is_none()
        && mode.sketch_for(r1).is_none();
    same.then_some(*b)
}

/// The parameters of a step's feature: [`step_params`], or a mirrored half's own.
fn def_params(s: &Session, def: &StepDef, body: &Body, step: &Step, sketches: &[u64], target: &str, body_name: &str) -> Result<Value> {
    if let Some((origin, normal)) = def.mirror {
        return Ok(json!({"bodies": [target], "plane": {"origin": origin, "normal": normal}, "combine": true}));
    }
    Ok(def.with_exprs(step_params(s, body, step, sketches, target, body_name)?))
}

/// How far along its axis a trimming blank (a bevel's back and front cones) is tried: the
/// kernel sometimes refuses the intersection where the cones graze a root fillet, and takes it
/// a hair further along.
const NUDGES: [f64; 4] = [0.0, 0.0137, -0.0213, 0.0391];

/// The sketches and the command of step `i` of `body` (a revolved blank moved `nudge` along
/// the axis).
fn step_def(body: &Body, i: usize, step: &Step, nudge: f64, mode: &equations::Mode) -> StepDef {
    let f: &Frame = &body.frame;
    let (x, y, z) = (f.x, f.y(), f.z);
    let label = format!("{} {}", body.name, i + 1);
    let sketch = |name: String, origin: [f64; 3], x: [f64; 3], y: [f64; 3], loops: Vec<Vec<Geom2>>| SketchDef { name: format!("{label} {name}"), origin, x, y, loops, eq: None };
    // A sweep's path: a line up the axis, on the plane through it and x.
    let path = |z0: f64, height: f64| sketch("path".into(), f.world([0.0, 0.0, z0]), z, x, vec![vec![Geom2::Line(Vec2::new(0.0, 0.0), Vec2::new(height, 0.0))]]);
    let outline = |o: &Outline| {
        let mut g = Vec::new();
        geometry(o, &mut g);
        vec![g]
    };
    if let Some(z_mid) = mirrored_half(body, i, step, mode) {
        let mut def = StepDef::new(Vec::new(), "solid.mirror", "mirrored half (join)".into(), false);
        def.mirror = Some((f.world([0.0, 0.0, z_mid]), z));
        return def;
    }
    match step {
        // A placed step (a radial hole, say): its own frame, given in the body's.
        Step::Placed { frame, step } => {
            let inner = Body { name: body.name.clone(), frame: body.frame.compose(frame), steps: Vec::new() };
            let mut def = step_def(&inner, i, step, nudge, mode);
            def.shape = format!("placed {}", def.shape);
            def
        }
        Step::Prism { region, z0, height, twist, op } => {
            let mut profile = sketch("profile".into(), f.world([0.0, 0.0, *z0]), x, y, region_loops(region));
            let picks = !region.holes.is_empty();
            // Teeth from their formulas: the profile's toothed loops, and with parameters the
            // extrusion (or the sweep's path and twist) from them too.
            profile.eq = mode.sketch_for(region);
            let (distance, turn) = profile.eq.as_ref().map(|eq| mode.prism_exprs(eq, *height, *twist)).unwrap_or_default();
            let mut def = if twist.abs() < 1e-12 {
                StepDef::new(vec![profile], "solid.extrude", format!("prism ({})", op_name(*op)), picks)
            } else {
                let mut along = path(*z0, *height);
                if let (Some(d), Some(eq)) = (&distance, &profile.eq) {
                    along.eq = Some(equations::EqSketch { cells: Vec::new(), prefix: eq.prefix.clone(), path: Some(d.clone()) });
                }
                StepDef::new(vec![profile, along], "solid.sweep", format!("twisted prism ({})", op_name(*op)), picks)
            };
            def.distance = distance;
            def.twist = turn;
            def
        }
        // Straight: a loft between the two ends builds faster than a scaled sweep.
        Step::Taper { outline: o, z0, height, twist, scale, op } if twist.abs() <= 1e-12 => {
            let ends = [(o.clone(), *z0), (o.scaled(*scale), z0 + height)];
            let sketches = ends.iter().enumerate().map(|(k, (o, h))| sketch(format!("section {}", k + 1), f.world([0.0, 0.0, *h]), x, y, outline(o))).collect();
            StepDef::new(sketches, "solid.loft", format!("taper ({})", op_name(*op)), false)
        }
        // Spiral: the turn follows the logarithm of the scale (`taper_turn`), which a sweep's even
        // twist does not, so the taper is a loft through its own sections, each the first one
        // scaled and turned.
        Step::Taper { outline: o, z0, height, twist, scale, op } => {
            // As many sections as keep the kernel's alignment (each section lined up with the one
            // before), at least 7.
            let n = solidcraft_gear::plan::taper_sections(o, *twist, *scale);
            let sketches = (0..n)
                .map(|i| {
                    let t = i as f64 / (n - 1) as f64;
                    let k = 1.0 + (scale - 1.0) * t;
                    let section = o.scaled(k).rotated(solidcraft_gear::plan::taper_turn(*twist, *scale, k));
                    sketch(format!("section {}", i + 1), f.world([0.0, 0.0, z0 + height * t]), x, y, outline(&section))
                })
                .collect();
            StepDef::new(sketches, "solid.loft", format!("spiral taper ({})", op_name(*op)), false)
        }
        // An internal bevel's teeth and rim in one loft: every section the rim circle and the
        // toothed loop (scaled and turned as a taper's), the profile between them picked.
        Step::RingTaper { outline: o, rim, z0, height, twist, scale, op } => {
            let n = if twist.abs() <= 1e-12 { 2 } else { solidcraft_gear::plan::taper_sections(o, *twist, *scale) };
            let sketches = (0..n)
                .map(|i| {
                    let t = i as f64 / (n - 1) as f64;
                    let k = 1.0 + (scale - 1.0) * t;
                    let section = o.scaled(k).rotated(solidcraft_gear::plan::taper_turn(*twist, *scale, k));
                    let mut loops = vec![vec![Geom2::Circle(Vec2::new(0.0, 0.0), *rim)]];
                    loops.extend(outline(&section));
                    sketch(format!("section {}", i + 1), f.world([0.0, 0.0, z0 + height * t]), x, y, loops)
                })
                .collect();
            StepDef::new(sketches, "solid.loft", format!("ring taper ({})", op_name(*op)), true)
        }
        Step::Loft { sections, op } => {
            let sketches: Vec<SketchDef> = sections.iter().enumerate().map(|(k, (o, h))| sketch(format!("section {}", k + 1), f.world([0.0, 0.0, *h]), x, y, outline(o))).collect();
            let shape = format!("loft of {} ({})", sketches.len(), op_name(*op));
            StepDef::new(sketches, "solid.loft", shape, false)
        }
        Step::Revolve { profile, op } => {
            let moved: Vec<solidcraft_gear::V2> = profile.iter().map(|q| solidcraft_gear::V2::new(q.x, q.y + nudge)).collect();
            StepDef::new(vec![sketch("blank".into(), f.origin, x, z, vec![polygon(&moved)])], "solid.revolve", format!("revolve ({})", op_name(*op)), false)
        }
        Step::Coil { section, op, .. } => {
            StepDef::new(vec![sketch("thread".into(), f.origin, x, z, vec![polygon(section)])], "solid.coil", format!("coil ({})", op_name(*op)), false)
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
        Step::Placed { frame, step } => {
            let inner = Body { name: body.name.clone(), frame: body.frame.compose(frame), steps: Vec::new() };
            return step_params(s, &inner, step, sketches, target, body_name);
        }
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
        Step::Taper { op, .. } => {
            let ids = (0..sketches.len()).map(|k| id(k).map(|i| json!({"sketch": i}))).collect::<Result<Vec<Value>>>()?;
            merge(json!({"sections": ids}), own(*op))
        }
        Step::Loft { sections, op } => {
            let ids = (0..sections.len()).map(|k| id(k).map(|i| json!({"sketch": i}))).collect::<Result<Vec<Value>>>()?;
            merge(json!({"sections": ids}), own(*op))
        }
        // The profile between the rim and the teeth: picked by a point half way across the rim
        // at the large end (the teeth only shrink towards the small end).
        Step::RingTaper { outline, rim, op, .. } => {
            let r = 0.5 * (rim + outline.max_radius());
            let ids = (0..sketches.len()).map(|k| id(k).map(|i| json!({"sketch": i, "profiles": [{"point": [r, 0.0]}]}))).collect::<Result<Vec<Value>>>()?;
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
fn build_set(s: &mut Session, plan: &Plan, names: &[String], warnings: &mut Vec<String>, mode: &equations::Mode) -> Result<Vec<BodyRecord>> {
    // A gear's teeth are hundreds of faces: the kernel's full check of them takes about as long
    // as making them (a spiral bevel's twisted lofts 7 s of 13, a planetary set 3 s of 5, a
    // hypoid's minutes for the ring gear), so the set is built without it. The check still runs whenever the features are computed
    // again (an edit before them, or the design opened).
    let checked = s.validation();
    s.set_validation(solidcraft_kernel::Validation::Off);
    let mut made = Vec::new();
    let mut built = Ok(());
    let gears = plan.bodies.len();
    // New bodies take names no body has yet (two z20 gears, or a second set like the first,
    // would otherwise fall back to "Body1"): "Gear z20 (2)".
    let mut taken: std::collections::BTreeSet<String> = s.model.state().bodies.iter().map(|b| b.name.clone()).collect();
    for (i, body) in built_bodies(plan).iter().enumerate() {
        let name = match names.get(i) {
            Some(n) => n.clone(),
            None => {
                let mut n = body.name.clone();
                let mut k = 2;
                while taken.contains(&n) {
                    n = format!("{} ({k})", body.name);
                    k += 1;
                }
                n
            }
        };
        taken.insert(name.clone());
        let named = Body { name, ..body.clone() };
        match build_body(s, &named, warnings, mode) {
            Ok(b) => made.push(b),
            // The clearance body is worth having, but not at the price of the gears.
            Err(e) if i >= gears => warnings.push(format!("{}: the clearance body could not be made ({e}); the gears are made without it", named.name)),
            Err(e) => {
                built = Err(EngineError::Other(format!("{}: {e}", named.name)));
                break;
            }
        }
    }
    s.set_validation(checked);
    // Lofts, sweeps and coils leave their sketches showing (an extrude hides its own): a worm
    // wheel's sections would draw over its teeth. The set's sketches are hidden; they stay in
    // the browser to show again.
    let sketches: Vec<Value> = made.iter().flat_map(|b: &BodyRecord| b.steps.iter()).flat_map(|st| st.sketches.iter()).map(|k| json!(k.id)).collect();
    if built.is_ok() && !sketches.is_empty() {
        s.execute("browser.visibility", &json!({"visible": false, "items": sketches})).ok();
    }
    built.map(|()| made)
}

/// Builds one body; returns what it was made of, under the name the design gave it.
fn build_body(s: &mut Session, body: &Body, warnings: &mut Vec<String>, mode: &equations::Mode) -> Result<BodyRecord> {
    let mut name = body.name.clone();
    let mut steps = Vec::new();
    for (i, step) in body.steps.iter().enumerate() {
        // A blank that trims (a bevel's back and front cones) is tried again a hair further
        // along when the kernel refuses it. A loft the kernel refuses is tried again through
        // every other section, then through its two ends: a coarser twist beats no gear. Any
        // step that changes a body already made (a hub, web, bore or trim) is left out with a
        // warning if it still fails: the gear is still worth having.
        let trims = matches!(step, Step::Revolve { op, .. } if *op != Op::New);
        let optional = !matches!(step_op(step), Op::New);
        let mut attempts: Vec<(Step, f64)> = if trims { NUDGES.iter().map(|n| (step.clone(), *n)).collect() } else { vec![(step.clone(), 0.0)] };
        // (Not a loft of many sections, as a hypoid's: they are as many as the kernel needs to
        // line them up tooth to tooth, and through fewer it would pair the wrong segments and cut
        // across the flanks.)
        if let Step::Loft { sections, op } = step
            && sections.len() <= 12
        {
            let n = sections.len();
            if n > 3 {
                let mut every_other: Vec<usize> = (0..n).step_by(2).collect();
                if every_other.last() != Some(&(n - 1)) {
                    every_other.push(n - 1);
                }
                attempts.push((Step::Loft { sections: every_other.iter().map(|k| sections[*k].clone()).collect(), op: *op }, 0.0));
            }
            if n > 2 {
                attempts.push((Step::Loft { sections: vec![sections[0].clone(), sections[n - 1].clone()], op: *op }, 0.0));
            }
        }
        let mut done = None;
        for (k, (step, nudge)) in attempts.iter().enumerate() {
            let def = step_def(body, i, step, *nudge, mode);
            let mut ids = Vec::new();
            for sketch in &def.sketches {
                ids.push(new_sketch(s, sketch)?);
            }
            if def.picks {
                // Picks are checked against the sketch's profiles, which a command run inside
                // another has not worked out yet.
                s.refresh();
            }
            let params = def_params(s, &def, body, step, &ids, &name, &body.name)?;
            match s.execute(def.command, &params) {
                Ok(reply) => {
                    if k > 0 && let (Step::Loft { sections: all, .. }, Step::Loft { sections: used, .. }) = (&body.steps[i], step) {
                        warnings.push(format!("{}: the loft through all {} sections failed; built through {} of them", body.name, all.len(), used.len()));
                    }
                    done = Some((def, ids, params, reply));
                    break;
                }
                Err(e) => {
                    s.execute("timeline.delete", &json!({"features": ids})).ok();
                    if k + 1 < attempts.len() {
                        continue;
                    }
                    if trims {
                        warnings.push(format!("{}: the back and front cones could not be trimmed ({e}); the teeth ends are left flat", body.name));
                    } else if optional {
                        warnings.push(format!("{}: step {} ({}) could not be made ({e}); it is left out", body.name, i + 1, op_name(step_op(step))));
                    } else {
                        return Err(e);
                    }
                }
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
        if def.sketches.iter().any(|d| d.eq.is_some()) {
            keep_point_picks(s, feature_id(&reply), &params);
        }
        let sketches = def.sketches.iter().zip(&ids).map(|(d, id)| SketchRecord { id: *id, plane: d.plane_key(), geometry: d.geometry_key() }).collect();
        steps.push(StepRecord { shape: def.shape, command: def.command.into(), feature: feature_id(&reply), params, sketches });
    }
    Ok(BodyRecord { name, steps })
}

/// Teeth drawn from formulas change their curves (and the curves' ids) when a parameter
/// changes the tooth count, so a profile the feature stored by its boundary's curve ids would be
/// lost. The gear's picks are points that stay inside the same profile for any tooth count (just
/// inside the rim or a tooth tip), so the feature keeps them as points.
fn keep_point_picks(s: &mut Session, feature: Option<u64>, params: &Value) {
    let Some(id) = feature else { return };
    let Some(picks) = params.get("profiles").and_then(Value::as_array) else { return };
    let points: Option<Vec<Vec2>> = picks
        .iter()
        .map(|p| {
            let q = p.get("point")?.as_array()?;
            Some(Vec2::new(q.first()?.as_f64()?, q.get(1)?.as_f64()?))
        })
        .collect();
    let Some(points) = points.filter(|p| !p.is_empty()) else { return };
    if let Some(f) = s.doc_mut().feature_mut(id) {
        for (_, sel) in f.kind.profile_sels_mut() {
            *sel = solidcraft_doc::ProfileSel::Points { points: points.clone() };
        }
    }
}

/// What a step does to its body.
fn step_op(step: &Step) -> Op {
    match step {
        Step::Prism { op, .. } | Step::Taper { op, .. } | Step::RingTaper { op, .. } | Step::Loft { op, .. } | Step::Revolve { op, .. } | Step::Coil { op, .. } => *op,
        Step::Placed { step, .. } => step_op(step),
    }
}

fn polygon(pts: &[solidcraft_gear::V2]) -> Vec<Geom2> {
    (0..pts.len()).map(|i| Geom2::Line(v2(pts[i]), v2(pts[(i + 1) % pts.len()]))).collect()
}

/// The first curve's id in a sketch.
fn first_curve(s: &Session, sketch: u64) -> Result<String> {
    let sk = super::sketch_store::read(&s.doc, sketch)?;
    sk.curves.first().map(|c| c.id.clone()).ok_or_else(|| EngineError::Other("the path sketch is empty".into()))
}

#[path = "gear_equations.rs"]
mod equations;

#[cfg(test)]
#[path = "gear_tests.rs"]
mod tests;
