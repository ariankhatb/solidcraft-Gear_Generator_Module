//! The clearance body: every member's material, turned (or slid, or carried round) through its
//! motion, lies inside it with the clearances to spare; bearing seats sit at their dots.

use std::f64::consts::TAU;

use crate::clearance::{Clearance, Motion, Seat, motions, spacing};
use crate::hub::{BEARINGS, Bore, BoreKind, bearing, bearing_series};
use crate::mesh3d::Solid;
use crate::plan::{Frame, Kind, Op, Plan, Spec, Step, plan};

type P3 = [f64; 3];

fn on(kind: Kind) -> Spec {
    Spec { clearance_body: Some(Clearance::default()), ..Spec::defaults_for(kind) }
}

fn planned(s: &Spec) -> Plan {
    plan(s).unwrap_or_else(|e| panic!("{:?}: {e}", s.kind))
}

/// `p` turned by `a` about the axis through `o` along the unit `z`.
fn turn_about(p: P3, o: P3, z: P3, a: f64) -> P3 {
    let v = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let (c, s) = (a.cos(), a.sin());
    let k = z[0] * v[0] + z[1] * v[1] + z[2] * v[2];
    let x = [z[1] * v[2] - z[2] * v[1], z[2] * v[0] - z[0] * v[2], z[0] * v[1] - z[1] * v[0]];
    [0, 1, 2].map(|i| o[i] + v[i] * c + x[i] * s + z[i] * k * (1.0 - c))
}

/// Every member's material points (a few thousand of each), at several turns of each member
/// (and of the carrier, and along a rack's travel), moved out by the radial clearance and along
/// the axis by the axial one less `tol`: all must be inside the clearance body. Returns how many
/// were tried.
fn contains_every_member(s: &Spec, p: &Plan, tol: f64) -> usize {
    let c = s.clearance_body.clone().unwrap();
    let keep_body = p.clearance.as_ref().unwrap_or_else(|| panic!("{:?}: no clearance body", s.kind));
    let keep = Solid::new(keep_body, 0.002);
    let ms = motions(s.kind, &p.bodies);
    let rings_only = ms.iter().all(|m| *m == Motion::Ring);
    let (dr, da) = ((c.radial - tol).max(0.0), (c.axial - tol).max(0.0));
    let mut tried = 0;
    for (b, m) in p.bodies.iter().zip(&ms) {
        if *m == Motion::Ring && c.ring_as_housing && !rings_only {
            continue;
        }
        let span = {
            let coarse = Solid::new(b, 1.0);
            coarse.z.1 - coarse.z.0
        };
        let (h, hz) = spacing(s.module, span.max(1e-3));
        let solid = Solid::new(b, h);
        let pts = solid.material_points(hz);
        let every = (pts.len() / 1500).max(1);
        let travel = if c.travel > 0.0 {
            c.travel
        } else {
            let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q[0]), a.1.max(q[0])));
            hi - lo
        };
        for (i, q) in pts.iter().enumerate().step_by(every) {
            let r = q[0].hypot(q[1]);
            for k in 0..6 {
                let a = TAU * f64::from(k) / 6.0 + 0.37 * i as f64;
                let mut tests: Vec<P3> = Vec::new();
                match m {
                    Motion::Slides => {
                        let t = travel * (f64::from(k) / 5.0 - 0.5);
                        for (dx, dy, dz) in [(0.0, 0.0, 0.0), (dr, 0.0, 0.0), (-dr, 0.0, 0.0), (0.0, dr, 0.0), (0.0, -dr, 0.0), (0.0, 0.0, da), (0.0, 0.0, -da)] {
                            tests.push(b.frame.world([q[0] + t + dx, q[1] + dy, q[2] + dz]));
                        }
                    }
                    _ => {
                        let (ca, sa) = (a.cos(), a.sin());
                        let (x, y) = (q[0] * ca - q[1] * sa, q[0] * sa + q[1] * ca);
                        let (ux, uy) = if r > 1e-9 { (x / r, y / r) } else { (0.0, 0.0) };
                        for (out, dz) in [(0.0, 0.0), (dr, 0.0), (0.0, da), (0.0, -da)] {
                            let w = b.frame.world([x + ux * out, y + uy * out, q[2] + dz]);
                            match m {
                                Motion::Orbits { about, .. } => tests.push(turn_about(w, about.origin, about.z, 0.9 * f64::from(k) + 0.21)),
                                _ => tests.push(w),
                            }
                        }
                    }
                }
                for w in tests {
                    tried += 1;
                    assert!(keep.contains(w), "{:?}: {} point {:?} (local {:?}, step {k}) is not inside the clearance body", s.kind, b.name, w, q);
                }
            }
        }
    }
    tried
}

