//! Gear teeth drawn from their formulas (`gear.create` with `tooth_curves: "equations"`): each
//! toothed outline of a prism step whose tooth data the plan keeps ([`Plan::teeth`]) becomes one
//! tooth of equation curves (involute flanks, trochoid root fillets) with its tip and root arcs,
//! repeated by a circular pattern (a rack: lines and a rectangular pattern). With `parametric`,
//! the formulas, the pattern count and the tooth extrusion name design parameters
//! (`<prefix>_module`, `<prefix>_teeth`…), so editing one regenerates the teeth.
//!
//! What a parameter edit moves: the teeth (shape and count) and their extrusion length. What it
//! does not: where each member sits (a pair's centre distance, the planets' places), hubs,
//! bores and the like, and a herringbone's upper half starts where it did; `gear.update` reads
//! the parameters back and regenerates all of it. An undercut gear's fillet/flank crossing is
//! numeric, so it follows edits only approximately.
//!
//! `gear.update` in place redraws an equation sketch from scratch in the same sketch (same
//! sketch and feature ids; its curve ids come out the same when the tooth is drawn the same way).

use serde_json::{Value, json};
use solidcraft_gear::equations::{Edge, End, Quantity, Repeat, TeethSource, ToothCell, cell_for, render};
use solidcraft_gear::plan::{Kind, Plan, Spec, Step};
use solidcraft_gear::{Outline, Region};
use solidcraft_sketch::{Analysis, Sketch};

use super::super::sketch::{Left, edit_solved};
use super::super::sketch_import::add_geometry;
use super::SketchDef;
use crate::params::bad;
use crate::schema::gear::{GearParams, ToothCurves};
use crate::{EngineError, Result, Session};

/// How a set's teeth are drawn.
#[derive(Clone, Debug, Default)]
pub(super) struct Mode {
    /// Teeth as formulas.
    pub on: bool,
    pub sources: Vec<TeethSource>,
    /// The design parameters' prefix (parametric teeth).
    pub prefix: Option<String>,
}

/// One sketch drawn by [`draw`]: its toothed loops (index into the sketch's loops) as cells, and
/// a sweep path's length as an expression.
#[derive(Clone, Debug)]
pub(super) struct EqSketch {
    pub cells: Vec<(usize, ToothCell)>,
    pub prefix: Option<String>,
    pub path: Option<String>,
}

/// The types whose teeth can be formulas.
fn in_scope(k: Kind) -> bool {
    matches!(k, Kind::Spur | Kind::Helical | Kind::Herringbone | Kind::Internal | Kind::Pair | Kind::Planetary | Kind::Rack)
}

/// A name usable as a parameter prefix: letters, digits and `_`, starting with a letter.
fn sanitize(name: &str) -> String {
    let mut out: String = name.trim().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) { out } else { format!("Gear_{out}").trim_end_matches('_').to_string() }
}

fn valid_prefix(p: &str) -> bool {
    !p.is_empty() && p.len() <= 40 && p.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A prefix no parameter of the design uses yet.
fn unique_prefix(s: &Session, base: &str) -> String {
    let taken = |p: &str| s.doc.params.iter().any(|q| q.name.starts_with(&format!("{p}_")));
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}{n}")).find(|p| !taken(p)).unwrap_or_else(|| base.to_string())
}

