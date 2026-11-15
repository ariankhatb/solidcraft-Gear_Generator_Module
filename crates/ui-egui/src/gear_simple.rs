//! Simple mode: the set is drawn in one sketch and sized by the Simple-mode solver.
//!
//! The user picks the mode (the type), and the window lists the mode's pick slots
//! (`sketch_rules::slots`: the origin dot first, then its circles and lines). Each slot is
//! filled from the window's drawing (in drawing order, or by a click there while the slot's Pick
//! is on) or from a sketch in the model ("Use selected"; everything comes from that one sketch).
//! `sketch_rules::measure` checks the picks against the mode's rule (its message says what is
//! wrong and what it would make instead) and measures them; `sketch_rules::recognize` only
//! hints at another mode. `sketch_rules::simple_of` lays the drawn sizes over the form's own
//! settings, `solidcraft_gear::simple::solve` sizes the set (standard modules, whole teeth,
//! shifts), and `sketch_rules::axes_for` gives its shafts in the world (the sketch's plane,
//! turned about the origin dot by the Turn buttons), which the window's axes step places it on.
//! Nothing here builds in the kernel: Create (or Show in model) does.

use egui::{Color32, RichText, Ui};
use solidcraft_gear::geom::V2;
use solidcraft_gear::plan::{Axes, Frame, Kind, Spec, Unit};
use solidcraft_gear::simple::{self, Field, Simple};
use solidcraft_gear::sketch_rules::{self, Circle, Drawn, Line, Measured, Picks, Role};

use crate::SolveApp;
use crate::gear_drawing::Canvas;
use crate::gear_window::{AxesStep, GearWindow, Note as _};

/// What a "Use selected" asked for: a slot (or just the sketch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UseSelected(pub Option<usize>);

/// Simple mode's state, kept by the window.
pub(crate) struct SimpleState {
    pub drawn: Drawn,
    /// The sketch's plane in the world (its z the normal).
    pub frame: Frame,
    /// The model sketch the drawing was read from, and its entities' ids (points, circles,
    /// lines, in the drawing's order).
    pub source: Option<u64>,
    pub ids: (Vec<String>, Vec<String>, Vec<String>),
    pub picks: Picks,
    /// The picks follow the drawing's order (until one is chosen by hand).
    pub auto: bool,
    /// The set turned about the origin dot (degrees about the sketch's x, y and normal).
    pub turn: [f64; 3],
    /// The form's own settings (what the drawing doesn't give).
    pub settings: Simple,
    /// Something changed: measure and solve again.
    pub dirty: bool,
    /// The drawing is the mode's starting picture, untouched.
    pub template: bool,
    pub measured: Option<Measured>,
    pub error: Option<String>,
    pub notes: Vec<String>,
    /// What the solver chose and what it came to.
    pub chosen: Vec<solidcraft_gear::Fact>,
    /// Another mode the drawing looks like.
    pub hint: Option<Kind>,
    pub canvas: Canvas,
    /// A slot waiting for a click in the drawing.
    pub armed: Option<usize>,
    /// A "Use selected" to take up after the window is drawn (where the design is at hand).
    pub request: Option<UseSelected>,
    /// What the last "Use selected" did.
    pub message: Option<String>,
    /// The spec's outputs, for the form's placeholders (per plan generation).
    pub outputs: Option<(u64, Simple)>,
}

impl SimpleState {
    pub fn new(spec: &Spec) -> SimpleState {
        SimpleState {
            drawn: template(spec.kind, spec),
            frame: Frame::at([0.0; 3]),
            source: None,
            ids: Default::default(),
            picks: Vec::new(),
            auto: true,
            turn: [0.0; 3],
            settings: Simple::default(),
            dirty: true,
            template: true,
            measured: None,
            error: None,
            notes: Vec::new(),
            chosen: Vec::new(),
            hint: None,
            canvas: Canvas::default(),
            armed: None,
            request: None,
            message: None,
            outputs: None,
        }
    }
}

