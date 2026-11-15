//! The differential designer in the Gear Generator window (`Kind::Differential`): a style picker
//! (open bevel, torque-biasing bevel, Torsen Type B, Torsen Type A), Simple mode designing it from
//! what it must do (axle, final drive ratio, torque, bias: `solidcraft_gear::diff::solve`),
//! Advanced mode with every member's settings (side gears and pinions or elements, the case, the
//! drive gear, the keep-out body), and a cross-section drawing of the layout.
//!
//! Written without a compile run (the integrator builds the UI): see the gear-diff branch notes.

use egui::{Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use solidcraft_gear::diff::{self, CASE_BEARINGS, Diff, DriveKind, Mount, Style};
use solidcraft_gear::plan::Spec;
use solidcraft_gear::strength::{MATERIALS, Material};

use crate::gear_window::{GearWindow, member_color};

fn num(ui: &mut Ui, label: &str, tip: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, unit: &str) -> bool {
    ui.horizontal(|ui| {
        let r = ui.add_sized([150.0, 20.0], egui::Label::new(label).truncate());
        if !tip.is_empty() {
            r.on_hover_text(tip);
        }
        ui.add(egui::DragValue::new(v).range(range).speed(speed).max_decimals(3).suffix(unit)).changed()
    })
    .inner
}

fn count(ui: &mut Ui, label: &str, tip: &str, v: &mut u32, range: std::ops::RangeInclusive<u32>) -> bool {
    ui.horizontal(|ui| {
        let r = ui.add_sized([150.0, 20.0], egui::Label::new(label).truncate());
        if !tip.is_empty() {
            r.on_hover_text(tip);
        }
        ui.add(egui::DragValue::new(v).range(range).speed(0.2)).changed()
    })
    .inner
}

fn heading(ui: &mut Ui, text: &str, color: Color32) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(11.0, 11.0), Sense::hover());
        ui.painter().rect_filled(r, 3.0, color);
        ui.label(RichText::new(text).strong());
    });
}

/// A small picture of a style: the side gears, the pinions or elements, the case round them.
fn style_icon(p: &egui::Painter, r: Rect, st: Style, ink: Color32) {
    let s = Stroke::new(1.4, ink);
    let c = r.center();
    let u = r.width().min(r.height()) / 2.0;
    // The case.
    p.rect_stroke(Rect::from_center_size(c, vec2(1.7 * u, 1.5 * u)), 3.0, s, egui::StrokeKind::Inside);
    p.line_segment([pos2(r.left(), c.y), pos2(r.right(), c.y)], Stroke::new(1.0, ink.gamma_multiply(0.6)));
    let gear = |a: Pos2, b: Pos2, w: f32| {
        p.add(egui::Shape::convex_polygon(vec![pos2(a.x, a.y - w), pos2(b.x, b.y - w * 0.6), pos2(b.x, b.y + w * 0.6), pos2(a.x, a.y + w)], ink.gamma_multiply(0.35), s));
    };
    match st {
        Style::Open | Style::Biased => {
            let (wa, wb) = if st == Style::Biased { (0.4 * u, 0.6 * u) } else { (0.5 * u, 0.5 * u) };
            gear(pos2(c.x - 0.7 * u, c.y), pos2(c.x - 0.3 * u, c.y), wa);
            gear(pos2(c.x + 0.7 * u, c.y), pos2(c.x + 0.3 * u, c.y), wb);
            let tilt = if st == Style::Biased { -0.12 * u } else { 0.0 };
            gear(pos2(c.x + tilt, c.y - 0.65 * u), pos2(c.x, c.y - 0.25 * u), 0.3 * u);
            gear(pos2(c.x + tilt, c.y + 0.65 * u), pos2(c.x, c.y + 0.25 * u), 0.3 * u);
        }
        Style::TorsenB => {
            p.rect_stroke(Rect::from_min_max(pos2(c.x - 0.75 * u, c.y - 0.35 * u), pos2(c.x - 0.1 * u, c.y + 0.35 * u)), 1.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(pos2(c.x + 0.1 * u, c.y - 0.35 * u), pos2(c.x + 0.75 * u, c.y + 0.35 * u)), 1.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(pos2(c.x - 0.75 * u, c.y - 0.7 * u), pos2(c.x + 0.25 * u, c.y - 0.4 * u)), 1.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(pos2(c.x - 0.25 * u, c.y + 0.4 * u), pos2(c.x + 0.75 * u, c.y + 0.7 * u)), 1.0, s, egui::StrokeKind::Inside);
        }
        Style::TorsenA => {
            p.rect_stroke(Rect::from_min_max(pos2(c.x - 0.75 * u, c.y - 0.2 * u), pos2(c.x - 0.1 * u, c.y + 0.2 * u)), 1.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(pos2(c.x + 0.1 * u, c.y - 0.2 * u), pos2(c.x + 0.75 * u, c.y + 0.2 * u)), 1.0, s, egui::StrokeKind::Inside);
            p.circle_stroke(pos2(c.x - 0.42 * u, c.y - 0.48 * u), 0.25 * u, s);
            p.circle_stroke(pos2(c.x + 0.42 * u, c.y - 0.48 * u), 0.25 * u, s);
        }
    }
}

