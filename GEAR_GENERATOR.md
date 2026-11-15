# SolidCraft Gear Generator

A gear generator built into [SolidCraft](https://github.com/tangency-labs/solidcraft), the open-source
parametric CAD app in pure Rust. It is part of the app, not a plugin: a **Gears** button in
**Model › Create** opens a window where you lay out the shafts, see the set mesh in 2D and 3D straight
away, and build real solid gears into the timeline.

This branch (`gear-generator` on [ariankhatb/solidcraft](https://github.com/ariankhatb/solidcraft/tree/gear-generator))
is a work in progress shared for testing and feedback. It tracks SolidCraft `main` (currently
**v0.42.0** plus later fixes).

## What's new in this build (2026-11-15)

- **Assembly.** Tick *Assembly (components + joints)* next to **Create** (or pass `assembly: true`):
  each gear becomes a component on a revolute joint (a slider for a rack), with a gear or
  rack-and-pinion link per mesh, so driving one gear turns the others. With a clearance body, the
  keep-out goes in as a grounded, see-through component. **Update** keeps the same components and
  joints, moves the joints onto the new shafts, gives the links the new ratios and keeps joints you
  made to the gears.
- **Differential designer** (`type: differential`): open bevel, torque-biasing bevel (uneven side
  gears), Torsen Type B and Torsen Type A, with the case, its drive gear (spiral bevel, straight
  bevel, hypoid or helical; bolted or integral), the assembly and a housing keep-out body. In
  Simple mode, the **Design** button sizes one from the axle, final drive ratio and torque.
- **Bevels at any shaft angle.** Teeth end on their back and front cones, so the larger gear of a set
  at 100° to 150° (a nearly flat cone) is built correctly. Each gear has its own face width and heel
  offset (`face_width2`, `face_offset2`), with rows in Advanced mode and toe and heel lines you can
  drag in the drawing.
- **Hypoid rework.** The ring gear has proper teeth, both members are trimmed at toe and heel, and
  the pinion has its own face width and offset.
- **Window.** The columns stack when the window is narrow, the previews size to the window and rows
  wrap. A set opens again by double-clicking its History folder, or by selecting one of its bodies
  or components and clicking **Gears**.
- **Also:** a warning for every set option a type doesn't use; a limit on the hypoid offset;
  parametric equation sets whose mates and clearance body follow the parameters; and the freeze
  when switching from Face gear to Bevel planetary is fixed (confirmed in the app).

## What it makes

| Shafts | Gear types |
|---|---|
| Parallel | spur, helical, herringbone (also as a pair option), internal (ring) gear, rack, gear pair, planetary set |
| Meeting | straight bevel and spiral bevel (any shaft angle, not only 90°), face (crown) gear (90°), **bevel planetary** (differential, single, or with an internal bevel ring) |
| Crossed | **hypoid** (offset spiral bevel, ISO 23509 pitch cones), crossed helical, worm and wheel (throated wheel hobbed by the worm, 1 to 12 starts) |
| Chain and belt | ISO 606 / ANSI chain sprockets, timing-belt pulleys (GT2, HTD, T2.5/T5/T10, MXL, XL), the chain or belt between two |
| Special | cycloidal (clock) gears, cycloidal drive, DIN 5480 / ANSI B92.1 involute splines, elliptical pairs |
| Differentials | **complete differentials**: open bevel, torque-biasing bevel (uneven side gears), Torsen Type B (helical planetary), Torsen Type A (worm); with the case (bearing journals, thrust faces, pin holes or element pockets, assembly windows, flange), its drive gear (spiral or straight bevel, hypoid, helical; bolted or integral), the assembly links and the housing keep-out body |

The tooth math is generated, not drawn from tables: involute teeth cut by a rack (true trochoid root,
undercut, profile shift), octoid bevel teeth cut by a crown gear, hypoid pinions generated from the
ring gear, and worm wheels cut plane by plane by the worm's thread. Each set reports its facts: centre
distance, contact ratio, span and over-pins measurements, and a strength rating when you give a load
(ISO 6336 Method B, simplified, for cylindrical gears; simplified ratings for bevels and worms).

## How you use it

- **Simple mode (default): design by what you need.** Pick a type, then drag or type sizes on a
  small drawing. Circles are pitch circles, lines are shafts, and a dot is the origin (a bevel's apex).
  A bevel is drawn as a side view: a shaft line, the apex dot, and a second line at the meeting angle
  (an 80° bevel is just a line at 80°). The 3D preview is drawn from the math in milliseconds, with
  no kernel build while you edit. Cycloidal drives, splines and elliptical pairs are in Advanced
  mode only. A differential in Simple mode is designed from what it must do instead: style, axle
  diameter, final drive ratio, torque, drive gear and mount, then **Design**.
- **Bevel faces.** In the drawing, each bevel gear's toe and heel lines can be dragged to set its
  face width and heel offset; Advanced mode has the same as rows per gear.
- **Advanced mode: every setting.** Tooth form (basic rack, clearance, tip shortening), profile shift,
  helix and spiral angles, backlash, bores (round, keyway, D, hex, square), hubs, integral and dual
  shafts, chamfers, lightening holes, bearing pockets, tip relief and crowning, and a **3D-printing**
  section (print preset, tip rounding, XY compensation, elephant-foot relief).
- **From one sketch.** Draw the set in a sketch in the model (circles and lines as above), pick it
  with **Use selected**, and the Gear Generator reads its sizes from it. From a script,
  `gear.create {"sketch": id}` also recognises the type from what is drawn.
- **Edit in place.** Right-click the set's folder in the History and choose **Edit Gears**, double-click
  the folder, or select one of the set's bodies or components and click **Gears**. **Update**
  rebuilds the bodies under the same names, so features that use them keep working.
- **Assembly (optional).** *Assembly (components + joints)* makes the set into components on joints
  with gear and rack-and-pinion links, ready to drive. Edits keep them.
- **Parametric teeth (optional).** For spur, helical, herringbone, internal, pair, planetary and rack
  sets, *Tooth curves: Equations* draws the teeth as formula curves, and *parametric* drives them
  from design parameters: change the tooth count or module and the gears, their mates and the
  clearance body follow.
- **Clearance body.** One solid keep-out volume for the whole set (turning envelopes, shafts, bearing
  seats from a bearing library), for cutting a housing with a single Boolean.
- **Colour-coded members.** Each gear has a colour and a label in the drawing, the previews and the
  settings, and hovering one lights it up everywhere.

Everything is also scriptable: `gear.create`, `gear.calculate` (facts only, builds nothing),
`gear.update`, `gear.from_axes` and `gear.inspect` are ordinary SolidCraft commands, available from
the CLI and the MCP server. `assembly: true` adds the components and joints; `type: differential`
takes its settings in `differential` (or a goal to design from in `differential_goal`).

## Build and run

Same as SolidCraft (see [CONTRIBUTING.md](CONTRIBUTING.md); Rust is pinned by `rust-toolchain.toml`):

```sh
git clone -b gear-generator https://github.com/ariankhatb/solidcraft && cd solidcraft
cargo run --release -p solidcraft                 # the app: Model › Create › Gears
cargo build --release -p solidcraft-cli           # the CLI
```

A release build takes a while and needs plenty of memory: build one thing at a time.

From the CLI (a script is a JSON array of `{"command", "params"}`):

```sh
echo '[{"command":"gear.create","params":{"type":"bevel","shaft_angle":100}},
       {"command":"inspect.interference","params":{}}]' > bevel.json
target/release/solidcraft-cli run bevel.json --results
echo '[{"command":"gear.create","params":{"type":"pair","assembly":true}}]' > pair.json
target/release/solidcraft-cli run pair.json --results
```

Tests: `cargo test --release -p solidcraft-gear` (the gear math, over 240 tests) and
`cargo test --release -p solidcraft-engine --lib gear` (building through the kernel).

## Where the code is

| Path | What |
|---|---|
| `crates/gear/` | the gear math, pure Rust with no dependencies: tooth generation, set planning (`plan.rs`), Simple-mode sizing (`simple.rs`), sketch rules, axes, clearance bodies, differentials (`diff.rs`), assemblies (`plan/assembly.rs`), 2D/3D mesh checkers, strength |
| `crates/engine/src/cmd/gear.rs`, `crates/engine/src/schema/gear.rs` | the commands, and turning a plan into sketches and features |
| `crates/engine/src/cmd/gear_equations.rs` | formula-driven teeth and design parameters |
| `crates/engine/src/cmd/gear_assembly.rs` | components, joints and links for `assembly: true` |
| `crates/ui-egui/src/gear_*.rs` | the window: Simple/Advanced modes, drawing, 3D preview, clearance UI, differential designer (`gear_diff.rs`), icons |
| `crates/gear/UI_FIELDS.md`, `crates/gear/CLEARANCE_UI.md` | which setting does what, per gear type |
| `GEAR_GENERATOR_NOTES.md` | the running design notes and status |

## Status and known issues

Checked in the app and the CLI on this build:

- A gear pair with *Assembly*: driving gear 1 by 32° turns gear 2 by −20°. **Edit Gears**, gear 2
  changed to Ø80, **Update**: the joints keep their ids, the link's ratio becomes −0.5 and driving
  still turns both correctly.
- A straight bevel at 100° and a spiral bevel at 110° (17:34, module 2.5) build with no trim
  warnings; the 100° bevel has no interference in the kernel's check.
- The hypoid builds with proper ring gear teeth; the default differential builds (8 bodies).
- Face gear → Bevel planetary five times in a row: no freeze, memory steady.

Known issues in this build (being worked on):

- The differential's case overlaps its side gears (about 900 mm³ each), and the default ring gear
  has no room for its bolts (the bolts are left out, with a warning).
- The face gear still shows a red *Interferes* in the window's 2D mesh check, which the kernel check
  doesn't confirm.
- Simple mode's settings labels are cut off ("Face …", "3D p…"), and the turn buttons show a
  missing-glyph box.
- The Gear Generator window jumps to another position when you switch types.
- Body and component names keep their old tooth counts after an update (for example "Gear z20"
  after changing to 24 teeth), so features that reference them don't break.
- The kernel's own interference check fails on spiral bevel, hypoid and some differential pairs
  ("proto edge is not listed", "not a closed solid", "Bad Infinite Region"): a kernel Boolean
  issue, not the gears. The gear crate's mesh checker covers those sets.
- Assembly limits: an elliptical pair is linked at its mean 1:1 ratio, so it drifts in and out of
  mesh when driven; a cycloidal drive's disc only turns on its own axis inside the grounded pin
  ring; planetary sets have no carrier body yet, so the carrier is held still.
- The bevel planetary's internal ring still has flat ends (it doesn't end on its cones yet).

## Feedback

Issues and pull requests are welcome on [ariankhatb/solidcraft](https://github.com/ariankhatb/solidcraft).
For a bug, the most useful thing is a `gear.create` JSON that reproduces it (`gear.inspect` on a
built set gives its parameters as `params`) and what you expected instead.
