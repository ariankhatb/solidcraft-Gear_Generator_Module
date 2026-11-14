//! What to hand on from a gear design: a design sheet (Markdown: the inputs, every number of
//! the plan, the mesh check, warnings and parts) and a gear's transverse profile as a DXF for
//! laser cutting, wire EDM or CNC.

use std::fmt::Write;

use crate::geom::{Outline, Region, Seg, V2};
use crate::hub::{Bore, BoreKind};
use crate::involute::BasicRack;
use crate::mesh2d::{check_mesh, polygon};
use crate::plan::{Axes, Kind, Plan, Spec, Unit};

/// A number as a sheet shows it: four decimals at most, no trailing zeros.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "–".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn bore_text(b: &Bore) -> Option<String> {
    let d = num(b.diameter);
    Some(match b.kind {
        BoreKind::None => return None,
        BoreKind::Keyway => format!("Ø{d} mm with keyway"),
        BoreKind::DFlat => format!("Ø{d} mm D flat"),
        BoreKind::Hex => format!("hexagon {d} mm"),
        BoreKind::Square => format!("square {d} mm"),
        _ => format!("Ø{d} mm"),
    })
}

fn rack_text(r: &BasicRack) -> String {
    let coefficients = format!("addendum {} × m, dedendum {} × m, root radius {} × m", num(r.addendum), num(r.dedendum), num(r.root_radius));
    match BasicRack::STANDARDS.iter().find(|(_, s)| s == r) {
        Some((name, _)) => format!("{name} ({coefficients})"),
        None => coefficients,
    }
}

/// The inputs that matter for the spec's kind, as (name, value) rows.
fn inputs(s: &Spec) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut row = |k: &str, v: String| rows.push((k.to_string(), v));
    let k = s.kind;
    row("Type", k.label().to_string());
    if k == Kind::CycloidalDrive {
        row("Ring pins", format!("{} × Ø{} mm on a Ø{} mm circle", s.pins, num(s.pin_diameter), num(s.pin_circle)));
        row("Eccentricity", format!("{} mm", num(s.eccentricity)));
    } else {
        row("Module", format!("{} mm", num(s.module)));
        match k {
            Kind::Rack => row("Teeth", s.rack_teeth.to_string()),
            Kind::Planetary => {
                row("Sun teeth", s.teeth.to_string());
                row("Planet teeth", s.teeth2.to_string());
                row("Ring teeth", if s.teeth3 > 0 { s.teeth3.to_string() } else { format!("{} (sun + 2 × planet)", s.teeth + 2 * s.teeth2) });
                row("Planets", s.planets.to_string());
            }
            Kind::Worm => {
                row("Wheel teeth", s.teeth.to_string());
                row("Worm starts", s.starts.to_string());
            }
            _ if k.is_set() && !(k == Kind::Cycloidal && s.teeth2 == 0) => {
                row("Gear 1 teeth", s.teeth.to_string());
                row("Gear 2 teeth", s.teeth2.to_string());
            }
            _ => row("Teeth", s.teeth.to_string()),
        }
    }
    if k.involute() {
        row("Pressure angle", format!("{}°", num(s.pressure_angle)));
        if s.helix != 0.0 && !matches!(k, Kind::Spur | Kind::Bevel | Kind::SpiralBevel | Kind::Hypoid | Kind::Worm) {
            row("Helix angle", format!("{}°", num(s.helix)));
        }
        if matches!(k, Kind::SpiralBevel | Kind::Hypoid) {
            row("Spiral angle", format!("{}°", num(s.spiral_angle)));
        }
        if matches!(k, Kind::Bevel | Kind::SpiralBevel | Kind::Hypoid | Kind::CrossedHelical) {
            row("Shaft angle", format!("{}°", num(s.shaft_angle)));
        }
        if s.shift != 0.0 {
            row("Profile shift", num(s.shift));
        }
        if s.shift2 != 0.0 && matches!(k, Kind::Pair | Kind::Planetary) {
            row("Profile shift (gear 2)", num(s.shift2));
        }
        row("Basic rack", rack_text(&s.rack));
    }
    if s.backlash != 0.0 {
        row("Backlash", format!("{} mm", num(s.backlash)));
    }
    row("Face width", format!("{} mm", num(s.face_width)));
    if k == Kind::Pair && s.center_distance > 0.0 {
        row("Centre distance (asked for)", format!("{} mm", num(s.center_distance)));
    }
    if let Some(b) = bore_text(&s.bore) {
        row("Bore", b);
    }
    if k.is_set()
        && let Some(b) = bore_text(&s.bore2)
    {
        row("Bore (gear 2)", b);
    }
    if s.hub_diameter > 0.0 && s.hub_length > 0.0 {
        row("Hub", format!("Ø{} × {} mm", num(s.hub_diameter), num(s.hub_length)));
    }
    if s.coupon && crate::coupon::applies(s) {
        row("Test coupon", format!("{} teeth a piece", s.coupon_teeth));
    }
    let l = &s.load;
    if l.rated() {
        row("Load", if l.speed > 0.0 { format!("{} N·m at {} rpm ({} kW)", num(l.torque_nm()), num(l.speed), num(l.power_kw())) } else { format!("{} N·m", num(l.torque_nm())) });
        let m = |m: crate::strength::Material| m.data().label.to_string();
        row("Materials", if l.material == l.material2 { m(l.material) } else { format!("{} / {}", m(l.material), m(l.material2)) });
    }
    rows
}

