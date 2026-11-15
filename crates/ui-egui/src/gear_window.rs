//! The Gear Generator window (`gear.create`'s ribbon button).
//!
//! Axis-first: the Axes step at the top holds the two shafts (picked in the model or entered;
//! axis 1 starts as world Z). The window says how they lie, offers the gear types that fit, works
//! out what the axes decide (centre distance, shaft angle, offset, and the module or teeth left
//! free; shown read-only) and builds the gears on them, while the ratio, teeth and module that
//! stay free are the user's to drive.
//!
//! Each member of a set (gear 1, gear 2, sun, planets, ring, worm, wheel…) has one colour: its
//! outline in the 2D preview, its name there, the marker on its settings section, and its body in
//! the view's preview. Pointing at a member's section (or changing a number in it) lights the
//! member in the 2D preview; pointing at a member there lights its section.
//!
//! The 2D preview turns every member with the motion the gear crate's mesh tests prove
//! ([`solidcraft_gear::PreviewGear::place`]). With Turn off, or after Reset, it shows the pose
//! the bodies are built in, which is what the view's preview shows. The view's preview is only
//! ever the one computed for the numbers in the window: while they change, and until the new one
//! is ready, it is taken down and the window says "updating".
//!
//! Opened with a gear body (or anything of a set) selected, or from the set's history folder,
//! the window is filled from what the set was made with and its main button is Update:
//! `gear.update` regenerates the set where it stands.
//!
//! The settings column is built from one function per section ([`params`] lists them): a new
//! `Spec` field gets its controls by adding rows to the section it belongs to, or a new section
//! function to that list.

use egui::{Color32, Pos2, Sense, Stroke, Ui, Vec2, pos2, vec2};
use serde_json::{Value, json};
use solidcraft_gear::axes::{self, Axis, Derived, Drive, Relation, Solved};
use solidcraft_gear::geom::{Region, V2};
use solidcraft_gear::hub::{BEARINGS, Bore, BoreKind, Chamfer, Hardware, Shaft, ShaftEnd};
use solidcraft_gear::design::{Goal, Prefer, Suggestion};
use solidcraft_gear::involute::BasicRack;
use solidcraft_gear::mesh2d::{MeshResult, Verdict};
use solidcraft_gear::plan::{Axes, Kind, Plan, Spec, Unit};
use solidcraft_gear::pulley::{BELTS, belt_by_id};
use solidcraft_gear::spline::SplineStandard;
use solidcraft_gear::sprocket::{CHAINS, CUSTOM, chain_by_id};

use crate::SolveApp;
use crate::ui_kit;

/// How long the numbers must stand still before the view's preview is computed for them (ms):
/// a dragged value would otherwise start a build of the whole set at every step.
const SETTLE_MS: f64 = 250.0;
/// How long a member stays lit in the 2D preview after one of its numbers changed (ms).
const EDIT_GLOW_MS: f64 = 1200.0;

/// A picked or entered shaft axis, and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slot {
    pub(crate) axis: Axis,
    pub(crate) label: String,
}

/// The Axes step: the two shafts and what drives the solve on them.
#[derive(Clone, Debug, PartialEq)]
pub struct AxesStep {
    pub(crate) slots: [Option<Slot>; 2],
    /// Choose the tooth counts to give `ratio` (gear 2 ÷ gear 1), instead of the counts typed.
    pub(crate) use_ratio: bool,
    pub(crate) ratio: f64,
    /// Choose a standard module to suit the axes, instead of the one typed.
    pub(crate) auto_module: bool,
    /// Bevels: size by the outer cone distance instead of the module.
    pub(crate) use_cone: bool,
    pub(crate) cone: f64,
    pub(crate) flip1: bool,
    pub(crate) flip2: bool,
    pub(crate) swap: bool,
    pub(crate) along: f64,
}

impl Default for AxesStep {
    fn default() -> Self {
        AxesStep {
            slots: [Some(Slot { axis: Axis::Z, label: "world Z".into() }), None],
            use_ratio: false,
            ratio: 2.0,
            auto_module: true,
            use_cone: false,
            cone: 40.0,
            flip1: false,
            flip2: false,
            swap: false,
            along: 0.0,
        }
    }
}

impl AxesStep {
    /// The shafts a Simple-mode drawing gives: the set sized by the solver is placed on them
    /// as it is (no ratio, module or flips chosen here).
    pub(crate) fn sketched(a1: Axis, a2: Option<Axis>) -> AxesStep {
        let slot = |axis: Axis| Slot { axis, label: "drawn".into() };
        AxesStep { slots: [Some(slot(a1)), a2.map(slot)], use_ratio: false, auto_module: false, use_cone: false, ..AxesStep::default() }
    }

    /// Axis 1, or axis 2 once axis 1 is set.
    fn axis(&self, i: usize) -> Option<Axis> {
        self.slots[0].as_ref()?;
        self.slots.get(i)?.as_ref().map(|s| s.axis)
    }

    /// How the two axes lie, as the solver sees them (swapped when asked).
    fn relation(&self) -> Option<Relation> {
        let (a1, a2) = (self.axis(0)?, self.axis(1)?);
        Some(if self.swap { axes::relate(&a2, &a1) } else { axes::relate(&a1, &a2) })
    }

    fn drive(&self) -> Drive {
        Drive {
            ratio: self.use_ratio.then_some(self.ratio),
            module_free: self.auto_module,
            teeth_free: self.use_ratio,
            teeth2_free: self.use_ratio,
            starts_free: false,
            cone_distance: self.use_cone.then_some(self.cone),
            flip1: self.flip1,
            flip2: self.flip2,
            swap: self.swap,
            along: self.along,
        }
    }

    /// The step as kept with a set, for the window to be filled from again.
    fn to_json(&self) -> Value {
        let slot = |s: &Option<Slot>| s.as_ref().map(|s| json!({"origin": s.axis.origin, "dir": s.axis.dir, "label": s.label}));
        json!({
            "slots": [slot(&self.slots[0]), slot(&self.slots[1])],
            "use_ratio": self.use_ratio,
            "ratio": self.ratio,
            "auto_module": self.auto_module,
            "use_cone": self.use_cone,
            "cone": self.cone,
            "flip1": self.flip1,
            "flip2": self.flip2,
            "swap": self.swap,
            "along": self.along,
        })
    }

    fn from_json(v: &Value) -> Option<AxesStep> {
        let slot = |v: &Value| -> Option<Slot> {
            let origin: [f64; 3] = serde_json::from_value(v.get("origin")?.clone()).ok()?;
            let dir: [f64; 3] = serde_json::from_value(v.get("dir")?.clone()).ok()?;
            Some(Slot { axis: Axis::new(origin, dir)?, label: v.get("label").and_then(Value::as_str).unwrap_or("stored").to_string() })
        };
        let slots = v.get("slots")?.as_array()?;
        let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        let number = |k: &str, or: f64| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(or);
        Some(AxesStep {
            slots: [slots.first().and_then(slot), slots.get(1).and_then(slot)],
            use_ratio: flag("use_ratio"),
            ratio: number("ratio", 2.0),
            auto_module: flag("auto_module"),
            use_cone: flag("use_cone"),
            cone: number("cone", 40.0),
            flip1: flag("flip1"),
            flip2: flag("flip2"),
            swap: flag("swap"),
            along: number("along", 0.0),
        })
    }
}

/// What the window shows or hides besides the set's own numbers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Opts {
    /// Module given as diametral pitch (teeth per inch).
    inch: bool,
    /// Show the basic rack's coefficients.
    advanced: bool,
    /// Designing for FDM printing: nozzle width (mm) the tooth tips are checked against.
    printing: bool,
    nozzle: f64,
    /// The print preset's layer height (mm).
    layer: f64,
    /// Show the back shaft of gear 1 and of gear 2 (a shaft on each side).
    dual: [bool; 2],
    /// Show the load inputs for the strength rating.
    load_open: bool,
    /// Teeth drawn from their formulas (`tooth_curves: equations`), and driven by design
    /// parameters (`parametric`).
    equations: bool,
    parametric: bool,
}

/// The set being edited (the window's main button is then Update).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Editing {
    /// The set's history folder.
    group: u64,
    name: String,
    /// Its bodies' names, in build order.
    bodies: Vec<String>,
}

/// What the window holds between frames.
pub struct GearWindow {
    /// The numbers as typed.
    pub spec: Spec,
    /// The Axes step.
    pub axes: AxesStep,
    /// What is built: `spec`, or with an axis set, the set solved on the axes.
    pub(crate) built: Spec,
    /// The solve on the axes, when there is one.
    pub(crate) solved: Option<Solved>,
    /// The slot a "Use selection" click asked to fill; taken up after the window is drawn, where
    /// the design is at hand.
    pub(crate) capture: Option<usize>,
    /// Each slot's typed origin and direction, and whether its entry rows are open.
    pub(crate) manual: [bool; 2],
    pub(crate) entered: [([f64; 3], [f64; 3]); 2],
    /// Why the last "Use selection" took nothing.
    pub(crate) axes_note: Option<String>,
    /// How the axes lay last frame, to notice a change.
    pub(crate) last_relation: Option<&'static str>,
    /// The plan for what is built, or why there is none; recomputed when an input changes.
    pub(crate) plan: Result<Plan, String>,
    pub(crate) planned: Option<(Spec, AxesStep)>,
    /// When the inputs last changed (ms): the view's preview waits for them to settle.
    pub(crate) changed_ms: f64,
    /// Filled triangles of each preview gear, in its own coordinates.
    pub(crate) meshes: Vec<GearMesh>,
    /// Whether the set turns in the 2D preview, whether that is paused, and how far it has
    /// turned (driver radians; 0 is the pose the bodies are built in).
    pub animate: bool,
    pub(crate) paused: bool,
    pub(crate) turned: f64,
    pub(crate) speed: f64,
    pub(crate) opts: Opts,
    /// The member whose settings section the pointer is over, the member under the pointer in
    /// the 2D preview, and the member whose numbers last changed (with when, ms).
    pub(crate) hover_section: Option<usize>,
    pub(crate) hover_preview: Option<usize>,
    pub(crate) edited: Option<(usize, f64)>,
    /// The set this window regenerates, when it was opened on one.
    pub(crate) editing: Option<Editing>,
    /// The last create's or update's error.
    pub(crate) error: Option<String>,
    /// Bumped whenever the plan is recomputed: what was worked out from a plan (the mesh check)
    /// carries the generation it was worked out for.
    pub(crate) plan_gen: u64,
    /// The 2D mesh check of the plan of that generation (computed once the numbers settle).
    pub(crate) mesh: Option<(u64, Vec<MeshResult>)>,
    /// The mesh result the pointer is on in the badge's list: both its members are lit.
    pub(crate) hover_mesh: Option<usize>,
    /// Design by goal: the goal as typed, whether its panel is open, and the last suggestions
    /// (for the goal they were made for), or why there are none.
    pub(crate) goal: Goal,
    pub(crate) suggest_open: bool,
    pub(crate) suggestions: Option<(Goal, Result<Vec<Suggestion>, String>)>,
    /// A profile DXF asked for: the 2D preview member (index into `plan.preview`) to export;
    /// taken up after the window is drawn, where the app's save dialog is at hand.
    pub(crate) export: Option<usize>,
    /// What the last sheet copy or export did.
    pub(crate) output_note: Option<String>,
    /// Simple (drawn in one sketch, a short form) or Advanced (every setting).
    pub(crate) mode: Mode,
    pub(crate) simple: crate::gear_simple::SimpleState,
    /// The instant 3D view and the schematic it (and the drawing) is drawn from, for the plan
    /// of `plan_gen`.
    pub(crate) view3d: crate::gear_view3d::View3d,
    pub(crate) schematic: Option<crate::gear_view3d::Schematic>,
    /// The right column's view.
    pub(crate) right: RightView,
    /// The set shown in the model as a kernel preview: asked for once by "Show in model" for
    /// the plan of this generation; any edit lets it lapse.
    pub(crate) model_preview: Option<u64>,
}

/// The window's two modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Simple,
    Advanced,
}

/// What the right column shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RightView {
    /// The drawing and the 3D view (side by side where there is room).
    Both,
    Drawing,
    View3d,
    /// The animated 2D mesh preview.
    Profile,
}

pub(crate) struct GearMesh {
    fill: Vec<[V2; 3]>,
    holes: Vec<Vec<[V2; 3]>>,
    outlines: Vec<Vec<V2>>,
    /// The farthest outline point from the gear's own centre.
    radius: f64,
}

/// One colour per member of a set, used wherever the member shows.
const PALETTE: [Color32; 6] = [
    Color32::from_rgb(86, 140, 220),
    Color32::from_rgb(232, 146, 64),
    Color32::from_rgb(96, 180, 120),
    Color32::from_rgb(196, 110, 190),
    Color32::from_rgb(210, 190, 80),
    Color32::from_rgb(120, 170, 190),
];

pub(crate) fn member_color(m: usize) -> Color32 {
    PALETTE[m % PALETTE.len()]
}

/// The members of a set of this type, as their settings sections are headed.
fn members(k: Kind) -> &'static [&'static str] {
    match k {
        Kind::Planetary => &["Sun", "Planets", "Ring"],
        Kind::Worm => &["Wheel", "Worm"],
        Kind::Hypoid => &["Pinion", "Ring gear"],
        Kind::Cycloidal => &["Wheel", "Pinion"],
        Kind::CycloidalDrive => &["Disc", "Ring pins"],
        Kind::Pair | Kind::Bevel | Kind::SpiralBevel | Kind::CrossedHelical | Kind::Elliptical => &["Gear 1", "Gear 2"],
        Kind::Sprocket => &["Sprocket 1", "Sprocket 2"],
        Kind::Pulley => &["Pulley 1", "Pulley 2"],
        Kind::Spline => &["Shaft", "Hub"],
        Kind::FaceGear => &["Face gear", "Pinion"],
        Kind::Internal => &["Ring gear"],
        Kind::Rack => &["Rack"],
        // A single gear (spur, helical, herringbone), and any family added later until it
        // names its members here.
        _ => &["Gear"],
    }
}

/// The member a 2D preview outline belongs to (the `i`-th of the plan's, called `name`); none
/// for what is drawn but not built (a chain, a belt).
pub(crate) fn member_of_outline(k: Kind, i: usize, name: &str) -> Option<usize> {
    let last = members(k).len() - 1;
    Some(match k {
        Kind::Planetary if name.starts_with("planet") => 1,
        Kind::Planetary if name == "ring" => 2,
        Kind::Planetary => 0,
        Kind::Sprocket | Kind::Pulley if name == "chain" || name == "belt" => return None,
        Kind::Sprocket | Kind::Pulley => usize::from(name.ends_with('2')),
        Kind::Spline => usize::from(name == "hub"),
        Kind::FaceGear => usize::from(name == "pinion"),
        _ => i.min(last),
    })
}

/// The member a built body belongs to (the `i`-th of the plan's, called `name`; `only` as in
/// the spec: one member built alone).
pub(crate) fn member_of_body(k: Kind, only: u8, i: usize, name: &str) -> usize {
    let last = members(k).len() - 1;
    match k {
        Kind::Planetary if name.contains("Planet ") => 1,
        Kind::Planetary if name.contains("Ring z") => 2,
        Kind::Planetary => 0,
        Kind::FaceGear => usize::from(name.contains("pinion")),
        Kind::Spline => usize::from(name.contains("hub")),
        _ if only > 0 => usize::from(only - 1).min(last),
        _ => i.min(last),
    }
}

/// Spur, helical and herringbone gears ("cyl" in `crates/gear/UI_FIELDS.md`).
fn cyl(k: Kind) -> bool {
    matches!(k, Kind::Spur | Kind::Helical | Kind::Herringbone)
}

/// The tip tuning of the gear crate's `body`: tip rounding, elephant foot, nozzle check, tip
/// relief (cyl, pair, planetary sun and planets).
fn tip_tuned(k: Kind) -> bool {
    cyl(k) || matches!(k, Kind::Pair | Kind::Planetary)
}

/// Kinds cut from the spec's rack: XY compensation, the clearance override, tip shortening.
fn rack_tuned(k: Kind) -> bool {
    cyl(k) || matches!(k, Kind::Internal | Kind::Rack | Kind::Pair | Kind::Planetary | Kind::CrossedHelical | Kind::Hypoid)
}