/// The style picker; a new style starts from that style's defaults (keeping the axle and the
/// drive). True when it changed.
fn style_picker(ui: &mut Ui, d: &mut Diff) -> bool {
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        for st in Style::ALL {
            let selected = d.style == st;
            let (rect, resp) = ui.allocate_exact_size(vec2(120.0, 64.0), Sense::click());
            let visuals = ui.style().interact_selectable(&resp, selected);
            if selected || resp.hovered() {
                ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
            }
            let ink = visuals.text_color();
            style_icon(ui.painter(), Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(rect.width() - 8.0, 40.0)), st, ink);
            let short = match st {
                Style::Open => "Open bevel",
                Style::Biased => "Uneven bevel",
                Style::TorsenB => "Torsen B",
                Style::TorsenA => "Torsen A",
            };
            ui.painter().text(pos2(rect.center().x, rect.bottom() - 9.0), egui::Align2::CENTER_CENTER, short, FontId::proportional(12.0), ink);
            if resp.on_hover_text(st.label()).clicked() {
                pick = Some(st);
            }
        }
    });
    match pick {
        Some(st) if st != d.style => {
            let keep = (d.axle_diameter, d.drive, d.mount, d.keep_out);
            *d = Diff::defaults_for(st);
            (d.axle_diameter, d.drive, d.mount, d.keep_out) = keep;
            true
        }
        _ => false,
    }
}

fn drive_combo(ui: &mut Ui, id: &str, v: &mut DriveKind) {
    egui::ComboBox::from_id_salt(id).selected_text(v.label()).show_ui(ui, |ui| {
        for k in DriveKind::ALL {
            ui.selectable_value(v, k, k.label());
        }
    });
}

fn mount_combo(ui: &mut Ui, id: &str, v: &mut Mount) {
    egui::ComboBox::from_id_salt(id).selected_text(v.label()).show_ui(ui, |ui| {
        for k in Mount::ALL {
            ui.selectable_value(v, k, k.label());
        }
    });
}

fn material_combo(ui: &mut Ui, id: &str, v: &mut Material) {
    let label = v.data().label;
    egui::ComboBox::from_id_salt(id).selected_text(label).show_ui(ui, |ui| {
        for (m, data) in MATERIALS {
            ui.selectable_value(v, *m, data.label);
        }
    });
}

