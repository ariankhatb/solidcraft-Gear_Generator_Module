//! The Gear Generator's drawing: the one sketch Simple mode designs in, and a side view for the
//! angles out of its plane.
//!
//! The sketch holds the origin dot, circles (pitch circles) and lines (shafts), as the sketch
//! rules read them (`solidcraft_gear::sketch_rules`). Tools: Select (drag a dot, a circle's
//! centre or rim, a line's ends or the line), Dot, Circle (centre, then radius), Line (two
//! points). Moves snap to the other entities' points, to circles touching (outside, inside,
//! concentric), to lines through a centre or tangent to a circle, to 15° steps, and else to a
//! round grid. Dimensions are drawn as on a CAD drawing; clicking one opens a number field and
//! Enter applies it (Esc leaves it): circle Ø, the centre distance of touching circles (both
//! grow, the ratio stays), a line's distance from gear 1's centre, the angle between a bevel's
//! two shafts. Radii edited keep what touched touching.
//!
//! The side view shows the shaft angle a single sketch can't (hypoid, crossed helicals) with
//! its value as a dimension and a handle to drag.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use solidcraft_gear::geom::V2;
use solidcraft_gear::plan::{Kind, Spec};
use solidcraft_gear::sketch_rules::{self, Circle, Drawn, Line, Picks, Role};

use crate::gear_view3d::fmt;

/// The drawing tool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    #[default]
    Select,
    Dot,
    Circle,
    Line,
}

/// Something drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Point(usize),
    Circle(usize),
    Line(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    Point(usize),
    Centre(usize),
    Rim(usize),
    End(usize, bool),
    Body(usize),
    NewCircle(V2),
    NewLine(V2),
    /// The side view's shaft-angle handle.
    SideAngle,
}

/// An editable dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dim {
    Diameter(usize),
    Centres(usize, usize),
    Offset(usize, usize),
    Angle(usize, usize),
    ShaftAngle,
    Helix,
    /// The `i`-th clearance seat's distance along its member's shaft.
    SeatAlong(usize),
}

/// Screen ↔ sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
struct View {
    scale: f64,
    mid: V2,
}

/// The canvas's own state, kept by the window.
#[derive(Default)]
pub(crate) struct Canvas {
    pub tool: Tool,
    pub selected: Option<Item>,
    grab: Option<Grab>,
    /// The view while a drag goes on (so the drawing doesn't swim under the pointer).
    frozen: Option<View>,
    /// The dimension being typed, its text, and whether its field still needs focus.
    edit: Option<(Dim, String, bool)>,
}

/// What a frame of the canvas changed.
#[derive(Default)]
pub(crate) struct Changed {
    pub drawing: bool,
    pub spec: bool,
    /// A slot filled by a click in the drawing.
    pub picked: bool,
}

/// Which member a slot's entity stands for, for its colour.
fn slot_member(k: Kind, slot: usize) -> Option<usize> {
    match (k, slot) {
        (_, 0) => None,
        (Kind::Hypoid, 1) => Some(1),
        (Kind::Hypoid, 2) => Some(0),
        (Kind::Internal, 2) | (Kind::Rack, 1) | (Kind::Worm, 3) => None,
        (_, s) => Some(s - 1),
    }
}

/// The slot an item fills, if any.
fn slot_of(k: Kind, picks: &Picks, item: Item) -> Option<usize> {
    let slots = sketch_rules::slots(k);
    (0..slots.len()).find(|i| match (slots[*i].role, item, picks.get(*i).copied().flatten()) {
        (Role::Point, Item::Point(j), Some(p)) | (Role::Circle, Item::Circle(j), Some(p)) | (Role::Line, Item::Line(j), Some(p)) => p == j,
        _ => false,
    })
}

fn nice_step(raw: f64) -> f64 {
    let p = 10f64.powf(raw.max(1e-9).log10().floor());
    for k in [1.0, 2.0, 5.0, 10.0] {
        if raw <= k * p {
            return k * p;
        }
    }
    10.0 * p
}

fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

fn unit(v: V2) -> V2 {
    let l = v.len();
    if l > 1e-12 { v * (1.0 / l) } else { V2::new(1.0, 0.0) }
}

/// Snaps a point being moved (not `skip`): onto another point, a circle's centre, a line's end,
/// the origin; else onto the grid.
fn snap_point(d: &Drawn, p: V2, skip: Option<Item>, tol: f64, grid: f64) -> V2 {
    let mut best: Option<(f64, V2)> = None;
    let mut offer = |q: V2| {
        let e = q.dist(p);
        if e < tol && best.is_none_or(|(b, _)| e < b) {
            best = Some((e, q));
        }
    };
    offer(V2::ZERO);
    for (i, q) in d.points.iter().enumerate() {
        if skip != Some(Item::Point(i)) {
            offer(*q);
        }
    }
    for (i, c) in d.circles.iter().enumerate() {
        if skip != Some(Item::Circle(i)) {
            offer(c.c);
        }
    }
    for (i, l) in d.lines.iter().enumerate() {
        if skip != Some(Item::Line(i)) {
            offer(l.a);
            offer(l.b);
        }
    }
    best.map(|(_, q)| q).unwrap_or(V2::new(round_to(p.x, grid), round_to(p.y, grid)))
}

/// Snaps a circle's centre so it touches another circle (outside or inside) or shares its
/// centre.
fn snap_centre(d: &Drawn, i: usize, p: V2, tol: f64, grid: f64) -> V2 {
    let r = d.circles[i].r;
    for (j, o) in d.circles.iter().enumerate() {
        if j == i {
            continue;
        }
        let dist = p.dist(o.c);
        if dist < tol {
            return o.c;
        }
        for t in [o.r + r, (o.r - r).abs()] {
            if (dist - t).abs() < tol && t > 1e-9 {
                return o.c + unit(p - o.c) * t;
            }
        }
    }
    snap_point(d, p, Some(Item::Circle(i)), tol, grid)
}

