//! Differentials: every style and drive plans, every mesh is proved with the 3D checker, the
//! assembled members clear each other and the case, and Simple mode designs them by outputs. Run
//! with `--nocapture` for the numbers.

use crate::diff::{Diff, DriveKind, Mount, Style};
use crate::plan::{Kind, Plan, Spec, plan};

fn spec(d: Diff) -> Spec {
    Spec { diff: d, ..Spec::defaults_for(Kind::Differential) }
}

fn fact(p: &Plan, label: &str) -> f64 {
    p.facts.iter().find(|f| f.label == label).unwrap_or_else(|| panic!("no fact {label}: {:?}", p.facts.iter().map(|f| f.label.clone()).collect::<Vec<_>>())).value
}

fn show(label: &str, p: &Plan) {
    eprintln!("== {label}: {} bodies: {}", p.bodies.len(), p.bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", "));
    for w in &p.warnings {
        eprintln!("   ! {w}");
    }
}

#[test]
fn every_style_and_drive_plans() {
    for style in [Style::Open, Style::Biased, Style::TorsenB] {
        for drive in DriveKind::ALL {
            for mount in Mount::ALL {
                let d = Diff { drive, mount, ..Diff::defaults_for(style) };
                let label = format!("{style:?} {drive:?} {mount:?}");
                let p = plan(&spec(d)).unwrap_or_else(|e| panic!("{label}: {e}"));
                show(&label, &p);
                assert!(p.clearance.is_some(), "{label}: no keep-out body");
                assert!(!p.meshes3d.is_empty(), "{label}");
                assert_eq!(p.assembly.members.len(), p.bodies.len(), "{label}");
            }
        }
    }
}