/// The prism outlines of the plan's bodies.
fn prism_outlines(plan: &Plan) -> Vec<&Outline> {
    fn walk<'a>(step: &'a Step, out: &mut Vec<&'a Outline>) {
        match step {
            Step::Prism { region, .. } => out.extend(region.outer.iter().chain(&region.holes)),
            Step::Placed { step, .. } => walk(step, out),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for b in &plan.bodies {
        for st in &b.steps {
            walk(st, &mut out);
        }
    }
    out
}

/// Why a member's teeth stay fitted splines: what about it has no formulas here.
fn fitted_why(spec: &Spec, plan: &Plan, src: &TeethSource) -> String {
    let who = &src.who;
    if spec.kind == Kind::Rack && spec.helix != 0.0 {
        return format!(
            "{who}: its teeth stay fitted splines (a helical rack's teeth slant across the face, so it is lofted between its end sections rather than extruded, and only extruded teeth take formulas)"
        );
    }
    let mut why = Vec::new();
    if spec.tip_relief > 0.0 {
        why.push("tip relief");
    }
    if spec.tip_round > 0.0 {
        why.push("tip rounding");
    }
    if spec.crowning > 0.0 && spec.helix == 0.0 {
        why.push("crowning");
    }
    if plan.warnings.iter().any(|w| w.starts_with(&format!("{who}: the teeth come to a point"))) {
        why.push("a tip lowered to keep a land");
    }
    let teeth = match src.member {
        1 => spec.teeth,
        2 => spec.teeth2,
        _ => if spec.teeth3 > 0 { spec.teeth3 } else { spec.teeth + 2 * spec.teeth2 },
    };
    if teeth > 500 {
        why.push("over 500 teeth");
    }
    if why.is_empty() {
        return format!(
            "{who}: its teeth stay fitted splines (tip relief, tip rounding, crowning, a tip lowered to keep a land or over 500 teeth have no formulas here)"
        );
    }
    format!("{who}: its teeth stay fitted splines ({} {} no formulas here)", why.join(", "), if why.len() == 1 { "has" } else { "have" })
}

impl Mode {
    /// The mode a request asks for; `group_name` names the parameters when no prefix is given (a
    /// new one, unused by the design's parameters, when `fresh`). Also warnings for members
    /// whose teeth stay fitted.
    pub fn of(s: &Session, cmd: &str, request: &Value, spec: &Spec, plan: &Plan, group_name: &str, fresh: bool) -> Result<(Mode, Vec<String>)> {
        let p: GearParams = serde_json::from_value(request.clone()).map_err(|e| bad(cmd, e.to_string()))?;
        let mut warnings = Vec::new();
        let asked = p.tooth_curves == Some(ToothCurves::Equations);
        if !asked {
            if p.parametric == Some(true) {
                warnings.push("parametric teeth need tooth_curves: equations; the teeth are fitted splines".into());
            }
            return Ok((Mode::default(), warnings));
        }
        if !in_scope(spec.kind) || spec.coupon {
            warnings.push(format!("{}: tooth curves from equations are made for spur, helical, herringbone, internal, rack, pair and planetary sets; the teeth are fitted splines", spec.kind.label()));
            return Ok((Mode::default(), warnings));
        }
        let prefix = if p.parametric == Some(true) {
            let given = p.parameter_prefix.as_deref().map(str::trim).filter(|x| !x.is_empty());
            if let Some(g) = given
                && !valid_prefix(g)
            {
                return Err(bad(cmd, "`parameter_prefix` must start with a letter and hold only letters, digits and _ (40 at most)"));
            }
            Some(match given {
                Some(g) => g.to_string(),
                None if fresh => unique_prefix(s, &sanitize(group_name)),
                None => sanitize(group_name),
            })
        } else {
            None
        };
        let mode = Mode { on: true, sources: plan.teeth.clone(), prefix };
        let outlines = prism_outlines(plan);
        for src in &plan.teeth {
            let found = outlines.iter().any(|o| cell_for(std::slice::from_ref(src), o).is_some());
            if !found {
                warnings.push(fitted_why(spec, plan, src));
            }
        }
        Ok((mode, warnings))
    }

    /// The formulas for a prism's region, if any of its outlines are teeth.
    pub fn sketch_for(&self, region: &Region) -> Option<EqSketch> {
        if !self.on {
            return None;
        }
        let cells: Vec<(usize, ToothCell)> = region.outer.iter().chain(&region.holes).enumerate().filter_map(|(i, o)| cell_for(&self.sources, o).map(|c| (i, c))).collect();
        (!cells.is_empty()).then(|| EqSketch { cells, prefix: self.prefix.clone(), path: None })
    }

    /// A parametric prism's extrusion distance and twist as expressions (none where the numbers
    /// aren't the face width, half of it, or the helix's twist over them).
    pub fn prism_exprs(&self, eq: &EqSketch, height: f64, twist: f64) -> (Option<String>, Option<String>) {
        let (Some(prefix), Some((_, cell))) = (&self.prefix, eq.cells.first()) else { return (None, None) };
        let name = |k: &str| format!("{prefix}_{k}");
        let Some(face) = cell.vars.value(&cell.face_key) else { return (None, None) };
        let tol = 1e-9 * face.max(1.0);
        let h = if (height - face).abs() <= tol {
            Some(format!("{{{}}}", cell.face_key))
        } else if (height - face / 2.0).abs() <= tol {
            Some(format!("{{{}}} / 2", cell.face_key))
        } else {
            None
        };
        let distance = h.as_ref().map(|h| render(h, &name));
        let turn = match (&h, &cell.helix_key, &cell.radius_key) {
            (Some(h), Some(hk), Some(rk)) if twist.abs() > 1e-12 => {
                let (Some(beta), Some(r)) = (cell.vars.value(hk), cell.vars.value(rk)) else { return (distance, None) };
                let full = height * beta.tan() / r;
                let k = twist / full;
                if (k - 1.0).abs() < 1e-6 {
                    Some(render(&format!("{h} * tan({{{hk}}}) / {{{rk}}} * 1 rad"), &name))
                } else if (k + 1.0).abs() < 1e-6 {
                    Some(render(&format!("-({h}) * tan({{{hk}}}) / {{{rk}}} * 1 rad"), &name))
                } else {
                    None
                }
            }
            _ => None,
        };
        (distance, turn)
    }
}

fn unit_of(q: Quantity) -> &'static str {
    match q {
        Quantity::Length => "mm",
        Quantity::Angle => "deg",
        Quantity::Number => "",
    }
}

