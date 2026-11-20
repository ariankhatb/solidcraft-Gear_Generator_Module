//! Gear teeth drawn from their formulas (`gear.create` with `tooth_curves: "equations"`): each
//! toothed outline of a prism step whose tooth data the plan keeps ([`Plan::teeth`]) becomes one
//! tooth of equation curves (involute flanks, trochoid root fillets) with its tip and root arcs,
//! repeated by a circular pattern (a rack: lines and a rectangular pattern). With `parametric`,
//! the formulas, the pattern count and the tooth extrusion name design parameters
//! (`<prefix>_module`, `<prefix>_teeth`…), so editing one regenerates the teeth.
//!
//! What a parameter edit moves: the teeth (shape and count) and their extrusion length; where
//! a pair's gear 2 and a planetary set's planets sit (their sketches' planes have origins over
//! the working centre distance, `<prefix>_mesh_cd`, worked out from the module, tooth counts
//! and shifts) and how they are turned to mesh (`<prefix>_gear2_phase`, `_planet<k>_phase`,
//! `_ring_phase`); and the clearance body's tip envelopes and face lengths (its profile's
//! vertices are points over the tip radii, the centre distance and the face width). What it
//! does not: hub, bore and shaft sizes, bearing seats' sizes, a ring's profile shift and rim,
//! and a herringbone's upper half starts where it did; `gear.update` reads the parameters back
//! and regenerates all of it. A tooth count edited into undercut makes the fillet and the flank
//! cross: the extrusion takes only the gear's own region (picked by a point that stays inside
//! it), not the small loops between the curves.
//!
//! `gear.update` in place redraws an equation sketch from scratch in the same sketch (same
//! sketch and feature ids; its curve ids come out the same when the tooth is drawn the same way).

use serde_json::{Value, json};
use std::collections::BTreeSet;

