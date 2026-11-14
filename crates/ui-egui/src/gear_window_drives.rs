//! The Gear Generator's rows for chain sprockets, timing pulleys, involute splines and face gears.

use egui::Ui;
use solidcraft_gear::plan::{Kind, Spec};
use solidcraft_gear::pulley::{BELTS, belt_by_id};
use solidcraft_gear::spline::SplineStandard;
use solidcraft_gear::sprocket::{CHAINS, CUSTOM, chain_by_id};

use super::{count, num, row};

/// The teeth rows of a sprocket or pulley (they have no module); false for other types.
pub(super) fn wrapped_rows(ui: &mut Ui, s: &mut Spec) -> bool {
    match s.kind {
        Kind::Sprocket => {
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
            count(ui, "Teeth", "17 or more runs smoothly", &mut s.teeth, 6..=250);
            count(ui, "Driven teeth", "A second sprocket joined by the chain; 0: none", &mut s.teeth2, 0..=250);
            count(ui, "Strands", "1 simplex, 2 duplex, 3 triplex", &mut s.strands, 1..=4);
            row(ui, "Side chamfer", "ISO 606 tooth side radius and chamfer, so the chain runs on", |ui| {
                ui.checkbox(&mut s.side_chamfer, "");
            });
        }
        Kind::Pulley => {
            let current = belt_by_id(s.belt).map(|b| b.label).unwrap_or("?");
            row(ui, "Belt", "Groove forms approximated from published belt dimensions", |ui| {
                egui::ComboBox::from_id_salt("gear_belt").selected_text(current).show_ui(ui, |ui| {
                    for b in BELTS {
                        if ui.selectable_value(&mut s.belt, b.id, b.label).changed()
                            && let Some(w) = b.widths.first()
                        {
                            s.face_width = *w;
                        }
                    }
                });
            });
            count(ui, "Teeth", "", &mut s.teeth, 6..=400);
            count(ui, "Second pulley teeth", "A second pulley joined by the belt; 0: none", &mut s.teeth2, 0..=400);
            row(ui, "Flanges", "Keep the belt on", |ui| {
                ui.checkbox(&mut s.flanges, "");
            });
        }
        _ => return false,
    }
    if s.teeth2 > 0 {
        let tip = if s.kind == Kind::Sprocket {
            "0: 40 pitches; rounded to an even number of links"
        } else {
            "0: three pitch diameters; rounded to a whole belt"
        };
        num(ui, "Centre distance", tip, &mut s.center_distance, 0.0..=1e5, 0.5, " mm");
    }
    true
}

/// Rows only splines and face gears have, after the teeth.
pub(super) fn extra_rows(ui: &mut Ui, s: &mut Spec) {
    match s.kind {
        Kind::Spline => {
            row(ui, "Standard", "", |ui| {
                egui::ComboBox::from_id_salt("gear_spline_std").selected_text(s.spline_standard.label()).show_ui(ui, |ui| {
                    for (k, _, label) in SplineStandard::ALL {
                        ui.selectable_value(&mut s.spline_standard, k, label);
                    }
                });
            });
            if s.spline_standard == SplineStandard::Din5480 {
                num(
                    ui,
                    "Reference Ø d_B",
                    "The nominal diameter; sets the profile shift. 0: module × (teeth + 1.1)",
                    &mut s.spline_diameter,
                    0.0..=5000.0,
                    0.1,
                    " mm",
                );
            }
            row(ui, "Fillet root", "A full round root instead of a flat one", |ui| {
                ui.checkbox(&mut s.fillet_root, "");
            });
        }
        Kind::Elliptical => {
            num(
                ui,
                "Eccentricity",
                "Of gear 1's ellipse; 0 is a circle. The mate is its exact conjugate",
                &mut s.ellipse_eccentricity,
                0.0..=0.7,
                0.005,
                "",
            );
        }
        Kind::FaceGear => {
            num(ui, "Teeth inner Ø", "Where the teeth start; 0: the undercut limit", &mut s.inner_diameter, 0.0..=1e5, 0.5, " mm");
        }
        _ => {}
    }
}
