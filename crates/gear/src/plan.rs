//! What to build: a gear set described by a [`Spec`] becomes a [`Plan`] of bodies, each a short
//! list of solid steps (prisms, twisted prisms, lofts, revolves, coils) in its own frame, with the
//! numbers that describe the set, warnings, and 2D outlines for a meshing preview.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::cycloid::{Cycloidal, CycloidalDrive, cycloidal_outline};
use crate::geom::{Outline, Region, V2, inv, wrap_period};
use crate::hub::{Bore, lightening_holes};
use crate::involute::{
    BasicRack, Internal, Involute, ToothFlank, contact_ratio_external, external_flank, external_outline, internal_tip_interference, mesh_external, mesh_internal, mesh_phase, rack_outline,
    shift_sum_for_center, teeth_outline,
};

/// Gear set types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum Kind {
    #[default]
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

/// How the axes of a set lie, for grouping types in a picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axes {
    Parallel,
    Intersecting,
    Crossed,
    Special,
}

impl Kind {
    pub const ALL: [Kind; 14] = [
        Kind::Spur,
        Kind::Helical,
        Kind::Herringbone,
        Kind::Internal,
        Kind::Rack,
        Kind::Pair,
        Kind::Planetary,
        Kind::Bevel,
        Kind::SpiralBevel,
        Kind::Hypoid,
        Kind::CrossedHelical,
        Kind::Worm,
        Kind::Cycloidal,
        Kind::CycloidalDrive,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Kind::Spur => "spur",
            Kind::Helical => "helical",
            Kind::Herringbone => "herringbone",
            Kind::Internal => "internal",
            Kind::Rack => "rack",
            Kind::Pair => "pair",
            Kind::Planetary => "planetary",
            Kind::Bevel => "bevel",
            Kind::SpiralBevel => "spiral_bevel",
            Kind::Hypoid => "hypoid",
            Kind::CrossedHelical => "crossed_helical",
            Kind::Worm => "worm",
            Kind::Cycloidal => "cycloidal",
            Kind::CycloidalDrive => "cycloidal_drive",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Kind::Spur => "Spur gear",
            Kind::Helical => "Helical gear",
            Kind::Herringbone => "Herringbone gear",
            Kind::Internal => "Internal (ring) gear",
            Kind::Rack => "Rack",
            Kind::Pair => "Gear pair",
            Kind::Planetary => "Planetary set",
            Kind::Bevel => "Straight bevel",
            Kind::SpiralBevel => "Spiral bevel",
            Kind::Hypoid => "Hypoid (offset bevel)",
            Kind::CrossedHelical => "Crossed helical",
            Kind::Worm => "Worm and wheel",
            Kind::Cycloidal => "Cycloidal (clock) gears",
            Kind::CycloidalDrive => "Cycloidal drive",
        }
    }
    pub fn axes(self) -> Axes {
        match self {
            Kind::Spur | Kind::Helical | Kind::Herringbone | Kind::Internal | Kind::Rack | Kind::Pair | Kind::Planetary => Axes::Parallel,
            Kind::Bevel | Kind::SpiralBevel => Axes::Intersecting,
            Kind::Hypoid | Kind::CrossedHelical | Kind::Worm => Axes::Crossed,
            Kind::Cycloidal | Kind::CycloidalDrive => Axes::Special,
        }
    }
    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|k| k.name() == s)
    }
    /// Involute tooth form (module, pressure angle, shift, rack apply).
    pub fn involute(self) -> bool {
        !matches!(self, Kind::Cycloidal | Kind::CycloidalDrive)
    }
    /// Makes two (or more) meshing members.
    pub fn is_set(self) -> bool {
        matches!(self, Kind::Pair | Kind::Planetary | Kind::Bevel | Kind::SpiralBevel | Kind::Hypoid | Kind::CrossedHelical | Kind::Worm | Kind::Cycloidal | Kind::CycloidalDrive)
    }
}

/// Everything that describes a gear set. Angles in degrees, lengths in millimetres; zero means
/// "work it out" where the field says so.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub kind: Kind,
    /// Normal module (bevel: at the large end; worm: axial module).
    pub module: f64,
    /// Gear 1 teeth (pinion, sun, the worm wheel's teeth for a worm set, the gear of a single type).
    pub teeth: u32,
    /// Gear 2 teeth (mate, planet).
    pub teeth2: u32,
    /// Ring teeth for a planetary set (0: sun + 2 × planet).
    pub teeth3: u32,
    pub planets: u32,
    pub pressure_angle: f64,
    /// Helix angle (spur types: 0; crossed helical: gear 1's).
    pub helix: f64,
    pub shift: f64,
    pub shift2: f64,
    pub rack: BasicRack,
    pub backlash: f64,
    pub face_width: f64,
    /// Pair: working centre distance (0: from the shifts; otherwise gear 2's shift is set to
    /// reach it).
    pub center_distance: f64,
    pub shaft_angle: f64,
    pub spiral_angle: f64,
    /// Hypoid offset between the axes (0: from equal spiral angles).
    pub offset: f64,
    /// Hypoid: where the face starts along the line of contact, from the throat (mm).
    pub face_start: f64,
    /// Worm threads (starts), pitch diameter (0: 10 modules) and length (0: enough to engage).
    pub starts: u32,
    pub worm_diameter: f64,
    pub worm_length: f64,
    /// Ring gear rim: radial thickness beyond the roots (0: 3 modules).
    pub rim: f64,
    /// Rack: teeth along it and its height below the pitch line (0: 3 modules).
    pub rack_teeth: u32,
    pub rack_height: f64,
    /// Cycloidal drive.
    pub pins: u32,
    pub pin_circle: f64,
    pub pin_diameter: f64,
    pub eccentricity: f64,
    pub bore: Bore,
    pub bore2: Bore,
    pub hub_diameter: f64,
    pub hub_length: f64,
    pub holes: u32,
    pub hole_diameter: f64,
    /// Which members to build: 0 all, 1 the first, 2 the second, 3 the third.
    pub only: u8,
    /// Where the set goes (gear 1's centre).
    pub origin: [f64; 3],
    /// Body name prefix.
    pub name: String,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            kind: Kind::Spur,
            module: 2.0,
            teeth: 20,
            teeth2: 30,
            teeth3: 0,
            planets: 3,
            pressure_angle: 20.0,
            helix: 0.0,
            shift: 0.0,
            shift2: 0.0,
            rack: BasicRack::default(),
            backlash: 0.0,
            face_width: 10.0,
            center_distance: 0.0,
            shaft_angle: 90.0,
            spiral_angle: 35.0,
            offset: 0.0,
            face_start: 0.0,
            starts: 1,
            worm_diameter: 0.0,
            worm_length: 0.0,
            rim: 0.0,
            rack_teeth: 10,
            rack_height: 0.0,
            pins: 12,
            pin_circle: 60.0,
            pin_diameter: 6.0,
            eccentricity: 1.5,
            bore: Bore::default(),
            bore2: Bore::default(),
            hub_diameter: 0.0,
            hub_length: 0.0,
            holes: 0,
            hole_diameter: 0.0,
            only: 0,
            origin: [0.0; 3],
            name: String::new(),
        }
    }
}

impl Spec {
    /// Sensible starting values for a type (keeps the module and tooth count already chosen).
    pub fn defaults_for(kind: Kind) -> Spec {
        let mut s = Spec { kind, ..Spec::default() };
        match kind {
            Kind::Helical | Kind::Herringbone => s.helix = 20.0,
            Kind::Internal => {
                s.teeth = 60;
                s.face_width = 10.0;
            }
            Kind::Pair => s.teeth2 = 32,
            Kind::Planetary => {
                s.teeth = 18;
                s.teeth2 = 18;
                s.teeth3 = 54;
                s.planets = 3;
            }
            Kind::Bevel => {
                s.teeth = 20;
                s.teeth2 = 30;
                s.face_width = 12.0;
            }
            Kind::SpiralBevel => {
                s.teeth = 17;
                s.teeth2 = 34;
                s.face_width = 12.0;
                s.spiral_angle = 35.0;
            }
            Kind::Hypoid => {
                s.teeth = 12;
                s.teeth2 = 36;
                s.module = 2.0;
                s.face_width = 14.0;
                s.offset = 0.0;
                s.face_start = 20.0;
            }
            Kind::CrossedHelical => {
                s.teeth = 15;
                s.teeth2 = 25;
                s.helix = 45.0;
                s.shaft_angle = 90.0;
            }
            Kind::Worm => {
                s.teeth = 30;
                s.starts = 1;
                s.module = 2.0;
                s.face_width = 12.0;
            }
            Kind::Cycloidal => {
                s.teeth = 60;
                s.teeth2 = 8;
                s.module = 1.0;
                s.face_width = 3.0;
            }
            Kind::CycloidalDrive => {
                s.face_width = 8.0;
            }
            _ => {}
        }
        s
    }

    fn involute(&self, teeth: u32, shift: f64, helix_deg: f64) -> Involute {
        Involute {
            teeth,
            module: self.module,
            pressure_angle: self.pressure_angle.to_radians(),
            helix: helix_deg.to_radians(),
            shift,
            rack: self.rack,
            backlash: self.backlash,
            tip_shortening: 0.0,
            virtual_teeth: 0.0,
        }
    }
}