/// Snaps a radius so circle `i` touches another circle or a line.
fn snap_radius(d: &Drawn, i: usize, r: f64, tol: f64, grid: f64) -> f64 {
    let c = d.circles[i].c;
    let mut offers = Vec::new();
    for (j, o) in d.circles.iter().enumerate() {
        if j != i {
            let dist = c.dist(o.c);
            offers.extend([dist - o.r, o.r - dist, dist + o.r, o.r]);
        }
    }
    for l in &d.lines {
        offers.push(l.distance(c));
    }
    offers.into_iter().filter(|t| *t > 1e-9 && (t - r).abs() < tol).min_by(|a, b| (a - r).abs().total_cmp(&(b - r).abs())).unwrap_or_else(|| round_to(r, grid).max(grid))
}

/// Snaps a line's moving end `p` (the other end `fixed`): through a circle's centre, tangent to
/// a circle, onto a point, or at a 15° step.
fn snap_end(d: &Drawn, li: usize, fixed: V2, p: V2, tol: f64, grid: f64) -> V2 {
    let len = fixed.dist(p);
    let dir = unit(p - fixed);
    for c in &d.circles {
        let line = Line { a: fixed, b: p };
        let e = line.distance(c.c);
        let to_c = c.c - fixed;
        let dc = to_c.len();
        if dc < 1e-9 {
            continue;
        }
        // Through the centre.
        if e < tol && to_c.dot(dir) > 0.0 {
            return fixed + unit(to_c) * len.max(dc);
        }
        // Tangent.
        if (e - c.r).abs() < tol && dc > c.r {
            let half = (c.r / dc).asin();
            let base = to_c.angle();
            let pick = [base + half, base - half].into_iter().min_by(|a, b| (V2::polar(1.0, *a).dot(dir) - 1.0).abs().total_cmp(&(V2::polar(1.0, *b).dot(dir) - 1.0).abs()));
            if let Some(a) = pick {
                return fixed + V2::polar(len, a);
            }
        }
    }
    let at = snap_point(d, p, Some(Item::Line(li)), tol, grid);
    if at.dist(p) < tol && d.points.iter().chain(d.circles.iter().map(|c| &c.c)).any(|q| q.dist(at) < 1e-9) {
        return at;
    }
    let a = dir.angle().to_degrees();
    let step = (a / 15.0).round() * 15.0;
    if (a - step).abs() < 2.0 {
        return fixed + V2::polar(round_to(len, grid), step.to_radians());
    }
    at
}

/// Snaps a whole line moved by its body: tangent to a circle, or through its centre.
fn snap_body(d: &Drawn, l: Line, tol: f64) -> Line {
    for c in &d.circles {
        let e = l.distance(c.c);
        let foot = l.foot(c.c);
        let out = if e > 1e-9 { unit(foot - c.c) } else { V2::new(-l.dir().y, l.dir().x) };
        for t in [0.0, c.r] {
            if (e - t).abs() < tol {
                let shift = c.c + out * t - foot;
                return Line { a: l.a + shift, b: l.b + shift };
            }
        }
    }
    l
}

/// After a radius changed, what touched keeps touching (by the mode's picks).
pub(crate) fn reimpose(d: &mut Drawn, k: Kind, picks: &Picks, changed: usize) {
    let c = |i: usize| picks.get(i).copied().flatten().filter(|j| *j < d.circles.len());
    match k {
        Kind::Pair | Kind::Cycloidal => {
            if let (Some(a), Some(b)) = (c(1), c(2)) {
                let (ca, cb) = (d.circles[a], d.circles[b]);
                d.circles[b].c = ca.c + unit(cb.c - ca.c) * (ca.r + cb.r);
            }
        }
        Kind::Internal => {
            if let (Some(ring), Some(p)) = (c(1), c(2)) {
                let (cr, cp) = (d.circles[ring], d.circles[p]);
                d.circles[p].c = cr.c + unit(cp.c - cr.c) * (cr.r - cp.r).max(0.0);
            }
        }
        Kind::Planetary => {
            if let (Some(sun), Some(pl), Some(ring)) = (c(1), c(2), c(3)) {
                let (s, p) = (d.circles[sun], d.circles[pl]);
                if changed == ring {
                    d.circles[pl].r = ((d.circles[ring].r - s.r) / 2.0).max(0.1);
                } else {
                    d.circles[ring].r = s.r + 2.0 * p.r;
                }
                d.circles[ring].c = s.c;
                let p = d.circles[pl];
                d.circles[pl].c = s.c + unit(p.c - s.c) * (s.r + p.r);
            }
        }
        Kind::Rack | Kind::Worm => {
            if let (Some(ci), Some(li)) = (c(1), picks.get(2).copied().flatten().filter(|j| *j < d.lines.len())) {
                let (cc, l) = (d.circles[ci], d.lines[li]);
                let foot = l.foot(cc.c);
                let out = if foot.dist(cc.c) > 1e-9 { unit(foot - cc.c) } else { V2::new(-l.dir().y, l.dir().x) };
                let shift = cc.c + out * cc.r - foot;
                d.lines[li] = Line { a: l.a + shift, b: l.b + shift };
            }
        }
        _ => {}
    }
}

/// Parses a typed value: "40", "40 mm", "12,5", "80°".
pub(crate) fn parse(text: &str) -> Option<f64> {
    let t: String = text.trim().replace(',', ".").chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')).collect();
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// What the drawing needs besides the sketch.
pub(crate) struct Ctx<'a> {
    pub kind: Kind,
    pub picks: &'a mut Picks,
    pub auto: &'a mut bool,
    /// A slot waiting for a click in the drawing.
    pub armed: &'a mut Option<usize>,
    pub spec: &'a mut Spec,
    /// The member lit elsewhere (a hovered section).
    pub lit: Option<usize>,
    /// The set's body names in order (clearance seats name their member by body).
    pub bodies: &'a [String],
}

struct Hit {
    rect: Rect,
    dim: Dim,
    value: f64,
}

