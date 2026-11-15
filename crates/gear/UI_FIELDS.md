# Gear body fields for the Gear Generator window

Every `Spec` field the gear-body work added (`crates/gear/src/plan.rs`, `body.rs`, `hub.rs`),
for building the window's controls. Lengths are mm, angles degrees. Every field is **off at its
default**: a spec that sets none of them plans exactly as before. The engine's `gear.create`
params use the same names (nested objects for `Chamfer`, `Shaft`, `Hardware`; see
`crates/engine/src/schema/gear.rs`). The ranges are the ones `plan::check` / `body::check`
enforce: outside them, planning fails with the message given. Inside them, a feature that doesn't
fit is left out or cut down with a warning, and the rest of the gear is still planned.

"m" is the module. For sprockets and pulleys the body code uses pitch / π as the module.

Kind shorthand, used in the "applies to" columns:

- **cyl**: Spur, Helical, Herringbone
- **pair**: Pair (gear 1 and gear 2)
- **plan.**: Planetary (sun = gear 1, planets = gear 2, ring)
- **hub kinds**: every kind whose members are built through `hub_steps`: cyl, Pair, Planetary
  (sun, planets), Crossed helical, Worm (the wheel), Cycloidal, Sprocket, Pulley, Face gear (the
  pinion only, as gear 2), Elliptical (through `hub_steps_tip`)

Hide (or grey out) a control when the kind it applies to isn't selected. If a field is set on a
kind that doesn't take it, the plan warns once per group (see "What each kind takes" below), so
leaving it set can't break anything. `plan::left_out::uses(&spec, Opt::…)` answers, for the
spec's kind, whether a control does anything: use it to hide or grey out controls.

## Tooth tuning