/// Kinds whose members are built through the gear crate's `hub_steps`: bores and hub
/// chamfers, hardware, bearing pockets and shafts.
fn hub_kind(k: Kind) -> bool {
    cyl(k) || matches!(k, Kind::Pair | Kind::Planetary | Kind::CrossedHelical | Kind::Worm | Kind::Cycloidal | Kind::Sprocket | Kind::Pulley | Kind::FaceGear | Kind::Elliptical)
}

/// Which gear of the spec (1: `shaft_*`, `hardware`; 2: `shaft2_*`, `hardware2`) member `m` of
/// [`members`] is, for the body features; none for a member that takes none (a planetary
/// ring, a face gear's face gear, a spline, a hypoid).
fn body_gear(k: Kind, m: usize) -> Option<u8> {
    let shafts_only = matches!(k, Kind::Bevel | Kind::SpiralBevel);
    if !(hub_kind(k) || shafts_only) {
        return None;
    }
    match (k, m) {
        (Kind::FaceGear, 0) | (Kind::Planetary, 2) => None,
        (Kind::FaceGear, _) => Some(2),
        (_, 0) => Some(1),
        (_, 1) if k.is_set() => Some(2),
        _ => None,
    }
}

/// A spec from `gear.create` parameters (as the window keeps its typed numbers with a set).
fn spec_from(v: &Value) -> Option<Spec> {
    let p: solidcraft_engine::schema::gear::GearParams = serde_json::from_value(v.clone()).ok()?;
    solidcraft_engine::gear_spec_of(&p).ok()
}

impl GearWindow {
    pub fn new() -> Self {
        let mut w = GearWindow {
            spec: Spec::defaults_for(Kind::Spur),
            axes: AxesStep::default(),
            built: Spec::defaults_for(Kind::Spur),
            solved: None,
            capture: None,
            manual: [false; 2],
            entered: [([0.0; 3], [0.0, 0.0, 1.0]); 2],
            axes_note: None,
            last_relation: None,
            plan: Err(String::new()),
            planned: None,
            changed_ms: 0.0,
            meshes: Vec::new(),
            animate: true,
            paused: false,
            turned: 0.0,
            speed: 0.4,
            opts: Opts { inch: false, advanced: false, printing: false, nozzle: 0.4, layer: 0.2, dual: [false; 2], load_open: false, equations: false, parametric: false },
            hover_section: None,
            hover_preview: None,
            edited: None,
            editing: None,
            error: None,
            plan_gen: 0,
            mesh: None,
            hover_mesh: None,
            goal: Goal::default(),
            suggest_open: false,
            suggestions: None,
            export: None,
            output_note: None,
            mode: Mode::Simple,
            simple: crate::gear_simple::SimpleState::new(&Spec::defaults_for(Kind::Spur)),
            view3d: crate::gear_view3d::View3d::default(),
            schematic: None,
            right: RightView::Both,
            model_preview: None,
        };
        crate::gear_simple::apply(&mut w);
        w.replan();
        w.changed_ms = 0.0;
        w
    }

    /// The window on a set the design keeps: filled from the window's own state where the set
    /// was made here (typed numbers, axes, options), otherwise from the set's parameters, placed
    /// by its stored frame.
    fn on_set(group: u64, name: &str, bodies: &[String], request: &Value, params: &Value) -> Self {
        let mut w = GearWindow::new();
        let kept = request.get("ui").filter(|ui| ui.get("window").is_some());
        match kept.and_then(|ui| Some((spec_from(ui.get("typed")?)?, AxesStep::from_json(ui.get("axes")?)?, ui))) {
            Some((spec, axes, ui)) => {
                w.spec = spec;
                w.axes = axes;
                let flag = |k: &str| ui.get(k).and_then(Value::as_bool).unwrap_or(false);
                w.opts.inch = flag("inch");
                w.opts.printing = flag("printing");
                w.opts.nozzle = ui.get("nozzle").and_then(Value::as_f64).filter(|n| n.is_finite() && *n > 0.0).unwrap_or(0.4);
                w.opts.layer = ui.get("layer").and_then(Value::as_f64).filter(|n| n.is_finite() && *n > 0.0).unwrap_or(0.2);
            }
            // Made by a script, or by a window that kept nothing: the set as it is, where it is.
            None => {
                if let Some(spec) = spec_from(params) {
                    w.spec = spec;
                }
                w.axes = AxesStep { slots: [None, None], auto_module: false, ..AxesStep::default() };
            }
        }
        for (i, slot) in w.axes.slots.iter().enumerate() {
            if let Some(s) = slot {
                w.entered[i] = (s.axis.origin, s.axis.dir);
            }
        }
        // The type is the set's: the axes are not asked for another.
        w.last_relation = w.axes.relation().as_ref().map(Relation::name);
        w.opts.load_open = w.spec.load.torque > 0.0 || w.spec.load.power > 0.0;
        w.opts.dual = [w.spec.shaft_back.exists(), w.spec.shaft2_back.exists()];
        // How the teeth were drawn: as the set was asked for.
        w.opts.equations = request.get("tooth_curves").and_then(Value::as_str) == Some("equations");
        w.opts.parametric = w.opts.equations && request.get("parametric").and_then(Value::as_bool) == Some(true);
        w.editing = Some(Editing { group, name: name.to_string(), bodies: bodies.to_vec() });
        // A set made elsewhere is regenerated from its numbers: Advanced shows them all.
        w.mode = Mode::Advanced;
        w.planned = None;
        w.replan();
        w.changed_ms = 0.0;
        w
    }

    /// The window's own state, kept with the set it makes (`ui` of `gear.create`).
    fn ui_state(&self) -> Value {
        json!({
            "window": 1,
            "typed": solidcraft_engine::gear_params_of(&self.spec),
            "axes": self.axes.to_json(),
            "inch": self.opts.inch,
            "printing": self.opts.printing,
            "nozzle": self.opts.nozzle,
            "layer": self.opts.layer,
            "equations": self.opts.equations,
            "parametric": self.opts.parametric,
        })
    }

    /// The set's body names, warnings and facts, for the clearance section.
    pub(crate) fn clearance_inputs(&self) -> (Vec<String>, Vec<String>, Vec<solidcraft_gear::Fact>) {
        match &self.plan {
            Ok(p) => (p.bodies.iter().map(|b| b.name.clone()).collect(), p.warnings.clone(), p.facts.clone()),
            Err(_) => (Vec::new(), Vec::new(), Vec::new()),
        }
    }

    pub(crate) fn replan(&mut self) {
        if self.mode == Mode::Simple {
            // The mode is the user's pick there; the axes follow the drawing.
            self.last_relation = self.axes.relation().as_ref().map(Relation::name);
        } else {
            self.settle_kind();
        }
        let key = (self.spec.clone(), self.axes.clone());
        if self.planned.as_ref() == Some(&key) {
            return;
        }
        self.planned = Some(key);
        self.changed_ms = crate::now_ms();
        self.solved = None;
        self.plan_gen += 1;
        // With an axis, the set is sized and placed on the axes; what is built is the solved
        // spec, and `spec` stays what was typed.
        let built = match self.axes.axis(0) {
            None => Ok(self.spec.clone()),
            Some(a1) => axes::solve(&a1, self.axes.axis(1).as_ref(), Some(self.spec.kind), &self.spec, &Drive {
                // A hypoid's size is the ring gear Ø the user gives: the axes never choose its module.
                module_free: self.axes.auto_module && self.spec.kind != Kind::Hypoid,
                ..self.axes.drive()
            }).map(|solved| {
                let spec = solved.spec.clone();
                self.solved = Some(solved);
                spec
            }),
        };
        self.plan = built.and_then(|b| {
            let plan = solidcraft_gear::plan(&b);
            self.built = b;
            plan
        });
        self.meshes = match &self.plan {
            Ok(p) => p.preview.iter().map(|g| mesh_of(&g.region)).collect(),
            Err(_) => Vec::new(),
        };
        // The shafts and pitch surfaces of the 3D view and the drawing: µs to ms, no teeth.
        if let Ok(p) = &self.plan {
            self.schematic = Some(crate::gear_view3d::schematic_of(&self.built, p));
        }
    }

    /// Whether the numbers have stood still long enough for the view's preview to follow.
    fn settled(&self) -> bool {
        (crate::now_ms() - self.changed_ms).abs() >= SETTLE_MS
    }

    /// Runs the 2D mesh check once the numbers have settled, for the plan they make (never per
    /// keystroke: it builds sampled members of every meshing pair).
    fn check_mesh(&mut self) {
        if !self.settled() || self.mesh.as_ref().is_some_and(|(g, _)| *g == self.plan_gen) {
            return;
        }
        let results = match &self.plan {
            Ok(plan) => solidcraft_gear::check_mesh(plan),
            Err(_) => Vec::new(),
        };
        self.mesh = Some((self.plan_gen, results));
    }

    /// The mesh check for the plan shown, or none while it is not worked out yet.
    fn mesh_now(&self) -> Option<&[MeshResult]> {
        self.mesh.as_ref().filter(|(g, _)| *g == self.plan_gen).map(|(_, r)| r.as_slice())
    }

    /// The command and parameters that make (or regenerate) the set as the window shows it.
    fn command(&self, ui: Value) -> (String, Value) {
        let mut params = solidcraft_engine::gear_params_of(&self.built);
        params["ui"] = ui;
        if self.opts.equations && equation_teeth(self.built.kind) {
            params["tooth_curves"] = json!("equations");
            if self.opts.parametric {
                // (An update keeps the set's parameter prefix by itself.)
                params["parametric"] = json!(true);
            }
        }
        match &self.editing {
            Some(e) => {
                params["group"] = json!(e.group);
                // Everything the set is now is in these parameters: nothing of the old request stays.
                params["reset"] = json!(true);
                ("gear.update".to_string(), params)
            }
            None => ("gear.create".to_string(), params),
        }
    }

    /// Each body's colour in the view's preview: its member's.
    fn tints(&self) -> Value {
        let Ok(plan) = &self.plan else { return json!({}) };
        let kept = self.editing.as_ref().map(|e| &e.bodies).filter(|names| names.len() == plan.bodies.len());
        let mut tints = serde_json::Map::new();
        for (i, body) in plan.bodies.iter().enumerate() {
            let name = kept.and_then(|names| names.get(i)).unwrap_or(&body.name);
            let color = member_color(member_of_body(self.built.kind, self.built.only, i, &body.name));
            tints.insert(name.clone(), json!(color.to_srgba_unmultiplied()));
        }
        Value::Object(tints)
    }

    /// The kernel preview in the model: only once asked for with "Show in model", for the plan
    /// it was asked for. Designing never builds solids: an edit lets the request lapse (the
    /// preview is taken down), and Create stays the only thing that builds.
    pub fn preview_commands(&self) -> Option<Vec<(String, Value)>> {
        (self.model_preview == Some(self.plan_gen) && self.plan.is_ok()).then(|| vec![self.command(json!({"preview_tints": self.tints()}))])
    }

    /// A new type starts from its own defaults, keeping the tooth form and hub choices.
    pub(crate) fn switch_kind(&mut self, k: Kind) {
        if self.spec.kind == k {
            return;
        }
        let keep = self.spec.clone();
        let mut s = Spec::defaults_for(k);
        if k.involute() == keep.kind.involute() && !matches!(k, Kind::Hypoid | Kind::Worm | Kind::Cycloidal) {
            s.module = keep.module;
            s.pressure_angle = keep.pressure_angle;
            s.rack = keep.rack;
            s.backlash = keep.backlash;
        }
        s.bore = keep.bore;
        s.bore2 = keep.bore2;
        s.origin = keep.origin;
        s.name = keep.name;
        s.load = keep.load;
        s.clearance_body = keep.clearance_body;
        self.spec = s;
        self.turned = 0.0;
    }

    /// Keeps the type one the axes allow: when how the axes lie changes, a type that doesn't
    /// span both of them gives way to the natural one (a pair for parallel axes, bevels for
    /// intersecting ones…); a single gear picked afterwards stays.
    fn settle_kind(&mut self) {
        let rel = self.axes.relation();
        let name = rel.as_ref().map(Relation::name);
        let changed = name != self.last_relation;
        self.last_relation = name;
        let Some(rel) = rel else { return };
        let fits = axes::candidates(Some(&rel)).into_iter().any(|c| c.kind == self.spec.kind && c.fits);
        if !fits || (changed && !self.spec.kind.is_set()) {
            self.switch_kind(axes::natural(Some(&rel)));
        }
    }

    /// The member to light up: the one whose section the pointer is over, else the one under
    /// the pointer in the 2D preview, else the one whose numbers just changed.
    pub(crate) fn lit(&self) -> Option<usize> {
        let recent = self.edited.filter(|(_, at)| (crate::now_ms() - at).abs() < EDIT_GLOW_MS).map(|(m, _)| m);
        self.hover_section.or(self.hover_preview).or(recent)
    }

    /// How far the 2D preview has turned: 0 (the pose the bodies are built in) with Turn off.
    fn pose(&self) -> f64 {
        if self.animate { self.turned } else { 0.0 }
    }
}

impl Default for GearWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Opens the window: on the gear set something selected belongs to (to regenerate it), else for
/// a new set. A window already open stays as it is.
pub fn open(app: &mut SolveApp) {
    if app.gear_window.is_some() {
        return;
    }
    app.gear_window = Some(match solidcraft_engine::gear_selected_set(&app.session) {
        Some(set) => GearWindow::on_set(set.group, &set.name, &set.bodies, &set.request, &set.params),
        None => {
            // Sketch geometry selected: the window starts from that sketch.
            let mut w = GearWindow::new();
            crate::gear_simple::open_on_selection(app, &mut w);
            crate::gear_simple::apply(&mut w);
            w
        }
    });
}

/// Whether a history folder is a gear set the window can be opened on.
pub fn is_set(app: &SolveApp, group: u64) -> bool {
    solidcraft_engine::gear_sets(&app.session).iter().any(|set| set.group == group)
}

/// Opens the window on the gear set of a history folder.
pub fn open_group(app: &mut SolveApp, group: u64) -> Result<(), String> {
    let sets = solidcraft_engine::gear_sets(&app.session);
    let set = sets.iter().find(|set| set.group == group).ok_or_else(|| "That folder is not a gear set the Gear Generator made".to_string())?;
    app.gear_window = Some(GearWindow::on_set(set.group, &set.name, &set.bodies, &set.request, &set.params));
    Ok(())
}

/// Triangles filling a region: the outer outlines by ear clipping, the holes likewise (drawn
/// over in the background colour).
fn mesh_of(r: &Region) -> GearMesh {
    let tol = 0.02;
    let mut fill = Vec::new();
    let mut outlines = Vec::new();
    for o in &r.outer {
        let p = simplify(o.polyline(tol));
        fill.extend(triangulate(&p));
        outlines.push(p);
    }
    let mut holes = Vec::new();
    for o in &r.holes {
        let p = simplify(o.polyline(tol));
        holes.push(triangulate(&p));
        outlines.push(p);
    }
    let radius = outlines.iter().flatten().map(|q| q.len()).fold(0.0, f64::max);
    GearMesh { fill, holes, outlines, radius }
}

/// Drops repeated points and the closing duplicate.
fn simplify(mut p: Vec<V2>) -> Vec<V2> {
    p.dedup_by(|a, b| a.dist(*b) < 1e-9);
    if p.len() > 1 && p[0].dist(p[p.len() - 1]) < 1e-9 {
        p.pop();
    }
    p
}

/// Ear clipping of a simple polygon (either orientation).
pub(crate) fn triangulate(poly: &[V2]) -> Vec<[V2; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f64 = (0..n).map(|i| poly[i].cross(poly[(i + 1) % n])).sum();
    let mut idx: Vec<usize> = if area >= 0.0 { (0..n).collect() } else { (0..n).rev().collect() };
    let mut out = Vec::with_capacity(n);
    let mut guard = 0;
    let mut i = 0;
    while idx.len() > 3 && guard < 4 * n * n.min(4000) {
        guard += 1;
        let m = idx.len();
        let (a, b, c) = (poly[idx[(i + m - 1) % m]], poly[idx[i % m]], poly[idx[(i + 1) % m]]);
        let convex = (b - a).cross(c - b) > 1e-12;
        let ear = convex
            && !idx.iter().any(|&k| {
                let q = poly[k];
                q != a && q != b && q != c && inside(q, a, b, c)
            });
        if ear {
            out.push([a, b, c]);
            idx.remove(i % m);
            i = if i == 0 { 0 } else { i - 1 };
        } else {
            i = (i + 1) % m;
        }
    }
    if idx.len() == 3 {
        out.push([poly[idx[0]], poly[idx[1]], poly[idx[2]]]);
    }
    out
}

