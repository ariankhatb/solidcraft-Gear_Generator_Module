//! The Gear Generator window's "Housing clearance" section: one more body holding all the space
//! the assembled set needs to turn (tip envelopes, shafts, bearing seats), to Boolean-cut out of
//! any housing. Fields and wording follow `crates/gear/CLEARANCE_UI.md`. Simple mode shows the
//! switch, the bearing and the seats; Advanced mode adds the clearances, the shaft run-on, the
//! ring, the fit, a custom bearing size and the rack's travel.

use egui::{RichText, Ui};
use solidcraft_gear::clearance::{BearingSize, Clearance, Seat};
use solidcraft_gear::hub::{BEARINGS, bearing, bearing_series};
use solidcraft_gear::plan::{Kind, Spec};

use crate::gear_window::Note as _;

const SWITCH_TIP: &str = "One more solid holding all the space the assembled set needs to turn: the teeth's swept envelopes, the shafts through the housing walls and the bearing seats. Model any housing block and cut this body out of it in one Boolean.";

/// The bearing's (OD, width) as the settings give it.
pub(crate) fn bearing_od_width(c: &Clearance) -> Option<(f64, f64)> {
    c.bearing.as_deref().and_then(bearing).map(|(_, od, w)| (od, w)).or(c.bearing_size.map(|b| (b.od, b.width)))
}

fn num(ui: &mut Ui, label: &str, tip: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, unit: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label).on_hover_text(tip);
        ui.add(egui::DragValue::new(v).range(range).speed(0.05).max_decimals(3).suffix(unit)).on_hover_text(tip);
    });
}

/// The section. `names` are the set's bodies in order (seat members are 1-based into them);
/// `warnings` and `facts` are the plan's, of which this shows the clearance ones.
pub(crate) fn section(ui: &mut Ui, s: &mut Spec, advanced: bool, names: &[String], warnings: &[String], facts: &[solidcraft_gear::Fact]) {
    ui.add_space(6.0);
    let mut on = s.clearance_body.is_some();
    ui.horizontal_wrapped(|ui| {
        crate::gear_icons::mark(ui, crate::gear_icons::HOUSING_CUTOUT);
        ui.label(RichText::new("Housing clearance").strong());
        if ui.checkbox(&mut on, "Make a clearance body").on_hover_text(SWITCH_TIP).changed() {
            s.clearance_body = on.then(Clearance::default);
        }
    });
    let kind = s.kind;
    let face = s.face_width;
    let Some(c) = s.clearance_body.as_mut() else { return };
    bearing_picker(ui, c, advanced);
    seats_ui(ui, c, names, face);
    if advanced {
        num(ui, "Radial clearance", "Gap around every tip envelope, shaft and chain or belt run. For FDM housings 0.3–0.6 mm is typical.", &mut c.radial, 0.0..=50.0, " mm");
        num(ui, "Axial clearance", "Gap past each gear's faces along its axis.", &mut c.axial, 0.0..=50.0, " mm");
        num(
            ui,
            "Shaft runs on",
            "How far each shaft's clearance runs past its gear on each side, enough to pass through a housing wall. 0 means twice the face width. Integral shafts (Shafts section) are taken as built. A shaft's diameter is the bore's (across the corners for hex, square, key or flat). With no bore it is the bearing's bore. With neither, there is no shaft clearance.",
            &mut c.shaft_extension,
            0.0..=10000.0,
            " mm",
        );
        if matches!(kind, Kind::Internal | Kind::Planetary | Kind::CycloidalDrive) {
            ui.checkbox(&mut c.ring_as_housing, "Ring is part of the housing").on_hover_text(
                "Internal ring gears (planetary ring, ring gear, cycloidal drive pin ring) are left out of the clearance because the housing carries them. Switch off to clear the ring's outer rim as a separate part.",
            );
        }
        if c.bearing.is_some() || c.bearing_size.is_some() {
            num(ui, "Seat fit", "How much larger than the bearing's OD each seat is, on the diameter. Use 0.1–0.2 for a printed slip fit and 0 or negative for a press fit.", &mut c.bearing_fit, -1.0..=5.0, " mm");
        }
        if kind == Kind::Rack {
            num(ui, "Rack travel", "How far the rack slides in all. Its slot is the rack's box made this much longer, half each way. 0: the rack's length.", &mut c.travel, 0.0..=100000.0, " mm");
        }
    }
    for w in warnings.iter().filter(|w| w.starts_with("clearance")) {
        ui.note(w, ui.visuals().warn_fg_color);
    }
    let ours: Vec<_> = facts.iter().filter(|f| f.label.starts_with("clearance")).collect();
    if !ours.is_empty() {
        egui::CollapsingHeader::new(RichText::new("Clearance numbers").small()).id_salt("gear_clearance_facts").show(ui, |ui| {
            egui::Grid::new("gear_clearance_facts_grid").num_columns(2).striped(true).show(ui, |ui| {
                for f in ours {
                    ui.label(RichText::new(&f.label).small());
                    ui.label(RichText::new(crate::gear_window::fact_value(f)).small());
                    ui.end_row();
                }
            });
        });
    }
}