/// Simple mode's left column for a differential: the style, then what it must do; Design solves
/// it (a few seconds: it plans every candidate) and fills the set.
pub(crate) fn simple_panel(ui: &mut Ui, w: &mut GearWindow) {
    ui.label(RichText::new("Differential").strong());
    let mut d = w.spec.diff.clone();
    if style_picker(ui, &mut d) {
        w.diff_goal.style = d.style;
        w.spec.diff = d;
    }
    w.diff_goal.style = w.spec.diff.style;
    let g = &mut w.diff_goal;
    heading(ui, "What it must do", member_color(0));
    num(ui, "Axle diameter", "The axle shafts' diameter: the side gears' bores", &mut g.axle_diameter, 3.0..=200.0, 0.2, " mm");
    num(ui, "Final drive ratio", "Ring gear teeth ÷ drive pinion teeth", &mut g.ratio, 1.0..=12.0, 0.01, "");
    num(ui, "Ring gear Ø", "The ring gear's pitch diameter (0: the smallest that fits round the case and carries the torque)", &mut g.ring_diameter, 0.0..=2000.0, 1.0, " mm");
    num(ui, "Input torque", "At the drive pinion, for the strength rating (0: sized by the axle only)", &mut g.torque, 0.0..=100_000.0, 1.0, " N·m");
    count(ui, if g.style.bevel() { "Pinions" } else { "Element pairs" }, "0: the style's usual (2 spider pinions, 3 Torsen pairs)", &mut g.pinions, 0..=6);
    match g.style {
        Style::Biased => {
            num(ui, "Torque split T_B / T_A", "Side gear B's torque over side gear A's: the side gears' tooth ratio (0: 1.5)", &mut g.bias, 0.0..=3.0, 0.01, "");
        }
        Style::TorsenB | Style::TorsenA => {
            num(ui, "Torque bias ratio", "Wanted TBR (estimate; 0: from the default helix): the solver picks the helix, and says what friction it needs", &mut g.bias, 0.0..=6.0, 0.05, "");
        }
        Style::Open => {}
    }
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new("Drive"));
        drive_combo(ui, "diff_goal_drive", &mut g.drive);
    });
    if g.drive != DriveKind::None {
        ui.horizontal(|ui| {
            ui.add_sized([150.0, 20.0], egui::Label::new("Ring gear"));
            mount_combo(ui, "diff_goal_mount", &mut g.mount);
        });
    }
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new("Material"));
        material_combo(ui, "diff_goal_material", &mut g.material);
    });
    ui.add_space(4.0);
    if ui.button("Design").on_hover_text("Pick teeth, modules and angles for these (plans every candidate: a few seconds)").clicked() {
        match diff::solve(&w.diff_goal, &w.spec) {
            Ok(sol) => {
                w.spec = sol.spec;
                w.diff_notes = sol.notes;
            }
            Err(e) => w.diff_notes = vec![format!("⛔ {e}")],
        }
    }
    for n in &w.diff_notes {
        ui.label(RichText::new(n).small());
    }
    ui.checkbox(&mut w.spec.diff.keep_out, "Keep-out body for the axle housing").on_hover_text("One solid of the space the turning differential needs (case, ring and pinion envelopes, axles, bearing seats): cut it out of a housing");
}

