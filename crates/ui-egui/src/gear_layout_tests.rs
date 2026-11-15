//! The Gear Generator window at many screen sizes, in both modes and for the main gear types:
//! it never runs past the screen, its controls never overlap, and Create and Close stay in sight.

use super::*;
use crate::scenario::Harness;
use egui::accesskit::{NodeId, Role};
use serde_json::json;

/// Wide, short, narrow, tall, tiny and this PC's ultrawide.
const SIZES: [(f32, f32); 8] =
    [(1600.0, 1000.0), (1920.0, 1080.0), (1280.0, 720.0), (1024.0, 640.0), (800.0, 600.0), (560.0, 900.0), (420.0, 640.0), (3440.0, 1392.0)];

const KINDS: [Kind; 9] =
    [Kind::Spur, Kind::Pair, Kind::Planetary, Kind::SpiralBevel, Kind::Hypoid, Kind::Worm, Kind::BevelPlanetary, Kind::Sprocket, Kind::CycloidalDrive];

/// A control the window drew: its label (for messages), its whole rect and the part of it not
/// clipped away (empty when scrolled out of sight).
#[derive(Debug)]
struct Control {
    label: String,
    rect: egui::Rect,
    shown: egui::Rect,
}

/// The node labelled `text` in the last frame's accessibility tree.
fn node_id(h: &Harness, text: &str) -> Option<NodeId> {
    let tree = h.output.accesskit_update.as_ref()?;
    tree.nodes.iter().find(|(_, n)| n.label() == Some(text) && n.role() == Role::Button).map(|(id, _)| *id)
}

/// The gear window's layer and its rect on screen (found through its Create button).
fn window(h: &Harness) -> Option<(egui::LayerId, egui::Rect)> {
    let create = node_id(h, solidcraft_ui_text::tr("ui.create"))?;
    let layer = h.ctx.viewport(|vp| {
        vp.prev_pass.widgets.layers().find_map(|(layer, widgets)| widgets.iter().any(|w| NodeId(w.id.value()) == create).then_some(*layer))
    })?;
    let rect = h.ctx.memory(|m| m.area_rect(layer.id))?;
    Some((layer, rect))
}

/// The window's controls: everything one clicks, ticks, picks from, drags or types into, and
/// the previews (big click-and-drag canvases without a role).
fn controls(h: &Harness, layer: egui::LayerId) -> Vec<Control> {
    let roles: std::collections::HashMap<NodeId, (Role, String)> = h
        .output
        .accesskit_update
        .as_ref()
        .map(|t| t.nodes.iter().map(|(id, n)| (*id, (n.role(), n.label().unwrap_or_default().to_string()))).collect())
        .unwrap_or_default();
    h.ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .get_layer(layer)
            .filter(|w| w.visible)
            .filter_map(|w| {
                let (role, label) = roles.get(&NodeId(w.id.value())).cloned().unwrap_or((Role::Unknown, String::new()));
                let canvas = w.sense.senses_click() && w.sense.senses_drag() && w.rect.width() >= 100.0 && w.rect.height() >= 100.0;
                (egui::accessibility::is_input(role) || canvas).then(|| Control {
                    label: if label.is_empty() { format!("{role:?}") } else { label },
                    rect: w.rect,
                    shown: w.interact_rect,
                })
            })
            .collect()
    })
}

fn visible(r: egui::Rect) -> bool {
    r.is_positive() && r.width() > 1.0 && r.height() > 1.0
}