/// Escapes what would break a Markdown table cell.
fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// A design sheet for a planned set, in Markdown: inputs, results (every fact of the plan),
/// the 2D mesh check where the set has one, warnings and the parts.
pub fn design_sheet(spec: &Spec, plan: &Plan) -> String {
    let mut o = String::new();
    let title = if spec.name.trim().is_empty() { spec.kind.label().to_string() } else { format!("{} ({})", spec.name.trim(), spec.kind.label().to_lowercase()) };
    let _ = writeln!(o, "# Gear design sheet: {title}\n");
    let _ = writeln!(o, "Lengths in millimetres, angles in degrees.\n");
    let _ = writeln!(o, "## Inputs\n\n| Parameter | Value |\n|---|---|");
    for (k, v) in inputs(spec) {
        let _ = writeln!(o, "| {} | {} |", cell(&k), cell(&v));
    }
    let _ = writeln!(o, "\n## Results\n\n| Quantity | Value | Unit |\n|---|---:|---|");
    for f in &plan.facts {
        let unit = match f.unit {
            Unit::Mm => "mm",
            Unit::Deg => "°",
            Unit::None => "",
        };
        let _ = writeln!(o, "| {} | {} | {unit} |", cell(&f.label), num(f.value));
    }
    let _ = writeln!(o, "\n## Mesh check\n");
    let checks = check_mesh(plan);
    if checks.is_empty() {
        let why = if spec.kind.axes() == Axes::Parallel || spec.kind.axes() == Axes::Special { "a single member: nothing to check" } else { "not checked in 2D (the axes are not parallel)" };
        let _ = writeln!(o, "- {why}");
    }
    for c in &checks {
        let _ = writeln!(o, "- {}", c.message);
    }
    let _ = writeln!(o, "\n## Warnings\n");
    if plan.warnings.is_empty() {
        let _ = writeln!(o, "- none");
    }
    for w in &plan.warnings {
        let _ = writeln!(o, "- {w}");
    }
    let _ = writeln!(o, "\n## Parts\n");
    for b in &plan.bodies {
        let _ = writeln!(o, "- {}", b.name);
    }
    o
}

// ---------------------------------------------------------------------------------------------
// DXF.

/// How close the DXF's polylines keep to a spline, per millimetre of module (the request is
/// 2 µm per mm of module; chords are fitted to half of that).
pub const DXF_TOLERANCE: f64 = 2e-3;

