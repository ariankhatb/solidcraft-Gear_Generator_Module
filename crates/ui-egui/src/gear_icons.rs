//! The Gear Generator's icon ids: one per kind of set for the window's type picker, and the
//! window's own buttons. The masters are in `crates/look/icons/design/svg` (`model/gear_*.svg`,
//! `flat/gear_*.svg`; `gear_view_3d` is an alias of `gear_pair`); draw one with
//! [`crate::ui_kit::icons::paint_icon`] in a rect, or pass the id to a kit control that takes an
//! icon (`ui_kit::icon_label_button`, `ui_kit::toggle_icon`, `ui_kit::segmented_icons`,
//! `ui_kit::checkbox_icon`).

use solidcraft_gear::plan::Kind;

/// The Gear Generator command (`gear.create`, `gear.update`) and its window's title.
pub const GENERATOR: &str = "gear_generator";
/// Simple mode: the set drawn in one sketch, a short form.
pub const MODE_SIMPLE: &str = "gear_mode_simple";
/// Advanced mode: every setting.
pub const MODE_ADVANCED: &str = "gear_mode_advanced";
/// Two views side by side ("Drawing + 3D", "3D + 2D").
pub const VIEW_SPLIT: &str = "gear_view_split";
/// The Simple-mode sketch ("Drawing"): pitch circles and their centres.
pub const VIEW_DRAWING: &str = "gear_view_drawing";
/// The set in 3D ("3D").
pub const VIEW_3D: &str = "gear_view_3d";
/// The teeth meshing in 2D ("2D mesh").
pub const VIEW_MESH: &str = "gear_view_mesh";
/// The housing clearance body ("Make a clearance body").
pub const HOUSING_CUTOUT: &str = "gear_housing_cutout";
/// A bearing seat in the clearance body.
pub const BEARING_SEAT: &str = "gear_bearing_seat";

/// The type picker's icon for a kind of set.
pub fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Spur => "gear_spur",
        Kind::Helical => "gear_helical",
        Kind::Herringbone => "gear_herringbone",
        Kind::Internal => "gear_internal",
        Kind::Rack => "gear_rack",
        Kind::Pair => "gear_pair",
        Kind::Planetary => "gear_planetary",
        Kind::Bevel => "gear_bevel",
        Kind::SpiralBevel => "gear_spiral_bevel",
        Kind::Hypoid => "gear_hypoid",
        Kind::CrossedHelical => "gear_crossed_helical",
        Kind::Worm => "gear_worm",
        Kind::Cycloidal => "gear_cycloidal",
        Kind::CycloidalDrive => "gear_cycloidal_drive",
        Kind::Sprocket => "gear_sprocket",
        Kind::Pulley => "gear_pulley",
        Kind::Spline => "gear_spline",
        Kind::FaceGear => "gear_face",
        Kind::Elliptical => "gear_elliptical",
        // Kind::BevelPlanetary => "gear_bevel_planetary", once that kind is merged.
    }
}

/// Every id above, the picker's and the buttons', including the one drawn for a kind still on its way.
pub const ALL: &[&str] = &[
    GENERATOR,
    MODE_SIMPLE,
    MODE_ADVANCED,
    VIEW_SPLIT,
    VIEW_DRAWING,
    VIEW_3D,
    VIEW_MESH,
    HOUSING_CUTOUT,
    BEARING_SEAT,
    "gear_bevel_planetary",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gear_icon_has_artwork_and_each_kind_its_own() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in Kind::ALL {
            let id = kind_icon(kind);
            assert!(!crate::icon_svg::resolve(id).placeholder, "{kind:?}: {id} has no master");
            assert!(seen.insert(id), "{kind:?} shares {id}");
        }
        for id in ALL {
            assert!(!crate::icon_svg::resolve(id).placeholder, "{id} has no master");
        }
        // The generator is not the coupling's icon, and the window's views say what they show.
        assert_ne!(crate::icon_svg::resolve(GENERATOR).master, crate::icon_svg::resolve("gear").master);
        assert_eq!(crate::icon_svg::resolve(VIEW_3D).master, crate::icon_svg::resolve(kind_icon(Kind::Pair)).master);
        assert_eq!(solidcraft_engine::find_command("gear.create").map(|c| c.icon), Some(GENERATOR));
    }
}