/// Makes or updates the design parameters the sketch's formulas name.
fn ensure_params(s: &mut Session, def: &SketchDef) -> Result<()> {
    let Some(eq) = &def.eq else { return Ok(()) };
    let Some(prefix) = &eq.prefix else { return Ok(()) };
    let name = |k: &str| format!("{prefix}_{k}");
    for (_, cell) in &eq.cells {
        for v in &cell.vars.list {
            let n = name(&v.key);
            let expr = if v.input { v.expr.clone() } else { render(&v.expr, &name) };
            let existing = s.doc.param(&n).map(|p| p.expr.clone());
            match existing {
                Some(e) if e == expr => {}
                Some(_) => {
                    s.execute("parameters.change", &json!({"name": n, "expression": expr}))?;
                }
                None => {
                    let comment = format!("Gear Generator: {}{}", v.comment, if v.input { "" } else { " (worked out)" });
                    s.execute("parameters.add", &json!({"name": n, "expression": expr, "unit": unit_of(v.quantity), "comment": comment}))?;
                }
            }
        }
    }
    Ok(())
}

/// The loops of `def` drawn as plain geometry (those that aren't teeth).
fn plain_loops(def: &SketchDef) -> Vec<&Vec<solidcraft_io::Geom2>> {
    let eq = def.eq.as_ref();
    if eq.is_some_and(|e| e.path.is_some()) {
        return Vec::new();
    }
    def.loops.iter().enumerate().filter(|(i, _)| !eq.is_some_and(|e| e.cells.iter().any(|(k, _)| k == i))).map(|(_, g)| g).collect()
}

fn id_of(r: &Value, key: &str, what: &str) -> Result<String> {
    r.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| EngineError::Other(format!("gear teeth: no {what} id")))
}

fn first_curve_of(r: &Value) -> Result<String> {
    r.get("curves").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str).map(str::to_string).ok_or_else(|| EngineError::Other("gear teeth: no curve drawn".into()))
}

/// Draws one cell in the sketch being edited: what it has made so far.
struct Drawer<'a> {
    cell: &'a ToothCell,
    prefix: Option<&'a str>,
    curves: Vec<String>,
    points: Vec<Option<String>>,
}

