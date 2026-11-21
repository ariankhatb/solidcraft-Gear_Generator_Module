//! `gear.create` and `gear.calculate`: the Gear Generator's parameters and replies.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use super::types::AxisLine;

/// A shaft axis for the axis-first workflow, as `cmd::gear` reads it: any [`AxisLine`] (X, Y, Z,
/// a construction axis's name, `{origin?, dir}`, or an edge or round face of a body), or a
/// straight line of a sketch (`{sketch, curve}`). Kept as given, for the handler, which has the
/// design.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct GearAxis(pub Value);

impl<'de> Deserialize<'de> for GearAxis {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        if v.is_object() && v.get("curve").is_some() {
            return match (v.get("curve").and_then(Value::as_str), v.get("sketch")) {
                (Some(_), Some(Value::String(_) | Value::Number(_))) => Ok(GearAxis(v)),
                _ => Err(D::Error::custom("a sketch line axis is {\"sketch\": id or name, \"curve\": the line's id}")),
            };
        }
        if v.is_object() && v.get("point").is_some() && v.get("sketch").is_some() {
            return match (v.get("point").and_then(Value::as_str), v.get("sketch")) {
                (Some(_), Some(Value::String(_) | Value::Number(_))) => Ok(GearAxis(v)),
                _ => Err(D::Error::custom("a sketch point axis is {\"sketch\": id or name, \"point\": the point's id}")),
            };
        }
        AxisLine::deserialize(v).map(|a| GearAxis(a.0)).map_err(D::Error::custom)
    }
}

impl JsonSchema for GearAxis {
    fn schema_name() -> Cow<'static, str> {
        "GearAxis".into()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = AxisLine::json_schema(generator);
        if let Some(o) = schema.as_object_mut() {
            o.insert(
                "description".into(),
                json!("A shaft axis: X, Y or Z, a construction axis's name, {\"origin\"?: [x, y, z] in mm, \"dir\": [x, y, z]}, a straight or circular edge or a cylinder, cone or torus face of a body ({\"edge\": [x, y, z]} or {\"face\": [x, y, z]}, a point on it), a straight sketch line ({\"sketch\": id or name, \"curve\": id}), or a sketch point, the shaft through it normal to its sketch ({\"sketch\": id or name, \"point\": id}). Read once: the gears do not follow the axis afterwards."),
            );
            if let Some(Value::Array(forms)) = o.get_mut("anyOf") {
                forms.push(json!({"type": "object", "properties": {"sketch": {"type": ["string", "integer"]}, "curve": {"type": "string"}}, "required": ["sketch", "curve"]}));
                forms.push(json!({"type": "object", "properties": {"sketch": {"type": ["string", "integer"]}, "point": {"type": "string"}}, "required": ["sketch", "point"]}));
            }
        }
        schema
    }
}

/// Where a whole gear set goes: a frame in world coordinates. Gear 1 turns about `z` through
/// `origin`; the set's own x axis (towards gear 2 for a pair) is `x`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct PlacementParams {
    /// A point on gear 1's axis, in mm.
    pub origin: [f64; 3],
    /// The set's x direction (made square to `z`).
    pub x: [f64; 3],
    /// Gear 1's axis direction.
    pub z: [f64; 3],
}

/// The gear set to make.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GearType {
    Spur,
    Helical,
    Herringbone,
    Internal,
    Rack,
    Pair,
    Planetary,
    Bevel,
    SpiralBevel,
    Hypoid,
    CrossedHelical,
    Worm,
    Cycloidal,
    CycloidalDrive,
    Sprocket,
    Pulley,
    Spline,
    FaceGear,
    Elliptical,
    BevelPlanetary,
    /// A complete differential: side gears, pinions or element gears, the case and its drive
    /// gear (settings in `differential`, or designed from `differential_goal`).
    Differential,
}

/// What meshes a bevel planetary set's planets besides the sun.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GearBevelPlanetMode {
    /// The sun and the planets only.
    Single,
    /// A second sun (side gear) opposite the first: a differential.
    Differential,
    /// An internal bevel ring round the planets: a bevel epicyclic.
    Ring,
}

/// A roller chain size: ISO 606 B series (European) or ANSI B29.1 (ISO 606 A series), or
/// `custom` with the sizes given.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChainSize {
    #[serde(rename = "04b")]
    B04,
    #[serde(rename = "05b")]
    B05,
    #[serde(rename = "06b")]
    B06,
    #[serde(rename = "08b")]
    B08,
    #[serde(rename = "10b")]
    B10,
    #[serde(rename = "12b")]
    B12,
    #[serde(rename = "16b")]
    B16,
    #[serde(rename = "20b")]
    B20,
    #[serde(rename = "24b")]
    B24,
    Ansi25,
    Ansi35,
    Ansi40,
    Ansi50,
    Ansi60,
    Ansi80,
    Ansi100,
    Ansi120,
    Custom,
}

/// A timing belt profile.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub enum BeltProfile {
    #[serde(rename = "gt2_2")]
    Gt2_2,
    #[serde(rename = "gt2_3")]
    Gt2_3,
    #[serde(rename = "gt2_5")]
    Gt2_5,
    #[serde(rename = "htd_3m")]
    Htd3m,
    #[serde(rename = "htd_5m")]
    Htd5m,
    #[serde(rename = "htd_8m")]
    Htd8m,
    #[serde(rename = "t2_5")]
    T2_5,
    #[serde(rename = "t5")]
    T5,
    #[serde(rename = "t10")]
    T10,
    #[serde(rename = "mxl")]
    Mxl,
    #[serde(rename = "xl")]
    Xl,
}

/// The involute spline standard.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplineStandardParam {
    /// DIN 5480: 30°, sized by the reference diameter (`spline_diameter`).
    Din5480,
    /// ANSI B92.1 flat root side fit: module = 25.4 / diametral pitch, no shift.
    AnsiB92,
}

/// The hand of a hypoid pinion's spiral (the ring gear gets the other).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpiralHand {
    Left,
    Right,
}

/// The basic rack profile.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RackStandard {
    Iso53A,
    Iso53B,
    Iso53C,
    Iso53D,
    AgmaFullDepth,
    Stub,
    /// For FDM 3D printing: dedendum 1.3, full root fillet.
    PrintFdm,
}

/// How gear teeth are drawn in their sketches.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToothCurves {
    /// Fit point splines through points computed from the tooth geometry.
    #[default]
    Fitted,
    /// Equation curves of the gear formulas, tip and root arcs, and a pattern.
    Equations,
}

/// How a gear is bored.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoreType {
    None,
    Round,
    Keyway,
    DFlat,
    Hex,
    Square,
}

/// A differential's style.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffStyle {
    /// Two equal bevel side gears and spider pinions on a cross shaft.
    Open,
    /// Two bevel side gears of different tooth counts on tilted pinions: a fixed torque split.
    Biased,
    /// Torsen Type B (T-2): helical side gears and paired helical element gears in case pockets.
    TorsenB,
    /// Torsen Type A (T-1, worm): worm side gears, crossed helical element wheels with spur ends.
    TorsenA,
}

/// The gear that drives a differential's case.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffDrive {
    None,
    SpiralBevel,
    Bevel,
    Hypoid,
    /// Helical ring and pinion on parallel shafts (transaxle).
    Helical,
}

/// How a differential's ring gear is held on its case.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffMount {
    Bolted,
    Integral,
}