use solidcraft_gear::clearance::VertexFollow;
use solidcraft_gear::equations::{Edge, End, Placing, Quantity, Repeat, TeethSource, ToothCell, Vars, cell_for, keys, literal, number, render};
use solidcraft_gear::plan::{Body, Kind, Plan, Spec, Step};
use solidcraft_gear::V2;
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
    /// Parametric: where the members sit as formulas, and how the clearance body's profiles
    /// follow them ([`Plan::placing`], [`Plan::clearance_follow`]).
    pub placing: Placing,
    pub follow: Vec<Vec<VertexFollow>>,
    /// Parametric: every key the set's formulas define (the design parameters it makes).
    pub keys: BTreeSet<String>,
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
        if spec.train.is_some() {
            warnings.push("gear trains: tooth curves from equations are not made yet; the teeth are fitted splines".into());
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
        let mut mode = Mode { on: true, sources: plan.teeth.clone(), prefix, ..Mode::default() };
        let outlines = prism_outlines(plan);
        if mode.prefix.is_some() {
            mode.placing = plan.placing.clone();
            mode.follow = plan.clearance_follow.clone();
            let mut known: BTreeSet<String> = mode.placing.vars.list.iter().map(|v| v.key.clone()).collect();
            for o in &outlines {
                if let Some(c) = cell_for(&plan.teeth, o) {
                    known.extend(c.vars.list.iter().map(|v| v.key.clone()));
                }
            }
            mode.keys = known;
        }
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

    /// How far body `index` of the plan (`body`) moves from where it was planned when the
    /// parameters change, per world axis (an expression of the parameters, `None`: it doesn't
    /// along that axis): a member placed by formulas ([`Placing::centres`]).
    pub fn moves(&self, index: usize, body: &Body) -> Option<[Option<String>; 3]> {
        let prefix = self.prefix.as_ref()?;
        let (_, x, y) = self.placing.centres.iter().find(|(k, ..)| *k == index)?;
        let (xv, yv) = self.placing.value(x, y).ok()?;
        let name = |k: &str| format!("{prefix}_{k}");
        let (ax, ay) = (body.frame.x, body.frame.y());
        let parts = |tpl: &str, at: f64, dir: [f64; 3], c: usize| {
            (!keys(tpl).is_empty() && dir[c].abs() > 1e-12).then(|| format!("{} * ({} - {})", number(dir[c]), render(tpl, &name), literal(at, Quantity::Length)))
        };
        Some([0, 1, 2].map(|c| {
            let both: Vec<String> = [parts(x.as_str(), xv, ax, c), parts(y.as_str(), yv, ay, c)].into_iter().flatten().collect();
            (!both.is_empty()).then(|| both.join(" + "))
        }))
    }

    /// A clearance profile's vertices (radius, z; `z` moved by `nudge`) as sketch point
    /// coordinates following the parameters ([`VertexFollow`]): expressions where they follow,
    /// numbers elsewhere. `None` when nothing follows, or a key isn't one of the set's.
    pub fn follow_poly(&self, profile: &[V2], follow: &[VertexFollow], nudge: f64) -> Option<Vec<[Value; 2]>> {
        let prefix = self.prefix.as_ref()?;
        if follow.len() != profile.len() || follow.iter().all(|f| f.r.is_none() && f.z.is_none()) {
            return None;
        }
        let name = |k: &str| format!("{prefix}_{k}");
        let known = |tpl: &str| keys(tpl).iter().all(|k| self.keys.contains(k));
        let mut out = Vec::new();
        for (q, f) in profile.iter().zip(follow) {
            let z = q.y + nudge;
            let x = match &f.r {
                Some((tpl, at)) => {
                    if !known(tpl.as_str()) {
                        return None;
                    }
                    json!(format!("{} + ({} - {})", literal(q.x, Quantity::Length), render(tpl, &name), literal(*at, Quantity::Length)))
                }
                None => json!(q.x),
            };
            let y = match &f.z {
                Some((k, key, face)) => {
                    if !self.keys.contains(key) {
                        return None;
                    }
                    json!(format!("{} + {} * ({} - {})", literal(z, Quantity::Length), number(*k), name(key.as_str()), literal(*face, Quantity::Length)))
                }
                None => json!(z),
            };
            out.push([x, y]);
        }
        Some(out)
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

/// What every parameter the Gear Generator makes is commented with, first.
const MADE_BY: &str = "Gear Generator: ";

/// Makes or updates the design parameters of `vars`, named `<prefix>_<key>`.
fn ensure_vars(s: &mut Session, prefix: &str, vars: &Vars) -> Result<()> {
    let name = |k: &str| format!("{prefix}_{k}");
    for v in &vars.list {
        let n = name(&v.key);
        let expr = if v.input { v.expr.clone() } else { render(&v.expr, &name) };
        let existing = s.doc.param(&n).map(|p| p.expr.clone());
        match existing {
            Some(e) if e == expr => {}
            Some(_) => {
                s.execute("parameters.change", &json!({"name": n, "expression": expr}))?;
            }
            None => {
                let comment = format!("{MADE_BY}{}{}", v.comment, if v.input { "" } else { " (worked out)" });
                s.execute("parameters.add", &json!({"name": n, "expression": expr, "unit": unit_of(v.quantity), "comment": comment}))?;
            }
        }
    }
    Ok(())
}

/// Makes or updates the design parameters the sketch's formulas name.
fn ensure_params(s: &mut Session, def: &SketchDef) -> Result<()> {
    let Some(eq) = &def.eq else { return Ok(()) };
    let Some(prefix) = &eq.prefix else { return Ok(()) };
    for (_, cell) in &eq.cells {
        ensure_vars(s, prefix, &cell.vars)?;
    }
    Ok(())
}

/// Makes or updates the design parameters the members' places name (before the sketches whose
/// planes use them).
pub(super) fn ensure_placing(s: &mut Session, mode: &Mode) -> Result<()> {
    match &mode.prefix {
        Some(prefix) if !mode.placing.vars.list.is_empty() => ensure_vars(s, prefix, &mode.placing.vars),
        _ => Ok(()),
    }
}

/// Deletes the design parameters `<prefix>_*` the Gear Generator made that nothing uses (a set
/// whose teeth are no longer drawn from parametric formulas): those used by something else, a
/// feature or another parameter of the user's, stay.
pub(super) fn prune(s: &mut Session, prefix: &str) {
    let start = format!("{prefix}_");
    loop {
        let names: Vec<String> = s.doc.params.iter().filter(|p| p.name.starts_with(&start) && p.comment.starts_with(MADE_BY)).map(|p| p.name.clone()).collect();
        let mut removed = false;
        for n in names {
            if s.doc.param_users(&n).is_empty() && s.doc_mut().remove_param(&n).is_ok() {
                removed = true;
            }
        }
        if !removed {
            break;
        }
    }
}

/// The loops of `def` drawn as plain geometry (those that aren't teeth).
fn plain_loops(def: &SketchDef) -> Vec<&Vec<solidcraft_io::Geom2>> {
    let eq = def.eq.as_ref();
    if eq.is_some_and(|e| e.path.is_some()) || def.poly.is_some() {
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
    // Parametric undercut teeth: formula ranges, so they still close after a parameter change
    // (numeric ones stop meeting once the tooth count changes).
    let following = prefix.and(cell.following.as_ref()).map(|(curves, joins)| ToothCell { curves: curves.clone(), joins: joins.clone(), ..cell.clone() });
    let cell = following.as_ref().unwrap_or(cell);
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
    if let Some(poly) = &def.poly {
        return draw_poly(s, poly);
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

/// A closed polygon of points placed by expressions (or numbers, then fixed), in the sketch
/// being edited.
fn draw_poly(s: &mut Session, poly: &[[Value; 2]]) -> Result<()> {
    let mut ids = Vec::new();
    for q in poly {
        let id = id_of(&s.execute("sketch.point", &json!({"point": [q[0], q[1]]}))?, "point", "point")?;
        if !q[0].is_string() && !q[1].is_string() {
            s.execute("sketch.constraint.fix", &json!({"entities": [id], "fixed": true}))?;
        }
        ids.push(id);
    }
    for i in 0..ids.len() {
        s.execute("sketch.line", &json!({"points": [ids[i], ids[(i + 1) % ids.len()]]}))?;
    }
    Ok(())
}

/// A new sketch for `def` (see [`super::new_sketch`]), its teeth drawn from their formulas.
pub(super) fn new_sketch(s: &mut Session, def: &SketchDef) -> Result<u64> {
    ensure_params(s, def)?;
    let id = def.create(s)?;
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

/// Does sketch `id` hold anything only [`redraw`] clears (equation curves, patterns, points
/// placed by expressions)?
pub(super) fn holds_formulas(s: &Session, id: u64) -> bool {
    s.doc.sketch(id).is_ok_and(|sk| !sk.equations.is_empty() || !sk.patterns.is_empty() || sk.points.iter().any(|q| q.exprs.is_some()))
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