impl Drawer<'_> {
    fn text(&self, tpl: &str) -> String {
        match self.prefix {
            Some(p) => render(tpl, &|k| format!("{p}_{k}")),
            None => self.cell.vars.numeric(tpl),
        }
    }
    fn fail(&self, e: impl std::fmt::Display) -> EngineError {
        EngineError::Other(format!("gear teeth ({}): {e}", self.cell.who))
    }
    fn point(&mut self, s: &mut Session, i: usize) -> Result<String> {
        if let Some(Some(id)) = self.points.get(i) {
            return Ok(id.clone());
        }
        let at = self.cell.points.get(i).ok_or_else(|| self.fail("a missing point"))?;
        let id = match self.prefix {
            Some(_) => id_of(&s.execute("sketch.point", &json!({"point": [self.text(&at[0]), self.text(&at[1])]}))?, "point", "point")?,
            None => {
                let q = self.cell.point(i).map_err(|e| self.fail(e))?;
                let id = id_of(&s.execute("sketch.point", &json!({"point": [q.x, q.y]}))?, "point", "point")?;
                s.execute("sketch.constraint.fix", &json!({"entities": [id], "fixed": true}))?;
                id
            }
        };
        if let Some(slot) = self.points.get_mut(i) {
            *slot = Some(id.clone());
        }
        Ok(id)
    }
    fn end(&mut self, s: &mut Session, e: &End) -> Result<String> {
        match e {
            End::Start(i) => self.curves.get(*i).map(|c| format!("{c}.start")).ok_or_else(|| self.fail("a missing curve")),
            End::End(i) => self.curves.get(*i).map(|c| format!("{c}.end")).ok_or_else(|| self.fail("a missing curve")),
            End::Point(i) => self.point(s, *i),
        }
    }
    fn edge(&mut self, s: &mut Session, e: &Edge) -> Result<String> {
        let (a, b, arc) = match e {
            Edge::Arc(a, b) => (a, b, true),
            Edge::Line(a, b) => (a, b, false),
        };
        let (a, b) = (self.end(s, a)?, self.end(s, b)?);
        let r = if arc {
            s.execute("sketch.arc.center_point", &json!({"center": "origin", "start": a, "end": b}))?
        } else {
            s.execute("sketch.line", &json!({"points": [a, b]}))?
        };
        first_curve_of(&r)
    }
    /// A count or spacing: an expression of the parameters, or its number.
    fn amount(&self, tpl: &str, whole: bool) -> Result<Value> {
        Ok(match self.prefix {
            Some(_) => json!(self.text(tpl)),
            None => {
                let v = self.cell.vars.eval(tpl, None).map_err(|e| self.fail(e))?;
                if whole { json!(v.round()) } else { json!(v) }
            }
        })
    }
}

fn draw_cell(s: &mut Session, cell: &ToothCell, prefix: Option<&str>) -> Result<()> {
    let mut d = Drawer { cell, prefix, curves: Vec::new(), points: vec![None; cell.points.len()] };
    for c in &cell.curves {
        let r = s.execute("sketch.equation_curve", &json!({"x": d.text(&c.x), "y": d.text(&c.y), "start": d.text(&c.start), "end": d.text(&c.end), "tolerance": c.tolerance}))?;
        d.curves.push(id_of(&r, "curve", "equation curve")?);
    }
    let mut seeds = d.curves.clone();
    for e in &cell.edges {
        seeds.push(d.edge(s, e)?);
    }
    for (a, b) in &cell.joins {
        let (a, b) = (d.end(s, a)?, d.end(s, b)?);
        s.execute("sketch.constraint.coincident", &json!({"a": a, "b": b}))?;
    }
    match &cell.repeat {
        Repeat::Circular { count } => {
            s.execute("sketch.pattern.circular", &json!({"entities": seeds, "center": "origin", "count": d.amount(count, true)?}))?;
        }
        Repeat::Linear { count, spacing } => {
            s.execute("sketch.pattern.rectangular", &json!({"entities": seeds, "count": d.amount(count, true)?, "spacing": d.amount(spacing, false)?, "dir": [1.0, 0.0]}))?;
        }
    }
    for e in &cell.closing {
        d.edge(s, e)?;
    }
    Ok(())
}

/// Draws the formulas (or the parametric path) of `def` in the sketch being edited, `id`, after
/// its plain loops.
fn draw(s: &mut Session, id: u64, def: &SketchDef) -> Result<()> {
    let loops = plain_loops(def);
    if !loops.is_empty() {
        edit_solved(s, id, false, Analysis::Freedom, |sk, _| {
            for g in &loops {
                add_geometry(sk, g, &|q| q, 1.0, 1e-7)?;
            }
            Ok(((), Left::Unsolved))
        })?;
    }
    let Some(eq) = &def.eq else { return Ok(()) };
    if let Some(length) = &eq.path {
        s.execute("sketch.line", &json!({"points": [[0.0, 0.0], [length, 0.0]]}))?;
        return Ok(());
    }
    for (_, cell) in &eq.cells {
        draw_cell(s, cell, eq.prefix.as_deref())?;
    }
    Ok(())
}