/// Advanced mode's settings column for a differential: every member's settings.
pub(crate) fn advanced(ui: &mut Ui, s: &mut Spec) {
    let d = &mut s.diff;
    style_picker(ui, d);
    let st = d.style;
    if st == Style::TorsenA {
        ui.label(RichText::new("Torsen Type A: side gears are worms (helix = 90° − lead angle), elements are crossed helical wheels with spur ends; two pairs fit elements this size").small().weak());
    }
    heading(ui, if st.bevel() { "Side gears and pinions" } else { "Side gears and element gears" }, member_color(3));
    num(ui, "Module", if st.bevel() { "At the large end" } else { "Normal module" }, &mut d.module, 0.3..=20.0, 0.05, "");
    count(ui, "Side gear A teeth", "", &mut d.side_teeth, 1..=200);
    if st == Style::Biased {
        count(ui, "Side gear B teeth", "Different from A: the torque splits A : B in that ratio, and the pinion axes tilt to the axle (cos Σ = (B − A) ÷ (2 × pinion))", &mut d.side_teeth2, 6..=200);
    }
    count(ui, if st.bevel() { "Pinion teeth" } else { "Element teeth" }, "", &mut d.pinion_teeth, 6..=200);
    count(ui, if st.bevel() { "Pinions" } else { "Element pairs" }, "Evenly spaced when (A + B) ÷ count is whole", &mut d.pinions, 1..=8);
    num(ui, "Face width", "0: from the size", &mut d.face_width, 0.0..=500.0, 0.2, " mm");
    num(ui, "Pressure angle", "0: 20°", &mut d.pressure_angle, 0.0..=30.0, 0.1, "°");
    if st.bevel() {
        num(ui, "Spiral angle", "0: straight (the usual for side gears)", &mut d.spiral, -45.0..=45.0, 0.2, "°");
    } else {
        num(ui, "Helix angle", "Side A right hand, side B left; more helix, more thrust on the faces", &mut d.helix, 0.0..=80.0, 0.2, "°");
        num(ui, "Friction μ (estimate)", "What the torque bias estimate assumes at the pockets and thrust faces", &mut d.friction, 0.0..=0.5, 0.005, "");
    }
    num(ui, "Backlash", "", &mut d.backlash, 0.0..=2.0, 0.01, " mm");

    heading(ui, "Case", member_color(0));
    num(ui, "Axle diameter", "The side gears' bores", &mut d.axle_diameter, 1.0..=300.0, 0.2, " mm");
    if st.bevel() {
        num(ui, "Pin diameter", "0: about half the pinions' small ends", &mut d.pin_diameter, 0.0..=100.0, 0.1, " mm");
        ui.checkbox(&mut d.windows, "Assembly windows between the pinions");
    }
    num(ui, "Wall", "0: from the size", &mut d.wall, 0.0..=50.0, 0.1, " mm");
    num(ui, "Thrust washers", "Room behind the side gears (and elements) for thrust washers (0: from the size)", &mut d.washer, 0.0..=10.0, 0.05, " mm");
    num(ui, "Running clearance", "Round the gears inside the case", &mut d.clearance, 0.0..=5.0, 0.05, " mm");
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new("Bearings"));
        let shown = if d.bearing.trim().is_empty() { "smallest that fits".to_string() } else { d.bearing.clone() };
        egui::ComboBox::from_id_salt("diff_bearing").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut d.bearing, String::new(), "smallest that fits");
            for (n, b, od, w) in CASE_BEARINGS {
                ui.selectable_value(&mut d.bearing, n.to_string(), format!("{n}  {b}×{od}×{w}"));
            }
            for (n, b, od, w) in solidcraft_gear::hub::BEARINGS.iter().filter(|(_, b, ..)| *b >= 10.0) {
                ui.selectable_value(&mut d.bearing, n.to_string(), format!("{n}  {b}×{od}×{w}"));
            }
        });
    });

    heading(ui, "Drive gear", member_color(1));
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new("Drive"));
        drive_combo(ui, "diff_drive", &mut d.drive);
    });
    if d.drive != DriveKind::None {
        ui.horizontal(|ui| {
            ui.add_sized([150.0, 20.0], egui::Label::new("Ring gear"));
            mount_combo(ui, "diff_mount", &mut d.mount);
        });
        count(ui, "Ring gear teeth", "", &mut d.ring_teeth, 10..=200);
        count(ui, "Drive pinion teeth", "", &mut d.drive_pinion_teeth, 5..=60);
        num(ui, "Ring module", "Bevel: at the large end; hypoid: ring pitch Ø ÷ teeth; helical: normal", &mut d.drive_module, 0.3..=30.0, 0.05, "");
        num(ui, "Ring face width", "0: from the size", &mut d.drive_face, 0.0..=500.0, 0.2, " mm");
        match d.drive {
            DriveKind::SpiralBevel | DriveKind::Hypoid => {
                num(ui, "Pinion spiral angle", "", &mut d.drive_spiral, -60.0..=60.0, 0.2, "°");
            }
            DriveKind::Helical => {
                num(ui, "Helix angle", "", &mut d.drive_helix, -44.0..=44.0, 0.2, "°");
            }
            _ => {}
        }
        if d.drive == DriveKind::Hypoid {
            num(ui, "Hypoid offset", "0: 12 % of the ring's pitch diameter", &mut d.drive_offset, 0.0..=500.0, 0.2, " mm");
        }
        num(ui, "Position along the axle", "Where the pinion's axis crosses the axle (helical: the ring's middle plane), from the set's middle", &mut d.drive_position, -500.0..=500.0, 0.2, " mm");
        if d.mount == Mount::Bolted {
            ui.checkbox(&mut d.ring_bolts, "Flange and ring bolts").on_hover_text("Off: a plain ring on the case, fixed however you like");
        }
        if d.mount == Mount::Bolted && d.ring_bolts {
            count(ui, "Ring bolts", "0: from the bolt circle", &mut d.bolts, 0..=64);
            num(ui, "Bolt size (M)", "0: the largest that fits", &mut d.bolt_diameter, 0.0..=30.0, 1.0, "");
        }
    }
    ui.checkbox(&mut d.keep_out, "Keep-out body for the axle housing");

    heading(ui, "Load", member_color(2));
    num(ui, "Input torque", "At the drive pinion (the case's without a drive); 0: not rated", &mut s.load.torque, 0.0..=100_000.0, 1.0, " N·m");
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new("Gears' material"));
        material_combo(ui, "diff_mat1", &mut s.load.material);
    });
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new("Pinions' material"));
        material_combo(ui, "diff_mat2", &mut s.load.material2);
    });
    // The spec's own sizes mirror the differential's (the engine does the same).
    s.module = s.diff.module;
    s.teeth = s.diff.side_teeth;
    s.teeth2 = s.diff.pinion_teeth;
}