/// The sketch canvas. Returns what changed.
pub(crate) fn sketch(ui: &mut Ui, cv: &mut Canvas, d: &mut Drawn, cx: Ctx<'_>, size: egui::Vec2) -> Changed {
    let mut out = Changed::default();
    ui.horizontal_wrapped(|ui| {
        for (t, name, tip) in [
            (Tool::Select, "Select", "Drag a dot, a circle's centre or rim, a line's ends or the line; click to select, Delete removes"),
            (Tool::Dot, "Dot", "The origin dot: the set's anchor (a bevel's apex)"),
            (Tool::Circle, "Circle", "A pitch circle: press at the centre, drag out the radius"),
            (Tool::Line, "Line", "A shaft (or a rack's or worm's pitch line): press at one end, drag to the other"),
        ] {
            ui.selectable_value(&mut cv.tool, t, name).on_hover_text(tip);
        }
        if let Some(sel) = cv.selected
            && ui.small_button("Delete").on_hover_text("Remove the selected item").clicked()
        {
            remove(d, sel);
            cv.selected = None;
            *cx.auto = true;
            out.drawing = true;
        }
    });
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let vis = ui.visuals();
    let (ink, bg, weak) = (vis.text_color(), vis.extreme_bg_color, vis.weak_text_color());
    painter.rect_filled(rect, 6.0, bg);
    // Fit what is drawn (frozen while dragging).
    let view = cv.frozen.unwrap_or_else(|| fit(d, rect, cone_reach(d, cx.kind, cx.spec)));
    let to_screen = |p: V2| -> Pos2 { pos2(rect.center().x + ((p.x - view.mid.x) * view.scale) as f32, rect.center().y - ((p.y - view.mid.y) * view.scale) as f32) };
    let to_sketch = |p: Pos2| -> V2 { V2::new(view.mid.x + f64::from(p.x - rect.center().x) / view.scale, view.mid.y - f64::from(p.y - rect.center().y) / view.scale) };
    let tol = 9.0 / view.scale;
    let grid = nice_step(6.0 / view.scale);
    // The sketch's axes, faint.
    let o = to_screen(V2::ZERO);
    painter.line_segment([pos2(rect.left(), o.y), pos2(rect.right(), o.y)], Stroke::new(0.6, weak.gamma_multiply(0.35)));
    painter.line_segment([pos2(o.x, rect.top()), pos2(o.x, rect.bottom())], Stroke::new(0.6, weak.gamma_multiply(0.35)));
    let k = cx.kind;
    let color_of = |item: Item| -> (Color32, bool) {
        let slot = slot_of(k, cx.picks, item);
        let member = slot.and_then(|s| slot_member(k, s));
        let c = member.map(crate::gear_window::member_color).unwrap_or(ink.gamma_multiply(if slot.is_some() { 1.0 } else { 0.55 }));
        (c, member.is_some() && member == cx.lit)
    };
    // Lines: the shaft extended faintly, the segment solid.
    for (i, l) in d.lines.iter().enumerate() {
        let (c, lit) = color_of(Item::Line(i));
        let u = l.dir();
        let far = 4.0 * (rect.width().max(rect.height()) as f64) / view.scale;
        crate::gear_drawing::dashed(&painter, &[to_screen(l.a - u * far), to_screen(l.b + u * far)], Stroke::new(0.8, c.gamma_multiply(0.5)), 10.0, 4.0);
        let w = if cv.selected == Some(Item::Line(i)) || lit { 2.6 } else { 1.6 };
        painter.line_segment([to_screen(l.a), to_screen(l.b)], Stroke::new(w, c));
        for p in [l.a, l.b] {
            painter.circle_filled(to_screen(p), 2.5, c);
        }
    }
    for (i, cc) in d.circles.iter().enumerate() {
        let (c, lit) = color_of(Item::Circle(i));
        let w = if cv.selected == Some(Item::Circle(i)) || lit { 2.6 } else { 1.6 };
        painter.circle_stroke(to_screen(cc.c), (cc.r * view.scale) as f32, Stroke::new(w, c));
        let m = to_screen(cc.c);
        painter.line_segment([m - vec2(4.0, 0.0), m + vec2(4.0, 0.0)], Stroke::new(1.0, c));
        painter.line_segment([m - vec2(0.0, 4.0), m + vec2(0.0, 4.0)], Stroke::new(1.0, c));
    }
    for (i, p) in d.points.iter().enumerate() {
        let sel = cv.selected == Some(Item::Point(i));
        painter.circle_filled(to_screen(*p), if sel { 5.5 } else { 4.5 }, ink);
        painter.circle_stroke(to_screen(*p), 7.0, Stroke::new(1.0, ink.gamma_multiply(0.6)));
    }
    // Dimensions.
    let mut hits: Vec<Hit> = Vec::new();
    let dim_ink = ink.gamma_multiply(0.85);
    let pick = |i: usize| cx.picks.get(i).copied().flatten();
    for (i, cc) in d.circles.iter().enumerate() {
        let a = std::f64::consts::FRAC_PI_4 + 0.35 * i as f64;
        let edge = to_screen(cc.c + V2::polar(cc.r, a));
        let out_pt = edge + vec2(18.0 * a.cos() as f32, -18.0 * a.sin() as f32);
        painter.line_segment([to_screen(cc.c + V2::polar(-cc.r, a)), edge], Stroke::new(0.8, dim_ink));
        arrowhead(&painter, edge, to_screen(cc.c + V2::polar(-cc.r, a)), dim_ink);
        painter.line_segment([edge, out_pt], Stroke::new(0.8, dim_ink));
        hits.push(text_dim(&painter, out_pt + vec2(4.0, 0.0), &format!("Ø{}", fmt(2.0 * cc.r)), Dim::Diameter(i), 2.0 * cc.r, ink, bg, egui::Align2::LEFT_CENTER));
    }
    match k {
        Kind::Pair | Kind::Cycloidal | Kind::Internal | Kind::Sprocket | Kind::Pulley => {
            if let (Some(a), Some(b)) = (pick(1).filter(|i| *i < d.circles.len()), pick(2).filter(|i| *i < d.circles.len())) {
                let (pa, pb) = (d.circles[a].c, d.circles[b].c);
                if pa.dist(pb) > 1e-6 {
                    hits.push(linear_dim(&painter, to_screen(pa), to_screen(pb), 26.0, &format!("{}", fmt(pa.dist(pb))), Dim::Centres(a, b), pa.dist(pb), ink, bg));
                }
            }
        }
        Kind::Rack | Kind::Worm | Kind::Hypoid | Kind::CrossedHelical | Kind::FaceGear => {
            if let (Some(ci), Some(li)) = (pick(1).filter(|i| *i < d.circles.len()), pick(2).filter(|i| *i < d.lines.len())) {
                let (c, l) = (d.circles[ci].c, d.lines[li]);
                let foot = l.foot(c);
                if foot.dist(c) > 1e-6 {
                    // A worm's or rack's line is its pitch line, tangent to the wheel's pitch
                    // circle: the distance is that circle's radius, not the shaft offset e (the
                    // 3D view's e is centre to centre).
                    let tag = if matches!(k, Kind::Rack | Kind::Worm) { "r" } else { "e" };
                    hits.push(linear_dim(&painter, to_screen(c), to_screen(foot), 0.0, &format!("{tag} {}", fmt(foot.dist(c))), Dim::Offset(li, ci), foot.dist(c), ink, bg));
                }
            }
        }
        Kind::Bevel | Kind::SpiralBevel | Kind::BevelPlanetary => {
            if let (Some(l1), Some(l2)) = (pick(1).filter(|i| *i < d.lines.len()), pick(2).filter(|i| *i < d.lines.len())) {
                let dot = pick(0).and_then(|i| d.points.get(i).copied());
                if let Some(h) = bevel_dims(&painter, &to_screen, d.lines[l1], d.lines[l2], dot, cx.spec, view.scale, ink, bg, (l1, l2)) {
                    hits.push(h);
                }
            }
        }
        _ => {}
    }
    // Clearance seats: a bearing glyph at the member's shaft, with its "along" to type.
    if let Some(c) = cx.spec.clearance_body.as_ref().filter(|c| c.bearing.is_some() || c.bearing_size.is_some()) {
        let slots = sketch_rules::slots(k);
        let mut stacked: Vec<(Pos2, usize)> = Vec::new();
        for (si, seat) in c.seats.iter().enumerate() {
            let solidcraft_gear::clearance::Seat::Along { member, along } = *seat else { continue };
            let b = (member as usize).saturating_sub(1);
            let name = cx.bodies.get(b).map_or("", String::as_str);
            let wm = crate::gear_window::member_of_body(k, cx.spec.only, b, name);
            let anchor = (0..slots.len()).find(|s| slot_member(k, *s) == Some(wm)).and_then(|s| {
                let i = pick(s)?;
                match slots[s].role {
                    Role::Circle => d.circles.get(i).map(|cc| cc.c),
                    Role::Line => d.lines.get(i).map(|l| (l.a + l.b) * 0.5),
                    Role::Point => d.points.get(i).copied(),
                }
            });
            let Some(at) = anchor else { continue };
            let at = to_screen(at);
            let n = match stacked.iter_mut().find(|(p, _)| p.distance(at) < 1.0) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    stacked.push((at, 0));
                    0
                }
            };
            let col = crate::gear_window::member_color(wm);
            let g = at + vec2(-14.0, 14.0 + 16.0 * n as f32);
            painter.circle_filled(g, 5.0, bg);
            painter.circle_stroke(g, 5.0, Stroke::new(1.4, col));
            painter.circle_stroke(g, 2.2, Stroke::new(1.0, col));
            hits.push(text_dim(&painter, g + vec2(8.0, 0.0), &format!("seat {}", fmt(along)), Dim::SeatAlong(si), along, ink, bg, egui::Align2::LEFT_CENTER));
        }
    }
    // Typing into a dimension.
    let mut applied = false;
    if let Some((dim, text, focus)) = &mut cv.edit {
        let at = hits.iter().find(|h| h.dim == *dim).map(|h| h.rect).unwrap_or(Rect::from_center_size(rect.center(), vec2(70.0, 20.0)));
        let field = ui.put(Rect::from_center_size(at.center(), vec2(at.width().max(64.0), 20.0)), egui::TextEdit::singleline(text).desired_width(at.width().max(64.0)));
        if *focus {
            field.request_focus();
            *focus = false;
        }
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if escape || (field.lost_focus() && !enter) {
            cv.edit = None;
        } else if enter {
            if let Some(v) = parse(text) {
                let dim = *dim;
                apply_dim(d, cx.spec, k, cx.picks, dim, v, &mut out);
                applied = true;
            }
            cv.edit = None;
        }
    }
    // Pointer.
    if !applied && resp.clicked() && cv.edit.is_none() {
        let p = resp.interact_pointer_pos();
        if let Some(h) = p.and_then(|p| hits.iter().find(|h| h.rect.expand(3.0).contains(p))) {
            cv.edit = Some((h.dim, fmt(h.value), true));
        } else if let Some(p) = p {
            let q = to_sketch(p);
            match cv.tool {
                Tool::Dot => {
                    d.points = vec![snap_point(d, q, None, tol, grid)];
                    out.drawing = true;
                }
                _ => {
                    cv.selected = nearest(d, q, tol);
                    if let (Some(slot), Some(item)) = (*cx.armed, cv.selected) {
                        let role = sketch_rules::slots(k).get(slot).map(|s| s.role);
                        let idx = match (role, item) {
                            (Some(Role::Point), Item::Point(i)) | (Some(Role::Circle), Item::Circle(i)) | (Some(Role::Line), Item::Line(i)) => Some(i),
                            _ => None,
                        };
                        if let Some(i) = idx {
                            if cx.picks.len() <= slot {
                                cx.picks.resize(slot + 1, None);
                            }
                            cx.picks[slot] = Some(i);
                            *cx.auto = false;
                            *cx.armed = None;
                            out.picked = true;
                        }
                    }
                }
            }
        }
    }
    if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(p) = resp.interact_pointer_pos()
    {
        let q = to_sketch(p);
        cv.frozen = Some(view);
        cv.grab = match cv.tool {
            Tool::Circle => Some(Grab::NewCircle(snap_point(d, q, None, tol, grid))),
            Tool::Line => Some(Grab::NewLine(snap_point(d, q, None, tol, grid))),
            Tool::Dot => None,
            Tool::Select => grab_at(d, q, tol),
        };
    }
    if let (Some(g), Some(p)) = (cv.grab, resp.interact_pointer_pos().filter(|_| resp.dragged())) {
        let q = to_sketch(p);
        let mut preview = None;
        match g {
            Grab::Point(i) => d.points[i] = snap_point(d, q, Some(Item::Point(i)), tol, grid),
            Grab::Centre(i) => d.circles[i].c = snap_centre(d, i, q, tol, grid),
            Grab::Rim(i) => {
                let r = snap_radius(d, i, q.dist(d.circles[i].c), tol, grid);
                d.circles[i].r = r;
                reimpose(d, k, cx.picks, i);
            }
            Grab::End(i, first) => {
                let l = d.lines[i];
                let (fixed, _) = if first { (l.b, l.a) } else { (l.a, l.b) };
                let p = snap_end(d, i, fixed, q, tol, grid);
                if first { d.lines[i].a = p } else { d.lines[i].b = p }
            }
            Grab::Body(i) => {
                let delta = resp.drag_delta();
                let shift = V2::new(f64::from(delta.x) / view.scale, -f64::from(delta.y) / view.scale);
                let l = d.lines[i];
                d.lines[i] = Line { a: l.a + shift, b: l.b + shift };
                // Snap a copy only, so the drag keeps following the pointer.
                preview = Some((i, snap_body(d, d.lines[i], tol)));
            }
            Grab::NewCircle(c) => {
                let r = q.dist(c).max(grid);
                painter.circle_stroke(to_screen(c), (r * view.scale) as f32, Stroke::new(1.2, ink.gamma_multiply(0.7)));
                painter.text(to_screen(c + V2::new(r, 0.0)) + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, format!("Ø{}", fmt(2.0 * round_to(r, grid))), egui::FontId::proportional(11.0), ink);
            }
            Grab::NewLine(a) => {
                let b = snap_end(d, usize::MAX, a, q, tol, grid);
                painter.line_segment([to_screen(a), to_screen(b)], Stroke::new(1.2, ink.gamma_multiply(0.7)));
            }
            Grab::SideAngle => {}
        }
        if let Some((_, snapped)) = preview {
            painter.line_segment([to_screen(snapped.a), to_screen(snapped.b)], Stroke::new(1.0, ink.gamma_multiply(0.5)));
        }
        if !matches!(g, Grab::NewCircle(_) | Grab::NewLine(_)) {
            out.drawing = true;
        }
    }
    if resp.drag_stopped() {
        if let (Some(g), Some(p)) = (cv.grab, resp.interact_pointer_pos()) {
            let q = to_sketch(p);
            match g {
                Grab::NewCircle(c) => {
                    let r = snap_radius(&Drawn { circles: [d.circles.clone(), vec![Circle { c, r: 1.0 }]].concat(), ..d.clone() }, d.circles.len(), q.dist(c), tol, grid);
                    if r > 1e-6 {
                        d.circles.push(Circle { c, r });
                        cv.selected = Some(Item::Circle(d.circles.len() - 1));
                        out.drawing = true;
                    }
                }
                Grab::NewLine(a) => {
                    let b = snap_end(d, usize::MAX, a, q, tol, grid);
                    if a.dist(b) > tol {
                        d.lines.push(Line { a, b });
                        cv.selected = Some(Item::Line(d.lines.len() - 1));
                        out.drawing = true;
                    }
                }
                Grab::Body(i) => {
                    d.lines[i] = snap_body(d, d.lines[i], tol);
                    out.drawing = true;
                }
                _ => {}
            }
        }
        cv.grab = None;
        cv.frozen = None;
    }
    if resp.hovered() && cv.edit.is_none() && cv.selected.is_some() && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        if let Some(sel) = cv.selected.take() {
            remove(d, sel);
            *cx.auto = true;
            out.drawing = true;
        }
    }
    if d.circles.is_empty() && d.lines.is_empty() {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Draw the set: Circle for each pitch circle, Line for a shaft, Dot for the origin", egui::FontId::proportional(12.0), weak);
    }
    if out.drawing {
        cv.edit = None;
    }
    out
}