/// A new sketch for `def` (see [`super::new_sketch`]), its teeth drawn from their formulas.
pub(super) fn new_sketch(s: &mut Session, def: &SketchDef) -> Result<u64> {
    ensure_params(s, def)?;
    let r = s.execute("sketch.create", &json!({"plane": {"origin": def.origin, "x_dir": def.x, "y_dir": def.y}, "name": def.name, "project_edges": false}))?;
    let id = r.get("sketch").and_then(Value::as_u64).ok_or_else(|| EngineError::Other("no sketch".into()))?;
    draw(s, id, def)?;
    s.execute("sketch.finish", &json!({}))?;
    Ok(id)
}

/// Draws `def` in sketch `id` in place of everything it holds (patterns, equations and their
/// relations too). Its ids start over, so a tooth drawn the same way gets the same curve ids.
pub(super) fn redraw(s: &mut Session, id: u64, def: &SketchDef) -> Result<()> {
    ensure_params(s, def)?;
    s.execute("sketch.edit", &json!({"sketch": id}))?;
    let cleared = edit_solved(s, id, false, Analysis::Freedom, |sk, _| {
        let view = sk.view.clone();
        *sk = Sketch::new();
        sk.view = view;
        Ok(((), Left::Unsolved))
    })
    .and_then(|_| draw(s, id, def));
    if let Err(e) = cleared {
        s.execute("sketch.cancel", &json!({})).ok();
        return Err(e);
    }
    s.execute("sketch.finish", &json!({}))?;
    Ok(())
}

/// Does sketch `id` hold anything only [`redraw`] clears (equation curves or patterns)?
pub(super) fn holds_formulas(s: &Session, id: u64) -> bool {
    s.doc.sketch(id).is_ok_and(|sk| !sk.equations.is_empty() || !sk.patterns.is_empty())
}

/// `gear.update` of a parametric set: the inputs the user changed through the design
/// parameters, laid over the request (those given to the update itself win).
pub(super) fn read_back(s: &Session, request: &mut serde_json::Map<String, Value>, stored: &Value, given: &[String]) {
    let parametric = request.get("parametric") == Some(&json!(true)) && request.get("tooth_curves") == Some(&json!("equations"));
    let Some(prefix) = request.get("parameter_prefix").and_then(Value::as_str).map(str::to_string) else { return };
    if !parametric {
        return;
    }
    let (vals, _) = s.doc.param_values();
    // (key, request field, angle in degrees)
    let fields = [
        ("module", "module", false),
        ("teeth", "teeth", false),
        ("teeth2", "teeth2", false),
        ("teeth3", "ring_teeth", false),
        ("pressure_angle", "pressure_angle", true),
        ("helix", "helix_angle", true),
        ("shift", "profile_shift", false),
        ("shift2", "profile_shift2", false),
        ("backlash", "backlash", false),
        ("face_width", "face_width", false),
        ("face_width2", "face_width2", false),
        ("rack_teeth", "rack_teeth", false),
        ("rack_height", "rack_height", false),
    ];
    for (key, field, angle) in fields {
        if given.iter().any(|g| g == field) {
            continue;
        }
        let Some(v) = vals.get(&format!("{prefix}_{key}")).map(|v| v.v) else { continue };
        let mut v = if angle { v.to_degrees() } else { v };
        let was = stored.get(field).and_then(Value::as_f64);
        if field == "helix_angle"
            && let Some(w) = was
        {
            // The parameter is the angle; the hand stays the set's.
            v = v.abs() * if w < 0.0 { -1.0 } else { 1.0 };
        }
        if was.is_some_and(|w| (w - v).abs() <= 1e-9 * w.abs().max(1.0)) {
            continue;
        }
        let value = if matches!(key, "teeth" | "teeth2" | "teeth3" | "rack_teeth") { json!(v.round() as i64) } else { json!(v) };
        request.insert(field.into(), value);
        if field == "module" {
            request.remove("diametral_pitch");
        }
    }
}