/// A differential's settings (`type: differential`); every one optional, defaults from the
/// style. Lengths in mm, angles in degrees; 0 means "work it out" where it says so.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DifferentialParams {
    /// The style (default open).
    #[serde(default)]
    pub style: Option<DiffStyle>,
    /// Module of the side gears and pinions (bevel: large end; Torsen: normal).
    #[serde(default)]
    pub module: Option<f64>,
    /// Side gear A's teeth.
    #[serde(default)]
    pub side_teeth: Option<u32>,
    /// Side gear B's teeth (0: as A; the biased style needs a different count).
    #[serde(default)]
    pub side_teeth2: Option<u32>,
    /// Spider pinions' or element gears' teeth.
    #[serde(default)]
    pub pinion_teeth: Option<u32>,
    /// Spider pinions (bevel) or element pairs (Torsen).
    #[serde(default)]
    pub pinions: Option<u32>,
    /// Side gears' face width (0: from the size).
    #[serde(default)]
    pub face_width: Option<f64>,
    /// Spiral angle of the bevel side gears and pinions (0: straight).
    #[serde(default)]
    pub spiral_angle: Option<f64>,
    /// Torsen helix angle.
    #[serde(default)]
    pub helix_angle: Option<f64>,
    /// Pressure angle of the differential gears (0: 20°).
    #[serde(default)]
    pub pressure_angle: Option<f64>,
    #[serde(default)]
    pub backlash: Option<f64>,
    /// Friction coefficient the Torsen torque bias estimate assumes (default 0.1).
    #[serde(default)]
    pub friction: Option<f64>,
    /// Axle shaft diameter.
    #[serde(default)]
    pub axle_diameter: Option<f64>,
    /// Pinion pin diameter (0: from the pinions).
    #[serde(default)]
    pub pin_diameter: Option<f64>,
    /// Case wall (0: from the size).
    #[serde(default)]
    pub wall: Option<f64>,
    /// Thrust washer allowance behind the side gears (0: from the size).
    #[serde(default)]
    pub washer: Option<f64>,
    /// Running clearance round the gears inside the case (default 0.5).
    #[serde(default)]
    pub clearance: Option<f64>,
    /// Case bearings by name ("30207", "6006"...; "" or absent: the smallest that fits).
    #[serde(default)]
    pub bearing: Option<String>,
    /// Assembly windows in the case between the pinions (default false: plain stock).
    #[serde(default)]
    pub windows: Option<bool>,
    /// The drive gear (default spiral_bevel).
    #[serde(default)]
    pub drive: Option<DiffDrive>,
    /// How the ring gear is held (default bolted).
    #[serde(default)]
    pub mount: Option<DiffMount>,
    #[serde(default)]
    pub ring_teeth: Option<u32>,
    #[serde(default)]
    pub drive_pinion_teeth: Option<u32>,
    /// Ring gear module (bevel: large end; hypoid: ring pitch diameter / teeth; helical: normal).
    #[serde(default)]
    pub drive_module: Option<f64>,
    /// Ring face width (0: from the size).
    #[serde(default)]
    pub drive_face_width: Option<f64>,
    /// Drive pinion's mean spiral angle (spiral bevel, hypoid).
    #[serde(default)]
    pub drive_spiral_angle: Option<f64>,
    /// Helical drive's helix angle.
    #[serde(default)]
    pub drive_helix_angle: Option<f64>,
    /// Hypoid offset (0: 12 % of the ring's pitch diameter).
    #[serde(default)]
    pub drive_offset: Option<f64>,
    /// Along the axle from the set's middle: where the drive pinion's axis crosses it (bevel,
    /// hypoid) or the ring's middle plane (helical).
    #[serde(default)]
    pub drive_position: Option<f64>,
    /// A flange with bolt holes on the case and tapped holes in the ring (default false: a plain
    /// ring on the case).
    #[serde(default)]
    pub ring_bolts: Option<bool>,
    /// Ring bolts (0: from the bolt circle) and their size (0: the largest that fits).
    #[serde(default)]
    pub bolts: Option<u32>,
    #[serde(default)]
    pub bolt_diameter: Option<f64>,
    /// Also make the keep-out body for the axle housing.
    #[serde(default)]
    pub keep_out: Option<bool>,
    /// Which bodies are built (every one by default); the differential is designed whole
    /// either way.
    #[serde(default)]
    pub parts: Option<DiffPartsParams>,
}

/// A differential's bodies to build, each on (default) or off.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DiffPartsParams {
    #[serde(default)]
    pub side_a: Option<bool>,
    #[serde(default)]
    pub side_b: Option<bool>,
    /// The spider pinions or element gears.
    #[serde(default)]
    pub pinions: Option<bool>,
    /// Which of them, 1-based in plan order (Torsen elements 1a, 1b, 2a… are 1, 2, 3…; empty:
    /// all).
    #[serde(default)]
    pub which_pinions: Option<Vec<u32>>,
    /// The pinion pins (cross shaft).
    #[serde(default)]
    pub pins: Option<bool>,
    #[serde(default)]
    pub case: Option<bool>,
    #[serde(default)]
    pub ring: Option<bool>,
    #[serde(default)]
    pub drive_pinion: Option<bool>,
}

/// A differential designed by what it must do (Simple mode): the solver picks teeth, modules
/// and angles; `differential` settings given too are laid over its answer.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DifferentialGoalParams {
    #[serde(default)]
    pub style: Option<DiffStyle>,
    #[serde(default)]
    pub drive: Option<DiffDrive>,
    #[serde(default)]
    pub mount: Option<DiffMount>,
    /// Axle shaft diameter in mm (default 16).
    #[serde(default)]
    pub axle_diameter: Option<f64>,
    /// Ring gear pitch diameter in mm (default: the smallest that fits and carries the torque).
    #[serde(default)]
    pub ring_diameter: Option<f64>,
    /// Final drive ratio, ring ÷ pinion (default 3.9).
    #[serde(default)]
    pub ratio: Option<f64>,
    /// Input torque at the drive pinion in N·m (default: not rated).
    #[serde(default)]
    pub torque: Option<f64>,
    /// Spider pinions or element pairs (default from the style).
    #[serde(default)]
    pub pinions: Option<u32>,
    /// Wanted torque split T_B / T_A (biased) or torque bias ratio (Torsen).
    #[serde(default)]
    pub bias: Option<f64>,
}

/// A gear material for the strength rating (representative allowable stresses).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GearMaterial {
    SteelNormalized,
    SteelThroughHardened,
    SteelCaseHardened,
    SteelNitrided,
    CastIron,
    Bronze,
    Pom,
    Pa66,
    Pla,
    Petg,
}

/// A bore through a gear's axis.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BoreParams {
    /// The bore's shape (default round).
    #[serde(default, rename = "type")]
    pub kind: Option<BoreType>,
    /// Diameter in mm (hex and square: across flats).
    pub diameter: f64,
    /// Keyway width in mm (default: DIN 6885 for the diameter).
    #[serde(default)]
    pub key_width: Option<f64>,
    /// Keyway depth into the hub in mm (default: DIN 6885).
    #[serde(default)]
    pub key_depth: Option<f64>,
    /// D flat: from the flat across to the far side of the bore, in mm (default 0.9 × diameter).
    #[serde(default)]
    pub flat: Option<f64>,
}

/// What the free end of an integral shaft carries.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShaftEndType {
    Plain,
    DFlat,
    Keyway,
    Hex,
    /// A "+" for hobby servo horns and toy axles.
    Cross,
    CentreHole,
}

/// A shaft made in one piece with a gear, on one side of it. Lengths are from the gear's face.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ShaftParams {
    /// Diameter in mm of the length next to the gear.
    pub diameter: f64,
    /// That length in mm.
    pub length: f64,
    /// A second length beyond the first (a shoulder or a collar): its diameter in mm.
    #[serde(default)]
    pub step_diameter: Option<f64>,
    /// The second length in mm.
    #[serde(default)]
    pub step_length: Option<f64>,
    /// What the free end carries (default plain).
    #[serde(default)]
    pub end: Option<ShaftEndType>,
    /// How far the end feature runs back from the free end in mm (default 1.5 × the end's
    /// diameter; a centre hole: its depth, default 1.5 × its diameter).
    #[serde(default)]
    pub end_length: Option<f64>,
    /// The end feature's size in mm: d_flat from the flat to the far side (default 0.9 × the
    /// diameter), keyway width (default DIN 6885), hex across flats (default 0.85 × the
    /// diameter), cross arm width (default a third of the diameter), centre hole diameter
    /// (default 0.3 × the diameter).
    #[serde(default)]
    pub end_size: Option<f64>,
    /// A 45° chamfer on the free end, in mm.
    #[serde(default)]
    pub chamfer: Option<f64>,
}

/// The Gears feature's tooth chamfer, one for every gear of the set.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ToothChamferParams {
    /// Width across the face from the tip circle, and depth along the axis (45°), in mm.
    pub size: f64,
    /// "both" (default), "a" or "b": the end faces it goes on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<String>,
}

/// An edge chamfer on the teeth.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ChamferParams {
    /// Width across the face (radially, from the tip circle) in mm.
    pub size: f64,
    /// Angle to the face in degrees, 5 to 85 (default 45): the chamfer runs size × tan(angle)
    /// along the axis.
    #[serde(default)]
    pub angle: Option<f64>,
}

/// Hardware for fixing a gear. Set screws, nut traps, insert pockets and the clamp slit go in
/// the hub (set `hub_diameter` and `hub_length`); the bearing pocket goes in the back face.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct HardwareParams {
    /// Set-screw hole diameter in mm: radial, through the hub wall at mid length, over the key
    /// or flat.
    #[serde(default)]
    pub set_screw: Option<f64>,
    /// How many set screws, 1 to 4, 90° apart (default 1).
    #[serde(default)]
    pub set_screws: Option<u32>,
    /// Captive nut trap on each set screw: the nut's width across flats in mm.
    #[serde(default)]
    pub nut_width: Option<f64>,
    /// The nut's thickness in mm (default 0.45 × its width).
    #[serde(default)]
    pub nut_thickness: Option<f64>,
    /// Heat-set insert pocket at the mouth of each set-screw hole: diameter in mm.
    #[serde(default)]
    pub insert_diameter: Option<f64>,
    /// The insert pocket's depth in mm (default: most of the hub wall).
    #[serde(default)]
    pub insert_depth: Option<f64>,
    /// Clamping hub: the width in mm of a slit through the hub wall.
    #[serde(default)]
    pub clamp_slit: Option<f64>,
    /// Clamping hub: diameter in mm of the clamp screw's hole across the slit.
    #[serde(default)]
    pub clamp_screw: Option<f64>,
    /// Bearing pocket in the back face, by a standard bearing's name: MR63, MR84, MR105, MR115,
    /// MR126, MR128, 623, 624, 625, 626, 608, 688, 6800..6804, 6000..6004, 6200..6202.
    #[serde(default)]
    pub bearing: Option<String>,
    /// Bearing pocket: the bearing's outside diameter in mm (instead of `bearing`).
    #[serde(default)]
    pub bearing_od: Option<f64>,
    /// Bearing pocket: the bearing's width in mm.
    #[serde(default)]
    pub bearing_width: Option<f64>,
    /// How much larger than the bearing the pocket is made, on the diameter, in mm.
    #[serde(default)]
    pub bearing_fit: Option<f64>,
}