/// The picture a mode starts from, sized by its defaults: the origin dot at gear 1's centre
/// (a bevel's apex), and the circles and lines its rule wants.
pub(crate) fn template(k: Kind, s: &Spec) -> Drawn {
    let r = |z: u32| s.module * f64::from(z) / 2.0;
    let c = |x: f64, y: f64, r: f64| Circle { c: V2::new(x, y), r };
    let l = |ax: f64, ay: f64, bx: f64, by: f64| Line { a: V2::new(ax, ay), b: V2::new(bx, by) };
    let mut d = Drawn { points: vec![V2::ZERO], ..Drawn::default() };
    match k {
        Kind::Pair | Kind::Cycloidal => {
            let (r1, r2) = (r(s.teeth), r(s.teeth2));
            d.circles = vec![c(0.0, 0.0, r1), c(r1 + r2, 0.0, r2)];
        }
        Kind::Internal => {
            let ring = r(s.teeth);
            d.circles = vec![c(0.0, 0.0, ring), c(ring * 2.0 / 3.0, 0.0, ring / 3.0)];
        }
        Kind::Planetary => {
            let (rs, rp) = (r(s.teeth), r(s.teeth2));
            d.circles = vec![c(0.0, 0.0, rs), c(rs + rp, 0.0, rp), c(0.0, 0.0, rs + 2.0 * rp)];
        }
        Kind::Rack => {
            let r1 = r(s.teeth);
            d.circles = vec![c(0.0, 0.0, r1)];
            d.lines = vec![l(-1.6 * r1, -r1, 1.6 * r1, -r1)];
        }
        Kind::Worm => {
            let r1 = r(s.teeth);
            d.circles = vec![c(0.0, 0.0, r1)];
            d.lines = vec![l(-1.3 * r1, r1, 1.3 * r1, r1)];
        }
        Kind::Bevel | Kind::SpiralBevel => {
            let len = 1.6 * r(s.teeth.max(10));
            let a = s.shaft_angle.to_radians();
            d.lines = vec![l(0.0, 0.0, 0.0, -len), l(0.0, 0.0, len * a.sin(), -len * a.cos())];
        }
        // The same side view: the apex dot, the sun's shaft down from it, the first planet's
        // at the cone angle.
        Kind::BevelPlanetary => {
            let len = 1.6 * r(s.teeth.max(10));
            let a = s.shaft_angle.to_radians();
            d.lines = vec![l(0.0, 0.0, 0.0, -len), l(0.0, 0.0, len * a.sin(), -len * a.cos())];
        }
        Kind::FaceGear => {
            let rf = r(s.teeth2);
            d.circles = vec![c(0.0, 0.0, rf)];
            d.lines = vec![l(0.0, 0.0, 1.4 * rf, 0.0)];
        }
        Kind::Hypoid => {
            let ring = r(s.teeth2);
            let e = if s.offset > 0.0 { s.offset } else { 0.125 * 2.0 * ring };
            d.circles = vec![c(0.0, 0.0, ring)];
            d.lines = vec![l(-1.2 * ring, -e, 1.2 * ring, -e)];
        }
        Kind::CrossedHelical => {
            let b1 = (s.shaft_angle / 2.0).to_radians();
            let (r1, r2) = (r(s.teeth) / b1.cos(), r(s.teeth2) / (s.shaft_angle.to_radians() - b1).cos());
            d.circles = vec![c(0.0, 0.0, r1)];
            d.lines = vec![l(-1.5 * (r1 + r2), r1 + r2, 1.5 * (r1 + r2), r1 + r2)];
        }
        Kind::Sprocket => {
            let p = solidcraft_gear::sprocket::chain_by_id(s.chain).map(|c| c.pitch).unwrap_or(s.chain_pitch);
            d.circles = vec![c(0.0, 0.0, p / (std::f64::consts::PI / f64::from(s.teeth.max(6))).sin() / 2.0)];
        }
        Kind::Pulley => {
            let p = solidcraft_gear::pulley::belt_by_id(s.belt).map(|b| b.pitch).unwrap_or(2.0);
            d.circles = vec![c(0.0, 0.0, p * f64::from(s.teeth.max(6)) / std::f64::consts::PI / 2.0)];
        }
        _ => d.circles = vec![c(0.0, 0.0, r(s.teeth))],
    }
    d
}