fn inside(q: V2, a: V2, b: V2, c: V2) -> bool {
    (b - a).cross(q - a) >= 0.0 && (c - b).cross(q - b) >= 0.0 && (a - c).cross(q - c) >= 0.0
}

/// Where the view's preview stands against the numbers in the window.
#[derive(Clone, Copy, PartialEq)]
enum ViewState {
    /// Not asked for, or there is no set to show.
    Off,
    /// The numbers changed: the preview for them is on its way, and nothing older is shown.
    Updating,
    /// The view shows the set these numbers make.
    Current,
}

pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(mut w) = app.gear_window.take() else { return };
    if w.mode == Mode::Simple {
        crate::gear_simple::apply(&mut w);
    }
    w.replan();
    w.check_mesh();
    let title = solidcraft_ui_text::tr_label("Gear Generator");
    let create = solidcraft_ui_text::tr("ui.create");
    let close = solidcraft_ui_text::tr("ui.close");
    let primary: &str = if w.editing.is_some() { "Update" } else { &create };
    let valid = w.plan.is_ok();
    let preview_error = app.preview.error.clone();
    let view = if !(w.model_preview == Some(w.plan_gen) && valid) {
        ViewState::Off
    } else if app.preview.busy || !app.preview.current {
        ViewState::Updating
    } else {
        ViewState::Current
    };
    let (closed, action) = ui_kit::Window::new(egui::Id::unique("sc_gear_window"), "gear", title).fields().cards(3.3).actions(primary, &close, None).show(ctx, valid, |ui| {
        body(ui, &mut w, view, preview_error.as_deref());
    });
    if w.animate && !w.paused {
        let dt = ctx.input(|i| i.stable_dt).min(0.1) as f64;
        w.turned += dt * w.speed;
        ctx.request_repaint();
    } else if !w.animate {
        w.turned = 0.0;
    }
    if view == ViewState::Updating || w.lit().is_some() || (w.plan.is_ok() && w.mesh_now().is_none()) {
        // The preview starts once the numbers settle, and an edit's glow fades: both need a
        // frame without the pointer moving.
        ctx.request_repaint_after(std::time::Duration::from_millis(80));
    }
    if closed || action == ui_kit::Action::Cancel {
        return;
    }
    if let Some(slot) = w.capture.take() {
        capture(app, &mut w, slot);
    }
    if let Some(req) = w.simple.request.take() {
        crate::gear_simple::use_selected(app, &mut w, req);
    }
    if let Some(i) = w.export.take() {
        export_dxf(app, &mut w, i);
    }
    if action == ui_kit::Action::Ok {
        let (command, params) = w.command(w.ui_state());
        match app.run(&command, params) {
            Ok(reply) => {
                if w.editing.is_some() {
                    report_update(app, &reply);
                }
                return;
            }
            Err(e) => w.error = Some(e),
        }
    }
    app.gear_window = Some(w);
}

/// Says how an update went: in place or rebuilt, and which features built on the gears now fail.
fn report_update(app: &mut SolveApp, reply: &Value) {
    let broken: Vec<&str> = reply.get("broken").and_then(Value::as_array).map(|b| b.iter().filter_map(|f| f.get("feature").and_then(Value::as_str)).collect()).unwrap_or_default();
    let how = match reply.get("updated").and_then(Value::as_str) {
        Some("in_place") => "Gears updated in place",
        _ => "Gears rebuilt under the same names",
    };
    if broken.is_empty() {
        app.set_status(how, false);
    } else {
        app.set_status(format!("{how}; built on them and failing now: {}", broken.join(", ")), true);
    }
}

/// Fills an axis slot from what is selected in the view: a construction or origin axis, a
/// straight or round edge, a cylinder, cone or torus face, or a straight sketch line. With two
/// such items selected, slot 1 takes the first and slot 2 the second.
fn capture(app: &SolveApp, w: &mut GearWindow, slot: usize) {
    use solidcraft_engine::Sel;
    use solidcraft_engine::geom::prelude::*;
    let found: Vec<Slot> = app
        .session
        .selection
        .iter()
        .filter_map(|sel| {
            if let Sel::SketchPoint { id } = sel {
                // A sketch point: the shaft through it, normal to its sketch.
                let st = crate::open_meshing::session_world(&app.session);
                let (sketch, point) = match id.split_once(':') {
                    Some((sketch, point)) => (sketch.parse().ok()?, point),
                    None => (app.session.active_sketch?, id.as_str()),
                };
                let ss = st.sketch(sketch)?;
                let p = ss.plane.to_world(ss.sketch.point(ss.sketch.resolve_point(point)?)?);
                let n = ss.plane.normal();
                let axis = Axis::new([p.x, p.y, p.z], [n.x, n.y, n.z])?;
                return Some(Slot { axis, label: format!("sketch point {id}") });
            }
            let (o, d) = crate::dialogs::axis_of(app, sel)?;
            let axis = Axis::new([o.x, o.y, o.z], [d.x, d.y, d.z])?;
            let label = match sel {
                Sel::Axis { name } => format!("axis {name}"),
                Sel::Edge { body, .. } => format!("edge of {body}"),
                Sel::Face { body, .. } => format!("face of {body}"),
                _ => "sketch line".to_string(),
            };
            Some(Slot { axis, label })
        })
        .collect();
    let Some(pick) = found.get(slot.min(found.len().saturating_sub(1))).cloned() else {
        w.axes_note = Some("Nothing selected has an axis: select a construction axis, a straight or round edge, a cylinder face or a sketch line in the view, then press Use selection".into());
        return;
    };
    w.axes_note = None;
    set_slot(w, slot, pick);
}

fn set_slot(w: &mut GearWindow, slot: usize, pick: Slot) {
    w.entered[slot] = (pick.axis.origin, pick.axis.dir);
    w.axes.slots[slot] = Some(pick);
}

/// The Axes step: two slots for the shafts, how they lie, and what drives the solve on them.
fn axes_step(ui: &mut Ui, w: &mut GearWindow) {
    ui.label(egui::RichText::new("Axes").strong()).on_hover_text("Start from the shafts: pick the two axes and the gears are sized and placed on them");
    for i in 0..2 {
        ui.horizontal(|ui| {
            ui.add_sized([110.0, 18.0], egui::Label::new(egui::RichText::new(if i == 0 { "Axis 1" } else { "Axis 2" }).small().weak()));
            if ui.small_button("Use selection").on_hover_text("Take the axis of what is selected in the view: a construction or origin axis, a straight or round edge, a cylinder face, a sketch line").clicked() {
                w.capture = Some(i);
            }
            for (name, dir) in [("X", [1.0, 0.0, 0.0]), ("Y", [0.0, 1.0, 0.0]), ("Z", [0.0, 0.0, 1.0])] {
                if ui.small_button(name).on_hover_text(format!("World {name} through the origin")).clicked() {
                    set_slot(w, i, Slot { axis: Axis { origin: [0.0; 3], dir }, label: format!("world {name}") });
                }
            }
            ui.toggle_value(&mut w.manual[i], "Enter").on_hover_text("Type a point on the axis and its direction");
            match &w.axes.slots[i] {
                Some(s) => {
                    let (o, d) = (s.axis.origin, s.axis.dir);
                    ui.label(&s.label).on_hover_text(format!("through ({:.3}, {:.3}, {:.3}) along ({:.4}, {:.4}, {:.4})", o[0], o[1], o[2], d[0], d[1], d[2]));
                    if ui.small_button("✕").on_hover_text("Clear this axis").clicked() {
                        w.axes.slots[i] = None;
                    }
                }
                None => {
                    ui.weak(if i == 0 { "not set: the set goes where Centre at says, on world Z" } else { "not set: one gear, or a set built about axis 1" });
                }
            }
        });
        if w.manual[i] {
            ui.horizontal(|ui| {
                ui.add_sized([110.0, 18.0], egui::Label::new(egui::RichText::new("  point, direction").small().weak()));
                let (o, d) = &mut w.entered[i];
                let mut changed = false;
                for v in o.iter_mut() {
                    changed |= ui.add(egui::DragValue::new(v).speed(0.5).max_decimals(3)).changed();
                }
                ui.label("→");
                for v in d.iter_mut() {
                    changed |= ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(4)).changed();
                }
                if changed && let Some(axis) = Axis::new(*o, *d) {
                    w.axes.slots[i] = Some(Slot { axis, label: "entered".into() });
                }
            });
        }
    }
    if let Some(why) = &w.axes_note {
        ui.note(why, ui.visuals().warn_fg_color);
    }
    if w.axes.slots[0].is_none() && w.axes.slots[1].is_some() {
        ui.note("Set Axis 1 too: gear 1 turns about it", ui.visuals().warn_fg_color);
    }
    if w.axes.axis(0).is_none() {
        return;
    }
    let rel = w.axes.relation();
    let two = rel.is_some() && w.spec.kind.is_set() && !matches!(w.spec.kind, Kind::Planetary | Kind::CycloidalDrive | Kind::Spline);
    let picks_module = matches!(w.spec.kind, Kind::Pair | Kind::Worm | Kind::CrossedHelical);
    ui.horizontal_wrapped(|ui| {
        ui.add_sized([110.0, 18.0], egui::Label::new(egui::RichText::new("Relation").small().weak()));
        let text = match &rel {
            Some(r) => r.describe(),
            None => "one axis: the gear turns about it".to_string(),
        };
        ui.label(egui::RichText::new(text).strong());
        // What the solve made of it, where that says more (which rays the bevels sit on).
        if let (Some(Relation::Intersecting { .. }), Some(s)) = (&rel, &w.solved) {
            ui.label(format!("shaft angle {:.2}°", s.spec.shaft_angle));
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_sized([110.0, 18.0], egui::Label::new(egui::RichText::new("Drive").small().weak()));
        let a = &mut w.axes;
        if two {
            ui.checkbox(&mut a.use_ratio, "Ratio").on_hover_text("Choose the tooth counts for this ratio (gear 2 ÷ gear 1; worm: wheel teeth ÷ starts) instead of typing them");
            if a.use_ratio {
                ui.add(egui::DragValue::new(&mut a.ratio).range(0.05..=500.0).speed(0.01).max_decimals(4));
            }
            match &rel {
                Some(Relation::Intersecting { .. }) => {
                    ui.checkbox(&mut a.use_cone, "Cone distance").on_hover_text("Size the bevels by the distance from where the axes meet to their large ends, instead of by the module");
                    if a.use_cone {
                        ui.add(egui::DragValue::new(&mut a.cone).range(1.0..=1e5).speed(0.2).max_decimals(3).suffix(" mm"));
                    }
                    ui.checkbox(&mut a.flip1, "Flip 1").on_hover_text("Gear 1 on the other side of where the axes meet");
                    ui.checkbox(&mut a.flip2, "Flip 2").on_hover_text("Gear 2 on the other side of where the axes meet");
                }
                _ => {
                    if picks_module {
                        ui.checkbox(&mut a.auto_module, "Standard module").on_hover_text("Choose the ISO 54 module that suits the axes, instead of the one typed");
                    }
                    if matches!(rel, Some(Relation::Skew { .. })) {
                        ui.checkbox(&mut a.flip1, "Other angle").on_hover_text("Read the shaft angle the other way round (its supplement)");
                    }
                }
            }
            ui.checkbox(&mut a.swap, "Swap").on_hover_text("Gear 1 on axis 2 and gear 2 on axis 1");
        } else {
            ui.checkbox(&mut a.flip1, "Flip").on_hover_text("Face the other way along the axis");
        }
        if !matches!(rel, Some(Relation::Intersecting { .. } | Relation::Skew { .. })) || !two {
            ui.label("Along axis");
            ui.add(egui::DragValue::new(&mut a.along).speed(0.5).max_decimals(3).suffix(" mm")).on_hover_text("How far along axis 1 from its picked point the gear sits");
        }
    });
    if let Some(s) = &w.solved {
        for n in &s.notes {
            ui.label(egui::RichText::new(n).small().weak());
        }
    }
}

/// Messages that wrap (warnings, errors, refusals, notes): never cut to one line.
pub(crate) trait Note {
    fn note(&mut self, text: impl AsRef<str>, color: Color32);
}

impl Note for Ui {
    fn note(&mut self, text: impl AsRef<str>, color: Color32) {
        self.add(egui::Label::new(egui::RichText::new(text.as_ref()).color(color)).wrap_mode(egui::TextWrapMode::Wrap));
    }
}

fn body(ui: &mut Ui, w: &mut GearWindow, view: ViewState, view_error: Option<&str>) {
    ui.horizontal(|ui| {
        let was = w.mode;
        ui.selectable_value(&mut w.mode, Mode::Simple, egui::RichText::new("Simple").strong())
            .on_hover_text("Draw the set in one sketch (circles are pitch circles, lines are shafts) and give a few sizes; the solver picks module, teeth and shifts");
        ui.selectable_value(&mut w.mode, Mode::Advanced, egui::RichText::new("Advanced").strong()).on_hover_text("Every setting: tooth form, 3D printing, chamfers, shafts, hardware, load, axes picked in the model");
        if was == Mode::Advanced && w.mode == Mode::Simple && w.right == RightView::Profile {
            w.right = RightView::Both;
        }
        if w.mode == Mode::Simple {
            ui.label(egui::RichText::new("Advanced keeps everything set there; editing the drawing sizes the set again").small().weak());
        }
    });
    ui.add_space(4.0);
    match w.mode {
        Mode::Simple => simple_body(ui, w, view, view_error),
        Mode::Advanced => advanced_body(ui, w, view, view_error),
    }
}

/// Simple mode: the mode, its picks and a short form on the left; the drawing and the 3D view
/// on the right, then the mesh badge, the key numbers and Show in model.
fn simple_body(ui: &mut Ui, w: &mut GearWindow, view: ViewState, view_error: Option<&str>) {
    let full = ui.available_width();
    let left = (full * 0.36).clamp(260.0, 340.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(left);
            egui::ScrollArea::vertical().id_salt("gear_simple_left").max_height(560.0).auto_shrink([false, true]).show(ui, |ui| {
                crate::gear_simple::panel(ui, w);
            });
        });
        ui.add_space(8.0);
        ui.vertical(|ui| {
            let width = (full - left - 8.0).max(260.0);
            ui.set_width(width);
            right_views(ui, w, width);
            motion_row(ui, w);
            mesh_badge(ui, w);
            view_row(ui, w, view, view_error);
            if let Some(e) = &w.error {
                ui.note(e, ui.visuals().error_fg_color);
            }
            simple_facts(ui, w);
        });
    });
}

/// The drawing, the 3D view and the 2D mesh preview, as the toggle above them says.
fn right_views(ui: &mut Ui, w: &mut GearWindow, width: f32) {
    let simple = w.mode == Mode::Simple;
    ui.horizontal(|ui| {
        let views: &[(RightView, &str, &str)] = if simple {
            &[(RightView::Both, "Drawing + 3D", "The sketch and the set in 3D"), (RightView::Drawing, "Drawing", "The sketch, larger"), (RightView::View3d, "3D", "The set in 3D, larger"), (RightView::Profile, "2D mesh", "The teeth meshing, turning")]
        } else {
            &[(RightView::Both, "3D + 2D", "The set in 3D and the teeth meshing in 2D"), (RightView::View3d, "3D", "The set in 3D"), (RightView::Profile, "2D mesh", "The teeth meshing, turning")]
        };
        for (v, name, tip) in views {
            ui.selectable_value(&mut w.right, *v, *name).on_hover_text(*tip);
        }
    });
    if !simple && w.right == RightView::Drawing {
        w.right = RightView::View3d;
    }
    let side = width >= 640.0;
    let half = if side { (width - 8.0) / 2.0 } else { width };
    let tall = |w: f32| (w * 0.78).clamp(220.0, 420.0);
    match (w.right, simple) {
        (RightView::Both, true) => {
            let size = vec2(half, tall(half));
            if side {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| crate::gear_simple::drawing(ui, w, size));
                    ui.vertical(|ui| view3d_ui(ui, w, size));
                });
            } else {
                crate::gear_simple::drawing(ui, w, size);
                view3d_ui(ui, w, size);
            }
        }
        (RightView::Both, false) => {
            let size = vec2(half, tall(half));
            if side {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| view3d_ui(ui, w, size));
                    ui.vertical(|ui| {
                        ui.set_width(half);
                        w.hover_preview = w.hover_preview.or(preview(ui, w));
                    });
                });
            } else {
                view3d_ui(ui, w, size);
                w.hover_preview = w.hover_preview.or(preview(ui, w));
            }
        }
        (RightView::Drawing, _) => crate::gear_simple::drawing(ui, w, vec2(width, tall(width))),
        (RightView::View3d, _) => view3d_ui(ui, w, vec2(width, tall(width))),
        (RightView::Profile, _) => w.hover_preview = preview(ui, w),
    }
}

