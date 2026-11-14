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
/// wheel; cycloidal gears and the cycloidal drive. Lengths in mm, angles in degrees. Each gear is
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
    /// spiral bevel pair). Too large an offset is reduced to the largest that works, with a
    /// warning.
    #[serde(default)]
    pub offset: Option<f64>,
    /// Ignored (the earlier hypoid builder's face position); kept so older scripts still run.
    #[serde(default)]
    pub face_start: Option<f64>,
    /// Hypoid: the pinion's spiral hand (default left, as in car axles; the offset goes to the
    /// side that makes the pinion's spiral angle the larger).
    #[serde(default)]
    pub hand: Option<SpiralHand>,
    /// Hypoid: the ring gear's outer pitch diameter in mm, instead of `module`.
    #[serde(default)]
    pub ring_diameter: Option<f64>,
    /// Hypoid: the face-milling cutter's radius in mm, which sets the tooth lines' curvature
    /// (default: a standard cutter near 0.9 × the gear's mean cone distance).
    #[serde(default)]
    pub cutter_radius: Option<f64>,
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