/// The form's fields the drawing gives (so the form leaves them out), and the size drivers
/// that would fight the drawn sizes.
fn drawn_field(k: Kind, f: Field, m: Option<&Measured>) -> bool {
    use Field as F;
    if matches!(f, F::OuterDiameter) {
        return true;
    }
    match k {
        Kind::Pair | Kind::Cycloidal => matches!(f, F::PitchDiameter | F::PitchDiameter2 | F::Ratio | F::CenterDistance | F::Teeth | F::Module),
        Kind::Sprocket | Kind::Pulley => matches!(f, F::PitchDiameter | F::Ratio | F::CenterDistance | F::Teeth),
        Kind::Internal => matches!(f, F::RingDiameter | F::PitchDiameter | F::Teeth | F::Module),
        Kind::Planetary => matches!(f, F::PitchDiameter | F::PitchDiameter2 | F::RingDiameter | F::Ratio),
        Kind::Rack => matches!(f, F::PitchDiameter | F::Length | F::Teeth | F::Module),
        Kind::Worm => matches!(f, F::PitchDiameter2 | F::CenterDistance) || (f == F::PitchDiameter && m.is_some_and(|m| m.worm_diameter > 0.0)),
        Kind::Bevel | Kind::SpiralBevel | Kind::BevelPlanetary => f == F::ShaftAngle,
        Kind::FaceGear => f == F::PitchDiameter2,
        Kind::Hypoid => matches!(f, F::PitchDiameter2 | F::PitchDiameter | F::Offset),
        Kind::CrossedHelical => matches!(f, F::PitchDiameter | F::CenterDistance | F::Ratio),
        _ => matches!(f, F::PitchDiameter | F::Teeth | F::Module),
    }
}

/// Measures the drawing for the mode, sizes the set and puts it on its shafts; called when
/// anything Simple mode holds changed.
///
/// ADAPTER: the only place Simple mode turns its inputs into the window's spec and axes.
pub(crate) fn apply(w: &mut GearWindow) {
    if !w.simple.dirty {
        return;
    }
    w.simple.dirty = false;
    let k = w.spec.kind;
    let s = &mut w.simple;
    if !sketch_rules::sketched(k) {
        s.error = Some(format!("A {} isn't drawn: set it up in Advanced", k.label()));
        s.measured = None;
        return;
    }
    if s.auto || s.picks.len() != sketch_rules::slots(k).len() {
        s.picks = sketch_rules::auto_picks(k, &s.drawn);
    }
    s.hint = sketch_rules::recognize(&s.drawn).filter(|h| !sketch_rules::same_family(*h, k));
    let m = match sketch_rules::measure(k, &s.drawn, &s.picks) {
        Ok(m) => m,
        Err(e) => {
            s.error = Some(e);
            s.measured = None;
            return;
        }
    };
    let sol = simple::solve(&sketch_rules::simple_of(&m, &s.settings), &w.spec);
    s.chosen = sol.chosen;
    s.notes = sol.notes;
    s.error = sol.error.clone();
    if sol.error.is_some() {
        s.measured = Some(m);
        return;
    }
    let (a1, a2) = sketch_rules::axes_for(&m, &sol.spec, &s.frame, s.turn);
    s.measured = Some(m);
    w.spec = Spec { placement: None, clearance_body: w.spec.clearance_body.clone(), ..sol.spec };
    w.axes = AxesStep::sketched(a1, a2);
    w.planned = None;
}

/// Switches Simple mode to another mode: the drawing stays when it fits the new mode or was
/// drawn by the user; an untouched starting picture is replaced by the new mode's.
pub(crate) fn set_mode(w: &mut GearWindow, k: Kind) {
    if w.spec.kind == k {
        return;
    }
    w.switch_kind(k);
    let s = &mut w.simple;
    s.settings = Simple::default();
    s.auto = true;
    s.armed = None;
    let fits = sketch_rules::measure(k, &s.drawn, &sketch_rules::auto_picks(k, &s.drawn)).is_ok();
    if s.template && !fits {
        s.drawn = template(k, &w.spec);
        s.template = true;
    }
    s.dirty = true;
}

/// The modes Simple mode draws, grouped as in the type picker (the bevel planetary set among the
/// meeting shafts).
fn mode_picker(ui: &mut Ui, w: &mut GearWindow) {
    let mut pick = None;
    for (group, label) in [(Axes::Parallel, "Parallel shafts"), (Axes::Intersecting, "Meeting shafts"), (Axes::Crossed, "Crossed shafts"), (Axes::Wrapped, "Chain and belt"), (Axes::Special, "Special")] {
        // The group's name on its own row, its kinds wrapped under it.
        ui.add_space(2.0);
        ui.label(RichText::new(label).small().weak());
        ui.horizontal_wrapped(|ui| {
            for k in Kind::ALL.iter().copied().filter(|k| k.axes() == group && sketch_rules::sketched(*k)) {
                if crate::gear_icons::choice(ui, w.spec.kind == k, crate::gear_icons::kind_icon(k), k.label()).clicked() {
                    pick = Some(k);
                }
            }
        });
    }
    if let Some(k) = pick {
        set_mode(w, k);
    }
}