/// The instant 3D view, its Teeth toggle and what it costs.
fn view3d_ui(ui: &mut Ui, w: &mut GearWindow, size: Vec2) {
    let (Ok(plan), Some(sch)) = (&w.plan, &w.schematic) else {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter().rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
        return;
    };
    let lit: Vec<usize> = w.lit().into_iter().collect();
    let inputs = crate::gear_view3d::Inputs { key: w.plan_gen, sch, plan, turned: w.pose(), lit: &lit, size, clear: w.built.clearance_body.as_ref() };
    let hovered = w.view3d.show(ui, inputs);
    w.hover_preview = hovered;
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut w.view3d.teeth, "Teeth").on_hover_text("The teeth from the build steps (off: the pitch surfaces, which show how the gears roll on each other)");
        if ui.small_button("Iso").on_hover_text("Back to the iso view, fitted (or double-click the view)").clicked() {
            w.view3d.reset();
        }
        if let Some((tris, ms, dropped)) = w.view3d.cost() {
            let more = if dropped > 0 { format!(", {dropped} without teeth (too many)") } else { String::new() };
            ui.label(egui::RichText::new(format!("{}k triangles, meshed in {ms:.0} ms{more}", tris / 1000)).small().weak())
                .on_hover_text("Drag to orbit, right-drag to pan, wheel to zoom, double-click to fit");
        }
    });
}

/// Simple mode's numbers: what the solver chose, then the warnings; everything else under
/// "All numbers".
fn simple_facts(ui: &mut Ui, w: &GearWindow) {
    if let Err(e) = &w.plan {
        ui.note(e, ui.visuals().error_fg_color);
        return;
    }
    if let Ok(p) = &w.plan {
        for warn in &p.warnings {
            ui.note(format!("⚠ {warn}"), ui.visuals().warn_fg_color);
        }
    }
    if !w.simple.chosen.is_empty() {
        egui::Grid::new("gear_simple_chosen").num_columns(2).striped(true).spacing([12.0, 2.0]).show(ui, |ui| {
            for f in w.simple.chosen.iter().take(10) {
                ui.label(egui::RichText::new(&f.label).small());
                ui.label(egui::RichText::new(fact_value(f)).small().monospace());
                ui.end_row();
            }
        });
    }
    egui::CollapsingHeader::new(egui::RichText::new("All numbers").small()).id_salt("gear_simple_all").show(ui, |ui| facts(ui, w));
}

pub(crate) fn fact_value(f: &solidcraft_gear::Fact) -> String {
    match f.unit {
        Unit::Mm => format!("{:.3} mm", f.value),
        Unit::Deg => format!("{:.3}°", f.value),
        Unit::None => format!("{:.4}", f.value),
    }
}

/// Advanced mode: the axes step, the type picker, every section, and the views.
fn advanced_body(ui: &mut Ui, w: &mut GearWindow, view: ViewState, view_error: Option<&str>) {
    if let Some(e) = w.editing.clone() {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("Editing {}", e.name)).strong());
            ui.label(egui::RichText::new("Update regenerates the set where it stands: its bodies keep their names, and its sketches and features their ids where the build steps stay the same").small().weak());
            if ui.small_button("New set instead").on_hover_text("Leave that set as it is and make another from these numbers").clicked() {
                w.editing = None;
            }
        });
        ui.add_space(4.0);
    }
    axes_step(ui, w);
    ui.add_space(4.0);
    type_picker(ui, w);
    ui.add_space(6.0);
    let full = ui.available_width();
    let left = (full * 0.42).clamp(240.0, 360.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(left);
            params(ui, w);
        });
        ui.add_space(8.0);
        ui.vertical(|ui| {
            let width = (full - left - 8.0).max(200.0);
            ui.set_width(width);
            w.hover_preview = None;
            right_views(ui, w, width);
            motion_row(ui, w);
            mesh_badge(ui, w);
            output_row(ui, w);
            view_row(ui, w, view, view_error);
            if let Some(e) = &w.error {
                ui.note(e, ui.visuals().error_fg_color);
            }
            facts(ui, w);
        });
    });
}

/// Turn, pause, speed, and back to the pose the bodies are built in.
fn motion_row(ui: &mut Ui, w: &mut GearWindow) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut w.animate, "Turn").on_hover_text("Turn the set in this preview. Off: the pose the bodies are built in, which is what the view shows");
        if w.animate {
            if ui.small_button(if w.paused { "Go on" } else { "Pause" }).on_hover_text(if w.paused { "Go on turning" } else { "Hold this pose" }).clicked() {
                w.paused = !w.paused;
            }
            ui.add(egui::Slider::new(&mut w.speed, 0.05..=2.0).show_value(false)).on_hover_text("How fast the preview turns");
            if w.paused && w.turned != 0.0 && ui.small_button("Reset to build pose").on_hover_text("Back to the pose the bodies are built in").clicked() {
                w.turned = 0.0;
            }
        }
        if w.pose() == 0.0 {
            ui.label(egui::RichText::new("build pose").small().weak()).on_hover_text("The set as it is built: the view's preview shows this pose");
        }
    });
}

/// The 2D mesh check's verdict on the set shown: green when every meshing pair meshes, red when
/// any cut into each other, amber when any lose contact; hidden for sets it cannot check (axes
/// that are not parallel). Pointing at it, or at one pair in its list, lights those members.
fn mesh_badge(ui: &mut Ui, w: &mut GearWindow) {
    w.hover_mesh = None;
    if w.plan.is_err() {
        return;
    }
    let Some(results) = w.mesh_now() else {
        ui.label(egui::RichText::new(solidcraft_ui_text::tr("gear.window.mesh_checking")).small().weak());
        return;
    };
    let Some(verdict) = solidcraft_gear::mesh2d::worst(results) else { return };
    let (text, color) = match verdict {
        Verdict::Ok => (solidcraft_ui_text::tr("gear.window.mesh_ok"), Color32::from_rgb(70, 170, 90)),
        Verdict::Interferes => (solidcraft_ui_text::tr("gear.window.mesh_interferes"), Color32::from_rgb(214, 72, 64)),
        Verdict::NoContact => (solidcraft_ui_text::tr("gear.window.mesh_no_contact"), Color32::from_rgb(220, 160, 40)),
    };
    let tip = results.iter().map(|r| r.message.as_str()).collect::<Vec<_>>().join("\n");
    let worst_at = results.iter().position(|r| r.verdict == verdict);
    let mut hovered = None;
    ui.horizontal(|ui| {
        let chip = egui::Frame::new().fill(color.gamma_multiply(0.22)).stroke(Stroke::new(1.0, color)).corner_radius(4.0).inner_margin(egui::Margin::symmetric(6, 1));
        let r = chip.show(ui, |ui| ui.label(egui::RichText::new(text).small().strong().color(color))).response.interact(Sense::hover());
        if r.on_hover_text(&tip).hovered() {
            hovered = worst_at;
        }
        ui.label(egui::RichText::new("2D mesh check over one tooth pitch").small().weak());
    });
    if results.len() > 1 {
        egui::CollapsingHeader::new(egui::RichText::new("each mesh").small()).id_salt("gear_mesh_list").show(ui, |ui| {
            for (i, r) in results.iter().enumerate() {
                let c = match r.verdict {
                    Verdict::Ok => ui.visuals().weak_text_color(),
                    Verdict::Interferes => Color32::from_rgb(214, 72, 64),
                    Verdict::NoContact => Color32::from_rgb(220, 160, 40),
                };
                if ui.label(egui::RichText::new(&r.message).small().color(c)).hovered() {
                    hovered = Some(i);
                }
            }
        });
    }
    w.hover_mesh = hovered;
}

/// The design sheet (Markdown, to the clipboard) and a member's profile as a DXF (through the
/// app's save dialog).
fn output_row(ui: &mut Ui, w: &mut GearWindow) {
    let Ok(plan) = &w.plan else { return };
    let kind = w.built.kind;
    let mut export = None;
    ui.horizontal_wrapped(|ui| {
        if ui.small_button(solidcraft_ui_text::tr("gear.window.copy_sheet")).on_hover_text("The inputs, every result, the mesh check and the parts, as a Markdown table, to the clipboard").clicked() {
            ui.ctx().copy_text(solidcraft_gear::sheet::design_sheet(&w.built, plan));
            w.output_note = Some("Design sheet copied".into());
        }
        ui.label(egui::RichText::new(solidcraft_ui_text::tr("gear.window.profile_dxf")).small().weak()).on_hover_text("A member's transverse profile (with its bore) as an R12 DXF in millimetres, centred on the origin");
        for (i, g) in plan.preview.iter().enumerate() {
            let Some(member) = member_of_outline(kind, i, &g.name) else { continue };
            let text = egui::RichText::new(&g.name).small().color(member_color(member));
            if ui.small_button(text).on_hover_text(format!("Save {}'s profile as a DXF", g.name)).clicked() {
                export = Some(i);
            }
        }
    });
    if export.is_some() {
        w.export = export;
    }
    if let Some(note) = &w.output_note {
        ui.label(egui::RichText::new(note).small().weak());
    }
}

/// Saves preview member `i`'s profile as a DXF, through the app's save dialog.
fn export_dxf(app: &mut SolveApp, w: &mut GearWindow, i: usize) {
    let Ok(plan) = &w.plan else { return };
    let Some(g) = plan.preview.get(i) else { return };
    let text = solidcraft_gear::sheet::region_dxf(&g.region, w.built.module);
    let stem: String = format!("{} {}", w.built.kind.name(), g.name).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    w.output_note = None;
    app.pick_save(&format!("{stem}.dxf"), &["dxf"], move |app, path| {
        if let Some(path) = path {
            match std::fs::write(&path, text) {
                Ok(()) => app.set_status(format!("Profile saved to {path}"), false),
                Err(e) => app.set_status(format!("Could not save {path}: {e}"), true),
            }
        }
    });
}

/// The view's preview: on or off, and whether it shows the numbers in the window yet.
fn view_row(ui: &mut Ui, w: &mut GearWindow, view: ViewState, view_error: Option<&str>) {
    ui.horizontal_wrapped(|ui| {
        match view {
            ViewState::Off => {
                let ok = w.plan.is_ok();
                if ui.add_enabled(ok, egui::Button::new("Show in model")).on_hover_text("Build this set once as a preview in the model's view (the full kernel build: can take a while). Any edit takes it down; Create builds it for good").clicked() {
                    w.model_preview = Some(w.plan_gen);
                }
                if w.model_preview.is_some() {
                    ui.label(egui::RichText::new("changed since it was shown: press again to see these numbers").small().weak());
                }
            }
            ViewState::Updating => {
                ui.spinner();
                ui.label(egui::RichText::new("building in the model…").small()).on_hover_text("The kernel is building this set for the model's view; the window stays live meanwhile");
                if ui.small_button("Cancel").clicked() {
                    w.model_preview = None;
                }
            }
            ViewState::Current => {
                if view_error.is_none() {
                    ui.label(egui::RichText::new("✓ the model shows this set").small().weak());
                }
                if ui.small_button("Hide").clicked() {
                    w.model_preview = None;
                }
            }
        }
    });
    if let Some(e) = view_error.filter(|_| view == ViewState::Current) {
        ui.note(format!("View preview: {e}"), ui.visuals().error_fg_color);
    }
}

fn type_picker(ui: &mut Ui, w: &mut GearWindow) {
    // With the axes set, the types that can't sit on them are greyed, with why.
    let fit = w.axes.axis(0).map(|_| axes::candidates(w.axes.relation().as_ref()));
    let mut pick = None;
    for (group, label) in [(Axes::Parallel, "Parallel axes"), (Axes::Intersecting, "Intersecting axes"), (Axes::Crossed, "Crossed axes"), (Axes::Wrapped, solidcraft_ui_text::tr("gear.window.group_wrapped")), (Axes::Special, "Special")] {
        ui.horizontal_wrapped(|ui| {
            ui.add_sized([110.0, 18.0], egui::Label::new(egui::RichText::new(label).small().weak()));
            for k in Kind::ALL.iter().copied().filter(|k| k.axes() == group) {
                let c = fit.as_ref().and_then(|f| f.iter().find(|c| c.kind == k));
                let fits = c.is_none_or(|c| c.fits);
                let r = ui.add_enabled_ui(fits, |ui| ui.selectable_label(w.spec.kind == k, k.label())).inner;
                let r = match c {
                    Some(c) => r.on_hover_text(&c.reason).on_disabled_hover_text(&c.reason),
                    None => r,
                };
                if r.clicked() {
                    pick = Some(k);
                }
            }
        });
    }
    if let Some(k) = pick {
        w.switch_kind(k);
    }
}

/// A row whose value the axes decided: shown, not editable.
fn locked(ui: &mut Ui, label: &str, value: String) {
    row(ui, label, "Worked out from the axes", |ui| {
        ui.label(egui::RichText::new(value).monospace());
        ui.label(egui::RichText::new("from the axes").small().weak());
    });
}

/// A row that follows another value (not the axes): shown, not editable.
fn locked_note(ui: &mut Ui, label: &str, value: String, why: &str) {
    row(ui, label, "", |ui| {
        ui.label(egui::RichText::new(value).monospace());
        ui.label(egui::RichText::new(why).small().weak());
    });
}

fn mm(v: f64) -> String {
    format!("{v:.3} mm")
}

fn row(ui: &mut Ui, label: &str, tip: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let r = ui.add_sized([128.0, 20.0], egui::Label::new(label).truncate());
        if !tip.is_empty() {
            r.on_hover_text(tip);
        }
        add(ui);
    });
}

fn num(ui: &mut Ui, label: &str, tip: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, unit: &str) {
    row(ui, label, tip, |ui| {
        ui.add(egui::DragValue::new(v).range(range).speed(speed).max_decimals(4).suffix(unit));
    });
}

fn count(ui: &mut Ui, label: &str, tip: &str, v: &mut u32, range: std::ops::RangeInclusive<u32>) {
    row(ui, label, tip, |ui| {
        ui.add(egui::DragValue::new(v).range(range).speed(0.2));
    });
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(text).strong());
}

/// A member's section header: its colour marker and its name, on a band of its colour while the
/// member is lit.
fn member_header(ui: &mut Ui, label: &str, color: Color32, lit: bool) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    let painter = ui.painter();
    if lit {
        painter.rect_filled(rect, 4.0, color.gamma_multiply(0.25));
    }
    let side = if lit { 14.0 } else { 11.0 };
    let mark = egui::Rect::from_center_size(pos2(rect.left() + 11.0, rect.center().y), vec2(side, side));
    painter.rect_filled(mark, 3.0, color);
    let ink = if lit { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
    painter.text(pos2(rect.left() + 26.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.0), ink);
}

/// What a settings section works on: the numbers as typed (`s`), the set as built (`b`: what
/// the axes decided is read from here), which of them the axes decided (`d`), and the window's
/// options (`o`).
struct Cx<'a> {
    s: &'a mut Spec,
    b: &'a Spec,
    d: Derived,
    k: Kind,
    /// The set is placed by the Axes step (no "Centre at").
    on_axes: bool,
    o: &'a mut Opts,
    /// The Axes step (a suggestion taken turns its ratio and standard module off, so the
    /// suggested numbers are the ones built), and the distance between parallel axes.
    axes: &'a mut AxesStep,
    axis_distance: Option<f64>,
    /// Design by goal: the goal, whether its panel is open, and the last suggestions.
    goal: &'a mut Goal,
    suggest_open: &'a mut bool,
    suggestions: &'a mut Option<(Goal, Result<Vec<Suggestion>, String>)>,
}

