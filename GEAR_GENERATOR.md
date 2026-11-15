# SolidCraft Gear Generator

A gear generator built into [SolidCraft](https://github.com/tangency-labs/solidcraft), the open-source
parametric CAD app in pure Rust. It is part of the app, not a plugin: a **Gears** button in
**Model › Create** opens a window where you lay out the shafts, see the set mesh in 2D and 3D straight
away, and build real solid gears into the timeline.

This branch (`gear-generator` on [ariankhatb/solidcraft](https://github.com/ariankhatb/solidcraft/tree/gear-generator))
is a work in progress shared for testing and feedback. It tracks SolidCraft `main` (currently
**v0.42.0** plus later fixes).

## What it makes

| Shafts | Gear types |
|---|---|
| Parallel | spur, helical, herringbone (also as a pair option), internal (ring) gear, rack, gear pair, planetary set |
| Meeting | straight bevel and spiral bevel (any shaft angle, not only 90°), face (crown) gear (90°), **bevel planetary** (differential, single, or with an internal bevel ring) |
| Crossed | **hypoid** (offset spiral bevel, ISO 23509 pitch cones), crossed helical, worm and wheel (throated wheel hobbed by the worm, 1 to 12 starts) |
| Chain and belt | ISO 606 / ANSI chain sprockets, timing-belt pulleys (GT2, HTD, T2.5/T5/T10, MXL, XL), the chain or belt between two |
| Special | cycloidal (clock) gears, cycloidal drive, DIN 5480 / ANSI B92.1 involute splines, elliptical pairs |
| Differentials | **complete differentials**: open bevel, torque-biasing bevel (uneven side gears), Torsen Type B (helical planetary), Torsen Type A (worm); with the case (bearing journals, thrust faces, pin holes or element pockets, windows, flange), its drive gear (spiral or straight bevel, hypoid, helical; bolted or integral), the assembly links and the housing keep-out body |

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
  mode only.
- **Advanced mode: every setting.** Tooth form (basic rack, clearance, tip shortening), profile shift,
  helix and spiral angles, backlash, bores (round, keyway, D, hex, square), hubs, integral and dual
  shafts, chamfers, lightening holes, bearing pockets, tip relief and crowning, and a **3D-printing**
  section (print preset, tip rounding, XY compensation, elephant-foot relief).
- **From one sketch.** Draw the set in a sketch in the model (circles and lines as above), pick it
  with **Use selected**, and the Gear Generator reads its sizes from it. From a script,
  `gear.create {"sketch": id}` also recognises the type from what is drawn.
- **Edit in place.** Right-click the set's folder in the History and choose **Edit Gears**. **Update**
  rebuilds the bodies under the same names, so features that use them keep working.
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
the CLI and the MCP server.

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
echo '[{"command":"gear.create","params":{"type":"spiral_bevel","shaft_angle":80}},
       {"command":"inspect.interference","params":{}}]' > bevel.json
target/release/solidcraft-cli run bevel.json --results
```

Tests: `cargo test --release -p solidcraft-gear` (the gear math, over 220 tests) and
`cargo test --release -p solidcraft-engine --lib gear` (building through the kernel).

## Where the code is

| Path | What |
|---|---|
| `crates/gear/` | the gear math, pure Rust with no dependencies: tooth generation, set planning (`plan.rs`), Simple-mode sizing (`simple.rs`), sketch rules, axes, clearance bodies, 2D/3D mesh checkers, strength |
| `crates/engine/src/cmd/gear.rs`, `crates/engine/src/schema/gear.rs` | the commands, and turning a plan into sketches and features |
| `crates/engine/src/cmd/gear_equations.rs` | formula-driven teeth and design parameters |
| `crates/ui-egui/src/gear_*.rs` | the window: Simple/Advanced modes, drawing, 3D preview, clearance UI, icons |
| `crates/gear/UI_FIELDS.md`, `crates/gear/CLEARANCE_UI.md` | which setting does what, per gear type |
| `GEAR_GENERATOR_NOTES.md` | the running design notes and status |

## Status and known issues

Checked with the kernel's own interference test (`inspect.interference`) in test runs: every type
built and meshed with no overlap, or only flank contact, at its defaults and at harder settings
(bevels at 60–120°, hypoids at 75–90°, 4-start worms, bevel planetary in all three modes), except
where the kernel's check itself fails (last item below). Known issues in the current build:

- Picking **Bevel planetary right after Face gear in Simple mode** can freeze the window and use a lot
  of memory. A fix (the drawing's dashed lines clipped to the view) comes with the next update of
  this branch; use Advanced mode for bevel planetary meanwhile.
- The window's mesh check can show a red *Interferes* on the default face gear, which the kernel
  check doesn't confirm (under investigation).
- Bevels whose larger gear's pitch cone goes past about 80° (for example a spiral bevel at a 110°
  shaft angle) build with thin, feathered teeth on that gear. The window warns about it. Being fixed so
  bevels work at 100–110° and beyond; until then keep both cones under 80°.
- Body names keep their tooth counts after an edit (for example "Gear z20" after changing to 24
  teeth), on purpose so features that reference them don't break.
- The kernel's interference check itself fails on some lofted sets (hypoid, worm) with "proto edge is
  not listed"; the gear crate's own mesh checker covers those.

## Feedback

Issues and pull requests are welcome on [ariankhatb/solidcraft](https://github.com/ariankhatb/solidcraft).
For a bug, the most useful thing is a `gear.create` JSON that reproduces it (`gear.inspect` on a
built set gives its parameters as `params`) and what you expected instead.
