//! Bores (round, keyed to DIN 6885, D flat, hex, splined later), hubs and lightening holes.

use std::f64::consts::{PI, TAU};

use crate::geom::{Outline, Seg, V2};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BoreKind {
    #[default]
    None,
    Round,
    /// A parallel key per DIN 6885 (width and depth from the bore unless given).
    Keyway,
    /// A round bore with a flat (motor shafts): `flat` is the distance from the flat to the far
    /// side of the bore.
    DFlat,
    /// Hexagonal: `diameter` across flats.
    Hex,
    /// Square: `diameter` across flats.
    Square,
}

impl BoreKind {
    pub const ALL: [(BoreKind, &'static str); 6] = [
        (BoreKind::None, "none"),
        (BoreKind::Round, "round"),
        (BoreKind::Keyway, "keyway"),
        (BoreKind::DFlat, "d_flat"),
        (BoreKind::Hex, "hex"),
        (BoreKind::Square, "square"),
    ];
    pub fn name(self) -> &'static str {
        Self::ALL.iter().find(|(k, _)| *k == self).map(|(_, n)| *n).unwrap_or("none")
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().find(|(_, n)| *n == s).map(|(k, _)| *k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Bore {
    pub kind: BoreKind,
    pub diameter: f64,
    /// Keyway width and depth into the hub (0: from DIN 6885).
    pub key_width: f64,
    pub key_depth: f64,
    /// D flat: distance across from the flat to the bore's far side (0: 0.9 × diameter).
    pub flat: f64,
}

/// DIN 6885 A: (bore over, bore up to, key width b, hub groove depth t2).
const DIN6885: &[(f64, f64, f64, f64)] = &[
    (6.0, 8.0, 2.0, 1.0),
    (8.0, 10.0, 3.0, 1.4),
    (10.0, 12.0, 4.0, 1.8),
    (12.0, 17.0, 5.0, 2.3),
    (17.0, 22.0, 6.0, 2.8),
    (22.0, 30.0, 8.0, 3.3),
    (30.0, 38.0, 10.0, 3.3),
    (38.0, 44.0, 12.0, 3.3),
    (44.0, 50.0, 14.0, 3.8),
    (50.0, 58.0, 16.0, 4.3),
    (58.0, 65.0, 18.0, 4.4),
    (65.0, 75.0, 20.0, 4.9),
    (75.0, 85.0, 22.0, 5.4),
    (85.0, 95.0, 25.0, 5.4),
    (95.0, 110.0, 28.0, 6.4),
    (110.0, 130.0, 32.0, 7.4),
    (130.0, 150.0, 36.0, 8.4),
    (150.0, 170.0, 40.0, 9.4),
    (170.0, 200.0, 45.0, 10.4),
];

/// The DIN 6885 key width and hub groove depth for a bore, if the table covers it.
pub fn din6885(d: f64) -> Option<(f64, f64)> {
    DIN6885.iter().find(|(lo, hi, _, _)| d > *lo && d <= *hi).map(|(_, _, b, t2)| (*b, *t2)).or_else(|| {
        // Below 6 mm: scale the smallest key.
        (d > 2.0 && d <= 6.0).then_some((d / 3.0, d / 6.0))
    })
}

impl Bore {
    /// The bore's outline, centred at the origin, the key or flat towards +y.
    pub fn outline(&self) -> Option<Outline> {
        let r = self.diameter / 2.0;
        if r <= 0.0 {
            return None;
        }
        Some(match self.kind {
            BoreKind::None => return None,
            BoreKind::Round => Outline::circle(V2::ZERO, r),
            BoreKind::Keyway => {
                let (b0, t0) = din6885(self.diameter).unwrap_or((self.diameter / 4.0, self.diameter / 8.0));
                let w = if self.key_width > 0.0 { self.key_width } else { b0 }.min(self.diameter * 0.9);
                let t = if self.key_depth > 0.0 { self.key_depth } else { t0 };
                let yb = (r * r - w * w / 4.0).max(0.0).sqrt();
                let top = r + t;
                let (p1, p2, p3, p4) = (V2::new(w / 2.0, yb), V2::new(w / 2.0, top), V2::new(-w / 2.0, top), V2::new(-w / 2.0, yb));
                Outline { segs: vec![Seg::Line(p1, p2), Seg::Line(p2, p3), Seg::Line(p3, p4), Seg::Arc { c: V2::ZERO, a: p4, b: p1, ccw: true }] }
            }
            BoreKind::DFlat => {
                let across = if self.flat > 0.0 { self.flat } else { 0.9 * self.diameter }.clamp(0.05 * self.diameter, self.diameter * 0.999);
                let yf = across - r;
                let x = (r * r - yf * yf).max(0.0).sqrt();
                let (a, b) = (V2::new(x, yf), V2::new(-x, yf));
                Outline { segs: vec![Seg::Line(a, b), Seg::Arc { c: V2::ZERO, a: b, b: a, ccw: true }] }
            }
            BoreKind::Hex | BoreKind::Square => {
                let n = if self.kind == BoreKind::Hex { 6 } else { 4 };
                let rc = r / (PI / f64::from(n)).cos();
                let pts: Vec<V2> = (0..n).map(|k| V2::polar(rc, PI / 2.0 - PI / f64::from(n) + TAU * f64::from(k) / f64::from(n))).collect();
                Outline { segs: (0..pts.len()).map(|i| Seg::Line(pts[i], pts[(i + 1) % pts.len()])).collect() }
            }
        })
    }
    /// The radius the bore reaches out to (keyway or corners included).
    pub fn reach(&self) -> f64 {
        match self.kind {
            BoreKind::None => 0.0,
            _ => self.outline().map(|o| o.max_radius()).unwrap_or(0.0),
        }
    }
}

/// Round lightening holes on a circle between the hub and the rim.
pub fn lightening_holes(count: u32, diameter: f64, at_radius: f64, phase: f64) -> Vec<Outline> {
    (0..count).map(|k| Outline::circle(V2::polar(at_radius, phase + TAU * f64::from(k) / f64::from(count.max(1))), diameter / 2.0)).collect()
}