/// What is wrong with the window as last drawn, if anything.
fn problems(h: &Harness, what: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let screen = h.ctx.content_rect();
    let Some((layer, win)) = window(h) else {
        return vec![format!("{what}: no Create button: the footer is not drawn")];
    };
    if !screen.expand(1.0).contains_rect(win) {
        bad.push(format!("{what}: window {win:?} runs past the screen {screen:?}"));
    }
    let widest = solidcraft_theme::Tokens::get().form_card_width * window_cards(&h.ctx) + 4.0;
    if win.width() > widest {
        bad.push(format!("{what}: window is {:.0} wide, more than its {widest:.0} (content wider than the window pushes it out)", win.width()));
    }
    let all = controls(h, layer);
    for name in [solidcraft_ui_text::tr("ui.create"), solidcraft_ui_text::tr("ui.close")] {
        match all.iter().find(|c| c.label == name) {
            None => bad.push(format!("{what}: no {name} button")),
            Some(c) if !screen.contains_rect(c.rect) || (c.shown.area() - c.rect.area()).abs() > 1.0 => {
                bad.push(format!("{what}: {name} is not wholly in sight: {:?} shown {:?}", c.rect, c.shown))
            }
            Some(_) => {}
        }
    }
    let shown: Vec<&Control> = all.iter().filter(|c| visible(c.shown)).collect();
    for c in &shown {
        if !win.expand(1.0).contains_rect(c.shown) {
            bad.push(format!("{what}: `{}` at {:?} is outside the window {win:?}", c.label, c.shown));
        }
    }
    for (i, a) in shown.iter().enumerate() {
        for b in &shown[i + 1..] {
            let x = a.shown.intersect(b.shown);
            // One inside the other is a field on a canvas, not an overlap.
            let nested = a.shown.expand(1.0).contains_rect(b.shown) || b.shown.expand(1.0).contains_rect(a.shown);
            if x.width() > 2.0 && x.height() > 2.0 && !nested {
                bad.push(format!("{what}: `{}` {:?} overlaps `{}` {:?}", a.label, a.shown, b.label, b.shown));
            }
        }
    }
    bad
}

fn resize(h: &mut Harness, (w, ht): (f32, f32)) {
    h.call("ui.resize", json!({"width": w, "height": ht}));
    h.frames(4);
}

fn set_kind(h: &mut Harness, mode: Mode, k: Kind) {
    let w = h.app.gear_window.as_mut().expect("the gear window is open");
    w.mode = mode;
    match mode {
        Mode::Simple => crate::gear_simple::set_mode(w, k),
        Mode::Advanced => w.switch_kind(k),
    }
    h.frames(6);
}

fn sweep(mode: Mode) -> Vec<String> {
    let mut h = Harness::new();
    open(&mut h.app);
    h.frames(5);
    let mut bad = Vec::new();
    for k in KINDS {
        if mode == Mode::Simple && !solidcraft_gear::sketch_rules::sketched(k) {
            continue;
        }
        set_kind(&mut h, mode, k);
        for size in SIZES {
            resize(&mut h, size);
            bad.extend(problems(&h, &format!("{mode:?} {k:?} at {}×{}", size.0, size.1)));
        }
        resize(&mut h, SIZES[0]);
    }
    bad
}