fn entity_name(d: &Drawn, role: Role, i: usize) -> String {
    match role {
        Role::Point => format!("dot {}", i + 1),
        Role::Circle => d.circles.get(i).map(|c| format!("circle {} · Ø{}", i + 1, crate::gear_view3d::fmt(2.0 * c.r))).unwrap_or_default(),
        Role::Line => d.lines.get(i).map(|l| format!("line {} · {}", i + 1, crate::gear_view3d::fmt(l.len()))).unwrap_or_default(),
    }
}

/// The mode's pick slots: what each wants, what fills it, and where to take it from.
fn slots_ui(ui: &mut Ui, w: &mut GearWindow) {
    let k = w.spec.kind;
    let slots = sketch_rules::slots(k);
    let s = &mut w.simple;
    if s.picks.len() != slots.len() {
        s.picks.resize(slots.len(), None);
    }
    ui.label(RichText::new("From the sketch").strong()).on_hover_text("One sketch for the whole set: circles are pitch circles, lines are shafts");
    for (i, slot) in slots.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            let filled = s.picks[i].is_some();
            let mark = if filled { "✓" } else if slot.optional { "○" } else { "•" };
            ui.label(RichText::new(mark).color(if filled { Color32::from_rgb(70, 170, 90) } else { ui.visuals().weak_text_color() }));
            ui.label(RichText::new(slot.label).strong().small()).on_hover_text(slot.hint);
            let count = match slot.role {
                Role::Point => s.drawn.points.len(),
                Role::Circle => s.drawn.circles.len(),
                Role::Line => s.drawn.lines.len(),
            };
            let current = s.picks[i].map(|j| entity_name(&s.drawn, slot.role, j)).unwrap_or_else(|| if slot.optional { "none".into() } else { "pick…".into() });
            let mut choice = s.picks[i];
            egui::ComboBox::from_id_salt(("gear_slot", i)).width(130.0).selected_text(current).show_ui(ui, |ui| {
                ui.selectable_value(&mut choice, None, "none");
                for j in 0..count {
                    ui.selectable_value(&mut choice, Some(j), entity_name(&s.drawn, slot.role, j));
                }
            });
            if choice != s.picks[i] {
                s.picks[i] = choice;
                s.auto = false;
                s.dirty = true;
            }
            let armed = s.armed == Some(i);
            if ui.selectable_label(armed, "Pick").on_hover_text("Then click it in the drawing").clicked() {
                s.armed = if armed { None } else { Some(i) };
                s.canvas.tool = crate::gear_drawing::Tool::Select;
            }
            if ui.small_button("Use selected").on_hover_text("Take what is selected in the model's sketch (the drawing becomes that sketch)").clicked() {
                s.request = Some(UseSelected(Some(i)));
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("In drawing order").on_hover_text("Fill every slot from the drawing again, in the order things were drawn").clicked() {
            s.auto = true;
            s.dirty = true;
        }
        if ui.small_button("Use selected sketch").on_hover_text("Read the circles, lines and points of the sketch selected in the model").clicked() {
            s.request = Some(UseSelected(None));
        }
        if ui.small_button(format!("Start from the {} picture", k.label())).clicked() {
            s.drawn = template(k, &w.spec);
            s.source = None;
            s.frame = Frame::at([0.0; 3]);
            s.template = true;
            s.auto = true;
            s.dirty = true;
        }
    });
    if let Some(id) = s.source {
        ui.label(RichText::new(format!("Read from sketch {id}: edits here don't change it")).small().weak());
    }
    if let Some(m) = &s.message {
        ui.note(m, ui.visuals().warn_fg_color);
    }
}

fn unit_suffix(u: Unit) -> &'static str {
    match u {
        Unit::Mm => " mm",
        Unit::Deg => "°",
        Unit::None => "",
    }
}