/// A bearing size for the clearance body's seats, in mm.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BearingSizeParams {
    /// Outside diameter.
    pub od: f64,
    /// Width.
    pub width: f64,
    /// Bore (optional: checked against the shaft when given).
    #[serde(default)]
    pub bore: Option<f64>,
}

/// The clearance body's bearing: a standard deep-groove ball bearing by name (MR52..MR148,
/// 603..629, 683..689, 693..699, 6800..6806, 6900..6905, 6000..6006, 6200..6206, 6300..6302;
/// e.g. "608"), or a size.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ClearanceBearing {
    Named(String),
    Size(BearingSizeParams),
}

/// A bearing seat on a member's shaft, by the member and the distance along its axis.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SeatAlongParams {
    /// The member: 1, 2, ... (the set's bodies in their order: gear 1 first; planetary: sun,
    /// planets, ring; worm: wheel, worm).
    pub member: u32,
    /// Signed distance in mm along the member's axis from the middle of its teeth.
    pub along: f64,
}

/// A bearing seat at a world point, moved onto the nearest member's axis.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SeatPointParams {
    /// [x, y, z] in mm.
    pub point: [f64; 3],
}

/// Where a bearing seat goes: a dot on a shaft.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ClearanceSeat {
    Along(SeatAlongParams),
    Point(SeatPointParams),
}

/// The clearance body: one more solid, the space the assembled set needs to turn (every
/// member's swept teeth, shafts through the housing walls, bearing seats), grown by the
/// clearances, to cut out of a housing block in one Boolean.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ClearanceParams {
    /// Radial clearance round tips, shafts and chain or belt runs, mm (default 0.5).
    #[serde(default)]
    pub radial: Option<f64>,
    /// Axial clearance past each member's faces, mm (default 0.5).
    #[serde(default)]
    pub axial: Option<f64>,
    /// How far shafts run on past each gear on each side, mm (default 2 × face width).
    /// Integral shafts (`shaft_*`) are taken as built.
    #[serde(default)]
    pub shaft_extension: Option<f64>,
    /// An internal ring (or a cycloidal drive's pin ring) is part of the housing and left out
    /// (default true); false clears its outer rim.
    #[serde(default)]
    pub ring_as_housing: Option<bool>,
    /// The bearing for the seats: a standard name ("608") or {od, width, bore?}.
    #[serde(default)]
    pub bearing: Option<ClearanceBearing>,
    /// How much larger than the bearing's outside diameter each seat is, on the diameter, mm
    /// (default 0).
    #[serde(default)]
    pub bearing_fit: Option<f64>,
    /// The seats: {member, along} or {point: [x, y, z]}.
    #[serde(default)]
    pub seats: Option<Vec<ClearanceSeat>>,
    /// Rack: how far it slides in all, mm (default its own length): its slot is that much
    /// longer.
    #[serde(default)]
    pub travel: Option<f64>,
    /// Make the keep-out body itself (default true); false places the seats only, for
    /// `bearing_bodies`.
    #[serde(default)]
    pub keep_out: Option<bool>,
    /// Also build each seat's bearing as two bodies, its outer and inner ring (plain sections, no
    /// balls); with `assembly` the outer ring is held and the inner ring rigid on its gear's
    /// member. Default false (with `selection` bearings: what `assembly` is).
    #[serde(default)]
    pub bearing_bodies: Option<bool>,
}

/// What a selected thing is for. A gear role on a circle makes that gear (its tooth style or
/// family); on a shaft line or axis, a gear on it (sized by `pitch_diameter`, `module`,
/// `teeth`). `shaft` is a bare shaft (a line other circles lie on, or a gear on it with the
/// set's style); `bearing` a bearing seat (a point on a shaft line, or `along` a shaft).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionRole {
    Spur,
    Helical,
    Herringbone,
    Bevel,
    SpiralBevel,
    Worm,
    Wheel,
    Rack,
    Internal,
    Face,
    Bearing,
    Shaft,
    /// Worked out from the geometry (the default).
    Auto,
}

impl SelectionRole {
    /// Every role, in the order a picker lists them.
    pub const ALL: [SelectionRole; 13] = [
        SelectionRole::Auto,
        SelectionRole::Spur,
        SelectionRole::Helical,
        SelectionRole::Herringbone,
        SelectionRole::Bevel,
        SelectionRole::SpiralBevel,
        SelectionRole::Worm,
        SelectionRole::Wheel,
        SelectionRole::Rack,
        SelectionRole::Internal,
        SelectionRole::Face,
        SelectionRole::Bearing,
        SelectionRole::Shaft,
    ];
    /// Its name in the parameters.
    pub fn name(self) -> &'static str {
        match self {
            SelectionRole::Spur => "spur",
            SelectionRole::Helical => "helical",
            SelectionRole::Herringbone => "herringbone",
            SelectionRole::Bevel => "bevel",
            SelectionRole::SpiralBevel => "spiral_bevel",
            SelectionRole::Worm => "worm",
            SelectionRole::Wheel => "wheel",
            SelectionRole::Rack => "rack",
            SelectionRole::Internal => "internal",
            SelectionRole::Face => "face",
            SelectionRole::Bearing => "bearing",
            SelectionRole::Shaft => "shaft",
            SelectionRole::Auto => "auto",
        }
    }
    /// A gear (not a bare shaft or a bearing).
    pub fn is_gear(self) -> bool {
        !matches!(self, SelectionRole::Bearing | SelectionRole::Shaft | SelectionRole::Auto)
    }
}

/// One member of a set as `gear.calculate` worked it out (`GearResult::members`): one per
/// gear, shaft and bearing read from the sketch or the selection, in their order.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GearMember {
    /// Its place in the selection (or the sketch's reading order: circles, then lines, then
    /// points).
    pub index: usize,
    /// What it is (a `SelectionRole` name).
    pub role: String,
    /// Gears: their module, teeth, pitch diameter and face width (mm); null for shafts and
    /// bearings, or where an error left them unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teeth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_width: Option<f64>,
    /// The members (indices) it meshes with.
    #[serde(default)]
    pub meshes_with: Vec<usize>,
    /// The shaft member (index) it sits on (a gear, a bearing), if one is drawn.
    pub shaft: Option<usize>,
    /// The cluster it belongs to: gears on one shaft joined as one part (compound gears).
    pub cluster: Option<usize>,
    /// What is wrong with this member, if anything.
    pub error: Option<String>,
    /// Its role was worked out from the geometry (`auto`, or none, asked).
    #[serde(default)]
    pub inferred: bool,
    /// A helical gear's hand: "LH" or "RH".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand: Option<String>,
    /// A gear on a shaft at an angle to its mate's (bevel, crossed, worm): the shaft angle, deg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaft_angle: Option<f64>,
}

/// A shaft built with a set (`GearResult::shafts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GearShaftInfo {
    /// Its number in the set (0 first).
    pub index: usize,
    /// The body it is part of (with its gears).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// The gears it carries, as member (row) indices.
    pub gears: Vec<usize>,
    /// The picked shaft line it follows, as a member (row) index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// Where two gears touch (`GearResult::contacts`): the pitch line across the faces they share
/// (`a` to `b`), or a point (`a`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GearContact {
    pub a: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<[f64; 3]>,
}

/// A gear's shape without teeth (`GearResult::outlines`), for the dialog's abstract preview:
/// pitch, tip and root circles on its back face (`center`, square to `axis`) and the same at
/// its front face `face_width` mm along the axis; a bevel's at the heel, with its apex and its
/// pitch circle at the toe (the cone between).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GearOutline {
    /// The member (row).
    pub index: usize,
    pub center: [f64; 3],
    pub axis: [f64; 3],
    pub face_width: f64,
    pub pitch_diameter: f64,
    pub tip_diameter: f64,
    pub root_diameter: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apex: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toe_pitch_diameter: Option<f64>,
}

/// `shafts`: whether shafts are built on the gears' axes (each joined to its gears as one
/// part), and per member (its row `index`: a shaft line or a gear) on or off and its diameter.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearShaftsParams {
    /// Build shafts (default true).
    #[serde(default)]
    pub generate: Option<bool>,
    #[serde(default)]
    pub overrides: Option<Vec<GearShaftOverride>>,
}

/// One shaft's own setting (`GearShaftsParams::overrides`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearShaftOverride {
    /// The member's row `index` (a shaft line, or a gear on the shaft).
    pub index: usize,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub diameter: Option<f64>,
}