/// The settings column. One function per section, in this order; a member's rows go in
/// [`section_member`], which is drawn once per member under that member's coloured header.
///
/// EXTENSION POINTS. Controls for new `Spec` fields go where the field belongs:
/// - one number for the whole set (a new family's size, a mesh option): [`section_set`];
/// - something each gear has for itself (teeth, shift, bore, material; integral shafts and
///   dual shafts, per-gear chamfers, per-gear hub hardware): [`section_member`], or the
///   per-member stubs [`member_shafts`], [`member_chamfers`], [`member_hardware`] it calls;
/// - the tooth form (rack, clearance, tip relief, crowning): [`section_tooth_form`];
/// - printing (print profile, XY compensation, elephant-foot relief): [`section_print`];
/// - hubs, holes and other shared body features: [`section_body`];
/// - inputs only one family has (hypoid cutter data, a new family's own): [`section_family`];
/// - the strength rating: [`section_load`]; what is built and where (and the print test
///   coupon): [`section_output`]; design by goal: [`section_suggest`];
/// - new tables (shafts, chamfers, print tuning, hardware from `gear-body`; hypoid inputs): a
///   section function of their own, called from the list below, or the member stubs.
fn params(ui: &mut Ui, w: &mut GearWindow) {
    // What the axes decided is shown from the solved set, read-only.
    let d: Derived = w.solved.as_ref().map(|s| s.derived).unwrap_or_default();
    let b = w.built.clone();
    let lit = w.lit();
    let k = w.spec.kind;
    let on_axes = w.axes.slots[0].is_some();
    let (names, warnings, facts) = w.clearance_inputs();
    let axis_distance = match w.axes.relation() {
        Some(Relation::Parallel { distance, .. }) => Some(distance),
        _ => None,
    };
    let mut cx = Cx {
        s: &mut w.spec,
        b: &b,
        d,
        k,
        on_axes,
        o: &mut w.opts,
        axes: &mut w.axes,
        axis_distance,
        goal: &mut w.goal,
        suggest_open: &mut w.suggest_open,
        suggestions: &mut w.suggestions,
    };
    let mut hovered = None;
    let mut edited = None;
    egui::ScrollArea::vertical().id_salt("gear_params").max_height(470.0).auto_shrink([false, true]).show(ui, |ui| {
        section_set(ui, &mut cx);
        section_suggest(ui, &mut cx);
        section_family(ui, &mut cx);
        for (m, label) in members(k).iter().enumerate() {
            let before = cx.s.clone();
            let section = ui.scope(|ui| {
                member_header(ui, label, member_color(m), lit == Some(m));
                section_member(ui, &mut cx, m);
                member_shafts(ui, &mut cx, m);
                member_chamfers(ui, &mut cx, m);
                member_hardware(ui, &mut cx, m);
            });
            if ui.rect_contains_pointer(section.response.rect) {
                hovered = Some(m);
            }
            if *cx.s != before {
                edited = Some(m);
            }
        }
        section_chamfers(ui, &mut cx);
        section_tooth_form(ui, &mut cx);
        section_print(ui, &mut cx);
        section_body(ui, &mut cx);
        crate::gear_clearance_ui::section(ui, cx.s, true, &names, &warnings, &facts);
        section_load(ui, &mut cx);
        section_output(ui, &mut cx);
    });
    w.hover_section = hovered;
    if let Some(m) = edited {
        w.edited = Some((m, crate::now_ms()));
    }
}

/// The numbers the whole set shares: module, angles, centre distance, backlash, face width.
fn section_set(ui: &mut Ui, cx: &mut Cx<'_>) {
    let (b, d, k) = (cx.b, cx.d, cx.k);
    let s = &mut *cx.s;
    let o = &mut *cx.o;
    heading(ui, if k.is_set() { "Whole set" } else { "Size" });
    if matches!(k, Kind::Sprocket | Kind::Pulley) {
        wrapped_rows(ui, s, b, d);
        return;
    }
    if k == Kind::Hypoid {
        hypoid_size(ui, s, b, d);
        return;
    }
    if k == Kind::CycloidalDrive {
        num(ui, "Pin circle Ø", "Diameter through the pin centres", &mut s.pin_circle, 1.0..=5000.0, 0.5, " mm");
        num(ui, "Eccentricity", "Offset of the disc's bearing; under pin circle radius ÷ pins", &mut s.eccentricity, 0.01..=100.0, 0.05, " mm");
        num(ui, "Clearance", "Taken off the disc round the pins", &mut s.backlash, 0.0..=5.0, 0.01, " mm");
        num(ui, "Thickness", "", &mut s.face_width, 0.1..=5000.0, 0.1, " mm");
        return;
    }
    let module = if k == Kind::Worm { "Axial module" } else { "Module" };
    if d.module {
        locked(ui, module, mm(b.module));
    } else if o.inch {
        let mut dp = 25.4 / s.module;
        row(ui, "Diametral pitch", "Teeth per inch of pitch diameter", |ui| {
            if ui.add(egui::DragValue::new(&mut dp).range(0.5..=200.0).speed(0.1).max_decimals(3)).changed() {
                s.module = 25.4 / dp;
            }
            ui.checkbox(&mut o.inch, "inch");
        });
    } else {
        row(ui, module, "Pitch diameter ÷ teeth (mm)", |ui| {
            ui.add(egui::DragValue::new(&mut s.module).range(0.05..=100.0).speed(0.01).max_decimals(4).suffix(" mm"));
            ui.checkbox(&mut o.inch, "inch");
        });
    }
    if k.involute() {
        num(ui, "Pressure angle", "Normal pressure angle; 20° is standard, 14.5° older, 25° stronger", &mut s.pressure_angle, 5.0..=45.0, 0.1, "°");
    }
    if matches!(k, Kind::Helical | Kind::Herringbone | Kind::Internal | Kind::Rack | Kind::Pair | Kind::Planetary) {
        num(ui, "Helix angle", "0 for straight teeth; the sign is the hand (gear 2 gets the opposite)", &mut s.helix, -60.0..=60.0, 0.2, "°");
    }
    if matches!(k, Kind::Bevel | Kind::SpiralBevel | Kind::Hypoid | Kind::CrossedHelical) {
        if d.shaft_angle {
            locked(ui, "Shaft angle", format!("{:.3}°", b.shaft_angle));
        } else {
            num(ui, "Shaft angle", "Angle between the axes", &mut s.shaft_angle, 5.0..=175.0, 0.5, "°");
        }
    }
    if k == Kind::SpiralBevel {
        num(ui, "Spiral angle", "Mean spiral angle; 35° is usual", &mut s.spiral_angle, 0.0..=60.0, 0.5, "°");
    }
    if k == Kind::Pair && d.center_distance {
        let a = if b.center_distance > 0.0 { b.center_distance } else { b.module * f64::from(b.teeth + b.teeth2) / 2.0 / b.helix.to_radians().cos() };
        locked(ui, "Centre distance", mm(a));
    } else if k == Kind::Pair {
        let mut fixed = s.center_distance > 0.0;
        row(ui, "Centre distance", "Set it to have gear 2's shift worked out", |ui| {
            if ui.checkbox(&mut fixed, "").changed() {
                s.center_distance = if fixed { s.module * f64::from(s.teeth + s.teeth2) / 2.0 / (s.helix.to_radians().cos()) } else { 0.0 };
            }
            if fixed {
                ui.add(egui::DragValue::new(&mut s.center_distance).range(0.1..=1e5).speed(0.05).max_decimals(4).suffix(" mm"));
            }
        });
    }
    match k {
        Kind::Spline => {
            num(ui, "Flank clearance", "Taken off the shaft's and the hub's flanks: the fit's side clearance", &mut s.backlash, 0.0..=5.0, 0.005, " mm");
            num(ui, "Length", "The spline's engaged length", &mut s.face_width, 0.1..=5000.0, 0.1, " mm");
        }
        Kind::FaceGear => {
            num(ui, "Backlash", "Taken off each tooth at the pitch circle", &mut s.backlash, 0.0..=5.0, 0.005, " mm");
            num(ui, "Tooth length", "Along the face gear's radius, from the inner end", &mut s.face_width, 0.1..=5000.0, 0.1, " mm");
        }
        _ => {
            num(ui, "Backlash", "Taken off each tooth at the pitch circle", &mut s.backlash, 0.0..=5.0, 0.005, " mm");
            num(ui, "Face width", "", &mut s.face_width, 0.1..=5000.0, 0.1, " mm");
        }
    }
}

/// A hypoid set's size: by the ring gear's diameter (which sets the module), its angles, its
/// face width and backlash. The axes, when set, decide the shaft angle (and the offset, in
/// [`section_family`]).
fn hypoid_size(ui: &mut Ui, s: &mut Spec, b: &Spec, d: Derived) {
    let mut ring = s.module * f64::from(s.teeth2.max(1));
    row(ui, "Ring gear Ø", "The ring gear's outer pitch diameter (module × ring gear teeth); sets the module", |ui| {
        if ui.add(egui::DragValue::new(&mut ring).range(5.0..=10000.0).speed(0.5).max_decimals(3).suffix(" mm")).changed() {
            s.module = ring / f64::from(s.teeth2.max(1));
        }
        ui.label(egui::RichText::new(format!("module {:.4}", s.module)).small().weak());
    });
    if d.shaft_angle {
        locked(ui, "Shaft angle", format!("{:.3}°", b.shaft_angle));
    } else {
        num(ui, "Shaft angle", "Angle between the axes; 90° in car axles", &mut s.shaft_angle, 5.0..=175.0, 0.5, "°");
    }
    num(ui, "Pressure angle", "Normal pressure angle; 20° is usual", &mut s.pressure_angle, 5.0..=45.0, 0.1, "°");
    num(ui, "Ring gear face width", "Along the ring gear's teeth; over a third of the cone distance is warned about", &mut s.face_width, 0.1..=5000.0, 0.5, " mm");
    num(ui, "Backlash", "Taken off each tooth at the pitch circle", &mut s.backlash, 0.0..=5.0, 0.005, " mm");
}

/// Chain sprockets and timing pulleys: the chain or belt, and the drive between two of them
/// (they have no module or pressure angle; the chain or belt sets the teeth's form).
fn wrapped_rows(ui: &mut Ui, s: &mut Spec, b: &Spec, d: Derived) {
    if s.kind == Kind::Sprocket {
        let current = chain_by_id(s.chain).map(|c| c.label).unwrap_or("custom");
        row(ui, "Chain", "ISO 606 B series, ANSI B29.1 (ISO 606 A series), or sizes of your own", |ui| {
            egui::ComboBox::from_id_salt("gear_chain").selected_text(current).show_ui(ui, |ui| {
                for c in CHAINS {
                    ui.selectable_value(&mut s.chain, c.id, c.label);
                }
                ui.selectable_value(&mut s.chain, CUSTOM, "custom");
            });
        });
        if chain_by_id(s.chain).is_none() {
            num(ui, "  Pitch", "", &mut s.chain_pitch, 1.0..=200.0, 0.05, " mm");
            num(ui, "  Roller Ø", "", &mut s.roller_diameter, 0.5..=150.0, 0.05, " mm");
            num(ui, "  Inner width", "Between the inner plates", &mut s.roller_width, 0.5..=150.0, 0.05, " mm");
            num(ui, "  Strand pitch", "Transverse pitch of a duplex or triplex chain", &mut s.strand_pitch, 0.5..=300.0, 0.05, " mm");
        }
        count(ui, "Strands", "1 simplex, 2 duplex, 3 triplex", &mut s.strands, 1..=4);
        row(ui, "Side chamfer", "ISO 606 tooth side radius and chamfer, so the chain runs on", |ui| {
            ui.checkbox(&mut s.side_chamfer, "");
        });
    } else {
        let current = belt_by_id(s.belt).map(|b| b.label).unwrap_or("?");
        row(ui, "Belt", "Groove forms approximated from published belt dimensions", |ui| {
            egui::ComboBox::from_id_salt("gear_belt").selected_text(current).show_ui(ui, |ui| {
                for belt in BELTS {
                    if ui.selectable_value(&mut s.belt, belt.id, belt.label).changed()
                        && let Some(width) = belt.widths.first()
                    {
                        s.face_width = *width;
                    }
                }
            });
        });
        num(ui, "Belt width", "The pulley's groove is this wide between the flanges", &mut s.face_width, 0.5..=500.0, 0.5, " mm");
        row(ui, "Flanges", "Keep the belt on", |ui| {
            ui.checkbox(&mut s.flanges, "");
        });
    }
    let second = if d.teeth2 { b.teeth2 } else { s.teeth2 };
    if second > 0 {
        if d.center_distance {
            locked(ui, "Centre distance", mm(b.center_distance));
        } else {
            let tip = if s.kind == Kind::Sprocket { "0: 40 pitches; rounded to an even number of links" } else { "0: three pitch diameters; rounded to a whole belt" };
            num(ui, "Centre distance", tip, &mut s.center_distance, 0.0..=1e5, 0.5, " mm");
        }
    }
}

/// Design by goal (gear pairs): a ratio, limits and a preference in, a ranked list of pairs
/// out, any of which can be taken into the window. Worked out on the button, never per
/// keystroke. The tooth form, helix, backlash, face width and load are the window's own.
fn section_suggest(ui: &mut Ui, cx: &mut Cx<'_>) {
    if cx.k != Kind::Pair {
        return;
    }
    ui.add_space(4.0);
    ui.toggle_value(cx.suggest_open, egui::RichText::new(solidcraft_ui_text::tr("gear.window.suggest_open")).strong())
        .on_hover_text("Find gear pairs for a ratio within limits (size, centre distance, modules, teeth), ranked by what matters most");
    if !*cx.suggest_open {
        return;
    }
    let g = &mut *cx.goal;
    num(ui, "  Ratio", "Gear 2's teeth ÷ gear 1's", &mut g.ratio, 0.05..=100.0, 0.01, "");
    let mut tol = g.ratio_tolerance * 100.0;
    row(ui, "  Within", "How far the ratio may be off (0: exact)", |ui| {
        if ui.add(egui::DragValue::new(&mut tol).range(0.0..=20.0).speed(0.05).max_decimals(2).suffix(" %")).changed() {
            g.ratio_tolerance = tol / 100.0;
        }
    });
    match cx.axis_distance {
        Some(a) => {
            g.center_distance = a;
            locked(ui, "  Centre distance", mm(a));
        }
        None => num(ui, "  Centre distance", "0: free", &mut g.center_distance, 0.0..=1e5, 0.5, " mm"),
    }
    num(ui, "  Largest tip Ø", "Of either gear; 0: free", &mut g.max_outer_diameter, 0.0..=1e5, 1.0, " mm");
    row(ui, "  Modules", "The standard series from … to", |ui| {
        ui.add(egui::DragValue::new(&mut g.min_module).range(0.05..=100.0).speed(0.05).max_decimals(3));
        ui.label("to");
        ui.add(egui::DragValue::new(&mut g.max_module).range(0.05..=100.0).speed(0.05).max_decimals(3));
        ui.checkbox(&mut g.second_choice_modules, "2nd choice");
    });
    row(ui, "  Teeth", "Fewest (0: no undercut) and most on a gear", |ui| {
        ui.add(egui::DragValue::new(&mut g.min_teeth).range(0..=400).speed(0.2));
        ui.label("to");
        ui.add(egui::DragValue::new(&mut g.max_teeth).range(6..=2000).speed(0.5));
    });
    row(ui, "  Profile shift", "May be used to avoid undercut on few teeth and to reach a centre distance", |ui| {
        ui.checkbox(&mut g.allow_shift, "allowed");
    });
    row(ui, "  Prefer", "How the suggestions are ranked", |ui| {
        egui::ComboBox::from_id_salt("gear_goal_prefer").selected_text(g.prefer.label()).show_ui(ui, |ui| {
            for p in Prefer::ALL {
                ui.selectable_value(&mut g.prefer, p, p.label());
            }
        });
    });
    row(ui, "  Show", "Most suggestions to list", |ui| {
        ui.add(egui::DragValue::new(&mut g.limit).range(1..=50).speed(0.2));
    });
    // The rest of the goal is the window's own.
    let s = &*cx.s;
    g.pressure_angle = s.pressure_angle;
    g.helix = s.helix;
    g.rack = s.rack;
    g.backlash = s.backlash;
    g.face_width = s.face_width;
    g.load = s.load.clone();
    if ui.button(solidcraft_ui_text::tr("gear.window.suggest")).clicked() {
        let found = solidcraft_gear::design::check(g).map(|()| solidcraft_gear::design::suggest(g));
        *cx.suggestions = Some((g.clone(), found));
    }
    let mut take = None;
    match cx.suggestions.as_ref() {
        None => {}
        Some((_, Err(e))) => {
            ui.note(e, ui.visuals().error_fg_color);
        }
        Some((_, Ok(list))) if list.is_empty() => {
            ui.note("No pair meets this goal: widen the modules, the teeth or the ratio tolerance", ui.visuals().warn_fg_color);
        }
        Some((asked, Ok(list))) => {
            if asked != &*g {
                ui.label(egui::RichText::new("The goal changed since: press Suggest again").small().weak());
            }
            egui::Grid::new("gear_suggestions").num_columns(7).striped(true).spacing([8.0, 2.0]).show(ui, |ui| {
                for h in ["z₁ : z₂", "m", "ratio", "a", "tip Ø", "SF / SH", ""] {
                    ui.label(egui::RichText::new(h).small().weak());
                }
                ui.end_row();
                for (i, sg) in list.iter().enumerate() {
                    let small = |t: String| egui::RichText::new(t).small().monospace();
                    ui.label(small(format!("{} : {}{}", sg.teeth, sg.teeth2, if sg.hunting { " ·" } else { "" })))
                        .on_hover_text(if sg.hunting { "Hunting tooth: every tooth meets every tooth of the mate" } else { "The counts share a factor" });
                    ui.label(small(format!("{}", sg.module)));
                    ui.label(small(format!("{:.4}", sg.ratio))).on_hover_text(format!("{:+.3} % from the goal", sg.ratio_error * 100.0));
                    ui.label(small(format!("{:.2}", sg.center_distance)));
                    ui.label(small(format!("{:.1}", sg.outer_diameter.max(sg.outer_diameter2))));
                    let safety = match (sg.safety_bending, sg.safety_contact) {
                        (Some(f), Some(h)) => format!("{f:.2} / {h:.2}"),
                        _ => "–".into(),
                    };
                    ui.label(small(safety)).on_hover_text(format!("contact ratio {:.3}", sg.total_contact_ratio));
                    if ui.small_button("Use").on_hover_text("Take this pair into the window").clicked() {
                        take = Some(i);
                    }
                    ui.end_row();
                }
            });
        }
    }
    if let Some((asked, Ok(list))) = cx.suggestions.as_ref()
        && let Some(sg) = take.and_then(|i| list.get(i))
    {
        let t = sg.spec(asked);
        let s = &mut *cx.s;
        (s.module, s.teeth, s.teeth2, s.shift, s.shift2, s.face_width) = (t.module, t.teeth, t.teeth2, t.shift, t.shift2, t.face_width);
        // A centre distance the goal fixed and no axes give is kept as the set's own.
        if cx.axis_distance.is_none() {
            s.center_distance = if asked.center_distance > 0.0 { sg.center_distance } else { 0.0 };
        }
        // The suggested numbers are the ones built: the axes no longer choose them.
        cx.axes.use_ratio = false;
        cx.axes.auto_module = false;
    }
}