fn remove(d: &mut Drawn, item: Item) {
    match item {
        Item::Point(i) if i < d.points.len() => {
            d.points.remove(i);
        }
        Item::Circle(i) if i < d.circles.len() => {
            d.circles.remove(i);
        }
        Item::Line(i) if i < d.lines.len() => {
            d.lines.remove(i);
        }
        _ => {}
    }
}

/// A bevel side view's apex and the reach of its pitch cones (their cone distance), which can
/// be longer than the shaft lines drawn: the view fits them too.
fn cone_reach(d: &Drawn, k: Kind, spec: &Spec) -> Option<(V2, f64)> {
    if k.axes() != solidcraft_gear::plan::Axes::Intersecting || d.lines.len() < 2 {
        return None;
    }
    let (l1, l2) = (d.lines[0], d.lines[1]);
    let apex = d.points.first().copied().or_else(|| {
        let (u, v) = (l1.dir(), l2.dir());
        let den = u.cross(v);
        (den.abs() > 1e-9).then(|| l1.a + u * ((l2.a - l1.a).cross(v) / den))
    })?;
    let ray = |l: &Line| {
        let far = if l.a.dist(apex) >= l.b.dist(apex) { l.a } else { l.b };
        unit(far - apex)
    };
    let (r1, r2) = (ray(&l1), ray(&l2));
    let sigma = r1.cross(r2).abs().atan2(r1.dot(r2));
    let (z1, z2) = (f64::from(spec.teeth.max(1)), f64::from(spec.teeth2.max(1)));
    let d1 = sigma.sin().atan2(z2 / z1 + sigma.cos());
    let re = spec.module * z1 / 2.0 / d1.sin().max(1e-6);
    (re.is_finite() && re > 0.0).then_some((apex, re.min(1e5)))
}