/// One selected thing of `selection` (gear.create, gear.calculate, gear.update): exactly one of
/// a sketch entity (`sketch` with `point` or `curve`: a line, or a circle, its centre the shaft
/// and its diameter the pitch diameter), a model `edge` or `face` (a point on it: a straight or
/// circular edge, a cylinder, cone or torus face) or an `axis` (X, Y, Z, a construction axis's
/// name, {origin?, dir}).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearSelection {
    #[serde(default)]
    pub sketch: Option<Value>,
    #[serde(default)]
    pub point: Option<String>,
    #[serde(default)]
    pub curve: Option<String>,
    #[serde(default)]
    pub edge: Option<Value>,
    #[serde(default)]
    pub face: Option<Value>,
    #[serde(default)]
    pub axis: Option<Value>,
    /// Shaft (default) or bearing (the default when `bearing` is given).
    #[serde(default)]
    pub role: Option<SelectionRole>,
    /// Shaft: the gear's pitch diameter in mm (a circle's own diameter when left out).
    #[serde(default)]
    pub pitch_diameter: Option<f64>,
    /// A gear: its teeth (a shaft's: the gear on it).
    #[serde(default)]
    pub teeth: Option<u32>,
    /// A gear: its module (its mesh group shares it).
    #[serde(default)]
    pub module: Option<f64>,
    /// A gear: its face width, mm (left out: worked out).
    #[serde(default)]
    pub face_width: Option<f64>,
    /// Its row in the dialog: members are reported by it (default: its place in `selection`).
    #[serde(default)]
    pub index: Option<usize>,
    /// A parallel-axis gear: its helix angle (deg; 0 straight; its mates get the opposite
    /// hand) and V (herringbone) teeth. A bevel: its spiral angle.
    #[serde(default)]
    pub helix_angle: Option<f64>,
    #[serde(default)]
    pub herringbone: Option<bool>,
    #[serde(default)]
    pub spiral_angle: Option<f64>,
    /// Shaft: how far along it from its origin (a sketch line's first end, an edge's or axis's
    /// origin) the gears sit, mm (the first shaft's; the others' must agree). Bearing on a
    /// shaft: how far along it the bearing's middle sits (default 0). Not for points.
    #[serde(default)]
    pub along: Option<f64>,
    /// Bearing: its standard name ("608", "6001", "MR105"...). One size per set: every bearing
    /// of a selection names the same one (or leaves it to `clearance_body.bearing`).
    #[serde(default)]
    pub bearing: Option<String>,
}

/// One picked thing of a gear item (`items[].components`): exactly one of a sketch entity
/// (`sketch` with `curve`: a circle is a pitch circle, a line a shaft or axis; or with `point`:
/// a bearing), a model `edge` or `face` (a point on it) or an `axis` (X, Y, Z, a construction
/// axis's name, {origin?, dir}). A sketch entity is kept by its ids, so the set follows the
/// sketch when it is edited; flat and 3D sketches, any number of them.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearComponent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<Value>,
    /// The slot it fills, as the dialog names it (pitch, pitch2, axis, axis2, bearing...); the
    /// engine fills slots by kind in pick order, so this is a label only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    /// The sketch's name when the pick was last kept (the feature fills it in; the sketch itself
    /// is found by its id): what an error names when the sketch is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch_name: Option<String>,
}

/// A gear's face: how far the teeth reach on side A (along the gear's axis: its sketch plane's
/// normal, or its shaft line's direction) and side B of the sketch plane, mm; null: Auto
/// (about 10 × module in all).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearFaceParams {
    /// symmetric | a_only | b_only | custom (left out: from `a` and `b`: equal or both null
    /// symmetric, b 0 A only, a 0 B only, else custom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default)]
    pub a: Option<f64>,
    #[serde(default)]
    pub b: Option<f64>,
}

/// A hub on a gear: its diameter (null: Auto) and how far it reaches past each face, mm.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearHubParams {
    #[serde(default)]
    pub diameter: Option<f64>,
    /// The Ø worked out when none is given or drawn: 1.6 × the bore, or half the root Ø.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<bool>,
    #[serde(default)]
    pub length_a: Option<f64>,
    #[serde(default)]
    pub length_b: Option<f64>,
}

fn auto() -> Value {
    json!("auto")
}

/// One gear of an item, its own settings (GEAR_UI_SPEC §3, §6); left out: Auto. The settings
/// an item gives for all its gears (`rim`, `starts`, `throated`, `spiral_angle`, `bevel_style`)
/// may be given per gear too.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearItemGear {
    /// Which of the item's gears (0-based; default its place in `gears`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gear: Option<usize>,
    /// module | teeth (left out: teeth when only `teeth` is given, else module).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teeth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<GearFaceParams>,
    /// Degrees; 0 straight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helix_angle: Option<f64>,
    /// left | right; null: opposite its mate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand: Option<SpiralHand>,
    /// V (herringbone) teeth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herringbone: Option<bool>,
    /// A number, or "auto" (avoids undercut below 17 teeth).
    #[serde(default = "auto")]
    pub profile_shift: Value,
    /// "auto" (fits its shaft, the default), null or "none" (no hole), or a diameter in mm.
    #[serde(default = "auto")]
    pub bore: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hub: Option<GearHubParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rim: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spiral_angle: Option<f64>,
    /// straight | spiral | chevron.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bevel_style: Option<String>,
}

impl Default for GearItemGear {
    fn default() -> Self {
        GearItemGear {
            gear: None,
            size_by: None,
            module: None,
            teeth: None,
            face: None,
            helix_angle: None,
            hand: None,
            herringbone: None,
            profile_shift: auto(),
            bore: auto(),
            hub: None,
            rim: None,
            starts: None,
            throated: None,
            spiral_angle: None,
            bevel_style: None,
        }
    }
}

/// One gear item of a Gears feature (GEAR_UI_SPEC §3): its type, the geometry picked into its
/// slots and its gears' settings.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearItemParams {
    /// Its id (A, B, …; default its letter by place): links, blocks and the drive name it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// spur | helical | herringbone | internal | rack | compound | planetary | bevel
    /// (spiral_bevel, chevron_bevel) | hypoid | worm | crossed_helical | shaft.
    #[serde(rename = "type")]
    pub kind: String,
    /// The picked geometry, in pick order.
    #[serde(default)]
    pub components: Vec<GearComponent>,
    /// One per gear (the first's settings fill the rest).
    #[serde(default)]
    pub gears: Vec<GearItemGear>,
    /// The module block it is in (`module_blocks[].id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_block: Option<String>,
    /// Ring gear: rim thickness, mm (Auto 2.5 m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rim: Option<f64>,
    /// Worm: thread starts; a throated wheel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throated: Option<bool>,
    /// Bevel / hypoid: straight | spiral | chevron, and the spiral angle (degrees).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bevel_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spiral_angle: Option<f64>,
    /// Rack: back height, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back_height: Option<f64>,
    /// Shaft: diameter (mm; Auto) and a keyway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyway: Option<bool>,
    /// Planetary: the number of planets (Auto: as many as fit, at most 3) and whether its
    /// carrier is made (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planets: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrier: Option<bool>,
}

/// A gear of an item: the item (its id, or its index) and which of its gears (0-based; `port`
/// is the same).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearEnd {
    pub item: Value,
    #[serde(default, alias = "port")]
    pub gear: usize,
}

/// A link of the graph between two gears: their teeth mesh, or they share a shaft.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearLinkParams {
    pub a: GearEnd,
    pub b: GearEnd,
    /// mesh (default) | shaft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// A block: items sharing one module (a value, or null: Auto, one for all): the items naming it
/// in their `module_block`, and those in `members`. Its other settings (each optional) go on
/// every member gear that gives none of its own: `tooth_chamfer` (mm, over the set's),
/// `pressure_angle`, `helix_angle` (deg), `face_width`, `backlash` (mm) and `shaft_diameter`
/// (mm), used as `shaft` says: `make` (the default) a shaft (a solid rod) of that Ø along each
/// axis its gears are on, one body with them (no bore); `bore` every member's bore (the hole
/// through the gear) at that Ø and no shaft; `none` neither.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearModuleBlockParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Its name for people (the dialog's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub module: Option<f64>,
    /// Items by id or index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooth_chamfer: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helix_angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backlash: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaft_diameter: Option<f64>,
    /// make (default) | bore | none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaft: Option<String>,
}

/// The gear that is turned (the assembly is driven from it) and, optionally, its speed (rpm)
/// and torque (N·m) for the results' speeds and torques.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearDriveParams {
    pub item: Value,
    #[serde(default, alias = "port")]
    pub gear: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torque: Option<f64>,
}

