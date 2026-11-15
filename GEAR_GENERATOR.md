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
| Meeting | straight bevel, spiral bevel (any shaft angle, not only 90°), face (crown) gear, **bevel planetary** (differential, single, or with an internal bevel ring) |
| Crossed | **hypoid** (offset spiral bevel, ISO 23509), crossed helical, worm and wheel (throated wheel hobbed by the worm, 1–4 starts) |
| Chain and belt | ISO 606 / ANSI chain sprockets, timing-belt pulleys (GT2, HTD, T2.5/T5/T10, MXL, XL), the chain or belt between two |
| Special | cycloidal (clock) gears, cycloidal drive, DIN 5480 / ANSI B92.1 involute splines, elliptical pairs |

The tooth math is generated, not drawn from tables: involute teeth cut by a rack (true trochoid root,
undercut, profile shift), octoid bevel teeth cut by a crown gear, hypoid pinions generated from the
ring gear, and worm wheels cut plane by plane by the worm's thread. Each set reports its facts: centre
distance, contact ratio, span and over-pins measurements, and ISO 6336 strength when you give a load.

## How you use it

- **Simple mode (default): design by what you need.** Pick a type, then drag or type sizes on a
  small drawing. Circles are pitch circles, lines are shafts, and a dot is the origin (a bevel's apex).
  A bevel is drawn as a side view: a shaft line, the apex dot, and a second line at the meeting angle
  (an 80° bevel is just a line at 80°). The 3D preview is drawn from the math in milliseconds, with
  no kernel build while you edit.
- **Advanced mode: every setting.** Tooth form (basic rack, clearance, tip shortening), profile shift,
  helix and spiral angles, backlash, bores (round, keyway, D, hex, square), hubs, integral and dual
  shafts, chamfers, lightening holes, bearing pockets, a **3D-print tooth profile** (tip rounding,
  XY compensation, elephant-foot relief), tip relief and crowning.
- **From one sketch.** Draw the set in a sketch (circles and lines as above) and the Gear Generator
  reads it, recognising the type from what is drawn.
- **Edit in place.** Right-click the set's group in History › **Edit Gears**. **Update** rebuilds the
  bodies under the same names, so features that use them keep working.
- **Parametric teeth (optional).** With *tooth curves: equations* the teeth are formula curves driven
  by design parameters: change the tooth count or module and the gears, their mates and the
  clearance body follow.
- **Clearance body.** One solid keep-out volume for the whole set (turning envelopes, shafts, bearing
  seats from a bearing library), for cutting a housing with a single Boolean.
- **Colour-coded members.** Each gear has a colour and a label in the drawing, the previews and the
  settings, and hovering one lights it up everywhere.

Everything is also scriptable: `gear.create`, `gear.calculate` (facts only, builds nothing),
`gear.update` and `gear.from_axes` are ordinary SolidCraft commands, available from the CLI and the
MCP server.

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

Tests: `cargo test --release -p solidcraft-gear` (the gear math, fast) and
`cargo test --release -p solidcraft-engine --lib gear` (building through the kernel).

## Where the code is

| Path | What |
|---|---|
| `crates/gear/` | the gear math, pure Rust with no dependencies: tooth generation, set planning (`plan.rs`), Simple-mode sizing (`simple.rs`), sketch rules, axes, clearance bodies, 2D/3D mesh checkers, strength |
| `crates/engine/src/cmd/gear.rs`, `schema/gear.rs` | the commands, and turning a plan into sketches and features |
| `crates/engine/src/cmd/gear_equations.rs` | formula-driven teeth and design parameters |
| `crates/ui-egui/src/gear_*.rs` | the window: Simple/Advanced modes, drawing, 3D preview, clearance UI, icons |
| `crates/gear/UI_FIELDS.md`, `crates/gear/CLEARANCE_UI.md` | which setting does what, per gear type |
| `GEAR_GENERATOR_NOTES.md` | the running design notes and status |

## Status and known issues

Checked with the kernel's own interference test: every type builds and meshes with no overlap, or
only flank contact, at its defaults and at harder settings (bevels at 60–120°, hypoids at 75–90°,
4-start worms, bevel planetary in all three modes). Known issues:

- Picking **Bevel planetary right after Face gear in Simple mode** can freeze the window and use a lot
  of memory. Being fixed; use Advanced mode for bevel planetary meanwhile.
- The window's mesh check can show a red *Interferes* on the default face gear, which the kernel
  check doesn't confirm (under investigation).
- Body names keep their tooth counts after an edit (for example "Gear z20" after changing to 24
  teeth), on purpose so features that reference them don't break.
- The kernel's interference check itself fails on some lofted sets (hypoid, worm) with "proto edge is
  not listed"; the gear crate's own mesh checker covers those.

## Feedback

Issues and pull requests are welcome on [ariankhatb/solidcraft](https://github.com/ariankhatb/solidcraft).
For a bug, the most useful thing is a `gear.create` JSON that reproduces it (Advanced mode shows the
parameters of the set on screen) and what you expected instead.