#[test]
fn gear_window_simple_fits_every_screen() {
    let bad = sweep(Mode::Simple);
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn gear_window_advanced_fits_every_screen() {
    let bad = sweep(Mode::Advanced);
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
}

/// Each of the right column's views, on a small screen and a big one.
#[test]
fn gear_window_each_view_fits() {
    let mut h = Harness::new();
    open(&mut h.app);
    h.frames(5);
    let mut bad = Vec::new();
    for mode in [Mode::Simple, Mode::Advanced] {
        set_kind(&mut h, mode, Kind::Pair);
        for view in [RightView::Both, RightView::Drawing, RightView::View3d, RightView::Profile] {
            h.app.gear_window.as_mut().unwrap().right = view;
            for size in [(800.0, 600.0), (480.0, 700.0), (2560.0, 1440.0)] {
                resize(&mut h, size);
                bad.extend(problems(&h, &format!("{mode:?} {view:?} at {}×{}", size.0, size.1)));
            }
        }
    }
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
}

/// On a 1600×1000 screen the previews leave room for what is under them: Show in model is in
/// sight without scrolling (two previews of a fixed height pushed it out of the window).
#[test]
fn gear_window_previews_leave_room_for_the_rows_under_them() {
    let mut h = Harness::new();
    open(&mut h.app);
    h.frames(5);
    resize(&mut h, (1600.0, 1000.0));
    let (layer, _) = window(&h).expect("the gear window is drawn");
    let all = controls(&h, layer);
    let show = all.iter().find(|c| c.label == "Show in model").expect("a Show in model button");
    assert!((show.shown.area() - show.rect.area()).abs() <= 1.0, "Show in model is scrolled out of sight: {:?} shown {:?}", show.rect, show.shown);
}

/// Narrow windows stack the columns; wide screens widen the window (to its limit) so the
/// views can sit side by side.
#[test]
fn gear_window_widens_on_wide_screens_and_stays_on_narrow_ones() {
    let mut h = Harness::new();
    open(&mut h.app);
    h.frames(5);
    resize(&mut h, (1600.0, 1000.0));
    let at_1600 = window(&h).expect("drawn").1.width();
    resize(&mut h, (3440.0, 1392.0));
    let wide = window(&h).expect("drawn").1.width();
    assert!(wide > at_1600 + 100.0, "the window should widen on a wide screen: {at_1600} → {wide}");
    resize(&mut h, (420.0, 640.0));
    let narrow = window(&h).expect("drawn").1.width();
    assert!(narrow <= 421.0, "the window must fit a 420 pt screen: {narrow}");
}

/// Texts drawn cut short ("Helix…") in the last frame, in full.
fn elided(h: &Harness) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) if t.galley.elided => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Simple mode's settings labels are never cut ("Helix…", "Gear …", "3D p…"), wide or narrow.
#[test]
fn gear_simple_settings_labels_show_whole() {
    let mut h = Harness::new();
    open(&mut h.app);
    h.frames(5);
    let mut bad = Vec::new();
    for k in Kind::ALL.iter().copied().filter(|k| solidcraft_gear::sketch_rules::sketched(*k)) {
        set_kind(&mut h, Mode::Simple, k);
        let labels: Vec<&str> = solidcraft_gear::simple::fields(Some(k)).iter().map(|f| f.label).collect();
        for size in [(1600.0, 1000.0), (1024.0, 640.0), (800.0, 600.0), (420.0, 640.0), (3440.0, 1392.0)] {
            resize(&mut h, size);
            for text in elided(&h) {
                if labels.contains(&text.as_str()) {
                    bad.push(format!("{k:?} at {}×{}: `{text}` is cut short", size.0, size.1));
                }
            }
        }
        resize(&mut h, SIZES[0]);
    }
    assert!(bad.is_empty(), "{} cut labels:\n{}", bad.len(), bad.join("\n"));
}

/// The Turn buttons are plain text: the font has no rotate glyph (it drew a box: "▯X 90°").
#[test]
fn gear_simple_turn_buttons_have_no_missing_glyph() {
    let mut h = Harness::new();
    open(&mut h.app);
    h.frames(5);
    for axis in ["X", "Y", "normal"] {
        assert!(h.widget(&format!("{axis} +90°")).is_some(), "no `{axis} +90°` button");
    }
    let tree = h.output.accesskit_update.as_ref().expect("an accessibility tree");
    let boxed: Vec<&str> = tree.nodes.iter().filter_map(|(_, n)| n.label()).filter(|l| l.contains('⟲')).collect();
    assert!(boxed.is_empty(), "labels with the missing rotate glyph: {boxed:?}");
}

/// The window keeps its place while its content changes height (centred on its content, it
/// jumped with every gear type): only its bottom edge moves.
#[test]
fn gear_window_stays_put_when_its_content_changes() {
    let mut bad = Vec::new();
    for mode in [Mode::Simple, Mode::Advanced] {
        let mut h = Harness::new();
        open(&mut h.app);
        h.frames(5);
        let top = window(&h).expect("drawn").1.top();
        for k in KINDS {
            if mode == Mode::Simple && !solidcraft_gear::sketch_rules::sketched(k) {
                continue;
            }
            set_kind(&mut h, mode, k);
            let now = window(&h).expect("drawn").1;
            // A few points' give: the screen may nudge a window as tall as the kit allows.
            if (now.top() - top).abs() > 16.0 {
                bad.push(format!("{mode:?} {k:?}: the window's top moved from {top} to {}", now.top()));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