/// A gear set: involute spur, helical, herringbone, internal and rack; pairs and planetary sets;
/// straight and spiral bevels; hypoid (offset bevel) gears; crossed helical gears; worm and
/// wheel; cycloidal gears and the cycloidal drive; ISO 606 roller chain sprockets and timing belt
/// pulleys (single or a pair with the chain or belt between); DIN 5480 and ANSI B92.1 involute
/// splines (shaft and hub); face (crown) gears with their spur pinion; an elliptical gear and
/// its conjugate. Lengths in mm, angles in degrees. Each gear is
/// built from ordinary sketches and features (extrude, sweep with twist, loft, revolve, coil),
/// grouped in the timeline; one undo step.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearParams {
    /// The set to make (default spur).
    #[serde(default, rename = "type")]
    pub kind: Option<GearType>,
    /// Normal module in mm (bevel: at the large end; hypoid: the ring gear's outer transverse
    /// module; worm: axial). Default 2.
    #[serde(default)]
    pub module: Option<f64>,
    /// Diametral pitch in teeth per inch, instead of `module`.
    #[serde(default)]
    pub diametral_pitch: Option<f64>,
    /// Teeth of the gear (pinion, sun, worm wheel).
    #[serde(default)]
    pub teeth: Option<u32>,
    /// Teeth of the mating gear (pair, bevel, hypoid, crossed, cycloidal), or of each planet.
    #[serde(default)]
    pub teeth2: Option<u32>,
    /// Planetary ring teeth (default: sun + 2 × planet).
    #[serde(default)]
    pub ring_teeth: Option<u32>,
    /// Planet count (default 3).
    #[serde(default)]
    pub planets: Option<u32>,
    /// Normal pressure angle in degrees (default 20).
    #[serde(default)]
    pub pressure_angle: Option<f64>,
    /// Helix angle in degrees; its sign is the hand (positive right-handed). Crossed helical:
    /// gear 1's helix (gear 2 takes the rest of the shaft angle). Worm: negative for a left-hand
    /// worm.
    #[serde(default)]
    pub helix_angle: Option<f64>,
    /// Profile shift coefficient x of gear 1.
    #[serde(default)]
    pub profile_shift: Option<f64>,
    /// Profile shift coefficient of gear 2 (ignored with `center_distance`).
    #[serde(default)]
    pub profile_shift2: Option<f64>,
    /// The basic rack (default iso53_a: addendum 1, dedendum 1.25, root radius 0.38).
    #[serde(default)]
    pub rack: Option<RackStandard>,
    /// Addendum coefficient, overriding the rack's.
    #[serde(default)]
    pub addendum: Option<f64>,
    /// Dedendum coefficient, overriding the rack's.
    #[serde(default)]
    pub dedendum: Option<f64>,
    /// Root fillet radius coefficient, overriding the rack's.
    #[serde(default)]
    pub root_radius: Option<f64>,
    /// Circular backlash in mm taken off each tooth at the pitch circle (cycloidal drive: the
    /// clearance round the pins).
    #[serde(default)]
    pub backlash: Option<f64>,
    /// Face width in mm (default 10).
    #[serde(default)]
    pub face_width: Option<f64>,
    /// Pair: the working centre distance in mm; gear 2's profile shift is set to reach it.
    #[serde(default)]
    pub center_distance: Option<f64>,
    /// Bevel, hypoid and crossed helical: the angle between the shafts (default 90).
    #[serde(default)]
    pub shaft_angle: Option<f64>,
    /// Spiral bevel: the mean spiral angle (default 35). Hypoid: the pinion's mean spiral angle
    /// (default 50; the ring gear's is smaller by the offset angle).
    #[serde(default)]
    pub spiral_angle: Option<f64>,
    /// Hypoid: the pinion's offset from the ring gear's centre in mm (default 25; 0 makes a
    /// spiral bevel pair). Too large an offset is refused, naming the largest that works.
    #[serde(default)]
    pub offset: Option<f64>,
    /// Ignored (the earlier hypoid builder's face position); kept so older scripts still run.
    #[serde(default)]
    pub face_start: Option<f64>,
    /// Hypoid: the pinion's spiral hand (default left, as in car axles; the offset goes to the
    /// side that makes the pinion's spiral angle the larger).
    #[serde(default)]
    pub hand: Option<SpiralHand>,
    /// Hypoid: the ring gear's outer pitch diameter in mm, instead of `module` (default 200 with
    /// 41 teeth). Wins over `module` and `diametral_pitch` when given.
    #[serde(default)]
    pub ring_diameter: Option<f64>,
    /// Worm: thread starts (default 1).
    #[serde(default)]
    pub starts: Option<u32>,
    /// Worm: pitch diameter in mm (default 10 modules).
    #[serde(default)]
    pub worm_diameter: Option<f64>,
    /// Worm: length in mm (default: enough to engage the wheel).
    #[serde(default)]
    pub worm_length: Option<f64>,
    /// Internal and planetary ring: rim thickness beyond the roots in mm (default 3 modules).
    #[serde(default)]
    pub rim: Option<f64>,
    /// Rack: teeth along it (default 10).
    #[serde(default)]
    pub rack_teeth: Option<u32>,
    /// Rack: from the pitch line to the back in mm (default 3 modules).
    #[serde(default)]
    pub rack_height: Option<f64>,
    /// Cycloidal drive: ring pins (default 12; the disc has one lobe fewer).
    #[serde(default)]
    pub pins: Option<u32>,
    /// Cycloidal drive: diameter of the circle through the pin centres, in mm.
    #[serde(default)]
    pub pin_circle: Option<f64>,
    /// Cycloidal drive: pin diameter in mm.
    #[serde(default)]
    pub pin_diameter: Option<f64>,
    /// Cycloidal drive: eccentricity in mm.
    #[serde(default)]
    pub eccentricity: Option<f64>,
    /// Gear 1's bore.
    #[serde(default)]
    pub bore: Option<BoreParams>,
    /// Gear 2's bore (planets, mates, the worm).
    #[serde(default)]
    pub bore2: Option<BoreParams>,
    /// Hub diameter in mm, on the gear's back face (0: no hub).
    #[serde(default)]
    pub hub_diameter: Option<f64>,
    /// Hub length in mm.
    #[serde(default)]
    pub hub_length: Option<f64>,
    /// Lightening holes through the web (cycloidal drive: output holes).
    #[serde(default)]
    pub holes: Option<u32>,
    /// Lightening hole diameter in mm (default: to fit; cycloidal drive: output pin diameter).
    #[serde(default)]
    pub hole_diameter: Option<f64>,
    /// Build only one member: 1 the first (gear, pinion, sun, wheel, disc), 2 the second (mate,
    /// planets, worm, pins), 3 the ring. Default: all.
    #[serde(default)]
    pub only: Option<u8>,
    /// Pair: herringbone teeth (two opposite helical halves; needs `helix_angle`).
    #[serde(default)]
    pub herringbone: Option<bool>,
    /// Bevel planetary: the first planet's azimuth about the sun's axis in degrees (the rest
    /// are spaced evenly from it).
    #[serde(default)]
    pub first_azimuth: Option<f64>,
    /// Bevel planetary: what meshes the planets besides the sun (default for the type:
    /// differential).
    #[serde(default)]
    pub bevel_planet_mode: Option<GearBevelPlanetMode>,
    /// Where gear 1's centre goes, in mm (default the origin).
    #[serde(default)]
    pub origin: Option<[f64; 3]>,
    /// A name put in front of the bodies' and the timeline group's names.
    #[serde(default)]
    pub name: Option<String>,
    /// Axis-first: the shaft gear 1 turns about. With `axis2`, the set is sized and placed on
    /// both: the centre distance, shaft angle or offset come from the axes, `type` may be left
    /// out (a pair for parallel axes, bevels for intersecting ones, a worm or crossed helicals
    /// for skew ones, a planetary set for one shared line), and `module`, `teeth`, `teeth2` and
    /// `starts` are chosen where they are not given. Alone, it places a single gear (or gear 1
    /// of a set). `origin` is not used with it.
    #[serde(default)]
    pub axis1: Option<GearAxis>,
    /// Axis-first: the shaft gear 2 turns about (the worm, for a worm and wheel).
    #[serde(default)]
    pub axis2: Option<GearAxis>,
    /// Axis-first: gear 2's teeth ÷ gear 1's (worm: wheel teeth ÷ starts; cycloidal: wheel ÷
    /// pinion). Tooth counts that are not given are chosen to give it.
    #[serde(default)]
    pub ratio: Option<f64>,
    /// Axis-first bevels: the outer cone distance in mm (from where the axes meet to the large
    /// ends' pitch circles); sets the module.
    #[serde(default)]
    pub cone_distance: Option<f64>,
    /// Axis-first: put gear 1 on the other side (bevels: the other ray from the meeting point;
    /// skew axes: the supplementary shaft angle; otherwise the gear faces the other way).
    #[serde(default)]
    pub flip1: Option<bool>,
    /// Axis-first bevels: put gear 2 on the other ray from the meeting point.
    #[serde(default)]
    pub flip2: Option<bool>,
    /// Axis-first: gear 1 on `axis2` and gear 2 on `axis1`.
    #[serde(default)]
    pub swap_axes: Option<bool>,
    /// Axis-first, parallel or single: how far along `axis1` from its origin the set sits, mm.
    #[serde(default)]
    pub along: Option<f64>,
    /// Where the whole set goes, instead of the world frame at `origin` (what the axes solve
    /// to; not with `axis1`).
    #[serde(default)]
    pub placement: Option<PlacementParams>,
    /// Sprocket: the chain (default 08b). `teeth2` > 0 adds a driven sprocket at
    /// `center_distance` (rounded to a whole, even number of links).
    #[serde(default)]
    pub chain: Option<ChainSize>,
    /// Custom chain: pitch in mm.
    #[serde(default)]
    pub chain_pitch: Option<f64>,
    /// Custom chain: roller diameter in mm.
    #[serde(default)]
    pub roller_diameter: Option<f64>,
    /// Custom chain: width between the inner plates in mm.
    #[serde(default)]
    pub roller_width: Option<f64>,
    /// Custom chain: transverse pitch between strands in mm.
    #[serde(default)]
    pub strand_pitch: Option<f64>,
    /// Sprocket: chain strands, 1 simplex … 4 (default 1).
    #[serde(default)]
    pub strands: Option<u32>,
    /// Sprocket: the ISO 606 tooth side chamfer (default true).
    #[serde(default)]
    pub side_chamfer: Option<bool>,
    /// Pulley: the belt profile (default gt2_2); `face_width` is the belt width. `teeth2` > 0
    /// adds a second pulley at `center_distance` (rounded to a whole number of belt teeth).
    #[serde(default)]
    pub belt: Option<BeltProfile>,
    /// Pulley: flanges on both sides (default true).
    #[serde(default)]
    pub flanges: Option<bool>,
    /// Spline: the standard (default din5480).
    #[serde(default)]
    pub spline_standard: Option<SplineStandardParam>,
    /// Spline (DIN 5480): the reference diameter d_B in mm (0: module × (teeth + 1.1)).
    #[serde(default)]
    pub spline_diameter: Option<f64>,
    /// Spline: a fillet root instead of a flat root.
    #[serde(default)]
    pub fillet_root: Option<bool>,
    /// Face gear: the teeth's inner diameter in mm (default: the undercut limit); `face_width` is
    /// the teeth's length; `teeth` the pinion's, `teeth2` the face gear's.
    #[serde(default)]
    pub inner_diameter: Option<f64>,
    /// Elliptical pair: the ellipse's eccentricity, 0 … 0.7 (default 0.3); `teeth` on each gear.
    #[serde(default)]
    pub ellipse_eccentricity: Option<f64>,
    /// Strength rating: input torque in N·m on the driving member (gear 1, sun, ring of an
    /// internal set; the worm of a worm set). Wins over `power`. Without a torque or power the
    /// set is not rated.
    #[serde(default)]
    pub torque: Option<f64>,
    /// Strength rating: input power in kW (needs `speed`).
    #[serde(default)]
    pub power: Option<f64>,
    /// Strength rating: speed of the driving member in rpm.
    #[serde(default)]
    pub speed: Option<f64>,
    /// Strength rating: gear 1's material (default steel_through_hardened).
    #[serde(default)]
    pub material: Option<GearMaterial>,
    /// Strength rating: gear 2's material (mate, planets, internal set's pinion, worm).
    #[serde(default)]
    pub material2: Option<GearMaterial>,
    /// Strength rating: the planetary ring's material (default: as gear 2).
    #[serde(default)]
    pub material3: Option<GearMaterial>,
    /// Strength rating: application factor KA (default 1; 1.25 light, 1.5 moderate, 1.75 heavy
    /// shocks).
    #[serde(default)]
    pub application_factor: Option<f64>,
    /// Strength rating: required life in hours (default: unlimited).
    #[serde(default)]
    pub life_hours: Option<f64>,
    /// Strength rating: ISO 1328 accuracy grade 5..12 for the dynamic factor (default 7).
    #[serde(default)]
    pub accuracy_grade: Option<u8>,
    /// Tip rounding radius in modules, up to 0.5: each tip corner becomes an arc tangent to the
    /// flank and the tip circle.
    #[serde(default)]
    pub tip_round: Option<f64>,
    /// Horizontal-expansion (XY) compensation for printing, in mm: the teeth are drawn this far
    /// inside where they should end up (flanks, tips and roots).
    #[serde(default)]
    pub xy_compensation: Option<f64>,
    /// Elephant-foot relief in mm: the first layers of the teeth are drawn this far inside the
    /// rest.
    #[serde(default)]
    pub elephant_foot: Option<f64>,
    /// How high the elephant-foot relief runs from the back face, in mm (default 0.2).
    #[serde(default)]
    pub elephant_foot_height: Option<f64>,
    /// Nozzle width in mm: tip lands under two of them are warned about.
    #[serde(default)]
    pub nozzle: Option<f64>,
    /// Tip clearance coefficient c*, overriding the rack's dedendum (dedendum = addendum + c*).
    #[serde(default)]
    pub clearance: Option<f64>,
    /// Tip shortening in mm, off every gear's tip radius.
    #[serde(default)]
    pub tip_shortening: Option<f64>,
    /// Gear 1's tip diameter in mm (only lowers the tips).
    #[serde(default)]
    pub tip_diameter: Option<f64>,
    /// Gear 2's tip diameter in mm (only lowers the tips).
    #[serde(default)]
    pub tip_diameter2: Option<f64>,
    /// Tip relief in mm: how far the flank falls away from the involute at the tip.
    #[serde(default)]
    pub tip_relief: Option<f64>,
    /// The radial height the tip relief runs over, in mm (default 0.4 modules).
    #[serde(default)]
    pub tip_relief_height: Option<f64>,
    /// Lead crowning in mm: each flank is this much thinner at the faces than at mid face
    /// (straight teeth only).
    #[serde(default)]
    pub crowning: Option<f64>,
    /// Gear 2's own backlash in mm (mate, planets, the worm's thread; default: `backlash`).
    #[serde(default)]
    pub backlash2: Option<f64>,
    /// Pair: gear 2's face width in mm (default: `face_width`). Bevel and spiral bevel: gear 2's
    /// face width along the cone; bevel planetary: the planets'. Hypoid: the pinion's face width
    /// along its pitch cone (default: covering the ring gear's face, `face_width`).
    #[serde(default)]
    pub face_width2: Option<f64>,
    /// Pair: how far gear 2's mid plane sits from gear 1's along the axis, in mm. Bevel and
    /// spiral bevel (gear 2) and bevel planetary (the planets): how far the heel (the teeth's
    /// outer end) sits in from the cone distance towards the apex, in mm. Hypoid: how far the
    /// pinion's face sits towards its heel from where it covers the ring gear's.
    #[serde(default)]
    pub face_offset2: Option<f64>,
    /// Tooth edge chamfer on the front face (the hub's side).
    #[serde(default)]
    pub tooth_chamfer_front: Option<ChamferParams>,
    /// Tooth edge chamfer on the back face.
    #[serde(default)]
    pub tooth_chamfer_back: Option<ChamferParams>,
    /// 45° chamfer on both ends of the bores, in mm.
    #[serde(default)]
    pub bore_chamfer: Option<f64>,
    /// 45° chamfer on the hub's outer edge, in mm.
    #[serde(default)]
    pub hub_chamfer: Option<f64>,
    /// Gear 1's integral shaft on its front face (the hub's side, +z).
    #[serde(default)]
    pub shaft_front: Option<ShaftParams>,
    /// Gear 1's integral shaft on its back face.
    #[serde(default)]
    pub shaft_back: Option<ShaftParams>,
    /// Gear 2's (mate, planets, the worm) integral shaft on its front face.
    #[serde(default)]
    pub shaft2_front: Option<ShaftParams>,
    /// Gear 2's integral shaft on its back face.
    #[serde(default)]
    pub shaft2_back: Option<ShaftParams>,
    /// Gear 1's hub hardware and bearing pocket.
    #[serde(default)]
    pub hardware: Option<HardwareParams>,
    /// Gear 2's hub hardware and bearing pocket.
    #[serde(default)]
    pub hardware2: Option<HardwareParams>,
    /// Pair and planetary: build a print test coupon instead of the set (a few teeth of each
    /// member at the true centre distance, on a base with pegs and fences). Default false.
    #[serde(default)]
    pub coupon: Option<bool>,
    /// Teeth on each coupon piece, 3..40 (default 4).
    #[serde(default)]
    pub coupon_teeth: Option<u32>,
    /// How the teeth of planar involute sets (spur, helical, herringbone, internal, pair,
    /// planetary, rack) are drawn: `fitted` (default) fit point splines through computed points;
    /// `equations` equation curves of the gear's own formulas (involute, trochoid root fillet)
    /// with tip and root arcs, one tooth and a circular pattern. Other types are always fitted.
    #[serde(default)]
    pub tooth_curves: Option<ToothCurves>,
    /// With `tooth_curves: equations`: make design parameters (`<prefix>_module`,
    /// `<prefix>_teeth`, `<prefix>_pressure_angle`, `<prefix>_shift`, `<prefix>_backlash`,
    /// `<prefix>_face_width`… and derived radii) and draw the teeth, pattern counts and tooth
    /// extrusions from them, so editing a parameter regenerates the teeth. Default false.
    #[serde(default)]
    pub parametric: Option<bool>,
    /// The design parameters' prefix (default: from the set's name).
    #[serde(default)]
    pub parameter_prefix: Option<String>,
    /// Also make the set's clearance body (keep-out volume for a housing), named
    /// "<set name> clearance": see [`ClearanceParams`]. Not made when absent. (`clearance` is
    /// the tip clearance coefficient c*.)
    #[serde(default)]
    pub clearance_body: Option<ClearanceParams>,
    /// Make the set a complete assembly: every gear a component of its own (named after its
    /// body), a revolute joint per gear on its shaft (a slider for a rack; held members such as
    /// a cycloidal drive's pin ring grounded; a planet on its carrier), and a gear,
    /// rack-and-pinion or rigid motion link per mesh with the set's ratio and direction, so
    /// driving one gear (`joint.drive`) turns the others. With `clearance_body` the keep-out
    /// body goes in too, as a grounded see-through component "<set name> keep-out", to cut a
    /// housing with. All in the set's undo step and timeline folder; `gear.update` keeps them
    /// in place: the same components (the bodies regenerated inside them), the same joints moved
    /// onto the new shafts, the links given the new ratios, and joints users made to them kept;
    /// only members that appear or go are added or taken away. Default false.
    #[serde(default)]
    pub assembly: Option<bool>,
    /// With `assembly`: the member that drives the rest (1-based, in the order of the set's
    /// bodies; default 1). Its joint is the one to drive (`joint.drive`); every other follows it
    /// through the motion links, each link driven from the end nearer it.
    #[serde(default)]
    pub assembly_driver: Option<u32>,
    /// How the set goes into the timeline: `feature` (the default), one Gears feature that reads
    /// its sketches again and builds every body itself whenever they change; `features`, a
    /// folder of ordinary sketches and features (always so for teeth drawn as formulas,
    /// `tooth_curves: equations`). `gear.update` with `build: feature` turns a folder into the
    /// feature.
    #[serde(default)]
    pub build: Option<String>,
    /// The Gears feature's bodies: `new` bodies (the default), or tools that `join`, `cut` or
    /// `intersect` the bodies in `targets`.
    #[serde(default)]
    pub operation: Option<String>,
    /// The bodies `operation` works on.
    #[serde(default)]
    pub targets: Option<Vec<String>>,
    /// Bodies the set's keep-out body is cut from (a housing gets its cavity in the same
    /// feature; the set needs `clearance_body`). The keep-out is then no body of its own.
    #[serde(default)]
    pub keep_out_cuts: Option<Vec<String>>,
    /// Differential (`type: differential`): its settings.
    #[serde(default)]
    pub differential: Option<DifferentialParams>,
    /// Differential: design it from what it must do (Simple mode) instead.
    #[serde(default)]
    pub differential_goal: Option<DifferentialGoalParams>,
    /// `gear.update` and `gear.inspect` only: the set, by the timeline folder `gear.create`
    /// made for it.
    #[serde(default)]
    pub group: Option<u64>,
    /// `gear.update` and `gear.inspect` only: the set, by the name of one of its bodies.
    #[serde(default)]
    pub body: Option<String>,
    /// `gear.update` only: true starts from the type's defaults instead of the parameters the
    /// set was last made with (otherwise the parameters given here are laid over those, and a
    /// `null` takes one away).
    #[serde(default)]
    pub reset: Option<bool>,
    /// A front end's own state, kept with the set as it is and given back by `gear.inspect`
    /// (the Gear Generator window keeps its typed values and picked axes here). Not read by the
    /// engine.
    #[serde(default)]
    pub ui: Option<Value>,
    /// Drawn in one sketch (id or name): circles are pitch circles, lines shafts (see
    /// `solidcraft_gear::sketch_rules`). `type` is the mode (left out: what the drawing looks
    /// like); the sizes the drawing gives (diameters, centre distance, offset, a bevel's shaft
    /// angle between its two lines) replace module and teeth. Not with `axis1` or `placement`.
    #[serde(default)]
    pub sketch: Option<Value>,
    /// With `sketch`: the mode's slots in order (the origin point first, then its circles and
    /// lines), as the sketch's point and curve ids; null or "" leaves a slot to be filled in
    /// drawing order. Left out: every slot in drawing order.
    #[serde(default)]
    pub picks: Option<Vec<Option<String>>>,
    /// With `sketch`: the set turned about the origin point by these angles (degrees about the
    /// sketch's x, then y, then its normal).
    #[serde(default)]
    pub sketch_turn: Option<[f64; 3]>,
    /// Selected shafts and bearing places, mixed (sketch points, lines and circles of one or
    /// more sketches, model edges and faces, axes): the shafts in order make the set (one: a
    /// gear on it; two: the set their lie implies, as gear.from_axes solves it; three or more,
    /// parallel: a gear train), each with its size; the bearings are seats of the keep-out body
    /// and, with `assembly`, bearing bodies. Not with `sketch`, `axis1` or `placement`.
    #[serde(default)]
    pub selection: Option<Vec<GearSelection>>,
    /// With `sketch` or `selection`: the shafts built on the gears' axes (default: built).
    #[serde(default)]
    pub shafts: Option<GearShaftsParams>,
    /// A drivetrain of gear items (GEAR_UI_SPEC §3), each its type, the sketch geometry picked
    /// into its slots (kept by id: the set follows its sketches) and each gear's own settings;
    /// planned whole and built as one Gears feature (an assembly with `assembly`). Not with
    /// `sketch`, `selection`, `axis1` or `placement`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<GearItemParams>>,
    /// With `items`: the graph's links (mesh or shaft) between their gears. None at all: the
    /// gears mesh and share shafts as the drawing suggests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<GearLinkParams>>,
    /// With `items`: items sharing one module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_blocks: Option<Vec<GearModuleBlockParams>>,
    /// With `items`: the gear that is turned (default the first), its rpm and torque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive: Option<GearDriveParams>,
    /// With `items`: one tooth chamfer for every gear of the set (45°, `size` mm off the tip
    /// circle's edge, on `faces`: "both" (default), "a" or "b", the gear's side A / side B).
    /// Bevel, hypoid, worm, crossed helical, rack and planetary gears are left plain, with a note.
    /// Absent or size 0: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooth_chamfer: Option<ToothChamferParams>,
    /// With `items`: the keep-out body's gap round the gears (radial and axial, mm); asking it
    /// (or `keep_out_cuts`) makes the keep-out body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_out_gap: Option<f64>,
    /// Kept by the engine (given back by `gear.inspect`; ignored when sent): the generator's
    /// name of each body of a Gears feature made of `items`, in the order of its body names, so
    /// each body keeps its name (and its component and joint) when a sketch edit plans the
    /// bodies in another order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_keys: Option<Vec<String>>,
}

