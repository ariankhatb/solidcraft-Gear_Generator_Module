# Clearance body: fields for the Gear Generator window

For whoever wires `crates/ui-egui/src/gear_window.rs` (not edited on `gear-keepout`). The gear
crate holds it as `Spec::clearance_body: Option<solidcraft_gear::clearance::Clearance>`. The
engine takes it as `clearance_body: {...}` in `gear.create`, `gear.calculate` and `gear.update`.
The key `clearance` was already taken: it is the tip clearance coefficient c*. The reply names
the extra body in `clearance`, and it is also the last entry of `bodies`.

Put it in its own section, "Housing clearance", after the shafts and hardware. It is off by
default. Simple mode shows only the switch and the bearing. Advanced mode shows the rest.

## Fields

| Field (`Clearance::`) | Engine key | Type | Default | Range | Label | Tooltip |
|---|---|---|---|---|---|---|
| (whole section) | `clearance_body` present | `Option<Clearance>` | `None` (off) | on/off | Make a clearance body | One more solid holding all the space the assembled set needs to turn: the teeth's swept envelopes, the shafts through the housing walls and the bearing seats. Model any housing block and cut this body out of it in one Boolean. When switched on, start from `Clearance::default()`. |
| `radial` | `radial` | f64 mm | 0.5 | 0 … 50 | Radial clearance | Gap around every tip envelope, shaft and chain or belt run. For FDM housings 0.3–0.6 mm is typical. |
| `axial` | `axial` | f64 mm | 0.5 | 0 … 50 | Axial clearance | Gap past each gear's faces along its axis. |
| `shaft_extension` | `shaft_extension` | f64 mm | 0 (= 2 × face width) | 0 … 10000 | Shaft runs on | How far each shaft's clearance runs past its gear on each side, enough to pass through a housing wall. 0 means twice the face width. Integral shafts (Shafts section) are taken as built. A shaft's diameter is the bore's (across the corners for hex, square, key or flat). With no bore it is the bearing's bore. With neither, there is no shaft clearance. |
| `ring_as_housing` | `ring_as_housing` | bool | true | – | Ring is part of the housing | Internal ring gears (planetary ring, ring gear, cycloidal drive pin ring) are left out of the clearance because the housing carries them. Switch off to clear the ring's outer rim as a separate part. Show only for internal, planetary and cycloidal drive. |
| `bearing` | `bearing: "608"` | `Option<String>` | None | a name in `hub::BEARINGS` | Bearing | One bearing size for every seat. Use a combo grouped by `hub::bearing_series(name)` (MR, 6xx, 68xx, 69xx, 60xx, 62xx, 63xx), showing "608 — 8 × 22 × 7". Add a "custom…" entry that shows the three fields below. |
| `bearing_size.od` | `bearing: {od, width, bore?}` | f64 mm | – | > 0, ≤ 2000 | Bearing OD | Custom bearing outside diameter (only when `bearing` is None). |
| `bearing_size.width` | (same) | f64 mm | – | > 0, ≤ 1000 | Bearing width | Custom bearing width. |
| `bearing_size.bore` | (same) `bore?` | f64 mm | 0 (unknown) | 0 … < OD | Bearing bore | Custom bearing bore; used to check it against the shaft (0 skips the check). |
| `bearing_fit` | `bearing_fit` | f64 mm | 0 | −1 … 5 | Seat fit | How much larger than the bearing's OD each seat is, on the diameter. Use 0.1–0.2 for a printed slip fit and 0 or negative for a press fit. |
| `seats` | `seats: [{member, along} \| {point: [x,y,z]}]` | `Vec<Seat>` | empty | ≤ 64 seats; needs a bearing | Bearing seats | Dots on the shafts. Each seat is a cylinder of the bearing's OD + fit and its width, centred on the dot. |
| `Seat::Along.member` | `member` | u32 | 1 | 1 … number of bodies | Member | Which member's shaft: the set's bodies in order (gear 1 first; planetary: sun, planets, ring; worm: wheel, then worm). Use a combo of the body names. |
| `Seat::Along.along` | `along` | f64 mm | e.g. ±(face width / 2 + bearing width) | ±100000 | Along the shaft | Signed distance along the member's axis from the middle of its teeth. Negative values are behind the gear (the back face side). |
| `Seat::Point` | `point` | [f64; 3] mm | – | finite | Point | A picked point (a sketch point, say). It is moved onto the nearest member's axis. Planets and racks are never chosen. |
| `travel` | `travel` | f64 mm | 0 (= rack length) | 0 … 100000 | Rack travel | How far the rack slides in all. Its slot is the rack's box made this much longer, half each way. Show only for rack. |

## What the window should show

- In the 3D preview: the body as a translucent volume in a neutral colour (it is not a member).
  Each seat is a small coloured ring on its member's axis, in the member's colour. Hovering a seat
  row highlights its ring.
- Seats list: a table with one row per seat. Columns: member combo, along value, a delete button.
  Below it, "Add seat", which adds `{member: 1, along: -(face/2 + bearing width)}`. Later, a
  "pick point" mode that adds `Seat::Point`.
- Facts appear in the facts list as they are, under labels that start with "clearance:" or
  "clearance bearing". There is an envelope diameter and length per member ("orbit" for planets
  and a cycloid disc), a shaft diameter, a slot length and travel for a rack, a chain or belt run
  width and height, the bearing's bore, OD and width, and each seat's position.
- Warnings that start with "clearance:" belong in this section: a seat beyond its shaft, a seat on
  the teeth, a bearing bore ≠ shaft Ø, a seat on a planet, rack or skipped ring, a bearing chosen
  with no seats, and a set that is only a ring.

## Defaults when switched on

`Clearance::default()`: radial 0.5, axial 0.5, shaft extension 0 (2 × face width), ring as
housing on, no bearing, fit 0, no seats, travel 0 (rack length).
