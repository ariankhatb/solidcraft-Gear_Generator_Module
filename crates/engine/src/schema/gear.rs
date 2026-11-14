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
                json!("A shaft axis: X, Y or Z, a construction axis's name, {\"origin\"?: [x, y, z] in mm, \"dir\": [x, y, z]}, a straight or circular edge or a cylinder, cone or torus face of a body ({\"edge\": [x, y, z]} or {\"face\": [x, y, z]}, a point on it), or a straight sketch line ({\"sketch\": id or name, \"curve\": id}). Read once: the gears do not follow the axis afterwards."),
            );
            if let Some(Value::Array(forms)) = o.get_mut("anyOf") {
                forms.push(json!({"type": "object", "properties": {"sketch": {"type": ["string", "integer"]}, "curve": {"type": "string"}}, "required": ["sketch", "curve"]}));
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
    /// Pair: gear 2's face width in mm (default: `face_width`).
    #[serde(default)]
    pub face_width2: Option<f64>,
    /// Pair: how far gear 2's mid plane sits from gear 1's along the axis, in mm.
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
    /// The set's timeline folder.
    pub group: u64,
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

/// What `gear.create` or `gear.update` made, or what `gear.calculate` worked out.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct GearResult {
    /// The bodies made (empty for `gear.calculate`).
    pub bodies: Vec<String>,
    /// The timeline group holding the gear's features, if any were made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Numbers describing the set: diameters, ratios, centre distances, contact ratios,
    /// measurements over teeth and pins.
    pub facts: Vec<GearFact>,
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