/// The form's settings: the family's fields the drawing doesn't give.
fn settings_ui(ui: &mut Ui, w: &mut GearWindow) {
    let k = w.spec.kind;
    let plan_gen = w.plan_gen;
    let s = &mut w.simple;
    if s.outputs.as_ref().is_none_or(|(g, _)| *g != plan_gen) {
        s.outputs = Some((plan_gen, simple::outputs_of(&w.spec)));
    }
    let outputs = s.outputs.as_ref().map(|(_, o)| o.clone()).unwrap_or_default();
    let rows: Vec<_> = simple::fields(Some(k)).into_iter().filter(|f| !drawn_field(k, f.field, s.measured.as_ref())).collect();
    if rows.is_empty() {
        return;
    }
    ui.add_space(4.0);
    ui.label(RichText::new("Settings").strong());
    let mut changed = false;
    egui::Grid::new("gear_simple_settings").num_columns(3).spacing([8.0, 3.0]).show(ui, |ui| {
        for f in rows {
            ui.label(f.label);
            let st = &mut s.settings;
            match f.field {
                Field::LeftHand => {
                    let mut v = st.left_hand.unwrap_or(outputs.left_hand.unwrap_or(false));
                    if ui.checkbox(&mut v, "").changed() {
                        st.left_hand = Some(v);
                        changed = true;
                    }
                    ui.label("");
                }
                Field::Herringbone | Field::Exact | Field::Printable => {
                    let v = match f.field {
                        Field::Herringbone => &mut st.herringbone,
                        Field::Exact => &mut st.exact,
                        _ => &mut st.printable,
                    };
                    changed |= ui.checkbox(v, "").changed();
                    ui.label("");
                }
                Field::Chain => {
                    let current = st.chain.or(outputs.chain).and_then(solidcraft_gear::sprocket::chain_by_id).map(|c| c.label).unwrap_or("?");
                    egui::ComboBox::from_id_salt("gear_simple_chain").selected_text(current).show_ui(ui, |ui| {
                        for c in solidcraft_gear::sprocket::CHAINS {
                            if ui.selectable_label(st.chain == Some(c.id), c.label).clicked() {
                                st.chain = Some(c.id);
                                changed = true;
                            }
                        }
                    });
                    ui.label("");
                }
                Field::PlanetMode => {
                    use solidcraft_gear::plan::BevelPlanetMode;
                    let now = st.bevel_planet_mode.or(outputs.bevel_planet_mode).unwrap_or_default();
                    egui::ComboBox::from_id_salt("gear_simple_planet_mode").selected_text(now.label()).show_ui(ui, |ui| {
                        for m in BevelPlanetMode::ALL {
                            if ui.selectable_label(now == m, m.label()).clicked() {
                                st.bevel_planet_mode = Some(m);
                                changed = true;
                            }
                        }
                    });
                    ui.label("");
                }
                Field::Belt => {
                    let current = st.belt.or(outputs.belt).and_then(solidcraft_gear::pulley::belt_by_id).map(|b| b.label).unwrap_or("?");
                    egui::ComboBox::from_id_salt("gear_simple_belt").selected_text(current).show_ui(ui, |ui| {
                        for b in solidcraft_gear::pulley::BELTS {
                            if ui.selectable_label(st.belt == Some(b.id), b.label).clicked() {
                                st.belt = Some(b.id);
                                changed = true;
                            }
                        }
                    });
                    ui.label("");
                }
                field => {
                    let given = st.get(field);
                    let mut v = given.or(outputs.get(field)).unwrap_or(0.0);
                    let whole = matches!(field, Field::Teeth | Field::Planets | Field::Starts);
                    let dv = egui::DragValue::new(&mut v).speed(if whole { 0.2 } else { 0.1 }).max_decimals(if whole { 0 } else { 3 }).suffix(unit_suffix(f.unit));
                    let r = ui.add(dv);
                    if r.changed() {
                        st.set(field, Some(if whole { v.round().max(1.0) } else { v }));
                        changed = true;
                    }
                    if given.is_some() {
                        if ui.small_button("auto").on_hover_text("Let the solver choose it again").clicked() {
                            st.set(field, None);
                            changed = true;
                        }
                    } else {
                        ui.label(RichText::new("auto").small().weak());
                    }
                }
            }
            ui.end_row();
        }
    });
    if changed {
        s.dirty = true;
    }
}