/// The view that fits what is drawn (and a bevel's pitch cones, `reach` about the apex).
fn fit(d: &Drawn, rect: Rect, reach: Option<(V2, f64)>) -> View {
    let (mut lo, mut hi) = (V2::new(f64::MAX, f64::MAX), V2::new(f64::MIN, f64::MIN));
    let mut take = |p: V2, r: f64| {
        lo = V2::new(lo.x.min(p.x - r), lo.y.min(p.y - r));
        hi = V2::new(hi.x.max(p.x + r), hi.y.max(p.y + r));
    };
    take(V2::ZERO, 0.0);
    for p in &d.points {
        take(*p, 0.0);
    }
    for c in &d.circles {
        take(c.c, c.r);
    }
    for l in &d.lines {
        take(l.a, 0.0);
        take(l.b, 0.0);
    }
    if let Some((apex, re)) = reach {
        take(apex, re);
    }
    let span = V2::new((hi.x - lo.x).max(10.0), (hi.y - lo.y).max(10.0));
    let scale = (f64::from(rect.width()) / (span.x * 1.45)).min(f64::from(rect.height()) / (span.y * 1.45));
    View { scale, mid: (lo + hi) * 0.5 }
}

/// What a press at `q` takes hold of (Select tool).
fn grab_at(d: &Drawn, q: V2, tol: f64) -> Option<Grab> {
    let mut best: Option<(f64, Grab)> = None;
    let mut offer = |e: f64, g: Grab| {
        if e < tol && best.is_none_or(|(b, _)| e < b) {
            best = Some((e, g));
        }
    };
    for (i, p) in d.points.iter().enumerate() {
        offer(p.dist(q) * 0.8, Grab::Point(i));
    }
    for (i, l) in d.lines.iter().enumerate() {
        offer(l.a.dist(q) * 0.9, Grab::End(i, true));
        offer(l.b.dist(q) * 0.9, Grab::End(i, false));
        let t = (q - l.a).dot(l.dir());
        if t > 0.0 && t < l.len() {
            offer(l.distance(q) * 1.2, Grab::Body(i));
        }
    }
    for (i, c) in d.circles.iter().enumerate() {
        offer(c.c.dist(q), Grab::Centre(i));
        offer((c.c.dist(q) - c.r).abs() * 1.1, Grab::Rim(i));
    }
    best.map(|(_, g)| g)
}

