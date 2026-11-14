//! The Gear Generator window (`gear.create`'s ribbon button): pick a gear type, set its
//! numbers, watch the set mesh in an animated 2D preview with its key figures and warnings, see
//! the solids previewed in the view, and Create them as one undo step.
//!
//! The Axes step at the top makes it axis-first: pick the two shafts in the model (or enter
//! them) and the window says how they lie, offers the gear types that fit, works out what the
//! axes decide (centre distance, shaft angle, offset, and the module or teeth left free) and
//! builds the gears on them.

use egui::{Color32, Pos2, Sense, Stroke, Ui, Vec2, pos2, vec2};
use solidcraft_gear::geom::{Region, V2};
use solidcraft_gear::hub::{Bore, BoreKind};
use solidcraft_gear::involute::BasicRack;
use solidcraft_gear::axes::{self, Axis, Derived, Drive, Relation, Solved};
use solidcraft_gear::plan::{Axes, Kind, Plan, Spec, Unit};

use crate::SolveApp;
use crate::ui_kit;

/// A picked or entered shaft axis, and where it came from.
#[derive(Clone, Debug, PartialEq)]
struct Slot {
    axis: Axis,
    label: String,
}

/// The Axes step: the two shafts and what drives the solve on them.
#[derive(Clone, Debug, PartialEq)]
pub struct AxesStep {
    slots: [Option<Slot>; 2],
    /// Choose the tooth counts to give `ratio` (gear 2 ÷ gear 1), instead of the counts typed.
    use_ratio: bool,
    ratio: f64,
    /// Choose a standard module to suit the axes, instead of the one typed.
    auto_module: bool,
    /// Bevels: size by the outer cone distance instead of the module.
    use_cone: bool,
    cone: f64,
    flip1: bool,
    flip2: bool,
    swap: bool,
    along: f64,
}

impl Default for AxesStep {
    fn default() -> Self {
        AxesStep { slots: [None, None], use_ratio: false, ratio: 2.0, auto_module: true, use_cone: false, cone: 40.0, flip1: false, flip2: false, swap: false, along: 0.0 }
    }
}

impl AxesStep {
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
}

/// What the window holds between frames.
pub struct GearWindow {
    /// The numbers as typed.
    pub spec: Spec,
    /// The Axes step.
    pub axes: AxesStep,
    /// What is built: `spec`, or with an axis set, the set solved on the axes.
    built: Spec,
    /// The solve on the axes, when there is one.
    solved: Option<Solved>,
    /// The slot a "Use selection" click asked to fill; taken up after the window is drawn, where
    /// the design is at hand.
    capture: Option<usize>,
    /// Each slot's typed origin and direction, and whether its entry rows are open.
    manual: [bool; 2],
    entered: [([f64; 3], [f64; 3]); 2],
    /// Why the last "Use selection" took nothing.
    axes_note: Option<String>,
    /// How the axes lay last frame, to notice a change.
    last_relation: Option<&'static str>,
    /// The plan for what is built, or why there is none; recomputed when an input changes.
    plan: Result<Plan, String>,
    planned: Option<(Spec, AxesStep)>,
    /// Filled triangles of each preview gear, in its own coordinates.
    meshes: Vec<GearMesh>,
    /// Whether the set turns in the preview, and how far it has turned (driver radians).
    pub animate: bool,
    turned: f64,
    speed: f64,
    /// Show the result in the view as the numbers change.
    pub preview_3d: bool,
    /// Module given as diametral pitch (teeth per inch).
    inch: bool,
    /// Show the basic rack's coefficients.
    advanced: bool,
    /// The last create's error.
    error: Option<String>,
}