/// The cross-section drawing of the layout: axle, case, side gears, pinions or elements, the
/// ring gear and its pinion, with the main sizes.
pub(crate) fn drawing(ui: &mut Ui, s: &Spec, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
    let d = &s.diff;
    let ink = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let m = d.module;
    let (ra, rb) = (m * f64::from(d.side_teeth) / 2.0, m * f64::from(d.side_b()) / 2.0);
    let rp = m * f64::from(d.pinion_teeth) / 2.0;
    let ring = if d.drive == DriveKind::None { 0.0 } else { d.drive_module * f64::from(d.ring_teeth) / 2.0 };
    let span = (2.2 * (ra.max(rb) + rp)).max(2.0 * ring * 1.05).max(1.0);
    let k = (rect.width().min(rect.height()) as f64 * 0.85 / span) as f32;
    let c = rect.center();
    let at = |x: f64, y: f64| pos2(c.x + x as f32 * k, c.y - y as f32 * k);
    let stroke = |i: usize| Stroke::new(2.0, member_color(i));
    // The axle.
    p.line_segment([pos2(rect.left() + 6.0, c.y), pos2(rect.right() - 6.0, c.y)], Stroke::new(1.0, weak));
    let quad = |pts: [(f64, f64); 4], i: usize| {
        p.add(egui::Shape::convex_polygon(pts.iter().map(|(x, y)| at(*x, *y)).collect(), member_color(i).gamma_multiply(0.25), stroke(i)));
    };
    if d.style.bevel() {
        // Side gears on the axle (x), pinions across it, all meeting at the middle.
        let cs = (rb - ra) / (2.0 * rp);
        let sigma = cs.clamp(-0.9, 0.9).acos();
        let ha = rp.hypot(ra) * (ra.atan2(rp)).cos();
        let hb = rp.hypot(rb) * (rb.atan2(rp)).cos();
        for (h, r, sign) in [(ha, ra, -1.0), (hb, rb, 1.0)] {
            quad([(sign * h, r), (sign * 0.55 * h, 0.55 * r), (sign * 0.55 * h, -0.55 * r), (sign * h, -r)], 3);
        }
        for up in [1.0, -1.0] {
            let (sx, sy) = (sigma.cos(), up * sigma.sin());
            let hp = rp.hypot(ra) * rp.atan2(ra).cos();
            let (ex, ey) = (-sx * hp, sy * hp);
            let (nx, ny) = (-sy, -sx);
            quad([(ex + nx * rp, ey + ny * rp), (0.55 * ex + 0.55 * nx * rp, 0.55 * ey + 0.55 * ny * rp), (0.55 * ex - 0.55 * nx * rp, 0.55 * ey - 0.55 * ny * rp), (ex - nx * rp, ey - ny * rp)], 4);
        }
        let rc = 1.08 * ra.max(rb).hypot(rp);
        p.circle_stroke(c, rc as f32 * k, stroke(0));
    } else {
        let (w, re) = (if d.face_width > 0.0 { d.face_width } else { 8.0 * m }, rp);
        for sign in [-1.0, 1.0] {
            quad([(sign * 0.2 * w, ra), (sign * 1.2 * w, ra), (sign * 1.2 * w, -ra), (sign * 0.2 * w, -ra)], 3);
        }
        let a = ra + re;
        quad([(-1.2 * w, a + re), (0.5 * w, a + re), (0.5 * w, a - re), (-1.2 * w, a - re)], 4);
        quad([(-0.5 * w, -a + re), (1.2 * w, -a + re), (1.2 * w, -a - re), (-0.5 * w, -a - re)], 4);
        p.rect_stroke(Rect::from_two_pos(at(-1.4 * w, a + re + 0.6 * m), at(1.4 * w, -a - re - 0.6 * m)), 4.0, stroke(0), egui::StrokeKind::Outside);
    }
    if ring > 0.0 {
        let x = -0.3 * span + d.drive_position;
        p.line_segment([at(x, ring), at(x, -ring)], Stroke::new(5.0, member_color(1)));
        if d.drive != DriveKind::Helical {
            let rpin = d.drive_module * f64::from(d.drive_pinion_teeth) / 2.0;
            quad([(x + 0.02 * span, ring * 0.95), (x + 0.02 * span + 2.0 * rpin, ring * 0.9), (x + 0.02 * span + 2.0 * rpin, ring * 0.7), (x + 0.02 * span, ring * 0.65)], 2);
        }
        p.text(at(x, ring) + vec2(4.0, -2.0), egui::Align2::LEFT_BOTTOM, format!("ring Ø{:.0}", 2.0 * ring), FontId::proportional(11.0), ink);
    }
    let what = match d.style {
        Style::Open => format!("open: {}:{}:{} ×{}", d.side_teeth, d.pinion_teeth, d.side_teeth, d.pinions),
        Style::Biased => format!("uneven: {} : {} with {} ×{}  (split {:.2})", d.side_teeth, d.side_b(), d.pinion_teeth, d.pinions, f64::from(d.side_b()) / f64::from(d.side_teeth.max(1))),
        Style::TorsenB => format!("Torsen B: sides {}, elements {} ×{} pairs, helix {:.0}°", d.side_teeth, d.pinion_teeth, d.pinions, d.helix),
        Style::TorsenA => format!("Torsen A: worms {}, wheels {} ×{} pairs", d.side_teeth, d.pinion_teeth, d.pinions),
    };
    p.text(rect.left_top() + vec2(8.0, 6.0), egui::Align2::LEFT_TOP, what, FontId::proportional(12.0), ink);
    let ratio = if d.drive == DriveKind::None { String::new() } else { format!("final drive {}:{} = {:.3}", d.ring_teeth, d.drive_pinion_teeth, f64::from(d.ring_teeth) / f64::from(d.drive_pinion_teeth.max(1))) };
    p.text(rect.left_bottom() + vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("axle Ø{:.1}   {ratio}", d.axle_diameter), FontId::proportional(11.0), weak);
}