/// Inputs only one family has. EXTENSION POINT: a new family's own inputs, or more of an
/// existing family's (hypoid cutter radius, pinion offset side, worm throat), go here under a
/// `if k == Kind::…` of their own.
fn section_family(ui: &mut Ui, cx: &mut Cx<'_>) {
    let (b, d, k) = (cx.b, cx.d, cx.k);
    let s = &mut *cx.s;
    if k == Kind::Hypoid {
        if d.offset {
            locked(ui, "Pinion offset", mm(b.offset));
        } else {
            num(
                ui,
                "Pinion offset",
                "Distance between the axes; 0 makes a spiral bevel pair. Car axles use about 10–15% of the ring gear Ø",
                &mut s.offset,
                0.0..=5000.0,
                0.5,
                " mm",
            );
        }
        num(ui, "Pinion spiral angle", "Mean spiral angle of the pinion; the ring gear's is smaller by the offset angle. 45–50° is usual", &mut s.spiral_angle, 0.0..=60.0, 0.5, "°");
        let hand_tip = "Left is usual (car axles: pinion below centre); the ring gear gets the other hand";
        if d.offset && b.left_hand != s.left_hand {
            // The axes cross the other way round: the solve took the hand that fits them.
            locked_note(ui, "Pinion hand", (if b.left_hand { "left" } else { "right" }).to_string(), "to fit how the axes cross");
        } else {
            row(ui, "Pinion hand", hand_tip, |ui| {
                ui.radio_value(&mut s.left_hand, true, "left");
                ui.radio_value(&mut s.left_hand, false, "right");
            });
        }
    }
    match k {
        Kind::Spline => {
            row(ui, "Standard", "", |ui| {
                egui::ComboBox::from_id_salt("gear_spline_std").selected_text(s.spline_standard.label()).show_ui(ui, |ui| {
                    for (std, _, label) in SplineStandard::ALL {
                        ui.selectable_value(&mut s.spline_standard, std, label);
                    }
                });
            });
            if s.spline_standard == SplineStandard::Din5480 {
                num(ui, "Reference Ø d_B", "The nominal diameter; sets the profile shift. 0: module × (teeth + 1.1)", &mut s.spline_diameter, 0.0..=5000.0, 0.1, " mm");
            }
            row(ui, "Fillet root", "A full round root instead of a flat one", |ui| {
                ui.checkbox(&mut s.fillet_root, "");
            });
        }
        Kind::Elliptical => {
            num(ui, "Eccentricity", "Of gear 1's ellipse; 0 is a circle. The mate is its exact conjugate", &mut s.ellipse_eccentricity, 0.0..=0.7, 0.005, "");
        }
        Kind::FaceGear => {
            num(ui, "Teeth inner Ø", "Where the teeth start; 0: the undercut limit", &mut s.inner_diameter, 0.0..=1e5, 0.5, " mm");
        }
        _ => {}
    }
}

