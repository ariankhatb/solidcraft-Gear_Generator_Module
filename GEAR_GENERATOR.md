# SolidCraft Gear Generator

> [!IMPORTANT]
> **This page describes the earlier Gear Generator window, which this branch replaced.** Gears are now made with the
> Gears dialog working on your own sketch: one card per gear, picks in the view, one editable Gears feature for the
> whole drivetrain. The current user guide is [docs/gears/](docs/gears/README.md). This page is kept for its history,
> its notes on the tooth math and the types that are now made only by the `gear.create` command (sprockets, pulleys,
> cycloidal gears and drives, splines, elliptical pairs, face gears, bevel planetary sets, differentials).


A gear generator built into [SolidCraft](https://github.com/tangency-labs/solidcraft), the open-source
parametric CAD app in pure Rust. It is part of the app, not a plugin: you draw the set in an ordinary sketch,
and the **Gears** button in **Model › Create** opens a command dialog in the right panel (like Thread
or Extrude) that turns the sketch into real solid gears in the timeline, linked to the sketch.

This branch (`gear-generator` on [ariankhatb/solidcraft](https://github.com/ariankhatb/solidcraft/tree/gear-generator))
is a work in progress shared for testing and feedback. It tracks SolidCraft `main` (currently
**v0.42.0** plus later fixes).

## What's new in test build 2 (2026-11-15, later)

- **Smoother window:** the 3D preview draws on the GPU instead of the CPU (before: 15–57 ms a frame
  while turning big sets, more on high-DPI screens).
- **Differential, simpler case:** the case is plain stock that keeps its material: only the gears'
  clearance, the axle bore and the pin bores are cut. No windows, bolt or tapped holes by default
  (ring bolts are an option). The case no longer overlaps the side gears.
- **Fixes:** the face gear's false red *Interferes* is gone; setting labels show in full; the turn
  buttons read "X +90°" etc.; the window stays put when you switch types and fits narrow screens;
  bodies and components the generator named follow the new tooth counts after an edit (names you
  gave stay); keep-out bodies cover hubs and shafts on trimmed bevels and hypoids.
- **Bevel ends:** ordinary bevels (pitch cones up to 70°) end on planes again, which the kernel's own
  check accepts; flatter members keep the cone trim (see known issues).

## What's new in test build 1 (2026-11-15)

- **Assembly.** Tick *Assembly (components + joints)* next to **Create** (or pass `assembly: true`):
  each gear becomes a component on a revolute joint (a slider for a rack), with a gear or
  rack-and-pinion link per mesh, so driving one gear turns the others. With a clearance body, the
  keep-out goes in as a grounded, see-through component. **Update** keeps the same components and
  joints, moves the joints onto the new shafts, gives the links the new ratios and keeps joints you
  made to the gears.
- **Differential designer** (`type: differential`): open bevel, torque-biasing bevel (uneven side
  gears), Torsen Type B and Torsen Type A, with the case, its drive gear (spiral bevel, straight
  bevel, hypoid or helical; bolted or integral; helical by default, since the kernel's full check
  of a spiral bevel ring takes minutes), the assembly and a housing keep-out body. In
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

## How you used the earlier window

For the current Gears dialog see [docs/gears/quick-start.md](docs/gears/quick-start.md).

1. **Draw the set in a sketch** with the normal sketch tools: circles are pitch circles, lines are
   shafts, a point is the origin (a bevel's apex) or a bearing on a shaft line. Dimension them and
   make the meshing circles **Tangent**. Construction lines and points are helpers: they are never
   gears or shafts. A line from one circle's centre to another's says the two mesh.
2. **Start Gears** (Model › Create › Gears) with the sketch open or its curves selected: the sketch
   is taken as Extrude takes a profile.
3. **Pick and assign.** Every circle, line, point, 3D sketch curve, model edge or face you pick is a
   numbered row with its icon and name, lit in the view while hovered. Give each row its role:
   Spur, Helical, Herringbone, Bevel, Worm, Wheel, Rack, Ring Gear, Face Gear, **Shaft** (one solid
   shaft through the gears on it) or **Bearing** (a point that lies on a picked shaft line; sized
   from the bearing library). Rows move up or are removed with their buttons.
4. **Size** by Module or by Teeth (per row); Face Width is Auto unless you give one. The other
   families read the sketch as one drawing (hypoid, crossed helicals, bevel planetary, sprocket,
   pulley, cycloidal); spline, elliptical gears, cycloidal drive and the differential are made from
   their rows alone (the differential with its style, drive and which parts to build, as a plain
   case). The rest of the settings are in the collapsed **Advanced** section.
5. **Preview.** The set's abstract shape is drawn in the viewport at once: shafts and pitch surfaces
   in each member's colour, no teeth (tick *Preview Teeth* to build them). Each row shows what it
   came to (module, teeth, Ø, what it meshes with, the shaft it shares) or what is wrong with it,
   and a refused circle is drawn in red ("circle c3 touches no other gear: make it tangent").
6. **OK** builds the set, linked to its sketch: change a dimension of the sketch and the set is
   regenerated in place, in the same undo step.
7. **Edit Gears** (right-click the set's folder in the History, or double-click it) reopens the dialog
   on the set; OK updates it in place, keeping its bodies, components and joints.

- **Assembly (optional):** each gear a component on a joint, linked to its mates, ready to drive.
- **Clearance body (optional):** one keep-out solid for the whole set, with the bearings' seats.

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
| `crates/ui-egui/src/dialogs_gear.rs` | the Gears command dialog: pick rows and roles, settings, the viewport preview, Edit Gears |
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

- Bevel members with a pitch cone over 70° (for example the large gear at a 110° shaft angle) are
  trimmed by their back and front cones; the kernel's check flags those trims for tiny 2D curve
  kinks ("Bad CurveCusp", a warning-level test), so its own interference check can't run on them.
  The gears themselves are correct.
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