fn pair(o: &mut String, code: u32, v: f64) {
    // Trimmed to nine decimals: a nanometre, with no exponent for a reader to trip on.
    let s = format!("{v:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    let _ = writeln!(o, "{code:>3}\n{}", if s == "-0" || s.is_empty() { "0" } else { s });
}

fn head(o: &mut String, kind: &str, layer: &str) {
    let _ = writeln!(o, "  0\n{kind}\n  8\n{layer}");
}

fn point(o: &mut String, base: u32, p: V2) {
    pair(o, base, p.x);
    pair(o, base + 10, p.y);
    pair(o, base + 20, 0.0);
}

fn entities(o: &mut String, outline: &Outline, tol: f64, layer: &str, lo: &mut V2, hi: &mut V2) {
    let mut cover = |p: V2| {
        *lo = V2::new(lo.x.min(p.x), lo.y.min(p.y));
        *hi = V2::new(hi.x.max(p.x), hi.y.max(p.y));
    };
    for s in &outline.segs {
        let mut pts = Vec::new();
        s.sample(tol, &mut pts);
        pts.iter().for_each(|p| cover(*p));
        match s {
            Seg::Line(a, b) => {
                head(o, "LINE", layer);
                point(o, 10, *a);
                point(o, 11, *b);
            }
            Seg::Arc { c, a, b, ccw } => {
                // A DXF arc runs counter-clockwise from its start angle to its end angle.
                let (from, to) = if *ccw { (*a, *b) } else { (*b, *a) };
                head(o, "ARC", layer);
                point(o, 10, *c);
                pair(o, 40, c.dist(*a));
                pair(o, 50, (from - *c).angle().to_degrees().rem_euclid(360.0));
                pair(o, 51, (to - *c).angle().to_degrees().rem_euclid(360.0));
            }
            Seg::Circle(c, r) => {
                head(o, "CIRCLE", layer);
                point(o, 10, *c);
                pair(o, 40, *r);
            }
            Seg::Spline(_) => {
                // The curve the sketch fits through the points, as chords within the tolerance.
                let dense = polygon(&Outline { segs: vec![s.clone()] }, tol);
                dense.iter().for_each(|p| cover(*p));
                head(o, "POLYLINE", layer);
                let _ = writeln!(o, " 66\n1\n 70\n0");
                point(o, 10, V2::ZERO);
                for p in &dense {
                    head(o, "VERTEX", layer);
                    point(o, 10, *p);
                }
                head(o, "SEQEND", layer);
            }
        }
    }
}

fn dxf(loops: &[(&Outline, &str)], module: f64) -> String {
    let tol = 0.5 * DXF_TOLERANCE * if module.is_finite() && module > 0.0 { module } else { 1.0 };
    let mut body = String::new();
    let (mut lo, mut hi) = (V2::new(f64::INFINITY, f64::INFINITY), V2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for (outline, layer) in loops {
        entities(&mut body, outline, tol, layer, &mut lo, &mut hi);
    }
    if !lo.is_finite() || !hi.is_finite() {
        (lo, hi) = (V2::ZERO, V2::ZERO);
    }
    let mut o = String::new();
    // R12 (AC1009): the oldest form everything reads. Drawing units are millimetres.
    let _ = writeln!(o, "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1009\n  9\n$EXTMIN");
    point(&mut o, 10, lo);
    let _ = writeln!(o, "  9\n$EXTMAX");
    point(&mut o, 10, hi);
    let _ = writeln!(o, "  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES");
    o.push_str(&body);
    let _ = writeln!(o, "  0\nENDSEC\n  0\nEOF");
    o
}

/// A gear's transverse profile as an ASCII DXF (R12, millimetres): lines and arcs as LINE and
/// ARC, the flanks and fillets (splines) as POLYLINEs dense enough to keep within
/// [`DXF_TOLERANCE`] × `module` of the curve. On layer `PROFILE`, in the outline's own frame
/// (the gear's centre at the origin).
pub fn profile_dxf(outline: &Outline, module: f64) -> String {
    dxf(&[(outline, "PROFILE")], module)
}

/// A whole region (a gear with its bore, a ring gear's rim and teeth) as [`profile_dxf`] makes
/// one outline: outer outlines on layer `PROFILE`, holes on layer `HOLES`.
pub fn region_dxf(region: &Region, module: f64) -> String {
    let loops: Vec<(&Outline, &str)> = region.outer.iter().map(|o| (o, "PROFILE")).chain(region.holes.iter().map(|o| (o, "HOLES"))).collect();
    dxf(&loops, module)
}

#[cfg(test)]
#[path = "sheet_tests.rs"]
mod tests;