#[test]
fn every_kind_plans_a_clearance_body_that_holds_its_members() {
    for kind in Kind::ALL {
        let s = on(kind);
        let t = std::time::Instant::now();
        let p = planned(&s);
        let keep = p.clearance.as_ref().unwrap_or_else(|| panic!("{kind:?}: no clearance body"));
        assert!(keep.name.ends_with(" clearance"), "{kind:?}: {}", keep.name);
        assert!(!keep.steps.is_empty());
        for (i, st) in keep.steps.iter().enumerate() {
            let Step::Placed { step, .. } = st else { panic!("{kind:?}: step {i} is not placed") };
            let op = match step.as_ref() {
                Step::Revolve { op, .. } | Step::Prism { op, .. } => *op,
                other => panic!("{kind:?}: step {i} is {other:?}"),
            };
            assert_eq!(op, if i == 0 { Op::New } else { Op::Join }, "{kind:?}: step {i}");
        }
        // The gears themselves are as without it.
        let bare = planned(&Spec { clearance_body: None, ..s.clone() });
        assert_eq!(bare.bodies, p.bodies, "{kind:?}");
        assert!(bare.clearance.is_none());
        let planned_in = t.elapsed();
        let tried = contains_every_member(&s, &p, 0.01);
        assert!(tried > 1000, "{kind:?}: only {tried} points tried");
        assert!(p.facts.iter().any(|f| f.label.starts_with("clearance:")), "{kind:?}");
        eprintln!("{kind:?}: {} steps, planned in {planned_in:?}, {tried} points in {:?}", keep.steps.len(), t.elapsed());
    }
}

#[test]
fn envelopes_follow_the_teeth_not_a_bounding_cylinder() {
    // A bevel's envelope narrows with its cone: at the small end the clearance body is well
    // inside the large end's radius.
    let s = on(Kind::Bevel);
    let p = planned(&s);
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.01);
    let g1 = &p.bodies[0];
    let pts = Solid::new(g1, 0.05).material_points(0.2);
    let (zlo, zhi) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q[2]), a.1.max(q[2])));
    let rmax_at = |z: f64| pts.iter().filter(|q| (q[2] - z).abs() < 0.3).map(|q| q[0].hypot(q[1])).fold(0.0, f64::max);
    // (The most is at the heel, a little above the back face, which the back cone narrows.)
    let big = pts.iter().map(|q| q[0].hypot(q[1])).fold(0.0, f64::max);
    let small = rmax_at(zhi - 0.3);
    let _ = zlo;
    assert!(big > small + 3.0, "{big} {small}");
    // Just past the small end's need, at the small end, it is outside; it would be inside for a
    // cylinder of the big end's radius.
    let z = zhi - 0.3;
    // (Towards -y, away from gear 2.)
    let probe = g1.frame.world([0.0, -(small + 2.5), z]);
    assert!(!keep.contains(probe), "the bevel's clearance is a cylinder, not a cone");
    // A spur gear's envelope is the tip cylinder plus the clearance, not more.
    let s = on(Kind::Spur);
    let p = planned(&s);
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.005);
    let tip = 2.0 * (20.0 / 2.0 + 1.0);
    let c = Clearance::default();
    for (r, inside) in [(tip + c.radial - 0.02, true), (tip + c.radial + 0.15, false)] {
        assert_eq!(keep.contains([r, 0.0, 5.0]), inside, "r {r}");
    }
    // Axially: the face (0..10) plus the axial clearance, and on along the axis only the shaft
    // (none here: no bore, no bearing).
    assert!(keep.contains([tip - 1.0, 0.0, -c.axial + 0.02]));
    assert!(!keep.contains([tip - 1.0, 0.0, -c.axial - 1.2]));
}