| field | type | default | range | label | tooltip | applies to |
|---|---|---|---|---|---|---|
| `tip_round` | f64 (modules) | 0 | 0 to 0.5 | Tip rounding | Rounds each tip corner with an arc tangent to the flank and the tip circle. The radius is in modules. When the tip is too thin it is cut down, with a warning. | cyl, Pair, Planetary (sun, planets) |
| `xy_compensation` | f64 mm | 0 | 0 to 0.25·m | Horizontal expansion compensation | Draws the teeth this far inside where the printed teeth should end up (flanks, tips and roots). Set it to your slicer's measured over-extrusion per side. Tune it by measuring the "span over k teeth" fact on a printed gear. | involute kinds that use the spec's rack: cyl, Internal, Rack, Pair, Planetary, Crossed helical (not Hypoid: its teeth come from the ring gear's form) |
| `elephant_foot` | f64 mm | 0 | 0 to 0.25·m | Elephant-foot relief | Draws the first layers of the teeth this far inside the rest, so the squashed first layers don't stand proud of the flanks. | cyl, Pair, Planetary (sun, planets) |
| `elephant_foot_height` | f64 mm | 0 (means 0.2) | ≥ 0 | Elephant-foot height | How high the relief runs from the back face (the face on the print bed). This is usually the first layer height. | as `elephant_foot` |
| `nozzle` | f64 mm | 0 (no check) | ≥ 0 | Nozzle width | Warns when a tip land is narrower than two nozzle widths, which is too thin to print cleanly. This is only a check: it doesn't change the geometry. | cyl, Pair, Planetary (sun, planets) |
| `clearance` | f64 (c*) | 0 (the rack's) | 0 to 1 | Tip clearance c* | Overrides the rack's tip clearance: dedendum = addendum + c*. The cutter must still have a tip, or planning fails ("pointed teeth: lower one of them"). | as `xy_compensation` |
| `tip_shortening` | f64 mm | 0 | 0 to 0.5·addendum·m | Tip shortening | Takes this much off every gear's tip radius, on top of what a shifted pair needs. | as `xy_compensation` |
| `tip_diameter` | f64 mm | 0 (from the rack) | ≥ 0 | Tip diameter (gear 1) | The finished tip diameter. It only lowers the tips: a value above the teeth's own diameter, or one that leaves under ¼ module of addendum, is ignored with a warning. | cyl, Pair (gear 1) |
| `tip_diameter2` | f64 mm | 0 | ≥ 0 | Tip diameter (gear 2) | Gear 2's finished tip diameter. Only lowers the tips, as for gear 1. | Pair (gear 2) |
| `tip_relief` | f64 mm | 0 | 0 to 0.1·m | Tip relief | How far the flank falls away from the involute at the tip, measured normal to the flank. It grows linearly over `tip_relief_height`. It eases teeth into mesh under load. | cyl, Pair, Planetary (sun, planets) |
| `tip_relief_height` | f64 mm | 0 (means 0.4·m) | ≥ 0 | Tip relief height | The radial height, down from the tip, that the relief runs over. | as `tip_relief` |
| `crowning` | f64 mm | 0 | 0 to 0.1·m | Crowning | Each flank is this much thinner at the two faces than at mid face, so slightly misaligned shafts still bear mid face. Straight teeth only. | Spur, Pair, Planetary, all with helix 0 |
| `backlash2` | Option<f64> mm | None (same as `backlash`) | ≥ 0 | Gear 2 backlash | Gear 2's own backlash (the mate, the planets, the worm's thread). The mesh's backlash is the two added together. | Pair, Planetary, Crossed helical, Worm |
| `face_width2` | f64 mm | 0 (same as `face_width`) | ≥ 0 | Gear 2 face width | Gear 2's own face width. | Pair |
| `face_offset2` | f64 mm | 0 | any finite value | Gear 2 face offset | How far gear 2's mid plane sits from gear 1's along the axis. If the faces don't overlap, the plan is refused. | Pair |

Combined limit (involute kinds except Spline): `max(backlash, backlash2)` plus `2·(xy_compensation +
elephant_foot) / cos α / cos β` must be under π·m/4. The cutter must also keep a tip with
dedendum = rack dedendum + clearance override + `xy_compensation/m` + `elephant_foot/m`.

## Chamfers

`Chamfer { size, angle }`. `size` is measured radially across the face (0: none). `angle` is
measured to the face, so the chamfer runs `size·tan(angle)` along the axis.

| field | type | default | range | label | tooltip | applies to |
|---|---|---|---|---|---|---|
| `tooth_chamfer_front.size` | f64 mm | 0 | ≥ 0 | Front tooth chamfer | Chamfers the tooth edges at the tip circle on the front face (z = face width, the hub's side). It is cut down to fit, with a warning. | hub kinds except Elliptical, plus the Internal and Planetary ring |
| `tooth_chamfer_front.angle` | f64 ° | 45 | 5 to 85 (checked only when size > 0) | Front chamfer angle | The angle to the face: 45° is a plain chamfer, a smaller angle is shallower. | as above |
| `tooth_chamfer_back.size` | f64 mm | 0 | ≥ 0 | Back tooth chamfer | The same on the back face (z = 0, on the bed). It helps against elephant foot. | as above |
| `tooth_chamfer_back.angle` | f64 ° | 45 | 5 to 85 | Back chamfer angle | As for the front chamfer. | as above |
| `bore_chamfer` | f64 mm | 0 | ≥ 0 | Bore chamfer | A 45° chamfer on both ends of the bore. It ends where the bore comes out: the shaft end, the hub or the face. | hub kinds (gear 1 and gear 2 bores) |
| `hub_chamfer` | f64 mm | 0 | ≥ 0 | Hub chamfer | A 45° chamfer on the hub's outer end edge. It needs a hub. | hub kinds |

Chamfers are not built on bevel gears (a warning says so: their ends are cones).

## Integral shafts

There are four shafts: `shaft_front`, `shaft_back` (gear 1) and `shaft2_front`, `shaft2_back`
(gear 2: the mate, the planets, the worm). Each is a `Shaft` struct. The front shaft grows from
the front face along +z, through the hub if there is one. The back shaft grows from the back face
along −z. Lengths are measured from the gear's face. Without a diameter or a length there is no
shaft.

| field | type | default | range | label | tooltip | applies to |
|---|---|---|---|---|---|---|
| `diameter` | f64 mm | 0 | ≥ 0 | Shaft diameter | The diameter next to the gear. If it is bigger than the root circle, a warning says it buries the ends of the teeth. | hub kinds, Bevel, Spiral bevel; the worm takes `shaft2_*` |
| `length` | f64 mm | 0 | ≥ 0 | Shaft length | Measured from the gear's face. | as above |
| `step_diameter` | f64 mm | 0 (no step) | ≥ 0 | Step diameter | A second length beyond the first: smaller for a shoulder, larger for a collar. | as above |
| `step_length` | f64 mm | 0 | ≥ 0 | Step length | The second length (both step fields must be > 0). | as above |
| `end` | `ShaftEnd` | `Plain` | plain, d_flat, keyway, hex, cross, centre_hole | End feature | What the free end carries. | as above |
| `end_length` | f64 mm | 0 (means 1.5 × end Ø; a centre hole: 1.5 × its own Ø) | ≥ 0 | End feature length | How far back from the free end the feature runs. For a centre hole this is its depth. | as above |
| `end_size` | f64 mm | 0 (from the diameter) | ≥ 0 | End feature size | D flat: the size across the flat (default 0.9·d). Keyway: the slot width (default DIN 6885). Hex: across flats (default 0.85·d). Cross: arm width (default d/3). Centre hole: its diameter (default 0.3·d). | as above |
| `chamfer` | f64 mm | 0 | ≥ 0 | End chamfer | A 45° chamfer on the free end. | as above |

An end feature that doesn't fit is left plain with a warning. A cross or a centre hole can't be
combined with a bore.

## Hub hardware and bearing pockets

`hardware` is for gear 1 and `hardware2` for gear 2. Each is a `Hardware` struct. Everything is
off at 0.

| field | type | default | range | label | tooltip | applies to |
|---|---|---|---|---|---|---|
| `set_screw` | f64 mm | 0 | ≥ 0, under 0.9 × hub length | Set screw Ø | A radial hole through the hub wall at mid length, over the key or flat (+y). | hub kinds (needs a hub) |
| `set_screws` | u32 | 1 | ≤ 4 (used as 1 to 4) | Set screws | How many set screws, 90° apart. The nut traps and inserts follow them. | as above |
| `nut_width` | f64 mm | 0 | ≥ 0 | Nut trap width | The captive nut's width across flats, as a slot open to the hub's end, on each set screw. | as above |
| `nut_thickness` | f64 mm | 0 (means 0.45 × width) | ≥ 0, under 0.7 × hub wall | Nut thickness | — | as above |
| `insert_diameter` | f64 mm | 0 | ≥ 0, under 0.95 × hub length | Heat-set insert Ø | A pocket at the mouth of each set-screw hole. | as above |
| `insert_depth` | f64 mm | 0 (means 0.8 × hub wall) | ≥ 0 | Insert depth | — | as above |
| `clamp_slit` | f64 mm | 0 | ≥ 0 | Clamp slit width | A slit through the hub wall, opposite the set screw, down to the gear's face. It needs a bore wider than the slit. | as above |
| `clamp_screw` | f64 mm | 0 | ≥ 0 | Clamp screw Ø | A hole across the slit. It needs a clamp slit. | as above |
| `bearing_od` | f64 mm | 0 | ≥ 0 | Bearing OD | A bearing pocket in the back face. Pick a size from the `BEARINGS` list (`Hardware::set_bearing("608")`; the engine param is `bearing: "608"`) or type it in. | hub kinds (no hub needed) |
| `bearing_width` | f64 mm | 0 | ≥ 0 | Bearing width | The pocket's depth. | as above |
| `bearing_fit` | f64 mm | 0 | ≥ 0 | Bearing fit allowance | How much bigger than the bearing the pocket is made, on the diameter, for a printed fit. | as above |

`BEARINGS` (`hub.rs`): MR63, MR84, MR105, MR115, MR126, MR128, 623, 624, 625, 626, 608, 688,
6800 to 6804, 6000 to 6004, 6200 to 6202 (name, bore, OD, width).

## The print preset: `Spec::apply_print_defaults(nozzle, layer_height)`

Arguments are in mm, usually 0.4 and 0.2. A value that isn't a positive number falls back to
0.4 or 0.2. Values that compare with `max` never lower what the user already set.

| field | set to | for kinds |
|---|---|---|
| `rack` | `print_fdm` (addendum 1.0, dedendum 1.3, root radius 0.38) | involute kinds with `tuned_teeth()` (unchanged) |
| `pressure_angle` | max(current, 25°) | same |
| `nozzle` | nozzle | kinds that use it (`left_out::uses`) |
| `tip_round` | clamp(0.5·nozzle / m, 0.05, 0.25) modules | kinds that use it (`left_out::uses`) |
| `xy_compensation` | min(0.125·nozzle, 0.05·m) | kinds that use it (`left_out::uses`) |
| `elephant_foot` | min(0.5·layer, 0.05·m) | kinds that use it (`left_out::uses`) |
| `elephant_foot_height` | layer | kinds that use it (`left_out::uses`) |
| `backlash` | max(current, min(0.25·nozzle + 0.05, 0.3·m)) | all kinds (for a spline this is its flank clearance) |
| `bore_chamfer` | max(current, layer) | kinds that use it (not Spline, Internal, Rack, bevels, Hypoid, Cycloidal drive) |
| `hardware.bearing_fit`, `hardware2.bearing_fit` | max(current, 0.25·nozzle) | all kinds |

`Kind::tuned_teeth()` is false for Sprocket, Pulley, Spline, Face gear and Elliptical, and true
for every other kind. Show a "Print preset" button with nozzle and layer inputs. After it
applies, the user can still edit every field.

## Extras kinds (Sprocket, Pulley, Spline, Face gear, Elliptical)

| kind | tooth tuning | shafts and hardware | tooth chamfers | bore and hub chamfers |
|---|---|---|---|---|
| Sprocket | left out | gear 1 and gear 2 (driven sprocket) | yes (at the tip circle) | yes |
| Pulley | left out | gear 1 and gear 2 | yes (with flanges, at the flange's outer edge) | yes |
| Spline | left out | left out (the shaft and hub are the spline itself; `hub_diameter` is the hub part's OD) | left out | left out |
| Face gear | left out (generated from the plain rack) | the pinion takes `shaft2_*` and `hardware2`; the face gear takes none | pinion only | pinion only |
| Elliptical | left out | gear 1 and gear 2 (hub, lightening holes, shafts, hardware, bearing pocket) | left out (the tips are no circle) | yes |

Whatever is set and left out produces one warning per group, and only when it was set. For
example: "chain sprocket: tip rounding, crowning don't apply to its teeth (they have their own
form); left out".

## What each kind takes (`plan::left_out`)

Every kind answers for every option in `left_out::why_left_out` (one `match`, no catch-all, so a
new kind must too); the test `every_type_uses_or_names_every_option` checks each answer against
the planners. Gear 2's options on a single gear (and on a sprocket, pulley or cycloidal pair with
mating teeth 0) are left out with "a single gear has no gear 2" / "there is no gear 2".

| kind | tooth tuning taken | body options taken |
|---|---|---|
| cyl | all of gear 1's | all of gear 1's |
| Pair | all | all |
| Planetary | all but the tip diameters and gear 2's face width/offset | all (sun = gear 1, planets = gear 2; the ring takes tooth chamfers) |
| Internal | tip clearance, tip shortening, XY compensation | tooth chamfers only |
| Rack | tip clearance, tip shortening, XY compensation | none |
| Bevel, Spiral bevel, Bevel planetary | none | bores, hub, shafts (no holes, hardware or chamfers) |
| Hypoid | none | bores, hub diameter and length (the pinion's shank) |
| Crossed helical | tip clearance, tip shortening, XY compensation, gear 2's backlash | all |
| Worm | gear 2's (the thread's) backlash | all of the wheel's; the worm's bore and shafts (no hardware) |
| Cycloidal | none | all |
| Cycloidal drive | none | bore and holes (output holes) |
| extras | none | see the table above |

## Behaviour notes for the window

- **Shaft + bore = hollow shaft.** A bore runs the whole length, through the shafts and the hub.
  The plan warns "the bore runs through the … shaft (a hollow shaft)". If the bore leaves under
  10 % of wall in a shaft, that shaft is left out with a warning. A cross or centre-hole end
  can't go on a hollow shaft.
- **The tip diameter only lowers.** `tip_diameter` and `tip_diameter2` are finished diameters
  (`xy_compensation` still comes off them). A value above the teeth's own tip diameter, or one
  that leaves under ¼ module of addendum, is ignored with a warning. 0 means "from the rack".
- **Crowning is for straight teeth only.** On helical or herringbone teeth it is left out with a
  warning. It turns the teeth into a loft through three sections.
- **Hardware needs a hub.** Set screws, nut traps, inserts and clamp slits go in the hub
  (`hub_diameter` and `hub_length` > 0). Without one they are left out with a warning that names
  the hub fields. The bearing pocket doesn't need a hub. It goes in the back face and is left out
  when there is a back shaft or when it doesn't fit inside the root circle less a module. A clamp
  slit needs a bore.
- **The front shaft starts at the gear's face, not the hub's end.** A front shaft no longer and
  no thicker than the hub is hidden inside it, and a warning says so.
- **Gear 2 detection.** `hub_steps` / `hub_steps_tip` decide which member they are building by
  `std::ptr::eq(bore, &s.bore2)`: passing `&s.bore2` selects `shaft2_*` and `hardware2`.
  Anything that hands it a *copy* of the spec has to pass that copy's own `bore2`. Sprockets and
  pulleys do this with their module-adjusted copy.
- **Test coupons** (`coupon`, Pair and Planetary) keep the tooth tuning. Crowned and
  elephant-foot teeth are cut from the gear's own steps, as helical ones are. Shafts, chamfers,
  hardware, bearing pockets and gear 2's face width and offset are left off, with one note when
  any was set.
- **Defaults are all zero (off).** Show 0 as "off" or "auto" according to each field's default
  column. Those with a 0 meaning: `elephant_foot_height`, `tip_relief_height`, `clearance`,
  `tip_diameter(2)`, `face_width2`, `end_length`, `end_size`, `nut_thickness`, `insert_depth`.