fn nearest(d: &Drawn, q: V2, tol: f64) -> Option<Item> {
    grab_at(d, q, tol).map(|g| match g {
        Grab::Point(i) => Item::Point(i),
        Grab::Centre(i) | Grab::Rim(i) => Item::Circle(i),
        Grab::End(i, _) | Grab::Body(i) => Item::Line(i),
        _ => Item::Point(0),
    })
}

fn arrowhead(p: &egui::Painter, tip: Pos2, from: Pos2, color: Color32) {
    let d = tip - from;
    if d.length() < 1.0 {
        return;
    }
    let u = d / d.length();
    let n = vec2(-u.y, u.x);
    p.add(egui::Shape::convex_polygon(vec![tip, tip - u * 7.0 + n * 2.5, tip - u * 7.0 - n * 2.5], color, Stroke::NONE));
}

/// A dimension's value as clickable text.
#[allow(clippy::too_many_arguments)]
fn text_dim(p: &egui::Painter, at: Pos2, text: &str, dim: Dim, value: f64, ink: Color32, bg: Color32, align: egui::Align2) -> Hit {
    let g = p.layout_no_wrap(text.to_string(), egui::FontId::proportional(12.0), ink);
    let rect = align.anchor_size(at, g.size());
    p.rect_filled(rect.expand(2.0), 3.0, bg);
    p.rect_stroke(rect.expand(2.0), 3.0, Stroke::new(0.6, ink.gamma_multiply(0.35)), egui::StrokeKind::Outside);
    p.galley(rect.min, g, ink);
    Hit { rect, dim, value }
}

/// A linear dimension from `a` to `b`, offset `off` points to one side: extension lines, the
/// dimension line with arrows, the value.
#[allow(clippy::too_many_arguments)]
fn linear_dim(p: &egui::Painter, a: Pos2, b: Pos2, off: f32, text: &str, dim: Dim, value: f64, ink: Color32, bg: Color32) -> Hit {
    let d = b - a;
    let u = if d.length() > 1e-3 { d / d.length() } else { vec2(1.0, 0.0) };
    let n = vec2(-u.y, u.x);
    let (a2, b2) = (a + n * off, b + n * off);
    let thin = Stroke::new(0.7, ink.gamma_multiply(0.7));
    if off.abs() > 0.5 {
        p.line_segment([a + n * 3.0f32.copysign(off), a2 + n * 4.0f32.copysign(off)], thin);
        p.line_segment([b + n * 3.0f32.copysign(off), b2 + n * 4.0f32.copysign(off)], thin);
    }
    p.line_segment([a2, b2], thin);
    arrowhead(p, a2, b2, ink.gamma_multiply(0.85));
    arrowhead(p, b2, a2, ink.gamma_multiply(0.85));
    text_dim(p, a2.lerp(b2, 0.5) + n * 10.0f32.copysign(if off == 0.0 { 1.0 } else { off }), text, dim, value, ink, bg, egui::Align2::CENTER_CENTER)
}