#[test]
fn shafts_run_through_the_housing_walls() {
    let bore = Bore { kind: BoreKind::Round, diameter: 8.0, ..Bore::default() };
    let c = Clearance { shaft_extension: 15.0, radial: 0.3, ..Clearance::default() };
    let s = Spec { bore, bore2: bore, clearance_body: Some(c.clone()), ..Spec::defaults_for(Kind::Pair) };
    let p = planned(&s);
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.005);
    for b in &p.bodies {
        let pts = Solid::new(b, 0.1).material_points(0.2);
        let (zlo, zhi) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q[2]), a.1.max(q[2])));
        let r = 4.0 + c.radial;
        for z in [zlo - 14.9, zhi + 14.9, zlo - 5.0] {
            assert!(keep.contains(b.frame.world([r - 0.02, 0.0, z])), "{} z {z}", b.name);
            assert!(!keep.contains(b.frame.world([r + 0.1, 0.0, z])), "{} z {z}", b.name);
        }
        assert!(!keep.contains(b.frame.world([0.0, 0.0, zhi + 15.2])), "{} runs on", b.name);
        assert!(!keep.contains(b.frame.world([0.0, 0.0, zlo - 15.2])), "{} runs on", b.name);
    }
    // An integral shaft is taken as built: no more shaft past its end.
    let sh = crate::hub::Shaft { diameter: 6.0, length: 12.0, ..crate::hub::Shaft::default() };
    let s = Spec { shaft_front: sh, clearance_body: Some(c.clone()), ..Spec::defaults_for(Kind::Spur) };
    let p = planned(&s);
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.005);
    let f = p.bodies[0].frame;
    assert!(keep.contains(f.world([3.0 + c.radial - 0.02, 0.0, 10.0 + 11.0])));
    assert!(!keep.contains(f.world([0.0, 0.0, 10.0 + 12.0 + c.axial + 0.6])));
    assert!(!keep.contains(f.world([0.0, 0.0, -c.axial - 0.6])), "no shaft on the back");
}

#[test]
fn bearing_seats_sit_at_their_dots() {
    let bore = Bore { kind: BoreKind::Round, diameter: 8.0, ..Bore::default() };
    let base = Spec { bore, bore2: bore, ..Spec::defaults_for(Kind::Pair) };
    let bare = planned(&base);
    let f2 = bare.bodies[1].frame;
    // A world point beside gear 2's axis, 18 mm below the middle of its teeth.
    let dot = f2.world([0.4, -0.3, 5.0 - 18.0]);
    let c = Clearance {
        bearing: Some("608".into()),
        bearing_fit: 0.1,
        seats: vec![Seat::Along { member: 1, along: -20.0 }, Seat::Along { member: 2, along: 18.0 }, Seat::Point(dot)],
        ..Clearance::default()
    };
    let s = Spec { clearance_body: Some(c.clone()), ..base.clone() };
    let p = planned(&s);
    assert!(p.warnings.iter().all(|w| !w.contains("seat")), "{:?}", p.warnings);
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.002);
    let r = (22.0 + 0.1) / 2.0;
    for (member, along) in [(0usize, -20.0), (1, 18.0), (1, -18.0)] {
        let f = p.bodies[member].frame;
        let centre = 5.0 + along;
        for k in 0..12 {
            let a = TAU * f64::from(k) / 12.0;
            let at = |rr: f64, z: f64| f.world([rr * a.cos(), rr * a.sin(), z]);
            assert!(keep.contains(at(r - 0.02, centre)), "seat on member {} at {along}: inside", member + 1);
            assert!(keep.contains(at(r - 0.02, centre + 3.5 - 0.02)));
            assert!(keep.contains(at(r - 0.02, centre - 3.5 + 0.02)));
            assert!(!keep.contains(at(r + 0.1, centre)), "seat on member {} at {along}: too big", member + 1);
            assert!(!keep.contains(at(r - 0.3, centre + 3.5 + 0.1)), "seat on member {} at {along}: too wide", member + 1);
            assert!(!keep.contains(at(r - 0.3, centre - 3.5 - 0.1)));
        }
    }
    assert!(p.facts.iter().any(|f| f.label == "clearance bearing 608 outside diameter" && f.value == 22.0));
}

