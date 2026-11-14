//! `gear.create` and `gear.calculate`: the Gear Generator's parameters and replies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    /// Normal module in mm (bevel: at the large end; worm: axial). Default 2.
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
    /// Spiral bevel: the mean spiral angle (default 35).
    #[serde(default)]
    pub spiral_angle: Option<f64>,
    /// Hypoid: the distance between the axes in mm (default: from equal spiral angles).
    #[serde(default)]
    pub offset: Option<f64>,
    /// Hypoid: where the face starts along the line of contact, from the throat, in mm.
    #[serde(default)]
    pub face_start: Option<f64>,
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
    /// Pair and planetary: build a print test coupon instead of the set (a few teeth of each
    /// member at the true centre distance, on a base with pegs and fences). Default false.
    #[serde(default)]
    pub coupon: Option<bool>,
    /// Teeth on each coupon piece, 3..40 (default 4).
    #[serde(default)]
    pub coupon_teeth: Option<u32>,
}

/// One number describing the set.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GearFact {
    pub label: String,
    pub value: f64,
    /// "mm", "deg" or "".
    pub unit: String,
}

/// What `gear.create` made, or what `gear.calculate` worked out.
#[derive(Debug, Serialize, JsonSchema)]
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
}
