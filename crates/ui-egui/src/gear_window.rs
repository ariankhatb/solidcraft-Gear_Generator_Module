//! The Gear Generator window (`gear.create`'s ribbon button): pick a gear type, set its
//! numbers, watch the set mesh in an animated 2D preview with its key figures and warnings, see
//! the solids previewed in the view, and Create them as one undo step.

use egui::{Color32, Pos2, Sense, Stroke, Ui, Vec2, pos2, vec2};
use solidcraft_gear::geom::{Region, V2};
use solidcraft_gear::hub::{Bore, BoreKind};
use solidcraft_gear::involute::BasicRack;
use solidcraft_gear::plan::{Axes, Kind, Plan, Spec, Unit};

use crate::SolveApp;
use crate::ui_kit;

/// What the window holds between frames.
pub struct GearWindow {
    pub spec: Spec,
    /// The plan for `spec`, or why there is none; recomputed when the spec changes.
    plan: Result<Plan, String>,
    planned: Option<Spec>,
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
        if self.planned.as_ref() == Some(&self.spec) {
            return;
        }
        self.planned = Some(self.spec.clone());
        self.plan = solidcraft_gear::plan(&self.spec);
        self.meshes = match &self.plan {
            Ok(p) => p.preview.iter().map(|g| mesh_of(&g.region)).collect(),
            Err(_) => Vec::new(),
        };
    }

    /// The command the view previews while the window is open.
    pub fn preview_commands(&self) -> Option<Vec<(String, serde_json::Value)>> {
        (self.preview_3d && self.plan.is_ok()).then(|| vec![("gear.create".to_string(), solidcraft_engine::gear_params_of(&self.spec))])
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
    if action == ui_kit::Action::Ok {
        let params = solidcraft_engine::gear_params_of(&w.spec);
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

fn body(ui: &mut Ui, w: &mut GearWindow, busy: bool, view_error: Option<&str>) {
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
    for (axes, label) in [(Axes::Parallel, "Parallel axes"), (Axes::Intersecting, "Intersecting axes"), (Axes::Crossed, "Crossed axes"), (Axes::Special, "Special")] {
        ui.horizontal_wrapped(|ui| {
            ui.add_sized([110.0, 18.0], egui::Label::new(egui::RichText::new(label).small().weak()));
            for k in Kind::ALL.iter().copied().filter(|k| k.axes() == axes) {
                if ui.selectable_label(w.spec.kind == k, k.label()).clicked() && w.spec.kind != k {
                    // A new type starts from its own defaults, keeping the hub choices.
                    let keep = w.spec.clone();
                    let mut s = Spec::defaults_for(k);
                    if k.involute() == keep.kind.involute() && !matches!(k, Kind::Hypoid | Kind::Worm | Kind::Cycloidal) {
                        s.module = keep.module;
                        s.pressure_angle = keep.pressure_angle;
                        s.rack = keep.rack;
                        s.backlash = keep.backlash;
                    }
                    if k != Kind::Hypoid {
                        // (A hypoid ring gear keeps its own centre hole.)
                        s.bore = keep.bore;
                        s.bore2 = keep.bore2;
                    }
                    s.origin = keep.origin;
                    s.name = keep.name;
                    w.spec = s;
                    w.turned = 0.0;
                }
            }
        });
    }
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
            if w.inch {
                let mut dp = 25.4 / s.module;
                row(ui, "Diametral pitch", "Teeth per inch of pitch diameter", |ui| {
                    if ui.add(egui::DragValue::new(&mut dp).range(0.5..=200.0).speed(0.1).max_decimals(3)).changed() {
                        s.module = 25.4 / dp;
                    }
                    ui.checkbox(&mut w.inch, "inch");
                });
            } else {
                let (label, tip) = match k {
                    Kind::Worm => ("Axial module", "Pitch diameter ÷ teeth (mm)"),
                    Kind::Hypoid => ("Ring gear module", "The ring gear's outer pitch diameter ÷ its teeth (mm)"),
                    _ => ("Module", "Pitch diameter ÷ teeth (mm)"),
                };
                row(ui, label, tip, |ui| {
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
            } else {
                count(ui, t1, "", &mut s.teeth, 3..=2000);
            }
            if let Some(t2) = t2 {
                count(ui, t2, "", &mut s.teeth2, if k == Kind::Cycloidal { 0..=2000 } else { 3..=2000 });
            }
            if k == Kind::Hypoid {
                let mut d = s.module * f64::from(s.teeth2);
                row(ui, "Ring gear Ø", "Outer pitch diameter of the ring gear; sets the module", |ui| {
                    if ui.add(egui::DragValue::new(&mut d).range(5.0..=10000.0).speed(0.5).max_decimals(3).suffix(" mm")).changed() && s.teeth2 > 0 {
                        s.module = d / f64::from(s.teeth2);
                    }
                });
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
                num(ui, "Worm pitch Ø", "0: 10 modules", &mut s.worm_diameter, 0.0..=5000.0, 0.1, " mm");
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
                num(ui, "Shaft angle", "Angle between the axes", &mut s.shaft_angle, 5.0..=175.0, 0.5, "°");
            }
            if k == Kind::CrossedHelical {
                num(ui, "Gear 1 helix", "Gear 2 takes the rest of the shaft angle", &mut s.helix, -75.0..=75.0, 0.5, "°");
            }
            if k == Kind::SpiralBevel {
                num(ui, "Spiral angle", "Mean spiral angle; 35° is usual", &mut s.spiral_angle, 0.0..=60.0, 0.5, "°");
            }
            if k == Kind::Hypoid {
                num(ui, "Pinion offset", "Distance between the axes; 0 makes a spiral bevel pair. Car axles use about 10–15% of the ring gear Ø", &mut s.offset, 0.0..=5000.0, 0.5, " mm");
                num(ui, "Pinion spiral angle", "Mean spiral angle of the pinion; the ring gear's is smaller by the offset angle. 45–50° is usual", &mut s.spiral_angle, 0.0..=70.0, 0.5, "°");
                row(ui, "Pinion hand", "Left is usual (car axles: pinion below centre); the ring gear gets the other hand", |ui| {
                    ui.radio_value(&mut s.left_hand, true, "left");
                    ui.radio_value(&mut s.left_hand, false, "right");
                });
                num(ui, "Cutter radius", "Face-milling cutter radius: the curvature of the tooth lines; 0: a standard cutter", &mut s.cutter_radius, 0.0..=2000.0, 0.5, " mm");
            }
            if k.involute() && !matches!(k, Kind::Worm | Kind::Hypoid) {
                num(ui, if k.is_set() { "Shift x₁" } else { "Profile shift x" }, "Profile shift coefficient: positive thickens the root and avoids undercut", &mut s.shift, -1.5..=1.5, 0.01, "");
                if matches!(k, Kind::Pair | Kind::Planetary | Kind::CrossedHelical) {
                    num(ui, "Shift x₂", "Gear 2's (planets') profile shift", &mut s.shift2, -1.5..=1.5, 0.01, "");
                }
            }
            if k == Kind::Pair {
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
        if k == Kind::Hypoid {
            num(ui, "Ring gear web", "Thickness of the ring gear's back; 0: 8% of its Ø", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
        }
        if matches!(k, Kind::Internal | Kind::Planetary) {
            num(ui, "Ring rim", "Beyond the roots; 0: 3 modules", &mut s.rim, 0.0..=1000.0, 0.1, " mm");
        }
        if k == Kind::Rack {
            num(ui, "Height below pitch", "0: 3 modules", &mut s.rack_height, 0.0..=1000.0, 0.1, " mm");
        }
        if k != Kind::Rack && k != Kind::CycloidalDrive {
            bore(ui, if k == Kind::Hypoid { "Pinion bore" } else { "Bore" }, "bore1", &mut s.bore);
            if k.is_set() {
                let who = match k {
                    Kind::Planetary => "Planet bore",
                    Kind::Worm => "Worm bore",
                    Kind::Hypoid => "Ring gear bore",
                    _ => "Gear 2 bore",
                };
                bore(ui, who, "bore2", &mut s.bore2);
            }
            if k == Kind::Hypoid {
                num(ui, "Pinion shank Ø", "Out of the pinion's heel; 0: 60% of its root", &mut s.hub_diameter, 0.0..=5000.0, 0.2, " mm");
                num(ui, "Pinion shank length", "0: the pinion's face width", &mut s.hub_length, 0.0..=5000.0, 0.2, " mm");
            } else if k != Kind::Internal {
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
                    Kind::Hypoid => &["both", "pinion", "ring gear"],
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
        row(ui, "Centre at", if k == Kind::Hypoid { "Where the pinion's axis crosses the ring gear's" } else { "Where gear 1's centre goes" }, |ui| {
            for v in &mut s.origin {
                ui.add(egui::DragValue::new(v).speed(0.5).max_decimals(3));
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
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        Ok(p) => {
            for warn in &p.warnings {
                ui.colored_label(ui.visuals().warn_fg_color, format!("⚠ {warn}"));
            }
            egui::ScrollArea::vertical().id_salt("gear_facts").max_height(170.0).auto_shrink([false, true]).show(ui, |ui| {
                egui::Grid::new("gear_facts_grid").num_columns(2).striped(true).spacing([12.0, 2.0]).show(ui, |ui| {
                    for f in &p.facts {
                        ui.label(egui::RichText::new(&f.label).small());
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