#[test]
fn seat_warnings_say_what_is_wrong() {
    let bore = Bore { kind: BoreKind::Round, diameter: 10.0, ..Bore::default() };
    let c = Clearance { bearing: Some("608".into()), seats: vec![Seat::Along { member: 1, along: 100.0 }, Seat::Along { member: 1, along: 0.0 }], ..Clearance::default() };
    let s = Spec { bore, clearance_body: Some(c), ..Spec::defaults_for(Kind::Spur) };
    let p = planned(&s);
    let has = |t: &str| p.warnings.iter().any(|w| w.contains(t));
    assert!(has("seat 1") && has("beyond its shaft"), "{:?}", p.warnings);
    assert!(has("seat 2 overlaps the teeth"), "{:?}", p.warnings);
    assert!(has("has a 8 mm bore but") && has("Ø10"), "{:?}", p.warnings);
    // A member the set doesn't have is an error that lists the members.
    let c = Clearance { bearing: Some("608".into()), seats: vec![Seat::Along { member: 3, along: 0.0 }], ..Clearance::default() };
    let e = plan(&Spec { clearance_body: Some(c), ..Spec::defaults_for(Kind::Pair) }).unwrap_err();
    assert!(e.contains("member 3") && e.contains("1 Gear z20"), "{e}");
    // Seats without a bearing, an unknown bearing.
    let c = Clearance { seats: vec![Seat::Along { member: 1, along: 0.0 }], ..Clearance::default() };
    assert!(plan(&Spec { clearance_body: Some(c), ..Spec::defaults_for(Kind::Spur) }).unwrap_err().contains("need a bearing"));
    let c = Clearance { bearing: Some("9999".into()), ..Clearance::default() };
    assert!(plan(&Spec { clearance_body: Some(c), ..Spec::defaults_for(Kind::Spur) }).unwrap_err().contains("9999"));
    let c = Clearance { radial: -1.0, ..Clearance::default() };
    assert!(plan(&Spec { clearance_body: Some(c), ..Spec::defaults_for(Kind::Spur) }).is_err());
}

#[test]
fn rings_can_be_part_of_the_housing() {
    let s = on(Kind::Planetary);
    let p = planned(&s);
    let ring = p.bodies.last().unwrap();
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.01);
    // The ring's rim (outside the planets' orbit) is not cleared by default ...
    let rim = Solid::new(ring, 1.0).radius;
    assert!(!keep.contains(ring.frame.world([rim - 0.5, 0.0, 5.0])));
    // ... and is when the ring is a part of its own.
    let s2 = Spec { clearance_body: Some(Clearance { ring_as_housing: false, ..Clearance::default() }), ..s.clone() };
    let p2 = planned(&s2);
    let keep2 = Solid::new(p2.clearance.as_ref().unwrap(), 0.01);
    assert!(keep2.contains(ring.frame.world([rim - 0.5, 0.0, 5.0])));
    contains_every_member(&s2, &p2, 0.01);
    // The planets go round with the carrier: the whole orbit is clear.
    let planet = &p.bodies[1];
    let a = (planet.frame.origin[0].powi(2) + planet.frame.origin[1].powi(2)).sqrt();
    let rp = Solid::new(planet, 1.0).radius;
    for k in 0..24 {
        let t = TAU * f64::from(k) / 24.0;
        assert!(keep.contains([(a + rp - 0.2) * t.cos(), (a + rp - 0.2) * t.sin(), 5.0]));
    }
    // A ring gear on its own is cleared anyway, with a warning.
    let p = planned(&on(Kind::Internal));
    assert!(p.clearance.is_some());
    assert!(p.warnings.iter().any(|w| w.contains("only a ring gear")), "{:?}", p.warnings);
}