/// One member's own numbers (member `m` of [`members`]): teeth, profile shift, bore, material,
/// and what only that member has (the worm's starts and diameter, the ring's rim, the rack's
/// length, the pins).
fn section_member(ui: &mut Ui, cx: &mut Cx<'_>, m: usize) {
    let (b, d, k) = (cx.b, cx.d, cx.k);
    let rated = k.involute() && k != Kind::Rack && (cx.o.load_open || cx.s.load.torque > 0.0 || cx.s.load.power > 0.0);
    let s = &mut *cx.s;
    match (k, m) {
        (Kind::CycloidalDrive, 0) => {
            count(ui, "Output holes", "Holes for the output pins (0: none)", &mut s.holes, 0..=24);
            if s.holes > 0 {
                num(ui, "Output pin Ø", "0: 15% of the pin circle", &mut s.hole_diameter, 0.0..=500.0, 0.1, " mm");
            }
            bore(ui, "Bearing bore", "bore1", &mut s.bore);
        }
        (Kind::CycloidalDrive, _) => {
            count(ui, "Pins", "The disc gets one lobe fewer; the reduction is pins − 1", &mut s.pins, 3..=200);
            num(ui, "Pin Ø", "", &mut s.pin_diameter, 0.1..=500.0, 0.1, " mm");
        }
        (Kind::Rack, _) => {
            count(ui, "Teeth along", "", &mut s.rack_teeth, 1..=2000);
            num(ui, "Profile shift x", "Profile shift coefficient: positive thickens the root", &mut s.shift, -1.5..=1.5, 0.01, "");
            num(ui, "Height below pitch", "0: 3 modules", &mut s.rack_height, 0.0..=1000.0, 0.1, " mm");
        }
        (Kind::Worm, 1) => {
            count(ui, "Starts", "Threads on the worm; ratio = wheel teeth ÷ starts", &mut s.starts, 1..=12);
            if d.worm_diameter {
                locked(ui, "Pitch Ø", mm(b.worm_diameter));
            } else {
                num(ui, "Pitch Ø", "0: 10 modules", &mut s.worm_diameter, 0.0..=5000.0, 0.1, " mm");
            }
            num(ui, "Length", "0: enough to engage the wheel", &mut s.worm_length, 0.0..=10000.0, 0.5, " mm");
            let mut left = s.helix < 0.0;
            row(ui, "Hand", "", |ui| {
                ui.radio_value(&mut left, false, "right");
                ui.radio_value(&mut left, true, "left");
            });
            s.helix = if left { -1.0 } else { 0.0 };
            gear2_rows(ui, s, k);
            bore(ui, "Bore", "bore2", &mut s.bore2);
            if rated {
                material(ui, "Material", "gear_mat2", &mut s.load.material2);
            }
        }
        (Kind::Hypoid, 0) => {
            if d.teeth {
                locked(ui, "Teeth", b.teeth.to_string());
            } else {
                count(ui, "Teeth", "Car axles: 9–13", &mut s.teeth, 3..=s.teeth2.saturating_sub(1).max(3));
            }
            num(ui, "Shank Ø", "The pinion's shank; 0: auto", &mut s.hub_diameter, 0.0..=5000.0, 0.2, " mm");
            num(ui, "Shank length", "0: auto", &mut s.hub_length, 0.0..=5000.0, 0.2, " mm");
            bore(ui, "Pinion bore", "bore1", &mut s.bore);
        }
        (Kind::Hypoid, _) => {
            if d.teeth2 {
                locked(ui, "Teeth", b.teeth2.to_string());
            } else {
                count(ui, "Teeth", "41 in many car axles", &mut s.teeth2, 4..=2000);
            }
            num(ui, "Ring gear web", "Thickness behind the teeth; 0: 6% of its Ø", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
            bore(ui, "Ring gear bore", "bore2", &mut s.bore2);
        }
        (Kind::Sprocket | Kind::Pulley, 0) => {
            if d.teeth {
                locked(ui, "Teeth", b.teeth.to_string());
            } else {
                let (tip, most) = if k == Kind::Sprocket { ("17 or more runs smoothly", 250) } else { ("", 400) };
                count(ui, "Teeth", tip, &mut s.teeth, 6..=most);
            }
            bore(ui, "Bore", "bore1", &mut s.bore);
        }
        (Kind::Sprocket | Kind::Pulley, _) => {
            if d.teeth2 {
                locked(ui, "Teeth", b.teeth2.to_string());
            } else {
                let most = if k == Kind::Sprocket { 250 } else { 400 };
                let tip = if k == Kind::Sprocket { "Joined by the chain; 0: no second sprocket" } else { "Joined by the belt; 0: no second pulley" };
                count(ui, "Teeth", tip, &mut s.teeth2, 0..=most);
            }
            bore(ui, "Bore", "bore2", &mut s.bore2);
        }
        (Kind::Spline, 0) => {
            count(ui, "Teeth", "", &mut s.teeth, 6..=200);
            bore(ui, "Bore", "bore1", &mut s.bore);
        }
        (Kind::Spline, _) => {
            locked_note(ui, "Teeth", b.teeth.to_string(), "as the shaft");
            num(ui, "Outer Ø", "The hub's outside; 0: the rim below", &mut s.hub_diameter, 0.0..=5000.0, 0.2, " mm");
            if s.hub_diameter <= 0.0 {
                num(ui, "Rim", "Beyond the tooth roots; 0: about 3 modules", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
            }
        }
        (Kind::FaceGear, 0) => {
            if d.teeth2 {
                locked(ui, "Teeth", b.teeth2.to_string());
            } else {
                count(ui, "Teeth", "More than the pinion's by 2 or more", &mut s.teeth2, 8..=2000);
            }
            num(ui, "Base", "Below the tooth spaces; 0: 2.5 modules", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
            bore(ui, "Bore", "bore1", &mut s.bore);
        }
        (Kind::FaceGear, _) => {
            if d.teeth {
                locked(ui, "Teeth", b.teeth.to_string());
            } else {
                count(ui, "Teeth", "The spur pinion that cuts the face gear's teeth", &mut s.teeth, 6..=200);
            }
            bore(ui, "Bore", "bore2", &mut s.bore2);
        }
        (Kind::Elliptical, 1) => {
            locked_note(ui, "Teeth", b.teeth.to_string(), "as gear 1");
            bore(ui, "Bore", "bore2", &mut s.bore2);
        }
        (Kind::Planetary, 2) => {
            let mut ring = if s.teeth3 > 0 { s.teeth3 } else { s.teeth + 2 * s.teeth2 };
            row(ui, "Teeth", "Sun + 2 × planet for standard gears", |ui| {
                if ui.add(egui::DragValue::new(&mut ring).range(6..=4000).speed(0.2)).changed() {
                    s.teeth3 = ring;
                }
                if ui.small_button("auto").clicked() {
                    s.teeth3 = 0;
                }
            });
            num(ui, "Rim", "Beyond the roots; 0: 3 modules", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
            if rated {
                ring_material(ui, &mut s.load.material3);
            }
        }
        (_, 0) => {
            if d.teeth {
                locked(ui, "Teeth", b.teeth.to_string());
            } else {
                count(ui, "Teeth", "", &mut s.teeth, 3..=2000);
            }
            if k.involute() && !matches!(k, Kind::Worm | Kind::Elliptical | Kind::Hypoid) {
                num(ui, if k.is_set() { "Shift x₁" } else { "Profile shift x" }, "Profile shift coefficient: positive thickens the root and avoids undercut", &mut s.shift, -1.5..=1.5, 0.01, "");
                if k == Kind::Pair && d.center_distance && d.shift {
                    locked(ui, "  x₁ (auto while 0)", format!("{:.4}", b.shift));
                }
            }
            if k == Kind::CrossedHelical {
                if d.helix {
                    locked(ui, "Helix", format!("{:.3}°", b.helix));
                } else {
                    num(ui, "Helix", "Gear 2 takes the rest of the shaft angle", &mut s.helix, -75.0..=75.0, 0.5, "°");
                }
            }
            if k == Kind::Internal {
                num(ui, "Rim", "Beyond the roots; 0: 3 modules", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
            }
            if cyl(k) || k == Kind::Pair {
                num(ui, "Tip Ø", "The finished tip diameter; 0: from the rack. Only lowers the tips: a larger value, or one leaving under ¼ module of addendum, is ignored with a warning", &mut s.tip_diameter, 0.0..=1e5, 0.1, " mm");
            }
            bore(ui, "Bore", "bore1", &mut s.bore);
            if rated {
                material(ui, "Material", "gear_mat1", &mut s.load.material);
            }
        }
        (_, _) => {
            if d.teeth2 {
                locked(ui, "Teeth", b.teeth2.to_string());
            } else if k == Kind::Cycloidal {
                count(ui, "Teeth", "0: no pinion", &mut s.teeth2, 0..=2000);
            } else {
                count(ui, "Teeth", "", &mut s.teeth2, 3..=2000);
            }
            if k == Kind::Planetary {
                count(ui, "Planets", "(sun + ring) ÷ planets whole spaces them evenly", &mut s.planets, 1..=24);
            }
            if k == Kind::Pair && d.center_distance {
                locked(ui, "Shift x₂", format!("{:.4}", b.shift2));
            } else if matches!(k, Kind::Pair | Kind::Planetary | Kind::CrossedHelical) {
                num(ui, "Shift x₂", "This gear's profile shift", &mut s.shift2, -1.5..=1.5, 0.01, "");
            }
            gear2_rows(ui, s, k);
            bore(ui, "Bore", "bore2", &mut s.bore2);
            if rated {
                material(ui, "Material", "gear_mat2", &mut s.load.material2);
            }
        }
    }
}

/// Gear 2's own tip diameter, backlash, face width and face offset, where its kind takes them.
fn gear2_rows(ui: &mut Ui, s: &mut Spec, k: Kind) {
    if k == Kind::Pair {
        num(ui, "Tip Ø", "Gear 2's finished tip diameter; 0: from the rack. Only lowers the tips", &mut s.tip_diameter2, 0.0..=1e5, 0.1, " mm");
    }
    if matches!(k, Kind::Pair | Kind::Planetary | Kind::CrossedHelical | Kind::Worm) {
        let mut own = s.backlash2.is_some();
        row(ui, "Own backlash", "This gear's own backlash (off: the same as gear 1's); the mesh's backlash is the two added together", |ui| {
            if ui.checkbox(&mut own, "").changed() {
                s.backlash2 = own.then_some(s.backlash);
            }
            if let Some(b2) = &mut s.backlash2 {
                ui.add(egui::DragValue::new(b2).range(0.0..=5.0).speed(0.005).max_decimals(4).suffix(" mm"));
            }
        });
    }
    if k == Kind::Pair {
        num(ui, "Face width", "Gear 2's own face width; 0: the same as gear 1's", &mut s.face_width2, 0.0..=5000.0, 0.1, " mm");
        num(ui, "Face offset", "How far gear 2's mid plane sits from gear 1's along the axis; faces that don't overlap are refused", &mut s.face_offset2, -5000.0..=5000.0, 0.1, " mm");
    }
}

/// A member's integral shafts: one from the front face (through the hub) and, with Dual shaft
/// on, one from the back face, each with an optional step, an end feature and an end chamfer.
fn member_shafts(ui: &mut Ui, cx: &mut Cx<'_>, m: usize) {
    let Some(g) = body_gear(cx.k, m) else { return };
    let s = &mut *cx.s;
    let dual = &mut cx.o.dual[usize::from(g - 1)];
    let (front, back) = if g == 1 { (&mut s.shaft_front, &mut s.shaft_back) } else { (&mut s.shaft2_front, &mut s.shaft2_back) };
    let id = format!("gear_shaft{g}");
    shaft_rows(ui, "Shaft", "From the front face (the hub's side), through the hub; 0: none", &format!("{id}f"), front);
    row(ui, "Dual shaft", "A second shaft from the back face", |ui| {
        if ui.checkbox(dual, "").changed() && !*dual {
            *back = Shaft::default();
        }
    });
    if *dual {
        shaft_rows(ui, "Back shaft", "From the back face (on the print bed); a bearing pocket is left out with it", &format!("{id}b"), back);
    }
}

fn shaft_rows(ui: &mut Ui, label: &str, tip: &str, id: &str, sh: &mut Shaft) {
    row(ui, label, tip, |ui| {
        ui.add(egui::DragValue::new(&mut sh.diameter).range(0.0..=5000.0).speed(0.1).max_decimals(3).prefix("Ø ").suffix(" mm"));
        ui.add(egui::DragValue::new(&mut sh.length).range(0.0..=10000.0).speed(0.2).max_decimals(3).prefix("L ").suffix(" mm"));
    });
    if !sh.exists() {
        return;
    }
    row(ui, "  Step", "A second length beyond the first: smaller for a shoulder, larger for a collar (0: none)", |ui| {
        ui.add(egui::DragValue::new(&mut sh.step_diameter).range(0.0..=5000.0).speed(0.1).max_decimals(3).prefix("Ø ").suffix(" mm"));
        ui.add(egui::DragValue::new(&mut sh.step_length).range(0.0..=10000.0).speed(0.2).max_decimals(3).prefix("L ").suffix(" mm"));
    });
    row(ui, "  End feature", "What the free end carries; one that doesn't fit is left plain with a warning", |ui| {
        egui::ComboBox::from_id_salt(id).width(100.0).selected_text(sh.end.name()).show_ui(ui, |ui| {
            for (e, n) in ShaftEnd::ALL {
                ui.selectable_value(&mut sh.end, e, n);
            }
        });
    });
    if sh.end != ShaftEnd::Plain {
        let size_tip = match sh.end {
            ShaftEnd::DFlat => "Across the flat; 0: 0.9 × Ø",
            ShaftEnd::Keyway => "Slot width; 0: DIN 6885",
            ShaftEnd::Hex => "Across flats; 0: 0.85 × Ø",
            ShaftEnd::Cross => "Arm width; 0: Ø ÷ 3",
            _ => "The hole's Ø; 0: 0.3 × Ø",
        };
        num(ui, "    Feature length", "Back from the free end (a centre hole: its depth); 0: 1.5 × the end's Ø", &mut sh.end_length, 0.0..=10000.0, 0.1, " mm");
        num(ui, "    Feature size", size_tip, &mut sh.end_size, 0.0..=5000.0, 0.05, " mm");
    }
    num(ui, "  End chamfer", "45° on the free end; 0: none", &mut sh.chamfer, 0.0..=100.0, 0.05, " mm");
}

/// EXTENSION POINT: a member's own chamfers. The gear crate's chamfers are the set's (both
/// faces' tooth chamfers, the bores' and the hubs'): they are in [`section_chamfers`].
fn member_chamfers(_ui: &mut Ui, _cx: &mut Cx<'_>, _m: usize) {}

/// A member's hub hardware (set screws, nut traps, heat-set inserts, a clamp slit) and the
/// bearing pocket in its back face.
fn member_hardware(ui: &mut Ui, cx: &mut Cx<'_>, m: usize) {
    let k = cx.k;
    if !hub_kind(k) {
        return;
    }
    let Some(g) = body_gear(k, m) else { return };
    let s = &mut *cx.s;
    let hub = s.hub_diameter > 0.0 && s.hub_length > 0.0;
    let back_shaft = if g == 1 { s.shaft_back.exists() } else { s.shaft2_back.exists() };
    let h = if g == 1 { &mut s.hardware } else { &mut s.hardware2 };
    let id = format!("gear_hw{g}");
    let open = h.set_screw > 0.0 || h.clamp_slit > 0.0 || h.bearing_od > 0.0;
    egui::CollapsingHeader::new(egui::RichText::new("Hardware").small()).id_salt(&id).default_open(open).show(ui, |ui| {
        hardware_rows(ui, h, hub, back_shaft, &id);
    });
}

fn hardware_rows(ui: &mut Ui, h: &mut Hardware, hub: bool, back_shaft: bool, id: &str) {
    if !hub {
        ui.label(egui::RichText::new("Set screws, nut traps, inserts and clamps go in the hub: set Hub Ø and Hub length under Body").small().weak());
    }
    ui.add_enabled_ui(hub, |ui| {
        row(ui, "Set screw Ø", "A radial hole through the hub wall at mid length, over the key or flat; 0: none", |ui| {
            ui.add(egui::DragValue::new(&mut h.set_screw).range(0.0..=100.0).speed(0.05).max_decimals(3).suffix(" mm"));
            if h.set_screw > 0.0 {
                ui.add(egui::DragValue::new(&mut h.set_screws).range(1..=4).speed(0.1).prefix("× "));
            }
        });
        if h.set_screw > 0.0 {
            row(ui, "  Nut trap", "The captive nut's width across flats (a slot open to the hub's end) and its thickness (0: 0.45 × width)", |ui| {
                ui.add(egui::DragValue::new(&mut h.nut_width).range(0.0..=100.0).speed(0.05).max_decimals(3).prefix("w ").suffix(" mm"));
                if h.nut_width > 0.0 {
                    ui.add(egui::DragValue::new(&mut h.nut_thickness).range(0.0..=100.0).speed(0.05).max_decimals(3).prefix("t ").suffix(" mm"));
                }
            });
            row(ui, "  Heat-set insert", "A pocket at the mouth of each set-screw hole: Ø and depth (0: 0.8 × the hub wall)", |ui| {
                ui.add(egui::DragValue::new(&mut h.insert_diameter).range(0.0..=100.0).speed(0.05).max_decimals(3).prefix("Ø ").suffix(" mm"));
                if h.insert_diameter > 0.0 {
                    ui.add(egui::DragValue::new(&mut h.insert_depth).range(0.0..=100.0).speed(0.05).max_decimals(3).prefix("d ").suffix(" mm"));
                }
            });
        }
        row(ui, "Clamp slit", "A slit through the hub wall opposite the set screw, down to the face (needs a bore), and a screw hole across it", |ui| {
            ui.add(egui::DragValue::new(&mut h.clamp_slit).range(0.0..=50.0).speed(0.05).max_decimals(3).prefix("w ").suffix(" mm"));
            if h.clamp_slit > 0.0 {
                ui.add(egui::DragValue::new(&mut h.clamp_screw).range(0.0..=50.0).speed(0.05).max_decimals(3).prefix("screw Ø ").suffix(" mm"));
            }
        });
    });
    let tip = if back_shaft { "Left out while there is a back shaft" } else { "A pocket in the back face; pick a bearing or type its OD and width" };
    row(ui, "Bearing pocket", tip, |ui| {
        let current = BEARINGS.iter().find(|(_, _, od, w)| *od == h.bearing_od && *w == h.bearing_width).map(|(n, ..)| *n).unwrap_or(if h.bearing_od > 0.0 { "custom" } else { "none" });
        egui::ComboBox::from_id_salt(format!("{id}_bearing")).width(80.0).selected_text(current).show_ui(ui, |ui| {
            if ui.selectable_label(h.bearing_od == 0.0, "none").clicked() {
                h.bearing_od = 0.0;
                h.bearing_width = 0.0;
            }
            for (name, bore, od, w) in BEARINGS {
                if ui.selectable_label(current == *name, format!("{name}  ({bore}×{od}×{w})")).clicked() {
                    h.set_bearing(name);
                }
            }
        });
    });
    if h.bearing_od > 0.0 {
        row(ui, "  OD, width", "The bearing's outer diameter and the pocket's depth", |ui| {
            ui.add(egui::DragValue::new(&mut h.bearing_od).range(0.0..=1000.0).speed(0.05).max_decimals(3).prefix("Ø ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut h.bearing_width).range(0.0..=1000.0).speed(0.05).max_decimals(3).prefix("w ").suffix(" mm"));
        });
        num(ui, "  Fit allowance", "How much bigger than the bearing the pocket is made, on the diameter, for a printed fit", &mut h.bearing_fit, 0.0..=2.0, 0.01, " mm");
    }
}

/// The set's chamfers: the tooth edges at the tip circle on each face, the bores' ends and the
/// hubs' outer edges. Not built on bevels (their ends are cones), splines or racks.
fn section_chamfers(ui: &mut Ui, cx: &mut Cx<'_>) {
    let k = cx.k;
    let teeth = (hub_kind(k) && k != Kind::Elliptical) || matches!(k, Kind::Internal | Kind::Planetary);
    if !(teeth || hub_kind(k)) {
        return;
    }
    let s = &mut *cx.s;
    heading(ui, "Chamfers");
    if teeth {
        chamfer_row(ui, "Front teeth", "The tooth edges at the tip circle on the front face (the hub's side); cut down to fit, with a warning", &mut s.tooth_chamfer_front);
        chamfer_row(ui, "Back teeth", "The same on the back face (on the bed); helps against elephant foot", &mut s.tooth_chamfer_back);
    }
    if hub_kind(k) {
        num(ui, "Bore", "45° on both ends of the bores; 0: none", &mut s.bore_chamfer, 0.0..=50.0, 0.05, " mm");
        num(ui, "Hub", "45° on the hub's outer end edge (needs a hub); 0: none", &mut s.hub_chamfer, 0.0..=50.0, 0.05, " mm");
    }
}

fn chamfer_row(ui: &mut Ui, label: &str, tip: &str, c: &mut Chamfer) {
    row(ui, label, tip, |ui| {
        ui.add(egui::DragValue::new(&mut c.size).range(0.0..=50.0).speed(0.02).max_decimals(3).suffix(" mm"));
        if c.size > 0.0 {
            ui.add(egui::DragValue::new(&mut c.angle).range(5.0..=85.0).speed(0.5).max_decimals(1).suffix("° to the face"));
        }
    });
}

/// The tooth form: the basic rack and its coefficients, and the clearance. EXTENSION POINT:
/// tip relief, crowning, tip rounding and a separate root fillet go here.
fn section_tooth_form(ui: &mut Ui, cx: &mut Cx<'_>) {
    if !cx.k.involute() || cx.k == Kind::Spline {
        return;
    }
    let s = &mut *cx.s;
    let o = &mut *cx.o;
    heading(ui, "Tooth form");
    row(ui, "Basic rack", "Tooth proportions", |ui| {
        let current = BasicRack::STANDARDS.iter().find(|(_, r)| *r == s.rack).map(|(n, _)| *n).unwrap_or("custom");
        egui::ComboBox::from_id_salt("gear_rack").selected_text(current).show_ui(ui, |ui| {
            for (n, r) in BasicRack::STANDARDS {
                ui.selectable_value(&mut s.rack, *r, *n);
            }
        });
        ui.toggle_value(&mut o.advanced, "…");
    });
    if o.advanced {
        num(ui, "  Addendum ×m", "", &mut s.rack.addendum, 0.3..=2.0, 0.01, "");
        num(ui, "  Dedendum ×m", "", &mut s.rack.dedendum, 0.5..=2.5, 0.01, "");
        num(ui, "  Root radius ×m", "The cutter's tip radius; makes the root fillet", &mut s.rack.root_radius, 0.0..=0.6, 0.01, "");
    }
    let k = cx.k;
    if rack_tuned(k) {
        row(ui, "Tip clearance c*", "Gap between a tooth tip and the mating root, in modules: dedendum = addendum + c* (0: the rack's). 0.25 is standard, 0.3 or more for printed gears", |ui| {
            ui.add(egui::DragValue::new(&mut s.clearance).range(0.0..=1.0).speed(0.005).max_decimals(3));
            if s.clearance == 0.0 {
                ui.label(egui::RichText::new(format!("rack's: {:.3}", s.rack.dedendum - s.rack.addendum)).small().weak());
            }
        });
        num(ui, "Tip shortening", "Off every gear's tip radius, on top of what a shifted pair needs", &mut s.tip_shortening, 0.0..=0.5 * s.rack.addendum * s.module, 0.01, " mm");
    }
    if tip_tuned(k) {
        num(ui, "Tip relief", "How far the flank falls away from the involute at the tip; eases teeth into mesh under load. 0: none", &mut s.tip_relief, 0.0..=0.1 * s.module, 0.002, " mm");
        if s.tip_relief > 0.0 {
            num(ui, "  Relief height", "Radial height down from the tip it runs over; 0: 0.4 modules", &mut s.tip_relief_height, 0.0..=10.0 * s.module, 0.01, " mm");
        }
    }
    if matches!(k, Kind::Spur | Kind::Pair | Kind::Planetary) && s.helix == 0.0 {
        num(ui, "Crowning", "Each flank this much thinner at the two faces than at mid face, so slightly misaligned shafts still bear mid face", &mut s.crowning, 0.0..=0.1 * s.module, 0.002, " mm");
    }
    if equation_teeth(k) {
        row(ui, "Tooth curves", "Equations: one tooth drawn from the gear formulas (involute flank, trochoid root fillet, tip and root arcs) and a pattern. Fitted: splines through computed points. Tip relief, rounding, crowning and elephant foot stay fitted", |ui| {
            ui.selectable_value(&mut o.equations, true, "equations");
            ui.selectable_value(&mut o.equations, false, "fitted");
        });
        if o.equations {
            row(ui, "Drive by design parameters", "Makes design parameters (module, teeth, pressure angle, shift, backlash, face width) that the tooth formulas, pattern counts and extrusions use: edit one and the teeth regenerate. Positions, hubs and bores follow on Update", |ui| {
                ui.checkbox(&mut o.parametric, "");
            });
        }
    }
}

/// The types whose teeth can be drawn from their formulas.
fn equation_teeth(k: Kind) -> bool {
    matches!(k, Kind::Spur | Kind::Helical | Kind::Herringbone | Kind::Internal | Kind::Pair | Kind::Planetary | Kind::Rack)
}

/// Designing for FDM printing: the print preset and the nozzle the tooth tips are checked
/// against. EXTENSION POINT: the print profile's own fields (XY compensation, elephant-foot
/// relief, minimum tip thickness, printer presets) go here.
fn section_print(ui: &mut Ui, cx: &mut Cx<'_>) {
    let k = cx.k;
    let s = &mut *cx.s;
    let o = &mut *cx.o;
    heading(ui, "3D printing");
    row(ui, "Print preset", "For FDM: the print rack at 25° or more, rounded tips, XY and elephant-foot compensation, backlash that turns freely, a bore chamfer and bearing fit (never lowers what is set). Every value stays editable", |ui| {
        ui.add(egui::DragValue::new(&mut o.nozzle).range(0.1..=2.0).speed(0.01).max_decimals(2).prefix("nozzle ").suffix(" mm"));
        ui.add(egui::DragValue::new(&mut o.layer).range(0.04..=1.0).speed(0.01).max_decimals(2).prefix("layer ").suffix(" mm"));
        if ui.button("Apply").clicked() {
            s.apply_print_defaults(o.nozzle, o.layer);
            o.printing = true;
        }
    });
    if tip_tuned(k) {
        num(ui, "Tip rounding", "Rounds each tip corner, radius in modules (0: sharp); cut down with a warning where the tip is too thin", &mut s.tip_round, 0.0..=0.5, 0.005, " ×m");
    }
    if rack_tuned(k) {
        num(ui, "XY compensation", "Horizontal expansion: the teeth are drawn this far inside where they should print (flanks, tips, roots). Your slicer's over-extrusion per side; tune by measuring the span over k teeth", &mut s.xy_compensation, 0.0..=0.25 * s.module, 0.005, " mm");
    }
    if tip_tuned(k) {
        num(ui, "Elephant foot", "The first layers of the teeth drawn this far inside the rest; 0: none", &mut s.elephant_foot, 0.0..=0.25 * s.module, 0.005, " mm");
        if s.elephant_foot > 0.0 {
            num(ui, "  Height", "From the back face (on the bed); usually the first layer. 0: 0.2 mm", &mut s.elephant_foot_height, 0.0..=10.0, 0.01, " mm");
        }
        num(ui, "Nozzle check", "Warns when a tip land is under two nozzle widths (0: no check); changes no geometry", &mut s.nozzle, 0.0..=2.0, 0.01, " mm");
    }
}

/// What the bodies share besides their teeth: hub and lightening holes. EXTENSION POINT:
/// other shared body features (web thickness, spokes) go here.
fn section_body(ui: &mut Ui, cx: &mut Cx<'_>) {
    let k = cx.k;
    let s = &mut *cx.s;
    // A spline's hub is its own member; a face gear's base is in its section.
    let hub = !matches!(k, Kind::Rack | Kind::CycloidalDrive | Kind::Internal | Kind::Hypoid | Kind::Spline);
    let holes = matches!(k, Kind::Spur | Kind::Helical | Kind::Herringbone | Kind::Pair | Kind::Planetary | Kind::Cycloidal);
    if !(hub || holes) {
        return;
    }
    heading(ui, "Body");
    if hub {
        num(ui, "Hub Ø", "On the back face; 0: none", &mut s.hub_diameter, 0.0..=5000.0, 0.2, " mm");
        if s.hub_diameter > 0.0 {
            num(ui, "Hub length", "", &mut s.hub_length, 0.0..=5000.0, 0.2, " mm");
        }
    }
    if holes {
        count(ui, "Lightening holes", "Through the web between hub and rim", &mut s.holes, 0..=24);
        if s.holes > 0 {
            num(ui, "Hole Ø", "0: to fit", &mut s.hole_diameter, 0.0..=1000.0, 0.2, " mm");
        }
    }
}

/// The load the set is rated for (the strength rating shows among the facts); each member's
/// material is in its own section while this is open or a load is set.
fn section_load(ui: &mut Ui, cx: &mut Cx<'_>) {
    let k = cx.k;
    if !k.involute() || matches!(k, Kind::Rack | Kind::Spline | Kind::FaceGear | Kind::Elliptical) {
        return;
    }
    let o = &mut *cx.o;
    let l = &mut cx.s.load;
    ui.add_space(6.0);
    ui.toggle_value(&mut o.load_open, egui::RichText::new("Load (strength rating)").strong());
    if o.load_open {
        num(ui, "Torque", "On the driving member; 0: not rated (or give power and speed)", &mut l.torque, 0.0..=1e6, 0.1, " N·m");
        num(ui, "Power", "Used when the torque is 0", &mut l.power, 0.0..=1e5, 0.01, " kW");
        num(ui, "Speed", "Of the driving member", &mut l.speed, 0.0..=1e6, 10.0, " rpm");
        num(ui, "Application factor", "1.0 smooth; 1.25 light shocks; 1.5 moderate; 1.75+ heavy", &mut l.application_factor, 1.0..=3.0, 0.05, "");
        num(ui, "Life", "Required life; 0: unlimited", &mut l.life_hours, 0.0..=1e7, 100.0, " h");
        ui.label(egui::RichText::new("Each gear's material is in its own section above").small().weak());
    }
}

/// What is built and where: one member alone, the name, and the centre when no axis places the
/// set.
fn section_output(ui: &mut Ui, cx: &mut Cx<'_>) {
    let k = cx.k;
    let on_axes = cx.on_axes;
    let s = &mut *cx.s;
    heading(ui, "Build");
    if k.is_set() {
        row(ui, "Members", "Make every member, or just one", |ui| {
            let names: &[&str] = match k {
                Kind::Planetary => &["all", "sun", "planets", "ring"],
                Kind::Worm => &["both", "wheel", "worm"],
                Kind::CycloidalDrive => &["both", "disc", "pins"],
                Kind::Hypoid => &["both", "pinion", "ring gear"],
                Kind::Cycloidal => &["both", "wheel", "pinion"],
                Kind::Sprocket => &["both", "sprocket 1", "sprocket 2"],
                Kind::Pulley => &["both", "pulley 1", "pulley 2"],
                Kind::Spline => &["both", "shaft", "hub"],
                Kind::FaceGear => &["both", "face gear", "pinion"],
                _ => &["both", "gear 1", "gear 2"],
            };
            egui::ComboBox::from_id_salt("gear_only").selected_text(names.get(s.only as usize).copied().unwrap_or("all")).show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut s.only, i as u8, *n);
                }
            });
        });
    }
    if matches!(k, Kind::Pair | Kind::Planetary) {
        row(ui, solidcraft_ui_text::tr("gear.window.coupon"), "Instead of the set: a few teeth of each member at the true centre distance, on a base with pegs, to check the fit on your printer first", |ui| {
            ui.checkbox(&mut s.coupon, "");
            if s.coupon {
                ui.add(egui::DragValue::new(&mut s.coupon_teeth).range(3..=40).speed(0.2).suffix(" teeth"));
            }
        });
    }
    row(ui, "Name", "Put in front of the bodies' names", |ui| {
        ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(150.0));
    });
    if on_axes {
        return;
    }
    if s.placement.is_some() {
        // A set reopened without its axes stands where it was built.
        row(ui, "Placed", "The set keeps the place and direction it was built with", |ui| {
            ui.label(egui::RichText::new("as built").monospace());
            if ui.small_button("at a centre instead").on_hover_text("Forget the stored frame: the set goes on world Z at Centre at (or pick its axes above)").clicked() {
                s.placement = None;
            }
        });
    } else {
        let tip = if k == Kind::Hypoid { "Where the centre of the ring gear's back face goes" } else { "Where gear 1's centre goes" };
        row(ui, "Centre at", tip, |ui| {
            for v in &mut s.origin {
                ui.add(egui::DragValue::new(v).speed(0.5).max_decimals(3));
            }
        });
    }
}

fn material(ui: &mut Ui, label: &str, id: &str, m: &mut solidcraft_gear::strength::Material) {
    row(ui, label, "Sets the allowable contact and root stress", |ui| {
        egui::ComboBox::from_id_salt(id).width(170.0).selected_text(m.data().label).show_ui(ui, |ui| {
            for (k, d) in solidcraft_gear::strength::MATERIALS {
                ui.selectable_value(m, *k, d.label);
            }
        });
    });
}

/// The planetary ring's material: the planets' unless one is chosen.
fn ring_material(ui: &mut Ui, m: &mut Option<solidcraft_gear::strength::Material>) {
    row(ui, "Material", "Sets the allowable contact and root stress", |ui| {
        let current = m.map(|m| m.data().label).unwrap_or("as the planets");
        egui::ComboBox::from_id_salt("gear_mat3").width(170.0).selected_text(current).show_ui(ui, |ui| {
            ui.selectable_value(m, None, "as the planets");
            for (k, d) in solidcraft_gear::strength::MATERIALS {
                ui.selectable_value(m, Some(*k), d.label);
            }
        });
    });
}

fn bore(ui: &mut Ui, label: &str, id: &str, b: &mut Bore) {
    row(ui, label, "Round, keyed (DIN 6885), D flat, hex or square", |ui| {
        egui::ComboBox::from_id_salt(id).width(80.0).selected_text(b.kind.name()).show_ui(ui, |ui| {
            for (k, n) in BoreKind::ALL {
                ui.selectable_value(&mut b.kind, k, n);
            }
        });
        if b.kind != BoreKind::None {
            if b.diameter <= 0.0 {
                b.diameter = 8.0;
            }
            ui.add(egui::DragValue::new(&mut b.diameter).range(0.5..=5000.0).speed(0.1).max_decimals(3).prefix("Ø ").suffix(" mm"));
        }
    });
    if b.kind == BoreKind::DFlat {
        num(ui, "  Across flat", "From the flat to the far side; 0: 0.9 × Ø", &mut b.flat, 0.0..=5000.0, 0.05, " mm");
    }
    if b.kind == BoreKind::Keyway {
        num(ui, "  Key width", "0: DIN 6885", &mut b.key_width, 0.0..=500.0, 0.1, " mm");
        num(ui, "  Key depth", "Into the hub; 0: DIN 6885", &mut b.key_depth, 0.0..=500.0, 0.1, " mm");
    }
}

fn facts(ui: &mut Ui, w: &GearWindow) {
    match &w.plan {
        Err(e) => {
            ui.note(e, ui.visuals().error_fg_color);
        }
        Ok(p) => {
            for warn in &p.warnings {
                ui.note(format!("⚠ {warn}"), ui.visuals().warn_fg_color);
            }
            egui::ScrollArea::vertical().id_salt("gear_facts").max_height(170.0).auto_shrink([false, true]).show(ui, |ui| {
                egui::Grid::new("gear_facts_grid").num_columns(2).striped(true).spacing([12.0, 2.0]).min_col_width(200.0).show(ui, |ui| {
                    // What the axes fixed first, then the set's own numbers.
                    for f in w.solved.iter().flat_map(|s| s.facts.iter()).chain(p.facts.iter()) {
                        ui.add(egui::Label::new(egui::RichText::new(&f.label).small()).wrap_mode(egui::TextWrapMode::Extend));
                        let v = match f.unit {
                            Unit::Mm => format!("{:.3} mm", f.value),
                            Unit::Deg => format!("{:.3}°", f.value),
                            Unit::None => format!("{:.4}", f.value),
                        };
                        ui.label(egui::RichText::new(v).small().monospace());
                        ui.end_row();
                    }
                });
            });
        }
    }
}

/// The 2D preview: every member in its colour, turned by the motion the gear crate's mesh
/// tests prove, the lit member brighter. Returns the member under the pointer.
fn preview(ui: &mut Ui, w: &GearWindow) -> Option<usize> {
    let side = ui.available_width().min(420.0);
    let (rect, response) = ui.allocate_exact_size(vec2(side, side * 0.82), Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = ui.visuals().extreme_bg_color;
    painter.rect_filled(rect, 6.0, bg);
    let Ok(plan) = &w.plan else { return None };
    let kind = w.built.kind;
    let turned = w.pose();
    // Fit every gear (at any turn: its largest radius about its centre) in the box.
    let (mut lo, mut hi) = (V2::new(f64::MAX, f64::MAX), V2::new(f64::MIN, f64::MIN));
    for (g, m) in plan.preview.iter().zip(&w.meshes) {
        if g.slide == 0.0 && g.speed != 0.0 {
            // A centre that goes round the origin sweeps a circle about that instead.
            let (c, r) = if g.orbit != 0.0 { (V2::ZERO, m.radius + g.center.len()) } else { (g.center, m.radius) };
            lo = V2::new(lo.x.min(c.x - r), lo.y.min(c.y - r));
            hi = V2::new(hi.x.max(c.x + r), hi.y.max(c.y + r));
        } else {
            for q in m.outlines.iter().flatten() {
                let p = g.center + *q;
                lo = V2::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = V2::new(hi.x.max(p.x), hi.y.max(p.y));
            }
        }
    }
    if !(lo.x.is_finite() && hi.x > lo.x) {
        return None;
    }
    let margin = 12.0;
    let scale = ((rect.width() - 2.0 * margin) as f64 / (hi.x - lo.x)).min((rect.height() - 2.0 * margin) as f64 / (hi.y - lo.y).max(1e-9));
    let mid = (lo + hi) * 0.5;
    let c = rect.center();
    let to_screen = |p: V2| -> Pos2 { pos2(c.x + ((p.x - mid.x) * scale) as f32, c.y - ((p.y - mid.y) * scale) as f32) };
    // The member under the pointer: of the outlines the pointer is within, the smallest (a
    // planet rather than the ring round it).
    let pointer = response.hover_pos().map(|p| V2::new(mid.x + f64::from(p.x - c.x) / scale, mid.y - f64::from(p.y - c.y) / scale));
    let mut under: Option<(f64, usize)> = None;
    if let Some(p) = pointer {
        for (i, (g, m)) in plan.preview.iter().zip(&w.meshes).enumerate() {
            let reach = if g.slide == 0.0 {
                (p.dist(g.place(V2::ZERO, turned)) <= m.radius).then_some(m.radius)
            } else {
                // A sliding member (a rack, a worm's section): the box of its outline where it is.
                let pts = m.outlines.iter().flatten().map(|q| g.place(*q, turned));
                let (lo, hi) = pts.fold((V2::new(f64::MAX, f64::MAX), V2::new(f64::MIN, f64::MIN)), |(lo, hi), q| (V2::new(lo.x.min(q.x), lo.y.min(q.y)), V2::new(hi.x.max(q.x), hi.y.max(q.y))));
                (p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y).then_some(lo.dist(hi) / 2.0)
            };
            if let Some(reach) = reach
                && let Some(member) = member_of_outline(kind, i, &g.name)
                && under.is_none_or(|(least, _)| reach < least)
            {
                under = Some((reach, member));
            }
        }
    }
    let under = under.map(|(_, member)| member);
    // The members to light: the one asked for (a section, an edit), else the one under the
    // pointer; or both members of the mesh the pointer is on in the mesh badge.
    let lit: Vec<usize> = match w.hover_mesh.and_then(|r| w.mesh.as_ref()?.1.get(r)) {
        Some(r) => [r.a, r.b].iter().filter_map(|&i| plan.preview.get(i).and_then(|g| member_of_outline(kind, i, &g.name))).collect(),
        None => w.lit().or(under).into_iter().collect(),
    };
    let stroke_color = ui.visuals().text_color();
    // What is drawn but not built (a chain, a belt): in the text colour, never lit.
    let neutral = stroke_color.gamma_multiply(0.6);
    for (i, (g, m)) in plan.preview.iter().zip(&w.meshes).enumerate() {
        let place = |q: V2| -> Pos2 { to_screen(g.place(q, turned)) };
        let member = member_of_outline(kind, i, &g.name);
        let color = member.map(member_color).unwrap_or(neutral);
        let is_lit = member.is_some_and(|m| lit.contains(&m));
        // The lit member stands out; the others step back while one is lit.
        let (strength, line) = match (is_lit, lit.is_empty()) {
            (true, _) => (0.95, 2.2),
            (false, false) => (0.32, 1.0),
            (false, true) => (0.55, 1.0),
        };
        let fill = color.gamma_multiply(strength);
        let mut mesh = egui::Mesh::default();
        for t in &m.fill {
            let base = mesh.vertices.len() as u32;
            for q in t {
                mesh.colored_vertex(place(*q), fill);
            }
            mesh.add_triangle(base, base + 1, base + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        let mut holes = egui::Mesh::default();
        for h in &m.holes {
            for t in h {
                let base = holes.vertices.len() as u32;
                for q in t {
                    holes.colored_vertex(place(*q), bg);
                }
                holes.add_triangle(base, base + 1, base + 2);
            }
        }
        painter.add(egui::Shape::mesh(holes));
        for o in &m.outlines {
            let mut pts: Vec<Pos2> = o.iter().map(|q| place(*q)).collect();
            if let Some(f) = pts.first().copied() {
                pts.push(f);
            }
            painter.add(egui::Shape::line(pts, Stroke::new(line, color.gamma_multiply(0.9).lerp_to_gamma(stroke_color, 0.3))));
        }
        if g.pitch_radius > 0.0 {
            dashed_circle(&painter, to_screen(g.center), (g.pitch_radius * scale) as f32, Stroke::new(0.8, stroke_color.gamma_multiply(0.35)));
        }
        // A mark that shows the turning.
        if g.slide == 0.0 && g.speed != 0.0 {
            painter.circle_filled(place(V2::polar(g.pitch_radius.max(1.0) * 0.35, 0.0)), 2.5, stroke_color.gamma_multiply(0.6));
        }
        let at = to_screen(if g.slide == 0.0 { g.place(V2::ZERO, turned) } else { g.center });
        painter.circle_filled(at, 2.0, stroke_color.gamma_multiply(0.6));
        // The member's name in its own colour, on a chip so it reads over the gear.
        let font = egui::FontId::proportional(if is_lit { 12.5 } else { 11.0 });
        let galley = painter.layout_no_wrap(g.name.clone(), font, color.lerp_to_gamma(stroke_color, 0.55));
        let pos = at + vec2(-galley.size().x / 2.0, 5.0);
        painter.rect_filled(egui::Rect::from_min_size(pos, galley.size()).expand(2.0), 3.0, bg.gamma_multiply(0.8));
        painter.galley(pos, galley, stroke_color);
    }
    if !plan.preview_note.is_empty() {
        painter.text(rect.left_bottom() + vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, &plan.preview_note, egui::FontId::proportional(11.0), stroke_color.gamma_multiply(0.6));
    }
    under
}

fn dashed_circle(p: &egui::Painter, c: Pos2, r: f32, stroke: Stroke) {
    let n = ((r * 0.6) as usize).clamp(24, 240);
    let pts: Vec<Pos2> = (0..=n).map(|i| c + Vec2::angled(i as f32 / n as f32 * std::f32::consts::TAU) * r).collect();
    p.extend(egui::Shape::dashed_line(&pts, stroke, 4.0, 4.0));
}