/// One bearing for every seat: the library grouped by series, or a custom size (Advanced).
fn bearing_picker(ui: &mut Ui, c: &mut Clearance, advanced: bool) {
    let label = |name: &str| BEARINGS.iter().find(|b| b.0 == name).map_or(name.to_string(), |(n, bore, od, w)| format!("{n} — {bore} × {od} × {w}"));
    let current = match (&c.bearing, &c.bearing_size) {
        (Some(n), _) => label(n),
        (None, Some(_)) => "custom…".to_string(),
        (None, None) => "none".to_string(),
    };
    ui.horizontal_wrapped(|ui| {
        let tip = "One bearing size for every seat (bore × OD × width, mm).";
        ui.label("Bearing").on_hover_text(tip);
        egui::ComboBox::from_id_salt("gear_clearance_bearing").width(170.0).selected_text(current).show_ui(ui, |ui| {
            if ui.selectable_label(c.bearing.is_none() && c.bearing_size.is_none(), "none").clicked() {
                c.bearing = None;
                c.bearing_size = None;
            }
            let mut series = "";
            for (name, ..) in BEARINGS {
                let sr = bearing_series(name);
                if sr != series {
                    series = sr;
                    ui.label(RichText::new(sr).small().weak());
                }
                if ui.selectable_label(c.bearing.as_deref() == Some(*name), label(name)).clicked() {
                    c.bearing = Some((*name).to_string());
                    c.bearing_size = None;
                }
            }
            if advanced && ui.selectable_label(c.bearing.is_none() && c.bearing_size.is_some(), "custom…").clicked() {
                c.bearing = None;
                c.bearing_size = Some(c.bearing_size.unwrap_or(BearingSize { bore: 8.0, od: 22.0, width: 7.0 }));
            }
        });
    });
    if let (None, Some(b)) = (&c.bearing, c.bearing_size.as_mut()) {
        num(ui, "  Bearing OD", "Custom bearing outside diameter.", &mut b.od, 0.1..=2000.0, " mm");
        num(ui, "  Bearing width", "Custom bearing width.", &mut b.width, 0.1..=1000.0, " mm");
        let hi = (b.od - 0.01).max(0.0);
        num(ui, "  Bearing bore", "Custom bearing bore; used to check it against the shaft (0 skips the check).", &mut b.bore, 0.0..=hi, " mm");
    }
}

/// The seats: one row each (member, along, delete), then "Add seat". Also drawn as dots on the
/// shafts in the drawing, where their "along" can be typed too.
fn seats_ui(ui: &mut Ui, c: &mut Clearance, names: &[String], face: f64) {
    if c.bearing.is_none() && c.bearing_size.is_none() {
        return;
    }
    let width = bearing_od_width(c).map_or(5.0, |(_, w)| w);
    let name = |m: u32| names.get(m as usize - 1).cloned().unwrap_or_else(|| format!("member {m}"));
    let mut remove = None;
    for (i, seat) in c.seats.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            crate::gear_icons::mark(ui, crate::gear_icons::BEARING_SEAT);
            match seat {
                Seat::Along { member, along } => {
                    let tip = "Which member's shaft: the set's bodies in order (gear 1 first; planetary: sun, planets, ring; worm: wheel, then worm).";
                    egui::ComboBox::from_id_salt(("gear_seat_member", i)).width(110.0).selected_text(name(*member)).show_ui(ui, |ui| {
                        for m in 1..=names.len().max(1) as u32 {
                            ui.selectable_value(member, m, name(m));
                        }
                    })
                    .response
                    .on_hover_text(tip);
                    ui.add(egui::DragValue::new(along).range(-100000.0..=100000.0).speed(0.1).max_decimals(3).prefix("along ").suffix(" mm"))
                        .on_hover_text("Signed distance along the member's axis from the middle of its teeth. Negative values are behind the gear (the back face side).");
                }
                Seat::Point(p) => {
                    ui.label(RichText::new(format!("point ({:.1}, {:.1}, {:.1})", p[0], p[1], p[2])).small())
                        .on_hover_text("A picked point, moved onto the nearest member's axis.");
                }
            }
            if ui.small_button("✕").on_hover_text("Remove this seat").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        c.seats.remove(i);
    }
    ui.horizontal_wrapped(|ui| {
        if c.seats.len() < 64 && ui.small_button("Add seat").on_hover_text("A bearing seat on gear 1's shaft just behind it; each seat is a cylinder of the bearing's OD + fit and its width").clicked() {
            c.seats.push(Seat::Along { member: 1, along: -(face / 2.0 + width) });
        }
        if c.seats.is_empty() {
            ui.label(RichText::new("No seats yet: the bearing is not used.").small().weak());
        }
    });
}