struct GearMesh {
    fill: Vec<[V2; 3]>,
    holes: Vec<Vec<[V2; 3]>>,
    outlines: Vec<Vec<V2>>,
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
            meshes: Vec::new(),
            animate: true,
            turned: 0.0,
            speed: 0.4,
            preview_3d: true,
            inch: false,
            advanced: false,
            error: None,
        };
        w.replan();
        w
    }

    fn replan(&mut self) {
        self.settle_kind();
        let key = (self.spec.clone(), self.axes.clone());
        if self.planned.as_ref() == Some(&key) {
            return;
        }
        self.planned = Some(key);
        self.solved = None;
        // With an axis, the set is sized and placed on the axes; what is built is the solved
        // spec, and `spec` stays what was typed.
        let built = match self.axes.axis(0) {
            None => Ok(self.spec.clone()),
            Some(a1) => axes::solve(&a1, self.axes.axis(1).as_ref(), Some(self.spec.kind), &self.spec, &self.axes.drive()).map(|solved| {
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
    }

    /// The command the view previews while the window is open.
    pub fn preview_commands(&self) -> Option<Vec<(String, serde_json::Value)>> {
        (self.preview_3d && self.plan.is_ok()).then(|| vec![("gear.create".to_string(), solidcraft_engine::gear_params_of(&self.built))])
    }

    /// A new type starts from its own defaults, keeping the tooth form and hub choices.
    fn switch_kind(&mut self, k: Kind) {
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
}

impl Default for GearWindow {
    fn default() -> Self {
        Self::new()
    }
}

pub fn open(app: &mut SolveApp) {
    if app.gear_window.is_none() {
        app.gear_window = Some(GearWindow::new());
    }
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
    GearMesh { fill, holes, outlines }
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
fn triangulate(poly: &[V2]) -> Vec<[V2; 3]> {
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

pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(mut w) = app.gear_window.take() else { return };
    w.replan();
    let title = solidcraft_ui_text::tr_label("Gear Generator");
    let create = solidcraft_ui_text::tr("ui.create");
    let close = solidcraft_ui_text::tr("ui.close");
    let valid = w.plan.is_ok();
    let preview_error = app.preview.error.clone();
    let preview_busy = app.preview.busy;
    let (closed, action) = ui_kit::Window::new(egui::Id::unique("sc_gear_window"), "gear", title).fields().cards(3.0).actions(create, close, None).show(ctx, valid, |ui| {
        body(ui, &mut w, preview_busy, preview_error.as_deref());
    });
    if w.animate {
        let dt = ctx.input(|i| i.stable_dt).min(0.1) as f64;
        w.turned += dt * w.speed;
        ctx.request_repaint();
    }
    if closed || action == ui_kit::Action::Cancel {
        return;
    }
    if let Some(slot) = w.capture.take() {
        capture(app, &mut w, slot);
    }
    if action == ui_kit::Action::Ok {
        let params = solidcraft_engine::gear_params_of(&w.built);
        match app.run("gear.create", params) {
            Ok(_) => {
                w.error = None;
                return;
            }
            Err(e) => w.error = Some(e),
        }
    }
    app.gear_window = Some(w);
}

/// Fills an axis slot from what is selected in the view: a construction or origin axis, a
/// straight or round edge, a cylinder, cone or torus face, or a straight sketch line. With two
/// such items selected, slot 1 takes the first and slot 2 the second.
fn capture(app: &SolveApp, w: &mut GearWindow, slot: usize) {
    use solidcraft_engine::Sel;
    let found: Vec<Slot> = app
        .session
        .selection
        .iter()
        .filter_map(|sel| {
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
                    ui.weak(if i == 0 { "not set: the gear goes at Centre at, on world Z" } else { "not set" });
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
    if let Some(note) = &w.axes_note {
        ui.colored_label(ui.visuals().warn_fg_color, note);
    }
    if w.axes.slots[0].is_none() && w.axes.slots[1].is_some() {
        ui.colored_label(ui.visuals().warn_fg_color, "Set Axis 1 too: gear 1 turns about it");
    }
    let Some(a1) = w.axes.axis(0) else { return };
    let rel = w.axes.relation();
    let two = rel.is_some() && w.spec.kind.is_set() && !matches!(w.spec.kind, Kind::Planetary | Kind::CycloidalDrive);
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
                    ui.checkbox(&mut a.auto_module, "Standard module").on_hover_text("Choose the ISO 54 module that suits the axes, instead of the one typed");
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
    let _ = a1;
    if let Some(s) = &w.solved {
        for n in &s.notes {
            ui.label(egui::RichText::new(n).small().weak());
        }
    }
}

fn body(ui: &mut Ui, w: &mut GearWindow, busy: bool, view_error: Option<&str>) {
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
            ui.set_width((full - left - 8.0).max(200.0));
            preview(ui, w);
            ui.horizontal(|ui| {
                ui.checkbox(&mut w.animate, "Turn");
                ui.add(egui::Slider::new(&mut w.speed, 0.05..=2.0).show_value(false)).on_hover_text("How fast the preview turns");
                ui.checkbox(&mut w.preview_3d, "Preview in view");
                if w.preview_3d && busy {
                    ui.spinner();
                }
            });
            if let Some(e) = view_error.filter(|_| w.preview_3d) {
                ui.colored_label(ui.visuals().error_fg_color, format!("View preview: {e}"));
            }
            if let Some(e) = &w.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            facts(ui, w);
        });
    });
}

fn type_picker(ui: &mut Ui, w: &mut GearWindow) {
    // With the axes set, the types that can't sit on them are greyed, with why.
    let fit = w.axes.axis(0).map(|_| axes::candidates(w.axes.relation().as_ref()));
    let mut pick = None;
    for (group, label) in [(Axes::Parallel, "Parallel axes"), (Axes::Intersecting, "Intersecting axes"), (Axes::Crossed, "Crossed axes"), (Axes::Special, "Special")] {
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
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text).strong());
}

fn params(ui: &mut Ui, w: &mut GearWindow) {
    // What the axes decided is shown from the solved set, read-only.
    let d: Derived = w.solved.as_ref().map(|s| s.derived).unwrap_or_default();
    let b = w.built.clone();
    let on_axes = w.axes.axis(0).is_some();
    let s = &mut w.spec;
    let k = s.kind;
    egui::ScrollArea::vertical().id_salt("gear_params").max_height(470.0).auto_shrink([false, true]).show(ui, |ui| {
        heading(ui, "Teeth");
        if k == Kind::CycloidalDrive {
            count(ui, "Ring pins", "The disc gets one lobe fewer; the reduction is pins − 1", &mut s.pins, 3..=200);
            num(ui, "Pin circle Ø", "Diameter through the pin centres", &mut s.pin_circle, 1.0..=5000.0, 0.5, " mm");
            num(ui, "Pin Ø", "", &mut s.pin_diameter, 0.1..=500.0, 0.1, " mm");
            num(ui, "Eccentricity", "Offset of the disc's bearing; under pin circle radius ÷ pins", &mut s.eccentricity, 0.01..=100.0, 0.05, " mm");
            num(ui, "Clearance", "Taken off the disc round the pins", &mut s.backlash, 0.0..=5.0, 0.01, " mm");
            count(ui, "Output holes", "Holes for the output pins (0: none)", &mut s.holes, 0..=24);
            if s.holes > 0 {
                num(ui, "Output pin Ø", "0: 15% of the pin circle", &mut s.hole_diameter, 0.0..=500.0, 0.1, " mm");
            }
        } else {
            if d.module {
                locked(ui, if k == Kind::Worm { "Axial module" } else { "Module" }, mm(b.module));
            } else if w.inch {
                let mut dp = 25.4 / s.module;
                row(ui, "Diametral pitch", "Teeth per inch of pitch diameter", |ui| {
                    if ui.add(egui::DragValue::new(&mut dp).range(0.5..=200.0).speed(0.1).max_decimals(3)).changed() {
                        s.module = 25.4 / dp;
                    }
                    ui.checkbox(&mut w.inch, "inch");
                });
            } else {
                row(ui, if k == Kind::Worm { "Axial module" } else { "Module" }, "Pitch diameter ÷ teeth (mm)", |ui| {
                    ui.add(egui::DragValue::new(&mut s.module).range(0.05..=100.0).speed(0.01).max_decimals(4).suffix(" mm"));
                    ui.checkbox(&mut w.inch, "inch");
                });
            }
            let (t1, t2) = match k {
                Kind::Planetary => ("Sun teeth", Some("Planet teeth")),
                Kind::Worm => ("Wheel teeth", None),
                Kind::Hypoid => ("Pinion teeth", Some("Gear teeth")),
                Kind::Pair | Kind::Bevel | Kind::SpiralBevel | Kind::CrossedHelical => ("Gear 1 teeth", Some("Gear 2 teeth")),
                Kind::Cycloidal => ("Wheel teeth", Some("Pinion teeth (0: none)")),
                _ => ("Teeth", None),
            };
            if k == Kind::Rack {
                count(ui, "Teeth along", "", &mut s.rack_teeth, 1..=2000);
            } else if d.teeth {
                locked(ui, t1, b.teeth.to_string());
            } else {
                count(ui, t1, "", &mut s.teeth, 3..=2000);
            }
            if let Some(t2) = t2 {
                if d.teeth2 {
                    locked(ui, t2, b.teeth2.to_string());
                } else {
                    count(ui, t2, "", &mut s.teeth2, if k == Kind::Cycloidal { 0..=2000 } else { 3..=2000 });
                }
            }
            if k == Kind::Planetary {
                let mut ring = if s.teeth3 > 0 { s.teeth3 } else { s.teeth + 2 * s.teeth2 };
                row(ui, "Ring teeth", "Sun + 2 × planet for standard gears", |ui| {
                    if ui.add(egui::DragValue::new(&mut ring).range(6..=4000).speed(0.2)).changed() {
                        s.teeth3 = ring;
                    }
                    if ui.small_button("auto").clicked() {
                        s.teeth3 = 0;
                    }
                });
                count(ui, "Planets", "(sun + ring) ÷ planets whole spaces them evenly", &mut s.planets, 1..=24);
            }
            if k == Kind::Worm {
                count(ui, "Worm starts", "Threads on the worm; ratio = wheel teeth ÷ starts", &mut s.starts, 1..=12);
                if d.worm_diameter {
                    locked(ui, "Worm pitch Ø", mm(b.worm_diameter));
                } else {
                    num(ui, "Worm pitch Ø", "0: 10 modules", &mut s.worm_diameter, 0.0..=5000.0, 0.1, " mm");
                }
                num(ui, "Worm length", "0: enough to engage the wheel", &mut s.worm_length, 0.0..=10000.0, 0.5, " mm");
                let mut left = s.helix < 0.0;
                row(ui, "Hand", "", |ui| {
                    ui.radio_value(&mut left, false, "right");
                    ui.radio_value(&mut left, true, "left");
                });
                s.helix = if left { -1.0 } else { 0.0 };
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
            if k == Kind::CrossedHelical {
                if d.helix {
                    locked(ui, "Gear 1 helix", format!("{:.3}°", b.helix));
                } else {
                    num(ui, "Gear 1 helix", "Gear 2 takes the rest of the shaft angle", &mut s.helix, -75.0..=75.0, 0.5, "°");
                }
            }
            if k == Kind::SpiralBevel {
                num(ui, "Spiral angle", "Mean spiral angle; 35° is usual", &mut s.spiral_angle, 0.0..=60.0, 0.5, "°");
            }
            if k == Kind::Hypoid {
                if d.offset {
                    locked(ui, "Offset", mm(b.offset));
                } else {
                    num(ui, "Offset", "Distance between the axes; 0 for equal spiral angles", &mut s.offset, 0.0..=5000.0, 0.5, " mm");
                }
                num(ui, "Face start", "Where the teeth start along the line of contact, from the throat; larger looks more like a bevel", &mut s.face_start, -5000.0..=5000.0, 0.5, " mm");
            }
            if k.involute() && k != Kind::Worm {
                num(ui, if k.is_set() { "Shift x₁" } else { "Profile shift x" }, "Profile shift coefficient: positive thickens the root and avoids undercut", &mut s.shift, -1.5..=1.5, 0.01, "");
                if k == Kind::Pair && d.center_distance {
                    locked(ui, "Shift x₂", format!("{:.4}", b.shift2));
                    if d.shift {
                        locked(ui, "  x₁ (auto while 0)", format!("{:.4}", b.shift));
                    }
                } else if matches!(k, Kind::Pair | Kind::Planetary | Kind::CrossedHelical) {
                    num(ui, "Shift x₂", "Gear 2's (planets') profile shift", &mut s.shift2, -1.5..=1.5, 0.01, "");
                }
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
            if k != Kind::CycloidalDrive {
                num(ui, "Backlash", "Taken off each tooth at the pitch circle", &mut s.backlash, 0.0..=5.0, 0.005, " mm");
            }
            if k.involute() {
                row(ui, "Basic rack", "Tooth proportions", |ui| {
                    let current = BasicRack::STANDARDS.iter().find(|(_, r)| *r == s.rack).map(|(n, _)| *n).unwrap_or("custom");
                    egui::ComboBox::from_id_salt("gear_rack").selected_text(current).show_ui(ui, |ui| {
                        for (n, r) in BasicRack::STANDARDS {
                            ui.selectable_value(&mut s.rack, *r, *n);
                        }
                    });
                    ui.toggle_value(&mut w.advanced, "…");
                });
                if w.advanced {
                    num(ui, "  Addendum ×m", "", &mut s.rack.addendum, 0.3..=2.0, 0.01, "");
                    num(ui, "  Dedendum ×m", "", &mut s.rack.dedendum, 0.5..=2.5, 0.01, "");
                    num(ui, "  Root radius ×m", "The cutter's tip radius; makes the root fillet", &mut s.rack.root_radius, 0.0..=0.6, 0.01, "");
                }
            }
        }
        heading(ui, "Body");
        num(ui, if k == Kind::CycloidalDrive { "Thickness" } else { "Face width" }, "", &mut s.face_width, 0.1..=5000.0, 0.1, " mm");
        if matches!(k, Kind::Internal | Kind::Planetary) {
            num(ui, "Ring rim", "Beyond the roots; 0: 3 modules", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
        }
        if k == Kind::Rack {
            num(ui, "Height below pitch", "0: 3 modules", &mut s.rack_height, 0.0..=1000.0, 0.1, " mm");
        }
        if k != Kind::Rack && k != Kind::CycloidalDrive {
            bore(ui, "Bore", "bore1", &mut s.bore);
            if k.is_set() {
                let who = match k {
                    Kind::Planetary => "Planet bore",
                    Kind::Worm => "Worm bore",
                    _ => "Gear 2 bore",
                };
                bore(ui, who, "bore2", &mut s.bore2);
            }
            if !matches!(k, Kind::Internal | Kind::Hypoid) {
                num(ui, "Hub Ø", "On the back face; 0: none", &mut s.hub_diameter, 0.0..=5000.0, 0.2, " mm");
                if s.hub_diameter > 0.0 {
                    num(ui, "Hub length", "", &mut s.hub_length, 0.0..=5000.0, 0.2, " mm");
                }
            }
            if matches!(k, Kind::Spur | Kind::Helical | Kind::Herringbone | Kind::Pair | Kind::Planetary | Kind::Cycloidal) {
                count(ui, "Lightening holes", "Through the web between hub and rim", &mut s.holes, 0..=24);
                if s.holes > 0 {
                    num(ui, "Hole Ø", "0: to fit", &mut s.hole_diameter, 0.0..=1000.0, 0.2, " mm");
                }
            }
        } else if k == Kind::CycloidalDrive {
            bore(ui, "Bearing bore", "bore1", &mut s.bore);
        }
        if k.is_set() {
            row(ui, "Build", "Make every member, or just one", |ui| {
                let names: &[&str] = match k {
                    Kind::Planetary => &["all", "sun", "planets", "ring"],
                    Kind::Worm => &["both", "wheel", "worm"],
                    Kind::CycloidalDrive => &["both", "disc", "pins"],
                    Kind::Hypoid => &["both", "pinion", "gear"],
                    _ => &["both", "gear 1", "gear 2"],
                };
                egui::ComboBox::from_id_salt("gear_only").selected_text(names.get(s.only as usize).copied().unwrap_or("all")).show_ui(ui, |ui| {
                    for (i, n) in names.iter().enumerate() {
                        ui.selectable_value(&mut s.only, i as u8, *n);
                    }
                });
            });
        }
        row(ui, "Name", "Put in front of the bodies' names", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(150.0));
        });
        if !on_axes {
            row(ui, "Centre at", "Where gear 1's centre goes", |ui| {
                for v in &mut s.origin {
                    ui.add(egui::DragValue::new(v).speed(0.5).max_decimals(3));
                }
            });
        }
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
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        Ok(p) => {
            for warn in &p.warnings {
                ui.colored_label(ui.visuals().warn_fg_color, format!("⚠ {warn}"));
            }
            egui::ScrollArea::vertical().id_salt("gear_facts").max_height(170.0).auto_shrink([false, true]).show(ui, |ui| {
                egui::Grid::new("gear_facts_grid").num_columns(2).striped(true).spacing([12.0, 2.0]).min_col_width(200.0).show(ui, |ui| {
                    for f in &p.facts {
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

const PALETTE: [Color32; 6] = [
    Color32::from_rgb(86, 140, 220),
    Color32::from_rgb(232, 146, 64),
    Color32::from_rgb(96, 180, 120),
    Color32::from_rgb(196, 110, 190),
    Color32::from_rgb(210, 190, 80),
    Color32::from_rgb(120, 170, 190),
];

fn preview(ui: &mut Ui, w: &GearWindow) {
    let side = ui.available_width().min(420.0);
    let (rect, _) = ui.allocate_exact_size(vec2(side, side * 0.82), Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = ui.visuals().extreme_bg_color;
    painter.rect_filled(rect, 6.0, bg);
    let Ok(plan) = &w.plan else { return };
    // Fit every gear (at any turn: its largest radius about its centre) in the box.
    let (mut lo, mut hi) = (V2::new(f64::MAX, f64::MAX), V2::new(f64::MIN, f64::MIN));
    for (g, m) in plan.preview.iter().zip(&w.meshes) {
        if g.slide == 0.0 && g.speed != 0.0 {
            let r = m.outlines.iter().flatten().map(|q| q.len()).fold(0.0, f64::max);
            lo = V2::new(lo.x.min(g.center.x - r), lo.y.min(g.center.y - r));
            hi = V2::new(hi.x.max(g.center.x + r), hi.y.max(g.center.y + r));
        } else {
            for q in m.outlines.iter().flatten() {
                let p = g.center + *q;
                lo = V2::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = V2::new(hi.x.max(p.x), hi.y.max(p.y));
            }
        }
    }
    if !(lo.x.is_finite() && hi.x > lo.x) {
        return;
    }
    let margin = 12.0;
    let scale = ((rect.width() - 2.0 * margin) as f64 / (hi.x - lo.x)).min((rect.height() - 2.0 * margin) as f64 / (hi.y - lo.y).max(1e-9));
    let mid = (lo + hi) * 0.5;
    let c = rect.center();
    let to_screen = |p: V2| -> Pos2 { pos2(c.x + ((p.x - mid.x) * scale) as f32, c.y - ((p.y - mid.y) * scale) as f32) };
    let stroke_color = ui.visuals().text_color();
    for (i, (g, m)) in plan.preview.iter().zip(&w.meshes).enumerate() {
        let angle = g.speed * w.turned;
        let slide = g.slide * w.turned;
        let place = |q: V2| -> Pos2 {
            let q = if g.slide != 0.0 {
                // A rack: slides; the worm's section slides along its own axis (+y).
                if g.name == "worm" { q + V2::new(0.0, wrap_slide(slide, g)) } else { q + V2::new(wrap_slide(slide, g), 0.0) }
            } else {
                q.rot(angle)
            };
            to_screen(g.center + q)
        };
        let color = PALETTE[i % PALETTE.len()];
        let fill = color.gamma_multiply(0.55);
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
            painter.add(egui::Shape::line(pts, Stroke::new(1.0, color.gamma_multiply(0.9).lerp_to_gamma(stroke_color, 0.3))));
        }
        if g.pitch_radius > 0.0 {
            dashed_circle(&painter, to_screen(g.center), (g.pitch_radius * scale) as f32, Stroke::new(0.8, stroke_color.gamma_multiply(0.35)));
        }
        // A mark that shows the turning.
        if g.slide == 0.0 && g.speed != 0.0 {
            painter.circle_filled(place(V2::polar(g.pitch_radius.max(1.0) * 0.35, 0.0)), 2.5, stroke_color.gamma_multiply(0.6));
        }
        painter.circle_filled(to_screen(g.center), 2.0, stroke_color.gamma_multiply(0.6));
    }
    if !plan.preview_note.is_empty() {
        painter.text(rect.left_bottom() + vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, &plan.preview_note, egui::FontId::proportional(11.0), stroke_color.gamma_multiply(0.6));
    }
}

/// A sliding member moves on by whole tooth pitches only, so it stays in view and its teeth
/// keep meshing.
fn wrap_slide(s: f64, g: &solidcraft_gear::PreviewGear) -> f64 {
    if g.period > 0.0 { s.rem_euclid(g.period) - g.period / 2.0 } else { 0.0 }
}

fn dashed_circle(p: &egui::Painter, c: Pos2, r: f32, stroke: Stroke) {
    let n = ((r * 0.6) as usize).clamp(24, 240);
    let pts: Vec<Pos2> = (0..=n).map(|i| c + Vec2::angled(i as f32 / n as f32 * std::f32::consts::TAU) * r).collect();
    p.extend(egui::Shape::dashed_line(&pts, stroke, 4.0, 4.0));
}