/// `gear.inspect`: which set (neither: every set in the design).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearInspectParams {
    /// The set's timeline folder.
    #[serde(default)]
    pub group: Option<u64>,
    /// The name of one of the set's bodies.
    #[serde(default)]
    pub body: Option<String>,
}

/// A feature outside the set that worked before `gear.update` and fails after it.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearBroken {
    pub feature: String,
    pub error: String,
}

/// A gear set kept in the design: what `gear.create` or `gear.update` last made it from.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GearSet {
    /// The set's timeline folder, or its Gears feature (`feature`).
    pub group: u64,
    /// The Gears feature that builds the set, when it is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<u64>,
    /// The folder's name.
    pub name: String,
    /// The set's bodies.
    pub bodies: Vec<String>,
    /// The set's features (sketches and solids), in build order.
    pub features: Vec<u64>,
    /// The parameters as they were given (axis references, ratio and `ui` included).
    pub request: Value,
    /// The set they came to, as plain `gear.create` parameters: every value explicit, placed
    /// by `placement` when it is not at the world's origin frame.
    pub params: Value,
    /// The sketch the set is made from and follows (`gear.create {sketch}`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<u64>,
    /// The sketch reads differently than when the set was last made (an edit the set could not
    /// follow): `gear.update {group}` brings it up to date.
    #[serde(default)]
    pub sketch_changed: bool,
}