/// A bevel's side view: the angle between its shafts at the apex (editable), the pitch cones
/// as their sizes give them, and the cone angles.
#[allow(clippy::too_many_arguments)]
fn bevel_dims(p: &egui::Painter, to_screen: &dyn Fn(V2) -> Pos2, l1: Line, l2: Line, dot: Option<V2>, spec: &Spec, scale: f64, ink: Color32, bg: Color32, ids: (usize, usize)) -> Option<Hit> {
    let apex = dot.or_else(|| {
        let (u, v) = (l1.dir(), l2.dir());
        let den = u.cross(v);
        (den.abs() > 1e-9).then(|| l1.a + u * ((l2.a - l1.a).cross(v) / den))
    })?;
    let ray = |l: &Line| {
        let far = if l.a.dist(apex) >= l.b.dist(apex) { l.a } else { l.b };
        unit(far - apex)
    };
    let (r1, r2) = (ray(&l1), ray(&l2));
    let sigma = r1.cross(r2).abs().atan2(r1.dot(r2));
    // The pitch cones from the sizes: gear i's large-end pitch circle at its cone distance.
    let (z1, z2) = (f64::from(spec.teeth.max(1)), f64::from(spec.teeth2.max(1)));
    let d1 = sigma.sin().atan2(z2 / z1 + sigma.cos());
    let d2 = sigma - d1;
    // Shafts lying (nearly) along one line make no cones to draw: a cone distance blown up by a
    // near-zero angle would reach far past the view.
    if !(d1 > 1e-3 && d2 > 1e-3) {
        return None;
    }
    let re = spec.module * z1 / 2.0 / d1.sin();
    let b = spec.face_width.min(0.45 * re);
    let side = if r1.cross(r2) >= 0.0 { 1.0 } else { -1.0 };
    for (r, delta, member, toward) in [(r1, d1, 0usize, side), (r2, d2, 1usize, -side)] {
        let rim = r.rot(toward * delta);
        let other = r.rot(-toward * delta);
        let c = crate::gear_window::member_color(member);
        let outer = [apex + rim * re, apex + other * re];
        let inner = [apex + rim * (re - b), apex + other * (re - b)];
        let fill = c.gamma_multiply(0.22);
        p.add(egui::Shape::convex_polygon(vec![to_screen(inner[0]), to_screen(outer[0]), to_screen(outer[1]), to_screen(inner[1])], fill, Stroke::new(1.0, c)));
        crate::gear_drawing::dashed(&p, &[to_screen(apex), to_screen(outer[0])], Stroke::new(0.7, c.gamma_multiply(0.7)), 4.0, 3.0);
        crate::gear_drawing::dashed(&p, &[to_screen(apex), to_screen(outer[1])], Stroke::new(0.7, c.gamma_multiply(0.7)), 4.0, 3.0);
        let label = apex + r.rot(-toward * delta * 0.5) * (re * 0.55);
        let name = if member == 0 { "δ₁" } else { "δ₂" };
        p.text(to_screen(label), egui::Align2::CENTER_CENTER, format!("{name} {}°", fmt(delta.to_degrees())), egui::FontId::proportional(10.5), c.lerp_to_gamma(ink, 0.4));
    }
    p.circle_filled(to_screen(apex), 3.5, ink);
    // The shaft angle's arc.
    let rad = (re * 0.35).max(24.0 / scale);
    let n = 28;
    let start = r1.angle();
    let sweep = side * sigma;
    let pts: Vec<Pos2> = (0..=n).map(|i| to_screen(apex + V2::polar(rad, start + sweep * i as f64 / n as f64))).collect();
    p.add(egui::Shape::line(pts.clone(), Stroke::new(1.2, ink)));
    arrowhead(p, pts[n], pts[n - 2], ink);
    arrowhead(p, pts[0], pts[2], ink);
    let at = to_screen(apex + V2::polar(rad * 1.35, start + sweep * 0.5));
    Some(text_dim(p, at, &format!("Σ {}°", fmt(sigma.to_degrees())), Dim::Angle(ids.0, ids.1), sigma.to_degrees(), ink, bg, egui::Align2::CENTER_CENTER))
}

/// Applies a typed dimension.
fn apply_dim(d: &mut Drawn, spec: &mut Spec, k: Kind, picks: &Picks, dim: Dim, v: f64, out: &mut Changed) {
    match dim {
        Dim::SeatAlong(i) if v.abs() <= 100000.0 => {
            if let Some(solidcraft_gear::clearance::Seat::Along { along, .. }) = spec.clearance_body.as_mut().and_then(|c| c.seats.get_mut(i)) {
                *along = v;
                out.spec = true;
            }
        }
        Dim::Diameter(i) if v > 0.0 && i < d.circles.len() => {
            d.circles[i].r = v / 2.0;
            reimpose(d, k, picks, i);
            out.drawing = true;
        }
        Dim::Centres(a, b) if v > 0.0 && a < d.circles.len() && b < d.circles.len() => {
            let (ca, cb) = (d.circles[a], d.circles[b]);
            let dir = unit(cb.c - ca.c);
            if k == Kind::Internal {
                // The pinion's size follows: it keeps touching the ring inside.
                d.circles[b].r = (ca.r - v).max(0.1);
            } else if matches!(k, Kind::Pair | Kind::Cycloidal) {
                // Both pitch circles grow with the centre distance; the ratio stays.
                let f = v / (ca.r + cb.r).max(1e-9);
                d.circles[a].r *= f;
                d.circles[b].r *= f;
            }
            d.circles[b].c = ca.c + dir * v;
            out.drawing = true;
        }
        Dim::Offset(li, ci) if v >= 0.0 && li < d.lines.len() && ci < d.circles.len() => {
            let (c, l) = (d.circles[ci].c, d.lines[li]);
            let foot = l.foot(c);
            let n = if foot.dist(c) > 1e-9 { unit(foot - c) } else { V2::new(-l.dir().y, l.dir().x) };
            let shift = c + n * v - foot;
            d.lines[li] = Line { a: l.a + shift, b: l.b + shift };
            out.drawing = true;
        }
        Dim::Angle(l1, l2) if (5.0..=175.0).contains(&v) && l1 < d.lines.len() && l2 < d.lines.len() => {
            let (a, b) = (d.lines[l1], d.lines[l2]);
            let apex = picks.first().copied().flatten().and_then(|i| d.points.get(i).copied()).unwrap_or_else(|| {
                let (u, w) = (a.dir(), b.dir());
                let den = u.cross(w);
                if den.abs() > 1e-9 { a.a + u * ((b.a - a.a).cross(w) / den) } else { a.a }
            });
            let ray = |l: &Line| {
                let far = if l.a.dist(apex) >= l.b.dist(apex) { l.a } else { l.b };
                (unit(far - apex), far.dist(apex))
            };
            let ((r1, _), (r2, len2)) = (ray(&a), ray(&b));
            let side = if r1.cross(r2) >= 0.0 { 1.0 } else { -1.0 };
            d.lines[l2] = Line { a: apex, b: apex + r1.rot(side * v.to_radians()) * len2.max(1.0) };
            out.drawing = true;
        }
        Dim::ShaftAngle if (5.0..=175.0).contains(&v) => {
            spec.shaft_angle = v;
            out.spec = true;
        }
        Dim::Helix if v.abs() < 80.0 => {
            spec.helix = v;
            out.spec = true;
        }
        _ => {}
    }
}