/// Turning the set about the origin dot, to stand it in the plane wanted.
fn turn_ui(ui: &mut Ui, w: &mut GearWindow) {
    let s = &mut w.simple;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Turn about the origin").small()).on_hover_text("The set as drawn, turned about the origin dot: in 90° steps about the sketch's X, Y or normal, or by any angle about the normal");
        for (i, name) in [(0, "X"), (1, "Y"), (2, "normal")] {
            if ui.small_button(format!("⟲ {name} 90°")).clicked() {
                s.turn[i] = (s.turn[i] + 90.0).rem_euclid(360.0);
                s.dirty = true;
            }
        }
        if ui.add(egui::DragValue::new(&mut s.turn[2]).range(-360.0..=360.0).speed(0.5).suffix("°").prefix("normal ")).changed() {
            s.dirty = true;
        }
        if s.turn != [0.0; 3] && ui.small_button("Reset").clicked() {
            s.turn = [0.0; 3];
            s.dirty = true;
        }
    });
}

/// Simple mode's left column: mode, picks, messages, settings, turn.
pub(crate) fn panel(ui: &mut Ui, w: &mut GearWindow) {
    mode_picker(ui, w);
    ui.add_space(4.0);
    if let Some(h) = w.simple.hint {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("These picks look like a {}.", h.label())).small());
            if ui.small_button("Switch").clicked() {
                set_mode(w, h);
            }
        });
    }
    slots_ui(ui, w);
    if let Some(e) = &w.simple.error {
        ui.note(e, ui.visuals().error_fg_color);
    }
    for n in &w.simple.notes {
        ui.note(n, ui.visuals().weak_text_color());
    }
    settings_ui(ui, w);
    let (names, warnings, facts) = w.clearance_inputs();
    crate::gear_clearance_ui::section(ui, &mut w.spec, false, &names, &warnings, &facts);
    ui.add_space(4.0);
    turn_ui(ui, w);
}

/// The drawing (and the side view of an angle out of its plane), in the right column.
pub(crate) fn drawing(ui: &mut Ui, w: &mut GearWindow, size: egui::Vec2) {
    let k = w.spec.kind;
    let side = matches!(k, Kind::Hypoid | Kind::CrossedHelical);
    let lit = w.lit();
    let (bodies, _, _) = w.clearance_inputs();
    let s = &mut w.simple;
    let main = if side { egui::vec2(size.x, size.y * 0.66) } else { size };
    let changed = crate::gear_drawing::sketch(
        ui,
        &mut s.canvas,
        &mut s.drawn,
        crate::gear_drawing::Ctx { kind: k, picks: &mut s.picks, auto: &mut s.auto, armed: &mut s.armed, spec: &mut w.spec, lit, bodies: &bodies },
        main,
    );
    if changed.drawing {
        s.template = false;
        // An edit in the drawing no longer matches the model's sketch.
        s.dirty = true;
    }
    if changed.face {
        // A bevel's face typed or dragged in the drawing: gear 1's width is the form's own
        // setting (the solver keeps it); gear 2's width and heel offset ride on the spec.
        s.settings.face_width = Some(w.spec.face_width);
    }
    if changed.picked || changed.spec {
        s.dirty = true;
    }
    if side {
        // The shaft angle is a setting here: the side view edits it (and a hypoid's faces).
        let mut spec = Spec { shaft_angle: s.settings.shaft_angle.unwrap_or(w.spec.shaft_angle), helix: s.settings.helix_split.unwrap_or(w.spec.helix), ..w.spec.clone() };
        let before = spec.clone();
        if crate::gear_drawing::side_view(ui, &mut s.canvas, &mut spec, egui::vec2(size.x, size.y * 0.34)) {
            if spec.shaft_angle != before.shaft_angle {
                s.settings.shaft_angle = Some(spec.shaft_angle);
            }
            if k == Kind::CrossedHelical && (spec.helix != before.helix || spec.shaft_angle != before.shaft_angle) {
                s.settings.helix_split = Some(spec.helix);
            }
            if spec.face_width != before.face_width {
                s.settings.face_width = Some(spec.face_width);
                w.spec.face_width = spec.face_width;
            }
            w.spec.face_width2 = spec.face_width2;
            w.spec.face_offset2 = spec.face_offset2;
            s.dirty = true;
        }
    }
}