/// `gear.sketch`: a mode's drawing as a native sketch.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearSketchParams {
    /// The mode to draw.
    #[serde(rename = "type")]
    pub kind: GearType,
    /// The sketch's plane, as `sketch.create`'s `plane` (default XY).
    #[serde(default)]
    pub plane: Option<Value>,
    /// The sketch's name.
    #[serde(default)]
    pub name: Option<String>,
    /// `gear.create` parameters the drawing is sized by (module, teeth, shaft angle, offset...).
    #[serde(default)]
    pub gear: Option<Value>,
}

/// What `gear.sketch` made.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GearSketchResult {
    /// The sketch's feature id.
    pub sketch: u64,
    /// The mode's slots, as the sketch's point and curve ids (`gear.create`'s `picks`).
    pub picks: Vec<Option<String>>,
    /// The parameter each dimension's value is in: `diameter1`, `diameter2`, `shaft_angle`,
    /// `offset`, `length`, as the mode has them.
    pub dimensions: std::collections::BTreeMap<String, String>,
    /// The constraints added besides the curves' own.
    pub constraints: Vec<String>,
}

/// What `gear.inspect` found: the set asked for, or every set in the design.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearInspectResult {
    pub sets: Vec<GearSet>,
}

/// One number describing the set.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearFact {
    pub label: String,
    pub value: f64,
    /// "mm", "deg" or "".
    pub unit: String,
}

/// One member of a gear set made an assembly: its component and the joint it turns on.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GearAssemblyMember {
    /// The body it holds.
    pub body: String,
    /// Its component, by id.
    pub component: u64,
    /// Its component's occurrence, by id.
    pub occurrence: u64,
    /// The joint it turns (or slides) on, by id; none for a grounded member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint: Option<u64>,
    /// Held fixed (grounded).
    pub grounded: bool,
    /// The shaft it turns about (world): a point on it and its direction (a rack: the line it
    /// slides along).
    pub axis: [[f64; 3]; 2],
}

/// What `assembly: true` made of a gear set.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct GearAssembly {
    /// The members, in the order of the set's bodies.
    pub members: Vec<GearAssemblyMember>,
    /// The keep-out component (the clearance body, grounded), by id, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_out: Option<u64>,
    /// The joints made, by id.
    pub joints: Vec<u64>,
    /// The motion links made (gears, rack and pinion), by their index in `joint.list`'s `links`.
    pub links: Vec<usize>,
    /// The member that drives the others (1-based; `assembly_driver`).
    pub driver: usize,
    /// Loops of meshes whose ratios don't agree (the system would lock), each naming the members
    /// round it; the mesh closing each is left out of the joints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
}