/// The side view of a set whose shaft angle lies out of the sketch's plane (hypoid, crossed
/// helicals): shaft 1 upright, shaft 2 at the shaft angle, both as seen along the offset. The
/// angle is a dimension to type into, and shaft 2's end a handle to drag. Crossed helicals also
/// show their helix split (β₁ typed, β₂ = Σ − β₁).
pub(crate) fn side_view(ui: &mut Ui, cv: &mut Canvas, spec: &mut Spec, size: egui::Vec2) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let vis = ui.visuals();
    let (ink, bg) = (vis.text_color(), vis.extreme_bg_color);
    painter.rect_filled(rect, 6.0, bg);
    painter.text(rect.left_top() + vec2(6.0, 4.0), egui::Align2::LEFT_TOP, "Side view (along the offset)", egui::FontId::proportional(10.5), vis.weak_text_color());
    let c = rect.center() + vec2(0.0, rect.height() * 0.12);
    let r = rect.height().min(rect.width()) * 0.38;
    let sigma = spec.shaft_angle.to_radians() as f32;
    let up = vec2(0.0, -1.0);
    let dir2 = vec2(sigma.sin(), -sigma.cos());
    let (c1, c2) = (crate::gear_window::member_color(0), crate::gear_window::member_color(1));
    crate::gear_drawing::dashed(&painter, &[c - up * r * 0.5, c + up * r], Stroke::new(1.5, c1), 9.0, 3.0);
    crate::gear_drawing::dashed(&painter, &[c - dir2 * r * 0.5, c + dir2 * r], Stroke::new(1.5, c2), 9.0, 3.0);
    arrowhead(&painter, c + up * r, c, c1);
    arrowhead(&painter, c + dir2 * r, c, c2);
    let handle = c + dir2 * r;
    painter.circle_stroke(handle, 6.0, Stroke::new(1.2, c2));
    let arc_r = r * 0.42;
    let pts: Vec<Pos2> = (0..=24).map(|i| c + vec2((sigma * i as f32 / 24.0).sin(), -(sigma * i as f32 / 24.0).cos()) * arc_r).collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.2, ink)));
    let mid = c + vec2((sigma / 2.0).sin(), -(sigma / 2.0).cos()) * (arc_r + 16.0);
    let mut hits = vec![text_dim(&painter, mid, &format!("Σ {}°", fmt(spec.shaft_angle)), Dim::ShaftAngle, spec.shaft_angle, ink, bg, egui::Align2::CENTER_CENTER)];
    if spec.kind == Kind::CrossedHelical {
        let b2 = spec.shaft_angle - spec.helix;
        hits.push(text_dim(&painter, rect.left_bottom() + vec2(8.0, -10.0), &format!("β₁ {}°", fmt(spec.helix)), Dim::Helix, spec.helix, ink, bg, egui::Align2::LEFT_CENTER));
        painter.text(rect.left_bottom() + vec2(80.0, -10.0), egui::Align2::LEFT_CENTER, format!("β₂ = Σ − β₁ = {}°", fmt(b2)), egui::FontId::proportional(11.0), vis.weak_text_color());
    }
    let mut changed = false;
    if let Some((dim, text, focus)) = &mut cv.edit
        && matches!(dim, Dim::ShaftAngle | Dim::Helix)
    {
        let at = hits.iter().find(|h| h.dim == *dim).map(|h| h.rect).unwrap_or(rect);
        let field = ui.put(Rect::from_center_size(at.center(), vec2(64.0, 20.0)), egui::TextEdit::singleline(text).desired_width(64.0));
        if *focus {
            field.request_focus();
            *focus = false;
        }
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if escape || (field.lost_focus() && !enter) {
            cv.edit = None;
        } else if enter {
            let mut out = Changed::default();
            if let Some(v) = parse(text) {
                let dim = *dim;
                let kind = spec.kind;
                apply_dim(&mut Drawn::default(), spec, kind, &Vec::new(), dim, v, &mut out);
            }
            changed = out.spec;
            cv.edit = None;
        }
    } else if resp.clicked()
        && let Some(h) = resp.interact_pointer_pos().and_then(|p| hits.iter().find(|h| h.rect.expand(3.0).contains(p)))
    {
        cv.edit = Some((h.dim, fmt(h.value), true));
    }
    if resp.drag_started() && resp.interact_pointer_pos().is_some_and(|p| p.distance(handle) < 14.0) {
        cv.grab = Some(Grab::SideAngle);
    }
    if cv.grab == Some(Grab::SideAngle) {
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
            let v = p - c;
            let a = f64::from(v.x).atan2(-f64::from(v.y)).to_degrees().abs();
            let a = if (a - (a / 5.0).round() * 5.0).abs() < 1.0 { (a / 5.0).round() * 5.0 } else { (a * 2.0).round() / 2.0 };
            spec.shaft_angle = a.clamp(5.0, 175.0);
            changed = true;
        }
        if resp.drag_stopped() {
            cv.grab = None;
        }
    }
    changed
}

/// A dashed polyline, clipped to what can be seen: a line far longer than the view (a cone
/// distance blown up by a near-zero angle, an axis drawn "to infinity") would otherwise be cut
/// into billions of dashes and run the window out of memory. Points that aren't finite draw
/// nothing.
pub(crate) fn dashed(p: &egui::Painter, pts: &[Pos2], stroke: Stroke, dash: f32, gap: f32) {
    let clip = p.clip_rect().expand(64.0);
    let inside = |q: Pos2| q.x.is_finite() && q.y.is_finite();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if !inside(a) || !inside(b) {
            continue;
        }
        // Liang–Barsky: the part of a→b within the clip rectangle.
        let d = b - a;
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        let mut keep = true;
        for (pp, q) in [(-d.x, a.x - clip.min.x), (d.x, clip.max.x - a.x), (-d.y, a.y - clip.min.y), (d.y, clip.max.y - a.y)] {
            if pp == 0.0 {
                if q < 0.0 {
                    keep = false;
                }
            } else {
                let r = q / pp;
                if pp < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
        }
        if keep && t0 < t1 {
            p.extend(egui::Shape::dashed_line(&[a + d * t0, a + d * t1], stroke, dash, gap));
        }
    }
}