/// A placement: origin, x and z directions (unit, perpendicular) in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub z: [f64; 3],
}

impl Frame {
    pub fn at(origin: [f64; 3]) -> Frame {
        Frame { origin, x: [1.0, 0.0, 0.0], z: [0.0, 0.0, 1.0] }
    }
    pub fn y(&self) -> [f64; 3] {
        let (a, b) = (self.z, self.x);
        [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    }
    /// A point in this frame, in world coordinates.
    pub fn world(&self, p: [f64; 3]) -> [f64; 3] {
        let y = self.y();
        [0, 1, 2].map(|i| self.origin[i] + p[0] * self.x[i] + p[1] * y[i] + p[2] * self.z[i])
    }
    /// This frame moved by `d` (world).
    pub fn shifted(mut self, d: [f64; 3]) -> Frame {
        for i in 0..3 {
            self.origin[i] += d[i];
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    New,
    Join,
    Cut,
    Intersect,
}

/// One solid step, in its body's frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// The region on the plane z = `z0`, extruded `height` along +z, turned `twist` radians
    /// (counter-clockwise about +z) over that height.
    Prism { region: Region, z0: f64, height: f64, twist: f64, op: Op },
    /// Sections on planes z = h, joined in order.
    Loft { sections: Vec<(Outline, f64)>, op: Op },
    /// The outline on z = `z0` swept `height` along +z, turned `twist` radians and scaled
    /// linearly about the axis to `scale` at the far end (a cone's teeth, straight or spiral).
    Taper { outline: Outline, z0: f64, height: f64, twist: f64, scale: f64, op: Op },
    /// A closed polygon in the (radius, z) half plane, turned a full turn about z.
    Revolve { profile: Vec<V2>, op: Op },
    /// A closed polygon in the (x, z) plane (x radial) swept round z as a helix of `pitch`, from
    /// `z0` for `height`; right-handed unless `left`.
    Coil { section: Vec<V2>, pitch: f64, z0: f64, height: f64, left: bool, op: Op },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub name: String,
    pub frame: Frame,
    pub steps: Vec<Step>,
}

/// A gear outline for the 2D preview, placed in the preview's plane.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewGear {
    pub name: String,
    pub region: Region,
    pub center: V2,
    /// Angular speed against the driver's (signed); 0 for a fixed member.
    pub speed: f64,
    /// Sliding speed along +x for a rack, per driver radian.
    pub slide: f64,
    /// The pitch circle radius, drawn as a dashed circle (0: none).
    pub pitch_radius: f64,
    /// For a sliding member: the tooth pitch it repeats with (its slide wraps by this much).
    pub period: f64,
    /// Turns of `center` about the preview's origin per driver radian (0: the centre stays; 1 for
    /// a cycloidal drive's disc, whose centre goes round with the input crank).
    pub orbit: f64,
}

impl PreviewGear {
    /// Where the outline's point `q` is when the driver has turned `t` radians: turned about the
    /// centre (which itself goes round the origin when `orbit` is set), or slid along +x (a
    /// worm's axial section along +y) by whole-pitch-wrapped steps.
    pub fn place(&self, q: V2, t: f64) -> V2 {
        let c = if self.orbit != 0.0 { self.center.rot(self.orbit * t) } else { self.center };
        if self.slide != 0.0 {
            let s = if self.period > 0.0 { (self.slide * t).rem_euclid(self.period) - self.period / 2.0 } else { 0.0 };
            c + q + if self.name == "worm" { V2::new(0.0, s) } else { V2::new(s, 0.0) }
        } else {
            c + q.rot(self.speed * t)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unit {
    Mm,
    Deg,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fact {
    pub label: String,
    pub value: f64,
    pub unit: Unit,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Plan {
    pub bodies: Vec<Body>,
    pub facts: Vec<Fact>,
    pub warnings: Vec<String>,
    pub preview: Vec<PreviewGear>,
    /// What the 2D preview shows, when it is not the gears' own plane.
    pub preview_note: String,
}

impl Plan {
    fn fact(&mut self, label: impl Into<String>, value: f64, unit: Unit) {
        self.facts.push(Fact { label: label.into(), value, unit });
    }
    fn warn(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }
}

/// Validates the spec; an error says which value and why.
pub fn check(s: &Spec) -> Result<(), String> {
    let pos = |v: f64, what: &str| if v.is_finite() && v > 0.0 { Ok(()) } else { Err(format!("{what} must be greater than 0")) };
    pos(s.module, "module")?;
    pos(s.face_width, "face width")?;
    if s.kind.involute() && !(5.0..=45.0).contains(&s.pressure_angle) {
        return Err("pressure angle must be between 5° and 45°".into());
    }
    if s.helix.abs() >= 80.0 {
        return Err("helix angle must be under 80°".into());
    }
    let min = if s.kind == Kind::Cycloidal { 4 } else { 3 };
    if s.kind != Kind::CycloidalDrive && s.kind != Kind::Rack && !(min..=2000).contains(&s.teeth) {
        return Err(format!("teeth must be between {min} and 2000"));
    }
    if s.kind.is_set() && !matches!(s.kind, Kind::CycloidalDrive | Kind::Worm) && !(min..=2000).contains(&s.teeth2) && !(s.kind == Kind::Cycloidal && s.teeth2 == 0) {
        return Err(format!("mating teeth must be between {min} and 2000"));
    }
    if matches!(s.kind, Kind::Bevel | Kind::SpiralBevel | Kind::Hypoid | Kind::CrossedHelical) && !(5.0..=175.0).contains(&s.shaft_angle) {
        return Err("shaft angle must be between 5° and 175°".into());
    }
    if s.kind == Kind::Planetary && !(1..=24).contains(&s.planets) {
        return Err("planets must be between 1 and 24".into());
    }
    if s.kind == Kind::Worm && !(1..=12).contains(&s.starts) {
        return Err("worm starts must be between 1 and 12".into());
    }
    if s.kind == Kind::CycloidalDrive {
        if !(3..=200).contains(&s.pins) {
            return Err("pins must be between 3 and 200".into());
        }
        pos(s.pin_circle, "pin circle diameter")?;
        pos(s.pin_diameter, "pin diameter")?;
        pos(s.eccentricity, "eccentricity")?;
    }
    if s.rack.addendum <= 0.0 || s.rack.dedendum <= s.rack.addendum * 0.5 || s.rack.root_radius < 0.0 {
        return Err("the basic rack's addendum, dedendum and root radius don't make a tooth".into());
    }
    Ok(())
}

/// The plan for a spec.
pub fn plan(s: &Spec) -> Result<Plan, String> {
    check(s)?;
    let mut p = Plan::default();
    match s.kind {
        Kind::Spur | Kind::Helical | Kind::Herringbone => single(s, &mut p),
        Kind::Internal => internal(s, &mut p),
        Kind::Rack => rack(s, &mut p),
        Kind::Pair => pair(s, &mut p),
        Kind::Planetary => planetary(s, &mut p),
        Kind::Bevel | Kind::SpiralBevel => bevel(s, &mut p),
        Kind::Hypoid => hypoid(s, &mut p),
        Kind::CrossedHelical => crossed(s, &mut p),
        Kind::Worm => worm(s, &mut p),
        Kind::Cycloidal => cycloidal(s, &mut p),
        Kind::CycloidalDrive => drive(s, &mut p),
    }
    if p.bodies.is_empty() {
        return Err("nothing to build with these choices".into());
    }
    Ok(p)
}

fn wanted(s: &Spec, member: u8) -> bool {
    s.only == 0 || s.only == member
}

fn name(s: &Spec, base: &str) -> String {
    if s.name.trim().is_empty() { base.to_string() } else { format!("{} {base}", s.name.trim()) }
}

fn mm(v: f64) -> String {
    format!("{v:.3}").trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Notes about one involute gear's teeth.
fn tooth_warnings(p: &mut Plan, who: &str, g: &Involute, f: &ToothFlank) {
    if f.undercut {
        p.warn(format!(
            "{who}: undercut ({} teeth; about {:.1} or more avoid it at this shift, or raise the profile shift)",
            g.teeth,
            g.min_teeth_no_undercut()
        ));
    }
    if f.pointed {
        p.warn(format!("{who}: the teeth come to a point; the tip is lowered to Ø{}", mm(2.0 * f.ra)));
    } else {
        let sa = 2.0 * f.ra * f.tip_half;
        if sa < 0.25 * g.module {
            p.warn(format!("{who}: thin tooth tips ({} mm; 0.25 × module or more is usual)", mm(sa)));
        }
    }
}

fn involute_facts(p: &mut Plan, who: &str, g: &Involute, f: &ToothFlank) {
    let pre = if who.is_empty() { String::new() } else { format!("{who} ") };
    p.fact(format!("{pre}pitch diameter"), 2.0 * g.r(), Unit::Mm);
    p.fact(format!("{pre}tip diameter"), 2.0 * f.ra, Unit::Mm);
    p.fact(format!("{pre}root diameter"), 2.0 * f.rf, Unit::Mm);
    p.fact(format!("{pre}base diameter"), 2.0 * g.rb(), Unit::Mm);
    if g.helix != 0.0 {
        p.fact(format!("{pre}transverse module"), g.mt(), Unit::None);
        p.fact(format!("{pre}lead"), (TAU * g.r() / g.helix.tan()).abs(), Unit::Mm);
    }
    p.fact(format!("{pre}tooth thickness (pitch)"), g.thickness(), Unit::Mm);
    let (k, w) = g.span();
    p.fact(format!("{pre}span over {k} teeth"), w, Unit::Mm);
    let (m, dp) = g.over_pins(None);
    p.fact(format!("{pre}over two Ø{} pins", mm(dp)), m, Unit::Mm);
}

/// The steps that add a hub, bore and lightening holes to a gear body from z = 0 to `face`.
fn hub_steps(s: &Spec, bore: &Bore, root_radius: f64, face: f64, steps: &mut Vec<Step>, p: &mut Plan, who: &str) {
    let hub_r = s.hub_diameter / 2.0;
    let reach = bore.reach();
    if hub_r > 0.0 && s.hub_length > 0.0 {
        if hub_r <= reach {
            p.warn(format!("{who}: the hub is no bigger than the bore"));
        }
        steps.push(Step::Prism { region: Region { outer: vec![Outline::circle(V2::ZERO, hub_r)], holes: vec![] }, z0: face, height: s.hub_length, twist: 0.0, op: Op::Join });
    }
    let rim = root_radius - 2.0 * s.module;
    if s.holes > 0 && rim > 0.0 {
        let inner = hub_r.max(reach + s.module).max(rim * 0.25);
        if rim - inner > s.module {
            let d = if s.hole_diameter > 0.0 { s.hole_diameter } else { 0.8 * (rim - inner) };
            let at = (rim + inner) / 2.0;
            let ring = TAU * at / f64::from(s.holes);
            if d >= ring {
                p.warn(format!("{who}: the lightening holes overlap; use fewer or smaller ones"));
            }
            if at + d / 2.0 > rim || at - d / 2.0 < inner {
                p.warn(format!("{who}: lightening holes Ø{} don't fit between Ø{} and Ø{}", mm(d), mm(2.0 * inner), mm(2.0 * rim)));
            }
            let holes = lightening_holes(s.holes, d, at, PI / f64::from(s.holes));
            steps.push(Step::Prism { region: Region { outer: holes, holes: vec![] }, z0: -1.0, height: face + 2.0, twist: 0.0, op: Op::Cut });
        } else {
            p.warn(format!("{who}: no room for lightening holes"));
        }
    }
    if let Some(o) = bore.outline() {
        if reach >= root_radius {
            p.warn(format!("{who}: the bore reaches the tooth roots"));
        }
        let top = face + if hub_r > 0.0 { s.hub_length.max(0.0) } else { 0.0 };
        steps.push(Step::Prism { region: Region { outer: vec![o], holes: vec![] }, z0: -1.0, height: top + 2.0, twist: 0.0, op: Op::Cut });
    }
}

/// A cylindrical gear's tooth steps: plain, helical or herringbone, from z = 0 to `face`.
fn tooth_steps(region: Region, face: f64, twist_total: f64, herringbone: bool, op: Op) -> Vec<Step> {
    if herringbone {
        let half = face / 2.0;
        let turned = Region {
            outer: region.outer.iter().map(|o| o.rotated(twist_total / 2.0)).collect(),
            holes: region.holes.iter().map(|o| o.rotated(twist_total / 2.0)).collect(),
        };
        vec![
            Step::Prism { region, z0: 0.0, height: half, twist: twist_total / 2.0, op },
            Step::Prism { region: turned, z0: half, height: half, twist: -twist_total / 2.0, op: Op::Join },
        ]
    } else {
        vec![Step::Prism { region, z0: 0.0, height: face, twist: twist_total, op }]
    }
}

fn twist_for(g: &Involute, face: f64) -> f64 {
    face * g.helix.tan() / g.r()
}

fn single(s: &Spec, p: &mut Plan) {
    let helix = if s.kind == Kind::Spur { 0.0 } else { s.helix };
    let g = s.involute(s.teeth, s.shift, helix);
    let (outline, f) = external_outline(&g, 0.0);
    tooth_warnings(p, "gear", &g, &f);
    involute_facts(p, "", &g, &f);
    if helix != 0.0 {
        p.fact("overlap ratio", s.face_width * g.helix.abs().sin() / (PI * s.module), Unit::None);
    }
    let region = Region { outer: vec![outline], holes: vec![] };
    let mut steps = tooth_steps(region.clone(), s.face_width, twist_for(&g, s.face_width), s.kind == Kind::Herringbone, Op::New);
    hub_steps(s, &s.bore, f.rf, s.face_width, &mut steps, p, "gear");
    p.bodies.push(Body { name: name(s, &format!("Gear z{}", s.teeth)), frame: Frame::at(s.origin), steps });
    p.preview.push(PreviewGear { name: "gear".into(), region: with_bore(region, &s.bore), center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: g.r() , orbit: 0.0, period: 0.0});
}

fn with_bore(mut r: Region, b: &Bore) -> Region {
    if let Some(o) = b.outline() {
        r.holes.push(o);
    }
    r
}

fn internal(s: &Spec, p: &mut Plan) {
    let g = s.involute(s.teeth, s.shift, s.helix);
    let ring = Internal { gear: g };
    let rim = if s.rim > 0.0 { s.rim } else { 3.0 * s.module };
    let outer_r = ring.r_root() + rim;
    let teeth = ring.outline(0.0, 0.0);
    p.fact("pitch diameter", 2.0 * g.r(), Unit::Mm);
    p.fact("tip diameter (inner)", 2.0 * ring.r_tip(), Unit::Mm);
    p.fact("root diameter", 2.0 * ring.r_root(), Unit::Mm);
    p.fact("outside diameter", 2.0 * outer_r, Unit::Mm);
    p.fact("base diameter", 2.0 * g.rb(), Unit::Mm);
    // The biggest standard pinion whose tips still clear the ring's going in and out of mesh.
    let fits = |z: u32| {
        let pinion = s.involute(z, 0.0, s.helix);
        !internal_tip_interference(&pinion, pinion.ra(), &ring, &mesh_internal(&pinion, &ring, s.face_width))
    };
    if let Some(z) = (3..s.teeth).rev().find(|z| fits(*z)) {
        p.fact("largest pinion (no tip interference)", f64::from(z), Unit::None);
    }
    if ring.r_tip() < g.rb() {
        p.warn("ring: the inner tips are inside the base circle; a pinion may interfere (use more teeth or a positive shift)");
    }
    let region = Region { outer: vec![Outline::circle(V2::ZERO, outer_r)], holes: vec![teeth] };
    let steps = tooth_steps(region.clone(), s.face_width, twist_for(&g, s.face_width), false, Op::New);
    p.bodies.push(Body { name: name(s, &format!("Ring gear z{}", s.teeth)), frame: Frame::at(s.origin), steps });
    p.preview.push(PreviewGear { name: "ring".into(), region, center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: g.r() , orbit: 0.0, period: 0.0});
}

fn rack(s: &Spec, p: &mut Plan) {
    let g = s.involute(s.teeth.max(1), s.shift, s.helix);
    let below = if s.rack_height > 0.0 { s.rack_height } else { 3.0 * s.module };
    let n = s.rack_teeth.max(1);
    let o = rack_outline(&g, n, below);
    let length = PI * g.mt() * f64::from(n);
    p.fact("length", length, Unit::Mm);
    p.fact("pitch", PI * g.mt(), Unit::Mm);
    p.fact("height (tips to back)", below + (s.rack.addendum + s.shift) * s.module, Unit::Mm);
    p.fact("pitch line to back", below, Unit::Mm);
    let region = Region { outer: vec![o.clone()], holes: vec![] };
    let steps = if s.helix != 0.0 {
        // Teeth slanted across the face: a loft between the outline and the same outline slid
        // along the rack.
        let slide = s.face_width * s.helix.to_radians().tan();
        vec![Step::Loft { sections: vec![(o.clone(), 0.0), (o.translated(V2::new(slide, 0.0)), s.face_width)], op: Op::New }]
    } else {
        vec![Step::Prism { region: region.clone(), z0: 0.0, height: s.face_width, twist: 0.0, op: Op::New }]
    };
    p.bodies.push(Body { name: name(s, &format!("Rack m{}", mm(s.module))), frame: Frame::at(s.origin), steps });
    p.preview.push(PreviewGear { name: "rack".into(), region, center: V2::ZERO, speed: 0.0, slide: 1.0, pitch_radius: 0.0 , orbit: 0.0, period: PI * g.mt()});
}

fn pair(s: &Spec, p: &mut Plan) {
    let g1 = s.involute(s.teeth, s.shift, s.helix);
    let mut x2 = s.shift2;
    if s.center_distance > 0.0 {
        let probe = s.involute(s.teeth2, 0.0, -s.helix);
        match shift_sum_for_center(&g1, &probe, s.center_distance) {
            Some(sum) => {
                x2 = sum - s.shift;
                p.fact("shift sum for the centre distance", sum, Unit::None);
            }
            None => p.warn("the centre distance can't be reached by shifting"),
        }
    }
    let mut g1 = g1;
    let mut g2 = s.involute(s.teeth2, x2, -s.helix);
    // Shifted gears sit closer than their shifts push the tips out: shorten both tips by the
    // difference so the tip clearance stays the basic rack's.
    let shorten = (s.shift + x2) * s.module - (mesh_external(&g1, &g2, s.face_width).center_distance - g1.r() - g2.r());
    if shorten > 1e-9 {
        g1.tip_shortening = shorten;
        g2.tip_shortening = shorten;
        p.fact("tip shortening", shorten, Unit::Mm);
    }
    let mut m = mesh_external(&g1, &g2, s.face_width);
    let (o1, f1) = external_outline(&g1, 0.0);
    let phase2 = mesh_phase(s.teeth, 0.0, s.teeth2, 0.0);
    let (o2, f2) = external_outline(&g2, phase2);
    m.contact_ratio = contact_ratio_external(&g1, &f1, &g2, &f2, &m);
    m.total_contact_ratio = m.contact_ratio + m.overlap_ratio;
    tooth_warnings(p, "gear 1", &g1, &f1);
    tooth_warnings(p, "gear 2", &g2, &f2);
    mesh_facts(p, &m);
    p.fact("gear 2 profile shift", x2, Unit::None);
    involute_facts(p, "gear 1", &g1, &f1);
    involute_facts(p, "gear 2", &g2, &f2);
    let a = m.center_distance;
    let herring = s.kind == Kind::Herringbone;
    if wanted(s, 1) {
        let mut steps = tooth_steps(Region { outer: vec![o1.clone()], holes: vec![] }, s.face_width, twist_for(&g1, s.face_width), herring, Op::New);
        hub_steps(s, &s.bore, f1.rf, s.face_width, &mut steps, p, "gear 1");
        p.bodies.push(Body { name: name(s, &format!("Gear z{}", s.teeth)), frame: Frame::at(s.origin), steps });
    }
    if wanted(s, 2) {
        let mut steps = tooth_steps(Region { outer: vec![o2.clone()], holes: vec![] }, s.face_width, twist_for(&g2, s.face_width), herring, Op::New);
        hub_steps(s, &s.bore2, f2.rf, s.face_width, &mut steps, p, "gear 2");
        p.bodies.push(Body { name: name(s, &format!("Gear z{}", s.teeth2)), frame: Frame::at(s.origin).shifted([a, 0.0, 0.0]), steps });
    }
    p.preview.push(PreviewGear { name: "gear 1".into(), region: with_bore(Region { outer: vec![o1], holes: vec![] }, &s.bore), center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: g1.r(), orbit: 0.0, period: 0.0 });
    p.preview.push(PreviewGear {
        name: "gear 2".into(),
        region: with_bore(Region { outer: vec![o2], holes: vec![] }, &s.bore2),
        center: V2::new(a, 0.0),
        speed: -f64::from(s.teeth) / f64::from(s.teeth2),
        slide: 0.0,
        pitch_radius: g2.r(),
        orbit: 0.0, period: 0.0,
    });
}

fn mesh_facts(p: &mut Plan, m: &crate::involute::Mesh) {
    p.fact("ratio", m.ratio, Unit::None);
    p.fact("centre distance", m.center_distance, Unit::Mm);
    if (m.center_distance - m.center_distance_ref).abs() > 1e-6 {
        p.fact("reference centre distance", m.center_distance_ref, Unit::Mm);
    }
    p.fact("working pressure angle", m.working_pressure_angle.to_degrees(), Unit::Deg);
    p.fact("contact ratio", m.contact_ratio, Unit::None);
    if m.overlap_ratio > 0.0 {
        p.fact("overlap ratio", m.overlap_ratio, Unit::None);
        p.fact("total contact ratio", m.total_contact_ratio, Unit::None);
    }
    if m.contact_ratio < 1.0 {
        p.warn(format!("contact ratio {:.2} is below 1: the teeth lose contact", m.contact_ratio));
    } else if m.contact_ratio < 1.2 {
        p.warn(format!("contact ratio {:.2} is low (1.2 or more is usual)", m.contact_ratio));
    }
    if m.clearance_1 < 0.0 || m.clearance_2 < 0.0 {
        p.warn("tips hit the mating roots (negative tip clearance): shorten the tips or change the shifts");
    }
}

fn planetary(s: &Spec, p: &mut Plan) {
    let (zs, zp) = (s.teeth, s.teeth2);
    let zr = if s.teeth3 > 0 { s.teeth3 } else { zs + 2 * zp };
    let n = s.planets.max(1);
    let mut sun = s.involute(zs, s.shift, s.helix);
    let mut planet = s.involute(zp, s.shift2, -s.helix);
    let mut m_sp = mesh_external(&sun, &planet, s.face_width);
    // As for a pair: shifted gears keep the basic rack's tip clearance.
    let shorten = (s.shift + s.shift2) * s.module - (m_sp.center_distance - sun.r() - planet.r());
    if shorten > 1e-9 {
        sun.tip_shortening = shorten;
        planet.tip_shortening = shorten;
        m_sp = mesh_external(&sun, &planet, s.face_width);
    }
    // The ring's profile shift puts the planets at the same distance from it as from the sun:
    // the planet's own shift when the tooth counts close (ring = sun + 2 × planet), more or
    // less when they don't.
    let mut ring_gear = Internal { gear: s.involute(zr, 0.0, -s.helix) };
    let reach = (ring_gear.gear.r() - planet.r()) * planet.alpha_t().cos() / m_sp.center_distance;
    if zr > zp && reach > 0.0 && reach <= 1.0 {
        let x = s.shift2 + (inv(reach.acos()) - inv(planet.alpha_t())) * f64::from(zr - zp) / (2.0 * planet.pressure_angle.tan());
        if x.abs() > 1e-9 {
            ring_gear.gear.shift = x;
            p.fact("ring profile shift", x, Unit::None);
        }
        if (x - s.shift2).abs() > 0.6 {
            p.warn(format!("the ring needs a profile shift of {x:.2} to reach the planets: ring teeth = sun + 2 × planet ({}) fits without", zs + 2 * zp));
        }
    }
    // The ring's tips stay on the planets' involute: an undercut or shifted planet has less of
    // it than the ring's standard tip allows for.
    let planet_flank = external_flank(&planet);
    let clear = ring_gear.r_tip_clear(&planet, planet_flank.flank.first().map_or(planet_flank.rf, |q| q.0), &mesh_internal(&planet, &ring_gear, s.face_width));
    if clear > ring_gear.r_tip() {
        ring_gear.gear.tip_shortening = clear - ring_gear.r_tip();
    }
    let m_pr = mesh_internal(&planet, &ring_gear, s.face_width);
    if internal_tip_interference(&planet, planet_flank.ra, &ring_gear, &m_pr) {
        p.warn(format!("the planets' tips hit the ring's tips going in and out of mesh (ring − planet = {} teeth is too few)", i64::from(zr) - i64::from(zp)));
    }
    if m_pr.contact_ratio < 1.0 {
        p.warn(format!("planet–ring contact ratio {:.2} is below 1: the teeth lose contact", m_pr.contact_ratio));
    }
    if m_sp.clearance_1 < 0.0 || m_sp.clearance_2 < 0.0 || m_pr.clearance_1 < 0.0 || m_pr.clearance_2 < 0.0 {
        p.warn("tips hit the mating roots (negative tip clearance): change the shifts or the tooth counts");
    }
    let a = m_sp.center_distance;
    if (m_pr.center_distance - a).abs() > 0.01 * s.module {
        p.warn(format!(
            "the planets sit at {} from the sun but {} from the ring: use ring teeth = sun + 2 × planet ({}) or adjust shifts",
            mm(a),
            mm(m_pr.center_distance),
            zs + 2 * zp
        ));
    }
    if (zs + zr) % n != 0 {
        p.warn(format!("(sun + ring) / planets = {}/{} is not whole: the planets are spaced unevenly to mesh", zs + zr, n));
    }
    // Neighbouring planets must clear each other.
    let ra_p = planet.ra();
    if n > 1 && 2.0 * a * (PI / f64::from(n)).sin() <= 2.0 * ra_p + 0.5 * s.module {
        p.warn("neighbouring planets collide: use fewer or smaller planets");
    }
    p.fact("ratio (ring fixed, sun in, carrier out)", 1.0 + f64::from(zr) / f64::from(zs), Unit::None);
    p.fact("ratio (carrier fixed, sun in, ring out)", -f64::from(zr) / f64::from(zs), Unit::None);
    p.fact("ratio (sun fixed, ring in, carrier out)", 1.0 + f64::from(zs) / f64::from(zr), Unit::None);
    p.fact("planet centre radius", a, Unit::Mm);
    p.fact("ring teeth", f64::from(zr), Unit::None);
    p.fact("sun–planet contact ratio", m_sp.contact_ratio, Unit::None);
    p.fact("planet–ring contact ratio", m_pr.contact_ratio, Unit::None);
    let total = f64::from(zs + zr);
    let thetas: Vec<f64> = (0..n).map(|k| TAU * (f64::from(k) * total / f64::from(n)).round() / total).collect();
    let (o_sun, f_sun) = external_outline(&sun, 0.0);
    tooth_warnings(p, "sun", &sun, &f_sun);
    let mut planets = Vec::new();
    for th in &thetas {
        let ph = mesh_phase(zs, 0.0, zp, *th);
        let (o, f) = external_outline(&planet, ph);
        planets.push((*th, o, f));
    }
    if let Some((_, _, f)) = planets.first() {
        tooth_warnings(p, "planet", &planet, &f.clone());
    }
    let th0 = thetas[0];
    let ph0 = mesh_phase(zs, 0.0, zp, th0);
    let delta = wrap_period(ph0 - th0, TAU / f64::from(zp));
    let ring_phase = th0 + delta * f64::from(zp) / f64::from(zr) + PI / f64::from(zr);
    let rim = if s.rim > 0.0 { s.rim } else { 3.0 * s.module };
    let ring_teeth = ring_gear.outline(ring_phase, 0.0);
    let ring_region = Region { outer: vec![Outline::circle(V2::ZERO, ring_gear.r_root() + rim)], holes: vec![ring_teeth] };
    if wanted(s, 1) {
        let mut steps = tooth_steps(Region { outer: vec![o_sun.clone()], holes: vec![] }, s.face_width, twist_for(&sun, s.face_width), false, Op::New);
        hub_steps(s, &s.bore, f_sun.rf, s.face_width, &mut steps, p, "sun");
        p.bodies.push(Body { name: name(s, &format!("Sun z{zs}")), frame: Frame::at(s.origin), steps });
    }
    for (k, (th, o, f)) in planets.iter().enumerate() {
        let c = V2::polar(a, *th);
        if wanted(s, 2) {
            // Each planet in its own frame at its centre: its outline moved back to the origin.
            let local = o.translated(V2::ZERO);
            let mut steps = tooth_steps(Region { outer: vec![local], holes: vec![] }, s.face_width, twist_for(&planet, s.face_width), false, Op::New);
            hub_steps(s, &s.bore2, f.rf, s.face_width, &mut steps, p, "planet");
            p.bodies.push(Body { name: name(s, &format!("Planet {} z{zp}", k + 1)), frame: Frame::at(s.origin).shifted([c.x, c.y, 0.0]), steps });
        }
        p.preview.push(PreviewGear {
            name: format!("planet {}", k + 1),
            region: with_bore(Region { outer: vec![o.clone()], holes: vec![] }, &s.bore2),
            center: c,
            speed: -f64::from(zs) / f64::from(zp),
            slide: 0.0,
            pitch_radius: planet.r(),
            orbit: 0.0, period: 0.0,
        });
    }
    if wanted(s, 3) {
        let steps = tooth_steps(ring_region.clone(), s.face_width, twist_for(&ring_gear.gear, s.face_width), false, Op::New);
        p.bodies.push(Body { name: name(s, &format!("Ring z{zr}")), frame: Frame::at(s.origin), steps });
    }
    p.preview.insert(0, PreviewGear { name: "sun".into(), region: with_bore(Region { outer: vec![o_sun], holes: vec![] }, &s.bore), center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: sun.r(), orbit: 0.0, period: 0.0 });
    p.preview.push(PreviewGear { name: "ring".into(), region: ring_region, center: V2::ZERO, speed: -f64::from(zs) / f64::from(zr), slide: 0.0, pitch_radius: ring_gear.gear.r() , orbit: 0.0, period: 0.0});
}

/// A straight or spiral bevel gear's sections and blank, in a frame with the large end's pitch
/// circle on z = 0 and the cone's apex on +z.
fn bevel_member(s: &Spec, z: u32, delta: f64, spiral: f64, phase: f64, p: &mut Plan, who: &str) -> (Vec<Step>, Outline, ToothFlank, f64) {
    let m = s.module;
    let r = m * f64::from(z) / 2.0;
    let re = r / delta.sin();
    let b = s.face_width.min(re * 0.45);
    if s.face_width > re / 3.0 {
        p.warn(format!("{who}: face width over a third of the cone distance ({}) makes weak small ends", mm(re / 3.0)));
    }
    let h = re * delta.cos();
    // Tredgold's virtual spur gear on the back cone.
    let zv = f64::from(z) / delta.cos();
    // The module is the transverse one at the large end: the virtual gear's normal module is
    // m·cos β, so its transverse module (and pitch radius r / cos δ) come out right.
    let v = Involute {
        teeth: z,
        module: m * spiral.cos(),
        pressure_angle: s.pressure_angle.to_radians(),
        helix: spiral,
        shift: if who.contains('1') { s.shift } else { -s.shift },
        rack: s.rack,
        backlash: s.backlash,
        tip_shortening: 0.0,
        virtual_teeth: zv,
    };
    let flank = crate::involute::external_flank(&v);
    let rv = v.r();
    // Back-cone point (virtual radius R) to the plane z = zs by the line through the apex.
    let to_plane = |rr: f64, half: f64, zs: f64| -> (f64, f64) {
        let t = rr - rv;
        let rho = r + t * delta.cos();
        let zz = t * delta.sin();
        let lambda = (h - zs) / (h - zz);
        (rho * lambda, half * rv / r)
    };
    let section = |zs: f64| -> Outline {
        let map = |pts: &[(f64, f64)]| -> Vec<(f64, f64)> { pts.iter().map(|(rr, hh)| to_plane(*rr, *hh, zs)).collect() };
        let ff = ToothFlank {
            rf: to_plane(flank.rf, 0.0, zs).0,
            ra: to_plane(flank.ra, 0.0, zs).0,
            root_half: flank.root_half * rv / r,
            tip_half: flank.tip_half * rv / r,
            fillet: map(&flank.fillet),
            flank: map(&flank.flank),
            undercut: flank.undercut,
            pointed: flank.pointed,
        };
        teeth_outline(&ff, z, phase)
    };
    // The blank: back cone, front cone (both square to the pitch cone), flat back and front.
    let k = (re - b) / re;
    let (dc, ds) = (delta.cos(), delta.sin());
    let ha = (flank.ra - rv).max(0.0);
    let hf = (rv - flank.rf).max(0.0);
    let z_root_back = -hf * ds;
    let back = z_root_back - 0.6 * m;
    let along_back = |zz: f64| -> f64 { r + zz / ds.max(1e-6) * dc };
    let big = (ha + hf) * 3.0 + m;
    let small_pitch = V2::new(k * r, h * (1.0 - k));
    let front_root = small_pitch + V2::new(-hf * k * dc, -hf * k * ds);
    let mut profile = vec![V2::new(0.0, back)];
    let rb_back = if ds > 1e-3 { along_back(back).max(0.0) } else { r + big };
    profile.push(V2::new(rb_back.max(0.0), back));
    profile.push(V2::new(r + big * dc, big * ds));
    profile.push(small_pitch + V2::new(big * dc, big * ds));
    profile.push(front_root);
    profile.push(V2::new(0.0, front_root.y));
    // Sections on the planes through the large and small ends' pitch circles. The ends are
    // flat (square to the axis) rather than trimmed to the back and front cones: the kernel
    // refuses that intersection where the cones graze the root fillets.
    let _ = (&profile, back);
    let z_a = 0.0;
    let z_b = small_pitch.y;
    // One sweep towards the apex: scaled linearly (every section is the large end's, shrunk
    // about the axis towards the apex) and, for a spiral bevel, turned as it goes, which
    // follows the logarithmic spiral of constant spiral angle between the ends.
    let rm = re - b / 2.0;
    let turn = |zs: f64| if spiral.abs() > 1e-6 { spiral.tan() * (((h - zs) / dc) / rm).ln() / ds } else { 0.0 };
    let steps = vec![Step::Taper {
        outline: section(z_a).rotated(turn(z_a)),
        z0: z_a,
        height: z_b - z_a,
        twist: turn(z_b) - turn(z_a),
        scale: (h - z_b) / (h - z_a),
        op: Op::New,
    }];
    p.fact(format!("{who} pitch cone angle"), delta.to_degrees(), Unit::Deg);
    p.fact(format!("{who} outer pitch diameter"), 2.0 * r, Unit::Mm);
    p.fact(format!("{who} outer tip diameter"), 2.0 * (r + ha * dc), Unit::Mm);
    p.fact(format!("{who} cone distance"), re, Unit::Mm);
    p.fact(format!("{who} virtual teeth"), zv, Unit::None);
    tooth_warnings(p, who, &v, &flank);
    let preview = external_outline(&v, phase).0;
    (steps, preview, flank, 0.0)
}

fn bevel(s: &Spec, p: &mut Plan) {
    let sigma = s.shaft_angle.to_radians();
    let (z1, z2) = (f64::from(s.teeth), f64::from(s.teeth2));
    let d1 = sigma.sin().atan2(z2 / z1 + sigma.cos());
    let d2 = sigma - d1;
    let spiral = if s.kind == Kind::SpiralBevel { s.spiral_angle.to_radians() } else { 0.0 };
    if d1.to_degrees() > 80.0 || d2.to_degrees() > 80.0 {
        p.warn("a pitch cone angle over 80° (a crown gear) is outside what this bevel builder does well");
    }
    p.fact("ratio", z2 / z1, Unit::None);
    p.fact("shaft angle", s.shaft_angle, Unit::Deg);
    if spiral != 0.0 {
        p.fact("mean spiral angle", s.spiral_angle, Unit::Deg);
    }
    let r1 = s.module * z1 / 2.0;
    let r2 = s.module * z2 / 2.0;
    let re = r1 / d1.sin();
    let h1 = re * d1.cos();
    let h2 = re * d2.cos();
    // Gear 1 along +z, apex at (0, 0, h1); gear 2's axis through the same apex at the shaft
    // angle, in the xz plane.
    let (st1, pv1, _f1, back1) = bevel_member(s, s.teeth, d1, spiral, 0.0, p, "gear 1");
    let ph2 = PI / z2 + PI;
    let (st2, pv2, f2, back2) = bevel_member(s, s.teeth2, d2, -spiral, ph2, p, "gear 2");
    let apex = [s.origin[0], s.origin[1], s.origin[2] + h1];
    // Gear 2's axis points from its large end towards the apex: from the apex, back along the
    // direction at the shaft angle from gear 1's axis.
    let ax2 = [-sigma.sin(), 0.0, sigma.cos()];
    let origin2 = [apex[0] - ax2[0] * h2, apex[1] - ax2[1] * h2, apex[2] - ax2[2] * h2];
    let frame2 = Frame { origin: origin2, x: [sigma.cos(), 0.0, sigma.sin()], z: ax2 };
    if wanted(s, 1) {
        let mut steps = st1;
        bevel_hub(s, &s.bore, r1 * 0.6, back1, &mut steps, p, "gear 1");
        p.bodies.push(Body { name: name(s, &format!("Bevel z{}", s.teeth)), frame: Frame::at(s.origin), steps });
    }
    if wanted(s, 2) {
        let mut steps = st2;
        bevel_hub(s, &s.bore2, r2 * 0.6, back2, &mut steps, p, "gear 2");
        p.bodies.push(Body { name: name(s, &format!("Bevel z{}", s.teeth2)), frame: frame2, steps });
    }
    let _ = f2;
    // The preview: the two virtual (back-cone) spur gears in mesh.
    let (rv1, rv2) = (r1 / d1.cos(), r2 / d2.cos());
    p.preview_note = "back-cone (Tredgold) development at the large end".into();
    p.preview.push(PreviewGear { name: "gear 1".into(), region: Region { outer: vec![pv1], holes: vec![] }, center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: rv1, orbit: 0.0, period: 0.0 });
    let pv2 = pv2.rotated(-PI);
    p.preview.push(PreviewGear {
        name: "gear 2".into(),
        region: Region { outer: vec![pv2], holes: vec![] },
        center: V2::new(rv1 + rv2, 0.0),
        speed: -rv1 / rv2,
        slide: 0.0,
        pitch_radius: rv2,
        orbit: 0.0, period: 0.0,
    });
}

/// A bevel's hub on its back face and its bore.
fn bevel_hub(s: &Spec, bore: &Bore, max_r: f64, back: f64, steps: &mut Vec<Step>, p: &mut Plan, who: &str) {
    let hub_r = (s.hub_diameter / 2.0).min(max_r);
    if hub_r > 0.0 && s.hub_length > 0.0 {
        steps.push(Step::Prism { region: Region { outer: vec![Outline::circle(V2::ZERO, hub_r)], holes: vec![] }, z0: back - s.hub_length, height: s.hub_length + 0.01, twist: 0.0, op: Op::Join });
    }
    if let Some(o) = bore.outline() {
        if bore.reach() > max_r {
            p.warn(format!("{who}: the bore is large for this gear"));
        }
        let low = back - s.hub_length.max(0.0) - 1.0;
        steps.push(Step::Prism { region: Region { outer: vec![o], holes: vec![] }, z0: low, height: 1e4_f64.min(10.0 * s.module * f64::from(s.teeth.max(s.teeth2)) + 100.0), twist: 0.0, op: Op::Cut });
    }
}

/// Crossed helical and hypoid geometry share frames: gear 1 on +z at the origin, gear 2's
/// centre `e` along +x with its axis turned `sigma` about x.
fn crossed_frame2(origin: [f64; 3], e: f64, sigma: f64) -> Frame {
    Frame { origin: [origin[0] + e, origin[1], origin[2]], x: [-1.0, 0.0, 0.0], z: [0.0, sigma.sin(), sigma.cos()] }
}

fn crossed(s: &Spec, p: &mut Plan) {
    let sigma = s.shaft_angle.to_radians();
    let b1 = s.helix.to_radians();
    let b2 = sigma - b1;
    if b2.abs() >= 80f64.to_radians() || b1.abs() >= 80f64.to_radians() {
        p.warn("a helix angle near 90° makes impractical gears; balance gear 1's helix against the shaft angle");
    }
    let g1 = s.involute(s.teeth, s.shift, b1.to_degrees());
    let g2 = s.involute(s.teeth2, s.shift2, b2.to_degrees());
    let e = g1.r() + g2.r() + (s.shift + s.shift2) * s.module;
    let (o1, f1) = external_outline(&g1, 0.0);
    let (o2, f2) = external_outline(&g2, PI / f64::from(s.teeth2));
    tooth_warnings(p, "gear 1", &g1, &f1);
    tooth_warnings(p, "gear 2", &g2, &f2);
    p.fact("ratio", f64::from(s.teeth2) / f64::from(s.teeth), Unit::None);
    p.fact("centre distance", e, Unit::Mm);
    p.fact("gear 1 helix angle", b1.to_degrees(), Unit::Deg);
    p.fact("gear 2 helix angle", b2.to_degrees(), Unit::Deg);
    involute_facts(p, "gear 1", &g1, &f1);
    involute_facts(p, "gear 2", &g2, &f2);
    let face = s.face_width;
    if wanted(s, 1) {
        let mut steps = vec![Step::Prism { region: Region { outer: vec![o1.rotated(-twist_for(&g1, face) / 2.0)], holes: vec![] }, z0: -face / 2.0, height: face, twist: twist_for(&g1, face), op: Op::New }];
        hub_steps_centered(s, &s.bore, f1.rf, face, &mut steps, p, "gear 1");
        p.bodies.push(Body { name: name(s, &format!("Crossed helical z{}", s.teeth)), frame: Frame::at(s.origin), steps });
    }
    if wanted(s, 2) {
        let mut steps = vec![Step::Prism { region: Region { outer: vec![o2.rotated(-twist_for(&g2, face) / 2.0)], holes: vec![] }, z0: -face / 2.0, height: face, twist: twist_for(&g2, face), op: Op::New }];
        hub_steps_centered(s, &s.bore2, f2.rf, face, &mut steps, p, "gear 2");
        p.bodies.push(Body { name: name(s, &format!("Crossed helical z{}", s.teeth2)), frame: crossed_frame2(s.origin, e, sigma), steps });
    }
    p.preview_note = "each gear's transverse section".into();
    side_by_side(p, o1, g1.r(), o2, g2.r(), f64::from(s.teeth) / f64::from(s.teeth2));
}

/// Two gears drawn apart (their axes don't share a plane), each turning at its own speed.
fn side_by_side(p: &mut Plan, o1: Outline, r1: f64, o2: Outline, r2: f64, speed2: f64) {
    let gap = 0.3 * (r1 + r2);
    p.preview.push(PreviewGear { name: "gear 1".into(), region: Region { outer: vec![o1], holes: vec![] }, center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: r1, orbit: 0.0, period: 0.0 });
    p.preview.push(PreviewGear {
        name: "gear 2".into(),
        region: Region { outer: vec![o2], holes: vec![] },
        center: V2::new(r1 + r2 + gap + 0.0 * FRAC_PI_2, 0.0),
        speed: -speed2,
        slide: 0.0,
        pitch_radius: r2,
        orbit: 0.0, period: 0.0,
    });
}

/// Like [`hub_steps`] for a gear whose face runs from −face/2 to face/2.
fn hub_steps_centered(s: &Spec, bore: &Bore, rf: f64, face: f64, steps: &mut Vec<Step>, p: &mut Plan, who: &str) {
    let mut local = Vec::new();
    hub_steps(s, bore, rf, face, &mut local, p, who);
    for st in local {
        steps.push(match st {
            Step::Prism { region, z0, height, twist, op } => Step::Prism { region, z0: z0 - face / 2.0, height, twist, op },
            other => other,
        });
    }
}

/// Hypoid (offset bevel) gears on hyperboloid pitch surfaces with straight, skewed teeth: each
/// section is the throat section turned and scaled, so every tooth line is a ruling.
fn hypoid(s: &Spec, p: &mut Plan) {
    let sigma = s.shaft_angle.to_radians();
    let (z1, z2) = (f64::from(s.teeth), f64::from(s.teeth2));
    let mn = s.module;
    let span = |g1: f64| -> f64 { mn / 2.0 * (z1 / g1.cos() + z2 / (sigma - g1).cos()) };
    let lo = 1e-3_f64.max(sigma - 80f64.to_radians());
    let hi = (sigma - 1e-3).min(80f64.to_radians());
    // The smallest offset these tooth counts allow, and the angle at it.
    let (mut a, mut b) = (lo, hi);
    for _ in 0..100 {
        let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
        if span(m1) < span(m2) { b = m2 } else { a = m1 }
    }
    let g_min = 0.5 * (a + b);
    let e_min = span(g_min);
    let g1 = if s.offset > 0.0 {
        if s.offset < e_min {
            p.warn(format!("the offset must be at least {} for these teeth; using that", mm(e_min)));
            g_min
        } else {
            // The solution with the pinion's larger spiral angle.
            let (mut lo2, mut hi2) = (g_min, hi);
            if span(hi2) < s.offset {
                p.warn("the offset is too large for these teeth; using the largest that works");
                hi2
            } else {
                for _ in 0..100 {
                    let mid = 0.5 * (lo2 + hi2);
                    if span(mid) < s.offset { lo2 = mid } else { hi2 = mid }
                }
                0.5 * (lo2 + hi2)
            }
        }
    } else {
        sigma / 2.0
    };
    let g2 = sigma - g1;
    let gear1 = s.involute(s.teeth, s.shift, g1.to_degrees());
    let gear2 = s.involute(s.teeth2, -s.shift, g2.to_degrees());
    let (r1, r2) = (gear1.r(), gear2.r());
    let e = r1 + r2 + (gear1.shift + gear2.shift) * mn;
    let (o1, f1) = external_outline(&gear1, 0.0);
    let (o2, f2) = external_outline(&gear2, PI / z2);
    tooth_warnings(p, "pinion", &gear1, &f1);
    tooth_warnings(p, "gear", &gear2, &f2);
    p.fact("ratio", z2 / z1, Unit::None);
    p.fact("offset", e, Unit::Mm);
    p.fact("smallest offset for these teeth", e_min, Unit::Mm);
    p.fact("pinion spiral angle (throat)", g1.to_degrees(), Unit::Deg);
    p.fact("gear spiral angle (throat)", g2.to_degrees(), Unit::Deg);
    p.fact("pinion throat diameter", 2.0 * r1, Unit::Mm);
    p.fact("gear throat diameter", 2.0 * r2, Unit::Mm);
    // Along the line of contact (a common ruling) from u0 to u1.
    let (u0, u1) = (s.face_start, s.face_start + s.face_width);
    let member = |o: &Outline, r0: f64, g: f64| -> Vec<Step> {
        let k = g.tan() / r0;
        let section = |z: f64| -> (Outline, f64) {
            let t = z * k;
            (o.map(&|q: V2| V2::new(q.x - t * q.y, q.y + t * q.x), false), z)
        };
        let (za, zb) = (u0 * g.cos(), u1 * g.cos());
        vec![Step::Loft { sections: vec![section(za), section(zb)], op: Op::New }]
    };
    let mut flare = |who: &str, r0: f64, g: f64| {
        let k = g.tan() / r0;
        let at = |u: f64| r0 * (1.0 + (u * g.cos() * k).powi(2)).sqrt();
        p.fact(format!("{who} pitch diameter at the face"), 2.0 * at(u0), Unit::Mm);
        p.fact(format!("{who} pitch diameter at the face end"), 2.0 * at(u1), Unit::Mm);
    };
    flare("pinion", r1, g1);
    flare("gear", r2, g2);
    if wanted(s, 1) {
        let mut steps = member(&o1, r1, g1);
        if let Some(b) = s.bore.outline() {
            steps.push(Step::Prism { region: Region { outer: vec![b], holes: vec![] }, z0: u0.min(u1) * g1.cos() - 1.0, height: (u1 - u0).abs() * g1.cos() + 2.0, twist: 0.0, op: Op::Cut });
        }
        p.bodies.push(Body { name: name(s, &format!("Hypoid pinion z{}", s.teeth)), frame: Frame::at(s.origin), steps });
    }
    if wanted(s, 2) {
        let mut steps = member(&o2, r2, g2);
        if let Some(b) = s.bore2.outline() {
            steps.push(Step::Prism { region: Region { outer: vec![b], holes: vec![] }, z0: u0.min(u1) * g2.cos() - 1.0, height: (u1 - u0).abs() * g2.cos() + 2.0, twist: 0.0, op: Op::Cut });
        }
        p.bodies.push(Body { name: name(s, &format!("Hypoid gear z{}", s.teeth2)), frame: crossed_frame2(s.origin, e, sigma), steps });
    }
    let _ = (f1, f2);
    p.preview_note = "throat sections (the gears' axes cross without meeting)".into();
    side_by_side(p, o1, r1, o2, r2, z1 / z2);
}

fn worm(s: &Spec, p: &mut Plan) {
    let m = s.module;
    let z1 = s.starts.max(1);
    let zw = s.teeth;
    let d1 = if s.worm_diameter > 0.0 { s.worm_diameter } else { 10.0 * m };
    let lead = PI * m * f64::from(z1);
    let gamma = (lead / (PI * d1)).atan();
    let an = s.pressure_angle.to_radians();
    let ax = (an.tan() / gamma.cos()).atan();
    let ha = s.rack.addendum * m;
    let hf = s.rack.dedendum * m;
    let (r1, ra1, rf1) = (d1 / 2.0, d1 / 2.0 + ha, d1 / 2.0 - hf);
    // The wheel: a helical gear at the lead angle, transverse module = the worm's axial module.
    let wheel = Involute {
        teeth: zw,
        module: m * gamma.cos(),
        pressure_angle: an,
        helix: gamma,
        shift: s.shift / gamma.cos(),
        rack: s.rack,
        backlash: s.backlash,
        tip_shortening: 0.0,
        virtual_teeth: 0.0,
    };
    let a = r1 + wheel.r() + s.shift * m;
    let face = s.face_width.min(2.0 * (ra1 * ra1 - (r1 - m) * (r1 - m)).max(0.0).sqrt() + m).max(s.face_width.min(d1));
    let length = if s.worm_length > 0.0 { s.worm_length } else { (2.0 * ((wheel.ra() + m).powi(2) - (wheel.r() - m).powi(2)).max(0.0).sqrt() + lead).max(4.0 * PI * m) };
    p.fact("ratio", f64::from(zw) / f64::from(z1), Unit::None);
    p.fact("centre distance", a, Unit::Mm);
    p.fact("lead angle", gamma.to_degrees(), Unit::Deg);
    p.fact("lead", lead, Unit::Mm);
    p.fact("axial pitch", PI * m, Unit::Mm);
    p.fact("worm pitch diameter", d1, Unit::Mm);
    p.fact("worm tip diameter", 2.0 * ra1, Unit::Mm);
    p.fact("worm root diameter", 2.0 * rf1, Unit::Mm);
    p.fact("worm length", length, Unit::Mm);
    p.fact("wheel pitch diameter", 2.0 * wheel.r(), Unit::Mm);
    p.fact("wheel tip diameter", 2.0 * wheel.ra(), Unit::Mm);
    p.fact("diameter quotient q", d1 / m, Unit::None);
    if gamma.to_degrees() < 5.0 {
        p.fact("self-locking (likely, lead angle under ~5°)", 1.0, Unit::None);
    }
    if gamma.to_degrees() > 30.0 {
        p.warn("lead angle over 30°: use a larger worm diameter or fewer starts");
    }
    if rf1 <= 0.0 {
        p.warn("the worm's root is at or below its axis; increase the worm diameter");
    }
    let (wo, wf) = external_outline(&wheel, PI / f64::from(zw));
    tooth_warnings(p, "wheel", &wheel, &wf);
    if wanted(s, 1) {
        let mut steps = vec![Step::Prism { region: Region { outer: vec![wo.rotated(-twist_for(&wheel, face) / 2.0)], holes: vec![] }, z0: -face / 2.0, height: face, twist: twist_for(&wheel, face), op: Op::New }];
        hub_steps_centered(s, &s.bore, wf.rf, face, &mut steps, p, "wheel");
        p.bodies.push(Body { name: name(s, &format!("Worm wheel z{zw}")), frame: Frame::at(s.origin), steps });
    }
    if wanted(s, 2) {
        let s_ax = PI * m / 2.0 - s.backlash / 2.0;
        let half = |rr: f64| s_ax / 2.0 - (rr - r1) * ax.tan();
        let section_at = |z: f64| -> Vec<V2> {
            let inner = rf1 - 0.05 * m;
            vec![V2::new(inner, z - half(inner)), V2::new(ra1, z - half(ra1).max(0.02 * m)), V2::new(ra1, z + half(ra1).max(0.02 * m)), V2::new(inner, z + half(inner))]
        };
        let mut steps = vec![Step::Prism { region: Region { outer: vec![Outline::circle(V2::ZERO, rf1)], holes: vec![] }, z0: -length / 2.0, height: length, twist: 0.0, op: Op::New }];
        for k in 0..z1 {
            let z = -length / 2.0 - lead + PI * m * f64::from(k);
            steps.push(Step::Coil { section: section_at(z), pitch: lead, z0: z, height: length + 2.0 * lead, left: s.helix < 0.0, op: Op::Join });
        }
        steps.push(Step::Prism { region: Region { outer: vec![Outline::circle(V2::ZERO, ra1 + 0.01)], holes: vec![] }, z0: -length / 2.0, height: length, twist: 0.0, op: Op::Intersect });
        if let Some(b) = s.bore2.outline() {
            steps.push(Step::Prism { region: Region { outer: vec![b], holes: vec![] }, z0: -length / 2.0 - 1.0, height: length + 2.0, twist: 0.0, op: Op::Cut });
        }
        p.bodies.push(Body { name: name(s, &format!("Worm {z1}-start")), frame: crossed_frame2(s.origin, a, FRAC_PI_2), steps });
    }
    // The preview: the wheel and the worm's axial section (a rack) in the wheel's mid plane.
    p.preview_note = "wheel mid plane, worm in axial section".into();
    let rack_g = Involute { teeth: 1, module: m, pressure_angle: ax, helix: 0.0, shift: 0.0, rack: s.rack, backlash: s.backlash, tip_shortening: 0.0, virtual_teeth: 0.0 };
    let n = (length / (PI * m)).ceil().max(1.0) as u32;
    let rack_o = rack_outline(&rack_g, n, ha + 0.5 * m).map(&|q: V2| V2::new(-q.y, q.x - PI * m * f64::from(n) / 2.0 + PI * m / 4.0), false);
    p.preview.push(PreviewGear { name: "wheel".into(), region: Region { outer: vec![wo], holes: vec![] }, center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: wheel.r(), orbit: 0.0, period: 0.0 });
    p.preview.push(PreviewGear {
        name: "worm".into(),
        region: Region { outer: vec![rack_o], holes: vec![] },
        center: V2::new(a, 0.0),
        speed: 0.0,
        slide: wheel.r(),
        pitch_radius: 0.0,
        orbit: 0.0, period: PI * m,
    });
}

fn cycloidal(s: &Spec, p: &mut Plan) {
    let m = s.module;
    let (z1, z2) = (s.teeth, s.teeth2);
    let r1 = m * f64::from(z1) / 2.0;
    let r2 = m * f64::from(z2) / 2.0;
    let single_only = z2 == 0;
    let c1 = Cycloidal {
        teeth: z1,
        module: m,
        rolling_radius_addendum: if single_only { r1 / 2.0 } else { r2 / 2.0 },
        rolling_radius_dedendum: r1 / 2.0,
        addendum: 1.0 + 0.0 * s.rack.addendum,
        dedendum: s.rack.dedendum.max(1.2),
        backlash: s.backlash,
    };
    let (o1, f1) = cycloidal_outline(&c1, 0.0);
    p.fact("gear 1 pitch diameter", 2.0 * r1, Unit::Mm);
    p.fact("gear 1 tip diameter", 2.0 * f1.ra, Unit::Mm);
    if wanted(s, 1) {
        let mut steps = vec![Step::Prism { region: Region { outer: vec![o1.clone()], holes: vec![] }, z0: 0.0, height: s.face_width, twist: 0.0, op: Op::New }];
        hub_steps(s, &s.bore, f1.rf, s.face_width, &mut steps, p, "gear 1");
        p.bodies.push(Body { name: name(s, &format!("Cycloidal z{z1}")), frame: Frame::at(s.origin), steps });
    }
    p.preview.push(PreviewGear { name: "gear 1".into(), region: with_bore(Region { outer: vec![o1], holes: vec![] }, &s.bore), center: V2::ZERO, speed: 1.0, slide: 0.0, pitch_radius: r1, orbit: 0.0, period: 0.0 });
    if !single_only {
        let c2 = Cycloidal { teeth: z2, module: m, rolling_radius_addendum: r1 / 2.0, rolling_radius_dedendum: r2 / 2.0, addendum: 1.0, dedendum: s.rack.dedendum.max(1.2), backlash: s.backlash };
        let ph2 = mesh_phase(z1, 0.0, z2, 0.0);
        let (o2, f2) = cycloidal_outline(&c2, ph2);
        p.fact("ratio", f64::from(z1) / f64::from(z2), Unit::None);
        p.fact("centre distance", r1 + r2, Unit::Mm);
        p.fact("gear 2 pitch diameter", 2.0 * r2, Unit::Mm);
        p.fact("gear 2 tip diameter", 2.0 * f2.ra, Unit::Mm);
        if wanted(s, 2) {
            let mut steps = vec![Step::Prism { region: Region { outer: vec![o2.clone()], holes: vec![] }, z0: 0.0, height: s.face_width, twist: 0.0, op: Op::New }];
            hub_steps(s, &s.bore2, f2.rf, s.face_width, &mut steps, p, "gear 2");
            p.bodies.push(Body { name: name(s, &format!("Cycloidal z{z2}")), frame: Frame::at(s.origin).shifted([r1 + r2, 0.0, 0.0]), steps });
        }
        p.preview.push(PreviewGear {
            name: "gear 2".into(),
            region: with_bore(Region { outer: vec![o2], holes: vec![] }, &s.bore2),
            center: V2::new(r1 + r2, 0.0),
            speed: -f64::from(z1) / f64::from(z2),
            slide: 0.0,
            pitch_radius: r2,
            orbit: 0.0, period: 0.0,
        });
    }
}

fn drive(s: &Spec, p: &mut Plan) {
    let d = CycloidalDrive { pins: s.pins, pin_circle_radius: s.pin_circle / 2.0, pin_radius: s.pin_diameter / 2.0, eccentricity: s.eccentricity, clearance: s.backlash / 2.0 };
    let disc = d.disc();
    let n = f64::from(s.pins);
    p.fact("reduction", n - 1.0, Unit::None);
    p.fact("lobes", n - 1.0, Unit::None);
    p.fact("largest eccentricity without cusps", d.max_eccentricity(), Unit::Mm);
    if s.eccentricity >= d.max_eccentricity() {
        p.warn(format!("eccentricity {} at or over {}: the disc profile loops", mm(s.eccentricity), mm(d.max_eccentricity())));
    }
    if d.undercut() {
        p.warn("the pins are bigger than the lobes' tightest curve: the disc profile doubles back and the pins cut into it (smaller pins, less eccentricity or a bigger pin circle)");
    }
    let pitch_gap = TAU * d.pin_circle_radius / n - s.pin_diameter;
    if pitch_gap <= 0.0 {
        p.warn("the pins overlap: fewer pins, smaller pins or a bigger pin circle");
    }
    // The disc on its eccentric (shifted e along +x), the pins round the origin.
    let mut holes = Vec::new();
    if let Some(b) = s.bore.outline() {
        holes.push(b);
    }
    if s.holes > 0 {
        // Output holes: the output pins' diameter plus twice the eccentricity.
        let pin = if s.hole_diameter > 0.0 { s.hole_diameter } else { 0.15 * s.pin_circle };
        let at = 0.5 * (d.pin_circle_radius - s.pin_diameter) * 0.75;
        holes.extend(lightening_holes(s.holes, pin + 2.0 * s.eccentricity, at, 0.0));
        p.fact("output hole diameter", pin + 2.0 * s.eccentricity, Unit::Mm);
        p.fact("output pin circle", 2.0 * at, Unit::Mm);
    }
    let region = Region { outer: vec![disc.clone()], holes: holes.clone() };
    if wanted(s, 1) {
        p.bodies.push(Body {
            name: name(s, &format!("Cycloid disc {}-lobe", s.pins - 1)),
            frame: Frame::at(s.origin).shifted([s.eccentricity, 0.0, 0.0]),
            steps: vec![Step::Prism { region: region.clone(), z0: 0.0, height: s.face_width, twist: 0.0, op: Op::New }],
        });
    }
    let pins: Vec<Outline> = d.pin_centres().into_iter().map(|c| Outline::circle(c, d.pin_radius)).collect();
    if wanted(s, 2) {
        p.bodies.push(Body {
            name: name(s, &format!("Ring pins ×{}", s.pins)),
            frame: Frame::at(s.origin),
            steps: vec![Step::Prism { region: Region { outer: pins.clone(), holes: vec![] }, z0: 0.0, height: s.face_width, twist: 0.0, op: Op::New }],
        });
    }
    // The input crank carries the disc's centre round the pin circle's; the pins turn the disc
    // back one lobe per turn of the crank.
    p.preview.push(PreviewGear { name: "disc".into(), region, center: V2::new(s.eccentricity, 0.0), speed: -1.0 / (n - 1.0), slide: 0.0, pitch_radius: 0.0, orbit: 1.0, period: 0.0 });
    p.preview.push(PreviewGear { name: "pins".into(), region: Region { outer: pins, holes: vec![] }, center: V2::ZERO, speed: 0.0, slide: 0.0, pitch_radius: 0.0, orbit: 0.0, period: 0.0 });
    p.preview_note = "the disc turns slowly backwards while its centre goes round with the input".into();
}