/// Takes up a "Use selected": reads the model's sketch the selection is in (the drawing
/// becomes it) and fills the slot from the selected entity.
pub(crate) fn use_selected(app: &SolveApp, w: &mut GearWindow, req: UseSelected) {
    use solidcraft_engine::Sel;
    let st = crate::open_meshing::session_world(&app.session);
    let mut found: Option<(u64, Role, String)> = None;
    for sel in app.session.selection.iter() {
        match sel {
            Sel::SketchCurve { id, .. } => {
                if let Some(sid) = crate::dialogs::picks_sketch(app, std::slice::from_ref(sel), None) {
                    let role = st.sketch(sid).and_then(|ss| ss.sketch.curve_index(id).map(|i| matches!(ss.sketch.curves[i].kind, solidcraft_engine::sketch::CurveKind::Circle { .. }))).map(|circle| if circle { Role::Circle } else { Role::Line });
                    if let Some(role) = role {
                        found = Some((sid, role, id.clone()));
                        break;
                    }
                }
            }
            Sel::SketchPoint { id } => {
                let active = app.session.active_sketch.filter(|a| st.sketch(*a).is_some_and(|ss| ss.sketch.point_index(id).is_some()));
                let any = active.or_else(|| st.sketches.iter().rev().find(|ss| ss.sketch.point_index(id).is_some()).map(|ss| ss.feature));
                if let Some(sid) = any {
                    found = Some((sid, Role::Point, id.clone()));
                    break;
                }
            }
            _ => {}
        }
    }
    let s = &mut w.simple;
    let Some((sid, role, id)) = found else {
        s.message = Some("Select a circle, line or point of a sketch in the model first, then press Use selected".into());
        return;
    };
    if let (Some(src), Some(_)) = (s.source, req.0)
        && src != sid
    {
        s.message = Some(format!("That is in sketch {sid}; this drawing is sketch {src}. The whole set is drawn in one sketch: pick from it, or press Use selected sketch to switch to the other"));
        return;
    }
    if s.source != Some(sid) {
        match solidcraft_engine::gear_sketch(&app.session, sid) {
            Ok(gs) => {
                s.drawn = gs.drawn();
                s.frame = gs.frame;
                s.ids = (gs.points.iter().map(|p| p.0.clone()).collect(), gs.circles.iter().map(|c| c.0.clone()).collect(), gs.lines.iter().map(|l| l.0.clone()).collect());
                s.source = Some(sid);
                s.template = false;
                s.auto = true;
                s.dirty = true;
            }
            Err(e) => {
                s.message = Some(format!("Couldn't read sketch {sid}: {e}"));
                return;
            }
        }
    }
    s.message = None;
    let Some(slot) = req.0 else { return };
    let want = sketch_rules::slots(w.spec.kind).get(slot).map(|x| x.role);
    if want != Some(role) {
        s.message = Some(format!("This slot takes a {}", match want {
            Some(Role::Point) => "point",
            Some(Role::Circle) => "circle",
            _ => "line",
        }));
        return;
    }
    let list = match role {
        Role::Point => &mut s.ids.0,
        Role::Circle => &mut s.ids.1,
        Role::Line => &mut s.ids.2,
    };
    let idx = match list.iter().position(|x| *x == id) {
        Some(i) => i,
        // A point that is a curve's end or centre: taken in as the origin dot.
        None if role == Role::Point => {
            let p = st.sketch(sid).and_then(|ss| ss.sketch.point(ss.sketch.point_index(&id)?)).map(|p| V2::new(p.x, p.y));
            let Some(p) = p else { return };
            list.push(id);
            s.drawn.points.push(p);
            s.drawn.points.len() - 1
        }
        None => return,
    };
    if s.picks.len() <= slot {
        s.picks.resize(slot + 1, None);
    }
    s.picks[slot] = Some(idx);
    s.auto = false;
    s.dirty = true;
}

/// A window opened with sketch geometry selected starts from that sketch, in the mode it looks
/// like.
pub(crate) fn open_on_selection(app: &SolveApp, w: &mut GearWindow) {
    let any = app.session.selection.iter().any(|s| matches!(s, solidcraft_engine::Sel::SketchCurve { .. } | solidcraft_engine::Sel::SketchPoint { .. }));
    if !any {
        return;
    }
    use_selected(app, w, UseSelected(None));
    if w.simple.source.is_some()
        && let Some(k) = sketch_rules::recognize(&w.simple.drawn)
    {
        w.simple.template = false;
        set_mode(w, k);
    }
}