#[test]
fn chains_and_belts_have_their_run_cleared() {
    for kind in [Kind::Sprocket, Kind::Pulley] {
        let s = Spec { teeth2: 30, clearance_body: Some(Clearance::default()), ..Spec::defaults_for(kind) };
        let p = planned(&s);
        assert_eq!(p.bodies.len(), 2);
        let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.01);
        let (o1, o2) = (p.bodies[0].frame.origin, p.bodies[1].frame.origin);
        let pts = Solid::new(&p.bodies[0], 0.1).material_points(0.3);
        let zmid = pts.iter().map(|q| q[2]).sum::<f64>() / pts.len() as f64;
        // Midway between the wheels, a little above the tangent of the larger, it is clear:
        // the run.
        let mid = [(o1[0] + o2[0]) / 2.0, (o1[1] + o2[1]) / 2.0, o1[2] + zmid];
        assert!(keep.contains(mid), "{kind:?}: the middle of the run");
        let r2 = p.preview.iter().find(|g| g.name.ends_with(" 2")).unwrap().pitch_radius;
        assert!(keep.contains([mid[0], mid[1] + 0.7 * r2, mid[2]]), "{kind:?}");
        assert!(!keep.contains([mid[0], mid[1] + 1.5 * r2 + 10.0, mid[2]]), "{kind:?}");
        assert!(p.facts.iter().any(|f| f.label.contains("run width")), "{kind:?}");
        contains_every_member(&s, &p, 0.01);
    }
}

#[test]
fn placed_and_partial_sets_clear_the_whole_set_where_it_is() {
    let place = Frame { origin: [10.0, -20.0, 30.0], x: [0.0, 1.0, 0.0], z: [1.0, 0.0, 0.0] };
    let s = Spec { placement: Some(place), ..on(Kind::Bevel) };
    let p = planned(&s);
    contains_every_member(&s, &p, 0.01);
    // One member built: the clearance is still the whole set's.
    let one = planned(&Spec { only: 1, ..s.clone() });
    assert_eq!(one.bodies.len(), 1);
    assert_eq!(one.clearance, p.clearance);
    let worm = Spec { only: 2, ..on(Kind::Worm) };
    assert_eq!(planned(&worm).clearance, planned(&Spec { only: 0, ..worm.clone() }).clearance);
}

#[test]
fn racks_get_a_slot_for_their_travel() {
    let s = Spec { clearance_body: Some(Clearance { travel: 40.0, ..Clearance::default() }), ..Spec::defaults_for(Kind::Rack) };
    let p = planned(&s);
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.01);
    let pts = Solid::new(&p.bodies[0], 0.1).material_points(0.5);
    let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q[0]), a.1.max(q[0])));
    let y = pts.iter().map(|q| q[1]).sum::<f64>() / pts.len() as f64;
    assert!(keep.contains([hi + 19.9, y, 5.0]) && keep.contains([lo - 19.9, y, 5.0]));
    assert!(!keep.contains([hi + 21.0, y, 5.0]) && !keep.contains([lo - 21.0, y, 5.0]));
    contains_every_member(&s, &p, 0.01);
}

#[test]
fn the_bearing_library_is_sound() {
    for (name, d, od, w) in BEARINGS {
        assert!(*d > 0.0 && od > d && *w > 0.0 && *w < *od, "{name}");
        assert_eq!(BEARINGS.iter().filter(|b| b.0.eq_ignore_ascii_case(name)).count(), 1, "{name}");
    }
    assert_eq!(bearing("6902"), Some((15.0, 28.0, 7.0)));
    assert_eq!(bearing("mr148"), Some((8.0, 14.0, 4.0)));
    for (name, series) in [("608", "6xx"), ("MR105", "MR"), ("683", "68xx"), ("6804", "68xx"), ("6901", "69xx"), ("695", "69xx"), ("6001", "60xx"), ("6204", "62xx"), ("6301", "63xx")] {
        assert_eq!(bearing_series(name), series, "{name}");
    }
}