/// What `gear.create` or `gear.update` made, or what `gear.calculate` worked out.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct GearResult {
    /// The bodies made (empty for `gear.calculate`).
    pub bodies: Vec<String>,
    /// The Gears feature that builds the set (`build: feature`); `group` is its id too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<u64>,
    /// The timeline group holding the gear's features, if any were made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Numbers describing the set: diameters, ratios, centre distances, contact ratios,
    /// measurements over teeth and pins.
    pub facts: Vec<GearFact>,
    /// The clearance body made with the set (`clearance_body`), by name; also the last of
    /// `bodies`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearance: Option<String>,
    /// With `assembly: true`: the components, joints and motion links made for the set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<GearAssembly>,
    /// Problems worth knowing: undercut, pointed teeth, low contact ratio, interference.
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
    /// Axis-first: how the set was sized to its axes (module and teeth chosen, shifts, angles).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// `gear.update`: how the set was regenerated. "in_place": every sketch and feature kept
    /// its id and name, and the sketches' curves their ids as far as the new outline has as
    /// many; "rebuilt": the set's features were deleted and made again (new feature ids) under
    /// the same body names, in the same folder and place in the history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// `gear.update`: why the set could not be regenerated in place, when it was rebuilt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuilt_because: Option<String>,
    /// `gear.update`: features outside the set that worked before and fail now (what was built
    /// on a face or edge the new gear no longer has).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub broken: Vec<GearBroken>,
    /// `gear.calculate`: the set worked out, as explicit `gear.create` parameters (placed by
    /// `placement`), as `gear.inspect`'s `params` (a front end previews it from these: a set
    /// read from a sketch is read by the engine, not again by the front end).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// `gear.calculate` (and `gear.create`) of a set read from a sketch or a selection: each
    /// gear, shaft and bearing it read, with what it became or what is wrong with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<GearMember>,
    /// The shafts built with the set (each joined to its gears as one body).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shafts: Vec<GearShaftInfo>,
    /// Where the gears touch, for an abstract preview.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<GearContact>,
    /// Each gear's shape without teeth, for an abstract preview.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outlines: Vec<GearOutline>,
    /// `items`: each gear of the items, in item and gear order (GEAR_UI_SPEC §6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gears: Vec<GearItemReply>,
    /// `items`: each link used (the request's, or the ones the drawing suggests when it has
    /// none), with its ratio or what is wrong with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<GearLinkReply>,
    /// `items`: links the drawing suggests that the request doesn't have (drawn dashed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<GearSuggestionReply>,
    /// `items`: bore and hub circles the drawing suggests for a picked pitch circle (concentric
    /// circles in its sketch that nothing picks), not used until picked.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detail_suggestions: Vec<GearDetailSuggestion>,
    /// `items`: why nothing can be built (`gear.calculate` still says per gear and per link
    /// what was found; `gear.create` refuses with it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A gear of an item in a reply.
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct GearEndReply {
    /// The item's id.
    pub item: String,
    /// Which of its gears (0-based).
    pub gear: usize,
}

/// One gear of the items as planned (`GearResult::gears`).
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct GearItemReply {
    /// The item's id.
    pub item: String,
    pub gear: usize,
    pub teeth: u32,
    pub module: f64,
    /// Pitch diameter, mm.
    pub pitch_d: f64,
    /// Its speed over the driver's (signed: negative turns the other way).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    /// From `drive.rpm` / `drive.torque` (N·m), with each stage's efficiency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torque: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_shift: Option<f64>,
    pub warnings: Vec<String>,
    /// The body that holds it (`gear.create` / `gear.update`; a compound's gears share one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Its side A: the direction along its axis its face's side A reaches (unit, world): its
    /// pitch circle's sketch normal, or its shaft line's direction from its first point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side_a: Option<[f64; 3]>,
    /// Its bore and hub diameters as built (mm): from a `bore` / `hub` circle picked in its
    /// sketch, else its settings; none: no bore (or the shaft's), no hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bore: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hub: Option<f64>,
}

/// A detail the drawing suggests for a gear (`GearResult::detail_suggestions`): a circle drawn
/// concentric with its pitch circle, in the same sketch, that no item picks; inside it the
/// smallest is a bore, the next a hub. Shown as a suggestion; add it to the item's components
/// as `{sketch, curve, slot}` to use it.
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct GearDetailSuggestion {
    pub item: String,
    pub gear: usize,
    /// bore | hub.
    pub slot: String,
    /// The circle: its sketch's id and its id there.
    pub sketch: u64,
    pub curve: String,
    /// Its diameter, mm.
    pub diameter: f64,
}

/// One link as planned (`GearResult::links`).
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct GearLinkReply {
    pub a: GearEndReply,
    pub b: GearEndReply,
    /// mesh | shaft.
    pub kind: String,
    /// b's speed over a's (signed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    /// What is wrong ("centre distance 31.2, needs 30.0 ±0.05"), and what to do.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// A link the drawing suggests (`GearResult::suggestions`).
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct GearSuggestionReply {
    pub a: GearEndReply,
    pub b: GearEndReply,
    /// mesh | shaft.
    pub kind: String,
    /// Why, for the hover.
    pub why: String,
}

/// `gear.ratio`: the speed ratio between two gears of one Gears feature.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GearRatioParams {
    /// The set, by its timeline folder or Gears feature (left out: the set `a` belongs to).
    #[serde(default)]
    pub group: Option<u64>,
    /// A set not made yet: its `gear.calculate` parameters (the Gears dialog's request); `a` and
    /// `b` are then {item, gear}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
    /// From and to: a body's name, a component's id, or {item, gear} of the set's items.
    pub a: Value,
    pub b: Value,
}

/// One step of the path between the two gears.
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct GearRatioStep {
    /// The bodies at its two ends.
    pub from: String,
    pub to: String,
    /// The gears at its ends, when the set is made of items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_gear: Option<GearEndReply>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_gear: Option<GearEndReply>,
    /// shaft, or the mesh: spur, helical, herringbone, internal, rack, bevel, spiral_bevel,
    /// hypoid, crossed_helical, worm, wrap.
    pub kind: String,
    /// Teeth at its two ends (a worm: its starts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teeth: Option<[u32; 2]>,
    /// `to`'s motion per unit of `from`'s (signed).
    pub ratio: f64,
    pub efficiency: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub self_locking: bool,
}

/// What `gear.ratio` found.
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct GearRatioResult {
    /// The set's folder or feature.
    pub group: u64,
    /// The two ends' bodies.
    pub a: String,
    pub b: String,
    /// b's speed over a's, signed (rad/rad; mm/rad when b is a rack).
    pub ratio: f64,
    /// same | opposite.
    pub direction: String,
    /// |ω_a / ω_b| (> 1 slows down).
    pub reduction: f64,
    /// b is a rack: its travel per turn of a (mm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mm_per_rev: Option<f64>,
    /// a is a rack: b's turns per mm of it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev_per_mm: Option<f64>,
    /// The meshes and shared shafts from a to b.
    pub path: Vec<GearRatioStep>,
    /// The product of the steps' efficiencies.
    pub efficiency: f64,
    pub notes: Vec<String>,
}

/// A line in space.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearAxisLine {
    /// A point on it, in mm.
    pub origin: [f64; 3],
    /// Its unit direction.
    pub dir: [f64; 3],
}

/// How two shaft axes lie.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearRelation {
    /// "single" (one axis), "coincident", "parallel", "intersecting" or "skew".
    pub kind: String,
    /// In words: "parallel, 40 mm apart".
    pub text: String,
    /// Parallel: the distance between the axes; skew: the offset along their common
    /// perpendicular. mm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
    /// Intersecting and skew: the angle between the directions as given, in degrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// Intersecting: where the axes meet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apex: Option<[f64; 3]>,
    /// Parallel and skew: the nearest points, on axis 1 and on axis 2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foot1: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foot2: Option<[f64; 3]>,
}

/// Whether a gear type suits the axes.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearCandidate {
    #[serde(rename = "type")]
    pub kind: GearType,
    pub label: String,
    pub fits: bool,
    /// The type used when none is asked for.
    pub natural: bool,
    /// Why not, or how it is sized and placed.
    pub reason: String,
}

/// A body's placement: its origin, x and z (its axis) in world coordinates.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearFrame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub z: [f64; 3],
}

/// What `gear.from_axes` worked out: how the axes lie, which gear types fit, and the set solved
/// on them.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearAxesResult {
    pub axis1: GearAxisLine,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis2: Option<GearAxisLine>,
    pub relation: GearRelation,
    /// Every gear type, with whether it fits these axes.
    pub candidates: Vec<GearCandidate>,
    /// The type solved (the one asked for, or the natural one).
    #[serde(rename = "type")]
    pub kind: GearType,
    /// Why that type could not be solved on these axes (then `params` is absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The solved set as plain `gear.create` parameters: every value explicit, placed by
    /// `placement`, no axis references.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<GearParams>,
    /// Which parameters the axes and the solve decided (their `gear.create` names).
    pub derived: Vec<String>,
    /// Where gear 1 and gear 2 turn.
    pub frames: Vec<GearFrame>,
    pub facts: Vec<GearFact>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}