#[test]
fn a_custom_bearing_size_makes_seats() {
    let c = Clearance { bearing_size: Some(crate::clearance::BearingSize { bore: 0.0, od: 30.0, width: 4.0 }), seats: vec![Seat::Along { member: 1, along: -30.0 }], ..Clearance::default() };
    let s = Spec { clearance_body: Some(c), ..Spec::defaults_for(Kind::Spur) };
    let p = planned(&s);
    let keep = Solid::new(p.clearance.as_ref().unwrap(), 0.005);
    assert!(keep.contains([14.9, 0.0, -25.0]) && !keep.contains([15.2, 0.0, -25.0]));
    assert!(p.facts.iter().any(|f| f.label == "clearance bearing custom width" && f.value == 4.0));
    // The seat has no shaft to sit on (no bore, no bearing bore): said so.
    assert!(p.warnings.iter().any(|w| w.contains("no shaft")), "{:?}", p.warnings);
}

/// Writes, for each kind in `CLEARANCE_KINDS` (names, comma separated), a CLI script into the
/// folder `CLEARANCE_DUMP`: the set by `gear.create`, its clearance body built the way the
/// engine builds it (sketches, revolves and extrusions; arcs as fine polylines), and a box round
/// it all cut by the body ("Housing"). For kernel checks by hand.
#[test]
#[ignore]
fn dump_clearance_scripts() {
    let Ok(dir) = std::env::var("CLEARANCE_DUMP") else { return };
    let kinds = std::env::var("CLEARANCE_KINDS").unwrap_or_else(|_| "pair".into());
    let v3 = |v: [f64; 3]| format!("[{},{},{}]", v[0], v[1], v[2]);
    for name in kinds.split(',') {
        // "<kind>_housing": with Ø8 bores, 608 seats on both sides of each of the first two
        // members, and a box only 8 mm larger than the gears, so the shafts go through its walls.
        let (kind_name, housing) = match name.trim().strip_suffix("_housing") {
            Some(k) => (k, true),
            None => (name.trim(), false),
        };
        let kind = Kind::parse(kind_name).expect("a kind");
        let mut s = on(kind);
        let mut params = format!(r#""type":"{}""#, kind.name());
        if housing {
            let bore = Bore { kind: BoreKind::Round, diameter: 8.0, ..Bore::default() };
            s.bore = bore;
            s.bore2 = bore;
            let n = planned(&Spec { clearance_body: None, ..s.clone() }).bodies.len().min(2) as u32;
            let w = s.face_width;
            let c = s.clearance_body.as_mut().unwrap();
            c.bearing = Some("608".into());
            c.seats = (1..=n).flat_map(|m| [Seat::Along { member: m, along: -(w / 2.0 + 9.0) }, Seat::Along { member: m, along: w / 2.0 + 9.0 }]).collect();
            params.push_str(r#","bore":{"type":"round","diameter":8},"bore2":{"type":"round","diameter":8}"#);
        }
        let p = planned(&s);
        eprintln!("{name} warnings: {:?}", p.warnings.iter().filter(|w| w.contains("clearance")).collect::<Vec<_>>());
        let keep = p.clearance.as_ref().unwrap();
        let body = &keep.name;
        let mut cmds = vec![format!(r#"{{"command":"gear.create","params":{{{params}}}}}"#)];
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        let mut count = 0;
        for st in &keep.steps {
            let Step::Placed { frame, step } = st else { panic!() };
            let f = keep.frame.compose(frame);
            let op_of = |op: Op| if op == Op::New { format!(r#""operation":"new","body_name":"{body}""#) } else { format!(r#""operation":"join","targets":["{body}"]"#) };
            let (pts, origin, x, y, cmd): (Vec<[f64; 2]>, [f64; 3], [f64; 3], [f64; 3], String) = match step.as_ref() {
                Step::Revolve { profile, op } => {
                    (profile.iter().map(|q| [q.x, q.y]).collect(), f.origin, f.x, f.z, format!(r#"{{"command":"solid.revolve","params":{{"axis":"y",{}}}}}"#, op_of(*op)))
                }
                Step::Prism { region, z0, height, op, .. } => (
                    {
                        let mut l = region.outer[0].polyline(0.05);
                        if l.len() > 1 && l[0].dist(l[l.len() - 1]) < 1e-9 {
                            l.pop();
                        }
                        l.iter().map(|q| [q.x, q.y]).collect()
                    },
                    f.world([0.0, 0.0, *z0]),
                    f.x,
                    f.y(),
                    format!(r#"{{"command":"solid.extrude","params":{{"distance":{height},{}}}}}"#, op_of(*op)),
                ),
                _ => panic!(),
            };
            count += pts.len();
            for q in &pts {
                for sx in [1.0, -1.0] {
                    let w = [0, 1, 2].map(|k| origin[k] + sx * q[0] * x[k] + q[1] * y[k]);
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k] - q[0].abs());
                        hi[k] = hi[k].max(w[k] + q[0].abs());
                    }
                }
            }
            let list: Vec<String> = pts.iter().map(|q| format!("[{},{}]", q[0], q[1])).collect();
            cmds.push(format!(r#"{{"command":"sketch.create","params":{{"plane":{{"origin":{},"x_dir":{},"y_dir":{}}},"project_edges":false}}}}"#, v3(origin), v3(x), v3(y)));
            cmds.push(format!(r#"{{"command":"sketch.line","params":{{"points":[{}],"closed":true}}}}"#, list.join(",")));
            cmds.push(r#"{"command":"sketch.finish","params":{}}"#.to_string());
            cmds.push(cmd);
        }
        if housing {
            (lo, hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for b in &p.bodies {
                let solid = Solid::new(b, 0.2);
                for q in solid.material_points(1.0) {
                    let w = b.frame.world(q);
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k] - 8.0);
                        hi[k] = hi[k].max(w[k] + 8.0);
                    }
                }
            }
            lo = lo.map(|v| v - 4.0);
            hi = hi.map(|v| v + 4.0);
        }
        let size = [0, 1, 2].map(|k| hi[k] - lo[k] - 4.0);
        let centre = [0, 1, 2].map(|k| (hi[k] + lo[k]) / 2.0);
        cmds.push(format!(r#"{{"command":"solid.box","params":{{"length":{},"width":{},"height":{},"center":{},"body_name":"Housing"}}}}"#, size[0], size[1], size[2], v3(centre)));
        cmds.push(format!(r#"{{"command":"solid.combine","params":{{"target":"Housing","tools":["{body}"],"operation":"cut","keep_tools":false}}}}"#));
        cmds.push(r#"{"command":"inspect.interference","params":{"budget_s":600}}"#.to_string());
        eprintln!("{name}: {} steps, {count} outline points, box {size:?}", keep.steps.len());
        std::fs::write(format!("{dir}/{name}.json"), format!("{{\"commands\":[\n{}\n]}}\n", cmds.join(",\n"))).unwrap();
    }
}

#[test]
fn bevel_planet_clearance_is_quick_enough() {
    // Diagnostic of the planets' sweep cost: the clearance adds well under a second to the plan.
    for mode in [crate::plan::BevelPlanetMode::Differential, crate::plan::BevelPlanetMode::Ring] {
        let base = Spec { bevel_planet_mode: mode, ..Spec::defaults_for(Kind::BevelPlanetary) };
        let base = if mode == crate::plan::BevelPlanetMode::Ring { Spec { shaft_angle: 45.0, teeth: 20, teeth2: 10, planets: 3, ..base } } else { base };
        let t = std::time::Instant::now();
        let bare = planned(&base);
        let t_bare = t.elapsed();
        let t = std::time::Instant::now();
        let s = Spec { clearance_body: Some(Clearance::default()), ..base };
        let p = planned(&s);
        let t_all = t.elapsed();
        eprintln!("{mode:?}: plan {t_bare:?}, with clearance {t_all:?}");
        assert!(t_all.saturating_sub(t_bare).as_secs_f64() < 1.0, "{mode:?}: {t_bare:?} against {t_all:?}");
        assert_eq!(bare.bodies, p.bodies);
        contains_every_member(&s, &p, 0.01);
    }
}
