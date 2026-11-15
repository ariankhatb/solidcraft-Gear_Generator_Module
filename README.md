> [!NOTE]
> **This is a fork of [tangency-labs/solidcraft](https://github.com/tangency-labs/solidcraft) with a built-in Gear Generator.**
> The upstream SolidCraft README follows below the line. Hands-on guide: [GEAR_GENERATOR.md](GEAR_GENERATOR.md).
> Work in progress, shared for testing: issues and pull requests are welcome here.

# Gear Generator for SolidCraft

A gear generator that is part of the app, not a plugin. **Model › Create › Gears** opens a window where
you lay out the shafts, see the set mesh in 2D and 3D as you type, and build solid gears into the
parametric timeline. It covers parallel, intersecting and crossed shafts, from a spur gear to a
hypoid final drive, a multi-start worm or a complete differential with its case and drive gear,
and it can make any set an assembly whose gears turn each other.

<p align="center"><img src="crates/look/icons/design/svg/model/gear_generator.svg" width="72" height="72" alt="Gear Generator"/></p>

<table align="center">
<tr><td align="center"><img src="crates/look/icons/design/svg/model/gear_spur.svg" width="44" height="44" alt="Spur"/><br/><sub>Spur</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_helical.svg" width="44" height="44" alt="Helical"/><br/><sub>Helical</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_herringbone.svg" width="44" height="44" alt="Herringbone"/><br/><sub>Herringbone</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_internal.svg" width="44" height="44" alt="Internal"/><br/><sub>Internal</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_rack.svg" width="44" height="44" alt="Rack"/><br/><sub>Rack</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_pair.svg" width="44" height="44" alt="Pair"/><br/><sub>Pair</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_planetary.svg" width="44" height="44" alt="Planetary"/><br/><sub>Planetary</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_bevel.svg" width="44" height="44" alt="Bevel"/><br/><sub>Bevel</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_spiral_bevel.svg" width="44" height="44" alt="Spiral bevel"/><br/><sub>Spiral bevel</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_face.svg" width="44" height="44" alt="Face"/><br/><sub>Face</sub></td></tr>
<tr><td align="center"><img src="crates/look/icons/design/svg/model/gear_bevel_planetary.svg" width="44" height="44" alt="Bevel planetary"/><br/><sub>Bevel planetary</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_hypoid.svg" width="44" height="44" alt="Hypoid"/><br/><sub>Hypoid</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_crossed_helical.svg" width="44" height="44" alt="Crossed helical"/><br/><sub>Crossed helical</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_worm.svg" width="44" height="44" alt="Worm"/><br/><sub>Worm</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_cycloidal.svg" width="44" height="44" alt="Cycloidal"/><br/><sub>Cycloidal</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_cycloidal_drive.svg" width="44" height="44" alt="Cycloidal drive"/><br/><sub>Cycloidal drive</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_sprocket.svg" width="44" height="44" alt="Sprocket"/><br/><sub>Sprocket</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_pulley.svg" width="44" height="44" alt="Pulley"/><br/><sub>Pulley</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_spline.svg" width="44" height="44" alt="Spline"/><br/><sub>Spline</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_elliptical.svg" width="44" height="44" alt="Elliptical"/><br/><sub>Elliptical</sub></td><td align="center"><img src="crates/look/icons/design/svg/model/gear_differential.svg" width="44" height="44" alt="Differential"/><br/><sub>Differential</sub></td></tr>
</table>

### What's new in test build 2 (2026-11-15, later)

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

### What's new in test build 1 (2026-11-15)

- **Assembly:** tick *Assembly (components + joints)* and the set becomes components on revolute
  joints (a slider for a rack), tied by gear and rack-and-pinion links, so driving one gear turns
  the others. Edits keep the components, the joints and any joints you made to them.
- **Differential designer:** open bevel, torque-biasing bevel (uneven side gears), Torsen Type B and
  Torsen Type A, with the case, a drive gear (spiral bevel, straight bevel, hypoid or helical,
  bolted or integral) and a housing keep-out body. In Simple mode its **Design** button sizes one
  from the axle, ratio and torque.
- **Bevels at any angle:** teeth end on their back and front cones, so nearly flat cones (100° to
  150° shaft angles) build correctly. Each gear has its own face width and heel offset, with rows in
  the window and toe and heel lines to drag in the drawing.
- **Hypoid rework:** proper ring gear teeth, trimmed at toe and heel, and the pinion's own face
  width and offset.
- **Window:** columns stack when the window is narrow, previews fit the window, rows wrap. Reopen a
  set by double-clicking its History folder, or select one of its bodies or components and click
  **Gears**.
- **Fixes:** switching from Face gear to Bevel planetary no longer freezes the window; parametric
  equation sets keep their mates and clearance body in step with the parameters.

### It runs live while you design

Every set turns in the window's preview as you edit it, drawn from the gear math in milliseconds
(these are screen captures of that preview, not kernel renders):

<table align="center">
<tr><td align="center"><img src="docs/gear-generator/pair.gif" width="260" alt="Gear pair turning in the live preview"/><br/><sub>Gear pair</sub></td><td align="center"><img src="docs/gear-generator/helical-clip.gif" width="260" alt="Helical turning in the live preview"/><br/><sub>Helical</sub></td><td align="center"><img src="docs/gear-generator/herringbone-clip.gif" width="260" alt="Herringbone turning in the live preview"/><br/><sub>Herringbone</sub></td></tr>
<tr><td align="center"><img src="docs/gear-generator/internal.gif" width="260" alt="Internal ring turning in the live preview"/><br/><sub>Internal ring</sub></td><td align="center"><img src="docs/gear-generator/planetary.gif" width="260" alt="Planetary turning in the live preview"/><br/><sub>Planetary</sub></td><td align="center"><img src="docs/gear-generator/bevel.gif" width="260" alt="Straight bevel turning in the live preview"/><br/><sub>Straight bevel</sub></td></tr>
<tr><td align="center"><img src="docs/gear-generator/spiral_bevel.gif" width="260" alt="Spiral bevel turning in the live preview"/><br/><sub>Spiral bevel</sub></td><td align="center"><img src="docs/gear-generator/hypoid.gif" width="260" alt="Hypoid turning in the live preview"/><br/><sub>Hypoid</sub></td><td align="center"><img src="docs/gear-generator/crossed_helical.gif" width="260" alt="Crossed helical turning in the live preview"/><br/><sub>Crossed helical</sub></td></tr>
<tr><td align="center"><img src="docs/gear-generator/worm.gif" width="260" alt="Worm and wheel turning in the live preview"/><br/><sub>Worm and wheel</sub></td><td align="center"><img src="docs/gear-generator/cycloidal-clip.gif" width="260" alt="Cycloidal turning in the live preview"/><br/><sub>Cycloidal</sub></td></tr>
</table>

### Designed from one small drawing

Simple mode: circles are pitch circles, lines are shafts, a dot is the origin (a bevel's apex).
Each screenshot shows the type picker, the editable drawing, the turning 3D preview and the
numbers the solver chose:

<table align="center">
<tr><td align="center"><img src="docs/gear-generator/window_pair.png" width="380" alt="Gear pair: two pitch circles"/><br/><sub>Gear pair: two pitch circles</sub></td><td align="center"><img src="docs/gear-generator/window_spiral_bevel.png" width="380" alt="Spiral bevel: side view, apex dot and 90° meeting angle"/><br/><sub>Spiral bevel: side view, apex dot and 90° meeting angle</sub></td></tr>
<tr><td align="center"><img src="docs/gear-generator/window_hypoid.png" width="380" alt="Hypoid: ring circle, pinion shaft offset"/><br/><sub>Hypoid: ring circle, pinion shaft offset</sub></td><td align="center"><img src="docs/gear-generator/window_worm.png" width="380" alt="Worm: wheel circle and tangent worm line"/><br/><sub>Worm: wheel circle and tangent worm line</sub></td></tr>
</table>

| Shafts | Types |
|---|---|
| Parallel | spur, helical, herringbone, internal ring, rack, pair, planetary |
| Intersecting | straight bevel and spiral bevel (any shaft angle), face (crown) gear (90°), bevel planetary (differential, single, or with an internal bevel ring) |
| Crossed | hypoid, crossed helical, worm and throated wheel (1 to 12 starts) |
| Chain and belt | ISO 606 / ANSI roller-chain sprockets, timing pulleys (GT2, HTD, T2.5/T5/T10, MXL, XL) and the chain or belt between them |
| Special | cycloidal clock gears, cycloidal drive, DIN 5480 / ANSI B92.1 splines, elliptical pairs |
| Differentials | open bevel, torque-biasing bevel, Torsen Type B (helical planetary), Torsen Type A (worm), each with its case, drive gear and keep-out body |

## How it works

### Teeth are generated, not drawn from tables

- **Involute teeth** are the envelope of a basic rack (ISO 53 profiles A–D, AGMA full depth, stub,
  a 3D-print rack, or custom addendum, dedendum and root radius) rolled along the pitch circle.
  That gives the true trochoid root fillet, undercut where it really happens, profile shift, tip
  shortening and the working pressure angle. By default the flanks are fitted to splines within a
  set tolerance. Each set reports its centre distance,
  contact ratio, span and over-pins measurements.
- **Bevel teeth** are octoids: cut by a crown gear, so they are conjugate along the whole face. That
  holds for straight teeth and for spiral teeth on a logarithmic spiral, at any shaft angle. The
  teeth end on the back (heel) and front (toe) cones, square to the pitch cone, so a nearly flat
  cone (the larger gear at a 100° to 150° shaft angle) is built as correctly as a steep one.
- **Hypoid pitch cones** are worked out in the terms of ISO 23509: the wheel pitch angle is found by
  iteration with the Method 1 curvature condition, and the result reproduces a published ISO 23509
  Method 1 set. Tooth depths are Gleason's. The ring gear has straight-sided teeth on a
  logarithmic spiral, and the pinion is generated from the ring gear. Both are trimmed at toe and
  heel, and the pinion has its own face width and offset. An offset too large for the sizes is refused, and the
  message gives the largest offset that still works.
- **Worm wheels** are throated and hobbed: the hob is the worm's own thread, made taller by the root
  clearance. Each section through the face is what the thread leaves as it turns.
- **Other teeth:** sprockets follow ISO 606 (its A series is ANSI B29.1), and splines follow DIN 5480
  or ANSI B92.1. Timing-pulley grooves are approximations from published belt dimensions (GT2 and
  HTD are proprietary profiles). Cycloidal gears have epicycloid addenda and hypocycloid dedenda,
  and the cycloidal drive's disc is an offset epitrochoid. A face gear is cut by its spur pinion.
- **Strength:** ISO 6336 Method B (simplified) gives the tooth-root and flank safety factors SF and
  SH of cylindrical gears for a given torque or power. Bevels get a simplified rating on their
  virtual cylindrical gears (in the style of ISO 10300), and worm sets an approximate one.

### Plan, then build

The gear math is a dependency-free crate (`crates/gear`). `plan(spec)` turns a `Spec` (type, teeth,
module, angles, bores, shafts…) into a `Plan`:

- **bodies**, each a frame plus a list of solid steps:
  - `Prism`: an extrude, or a sweep with a twist for helical teeth (a herringbone is one twisted
    half and its mirror);
  - `Taper`: bevel teeth narrowing towards the cone apex, built as a loft (two sections for straight
    teeth, more for spiral teeth, whose turn follows the logarithm of the scale), run past both
    ends and then intersected with the blank;
  - `RingTaper`: the internal bevel ring of a bevel planetary set, teeth and rim in one loft;
  - `Loft`: through generated sections (worm wheels, hypoid members, face gears, helical racks);
  - `Revolve` (the bevel and hypoid blanks bounded by the back and front cones, edge chamfers, a
    sprocket's side chamfer, a differential's case, the clearance body) and `Coil` (a worm's
    thread);
  - `Placed`: a step in a frame of its own, such as a radial set-screw hole or a part of the
    clearance body;
  - each step is `New`, `Join`, `Cut` or `Intersect`;
- facts, warnings, a refusal when the set can't be made, and the 2D and 3D previews.

The engine (`crates/engine/src/cmd/gear.rs`) maps each step 1:1 onto ordinary SolidCraft sketches and
features. The whole set is one undo step and one timeline folder. A set is built with the kernel's
full face-by-face check off, because for gear teeth that check costs about as much as building
them. The check runs again the next time the features are computed.

Lofts are checked before the kernel sees them. The kernel restarts each section at the segment that
best lines up with the previous section, so a steep spiral or worm section turned too far would be
matched a few segments off, and the walls would cut across the teeth. The planner repeats that
alignment rule and adds sections until none slips. It also stays inside the kernel's limit of 16,384
curves per loft (all sections together, counting every spline span). Big multi-start worm wheels
draw each tooth side with fewer points when they need to.

### Fast to design with

- **Simple mode** designs by what you need. You draw circles (pitch circles), lines (shafts) and a dot
  (the origin, or a bevel's apex) in one small sketch. The solver picks standard ISO 54 modules,
  whole tooth counts and profile shift to hit those sizes and ratio, and lists what it chose.
  (Cycloidal drives, splines and elliptical pairs are not drawn this way: they are in Advanced
  mode only.)
- A bevel is drawn as a side view: a shaft line, the apex dot, and a second line at the meeting
  angle. An 80° bevel is just a line at 80°.
- The 3D preview is meshed straight from the plan in milliseconds (pitch surfaces, then teeth within
  a triangle budget), so nothing waits for the kernel while you edit.
- **Advanced mode** shows every option: tooth form, shift, helix and spiral, backlash, bores, hubs,
  integral and dual shafts, chamfers, lightening holes, bearing pockets, tip relief and crowning,
  and a **3D-print** section (print preset, tip rounding, XY compensation, elephant-foot relief).
- An option that is set but that the gear type can't use gets a warning (one per group of
  options) instead of being dropped silently.

### Checked, not assumed

- **2D mesh checker:** parallel-axis sets (and a face gear's pinion against the face gear's teeth
  unrolled at mid length) are turned through a tooth pitch. The checker flags overlap deeper than
  0.003 × module, or flanks further apart than the backlash plus 0.006 × module (for a cycloidal
  drive, × the pin radius). The window shows the verdict once the numbers settle.
- **3D mesh checker:** straight bevel, spiral bevel, hypoid, crossed helical, worm, bevel planetary
  and differential sets are sampled from the planned solids over the motion and checked against the same limits.
- **Kernel check:** sets are built through the real kernel and checked with `inspect.interference`
  for no overlap or only flank contact (a straight bevel at 100°, for example, shows none). On
  spiral bevel, hypoid and some differential pairs the kernel's own check fails; the gear crate's
  mesh checker covers those (see the known issues).

### Editing and parameters

- **`gear.update`** rebuilds a set in place. Bodies keep their names, and sketches and features keep
  their ids where the steps stay the same, so features built on the gears keep working.
- **Equation teeth:** for the planar involute types (spur, helical, herringbone, internal, pair,
  planetary, rack), `tooth_curves: equations` draws one tooth as formula curves (involute flank,
  trochoid root fillet, tip and root arcs) and patterns it round. Add `parametric: true` and the
  teeth are driven by design parameters (module, teeth, pressure angle, shift, backlash, face
  width). Change a tooth count and the teeth regenerate. The mates move to the new centre
  distance and turn to mesh again, and the clearance body follows.
- **Assembly:** with `assembly: true` every gear becomes a component of its own on a revolute joint
  (a slider for a rack; held members such as a cycloidal drive's pin ring grounded), with a gear or
  rack-and-pinion link per mesh at the set's ratio and direction, so `joint.drive` on one gear turns
  the others. With a clearance body it goes in as a grounded, see-through keep-out component. All
  of it is in the set's undo step and History folder, and `gear.update` keeps the same components
  and joints (moved onto the new shafts, links given the new ratios) and the joints you made to
  them.
- **Clearance body:** one closed keep-out solid for the whole set (turning envelopes, shafts, bearing
  seats from a bearing library), for cutting a housing with a single Boolean.
- **Sketch-driven sets:** draw the set in a sketch and `gear.create {"sketch": id}` recognises the
  type, sizes it and places it on the drawn shafts. A sketch line or point can also be a shaft
  (`axis1`, `axis2`) for `gear.create` and `gear.from_axes`.

### Scriptable

`gear.create`, `gear.calculate` (facts only), `gear.update`, `gear.inspect` and `gear.from_axes` (how
two shafts lie and the set solved on them, without building) are normal SolidCraft commands, so they
run from the CLI, from scripts and through the MCP server:

```sh
cargo build --release -p solidcraft-cli
echo '[{"command":"gear.create","params":{"type":"pair","assembly":true}}]' > pair.json
target/release/solidcraft-cli run pair.json --results
echo '[{"command":"gear.create","params":{"type":"differential",
         "differential":{"style":"torsen_b","drive":"helical","mount":"bolted"}}}]' > diff.json
target/release/solidcraft-cli run diff.json --results
```

### Code map

| Path | |
|---|---|
| `crates/gear/src/plan.rs`, `plan/` | `Spec`, `Plan`, every type's planner |
| `involute.rs`, `octoid.rs`, `hypoid.rs`, `worm.rs`, `cycloid.rs` | tooth generation |
| `simple.rs`, `sketch_rules.rs`, `axes.rs` | design by outputs, the one-sketch rules, placing sets on shafts |
| `mesh2d.rs`, `mesh3d.rs` | the mesh checkers |
| `diff.rs`, `plan/assembly.rs` | the differential designer; how each set goes together as an assembly |
| `equations.rs`, `clearance.rs`, `body.rs`, `hub.rs`, `strength.rs` | formula teeth, keep-out body, shafts, chamfers and print tuning, bores and hubs, strength (ISO 6336) |
| `crates/engine/src/cmd/gear*.rs`, `schema/gear.rs` | the commands, the plan-to-features builder, components and joints (`gear_assembly.rs`) |
| `crates/ui-egui/src/gear_*.rs` | the window: modes, drawing, 3D preview, clearance UI, differential designer, icons |

Build: `cargo run --release -p solidcraft`. Tests: `cargo test --release -p solidcraft-gear` (over
240 tests on the math) and `cargo test --release -p solidcraft-engine --lib gear`
(building through the kernel).

**Known issues:** see [GEAR_GENERATOR.md](GEAR_GENERATOR.md#status-and-known-issues).

---

<p align="center">
  <img src="assets/icons/app/linux/exports/hicolor/256x256/apps/io.tangencylabs.solidcraft.png" width="96" height="96" alt="SolidCraft" />
</p>

<h1 align="center">SolidCraft</h1>

<p align="center">
  <strong>An open-source parametric 3D CAD application built in pure Rust.</strong><br />
  Sketch, constrain, and build solids with a parametric feature timeline. Change a dimension
  and watch your model rebuild. Run it on your desktop, in your browser, or let AI agents
  model through the built-in MCP server.
</p>

<div align="center">

[![Download for Windows][badge-windows]][windows-msi]
[![Download for macOS][badge-macos]][macos-dmg]
[![Download for Linux][badge-linux]][linux-appimage]
[![Get it from the Snap Store][badge-snap]][snap-store]
[![Try it in your browser][badge-web]][web-app]

</div>

<p align="center">
  <a href="https://solidcraft.io">Website</a> ·
  <a href="https://solidcraft.io/download">Download</a> ·
  <a href="https://solidcraft.io/app">Try it in the browser</a> ·
  <a href="https://docs.solidcraft.io/">Documentation</a> ·
  <a href="https://www.blacksmith.sh"><img src="https://img.shields.io/badge/CI-Blacksmith-000000?logo=githubactions&logoColor=white" alt="CI powered by Blacksmith" /></a>
  <a href="https://discord.gg/FtU4NQNVDH">Discord</a> ·
  <a href="https://github.com/tangency-labs/solidcore">SolidCore</a>
</p>

<p align="center">
  <a href="https://github.com/tangency-labs/solidcraft/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://github.com/tangency-labs/solidcraft/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
  <a href="https://github.com/tangency-labs/solidcraft/releases/latest"><img src="https://img.shields.io/github/v/release/tangency-labs/solidcraft?label=release" alt="Latest release" /></a>
  <a href="https://github.com/tangency-labs/solidcraft/releases"><img src="https://img.shields.io/github/downloads/tangency-labs/solidcraft/total?label=downloads" alt="Downloads" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/tangency-labs/solidcraft" alt="Licence" /></a>
  <a href="https://discord.gg/FtU4NQNVDH"><img src="https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://remade.wiki/projects/solidcraft/"><img src="https://remade.wiki/badges/listed-on-remade-classic.svg" alt="Listed on Remade" /></a>
  <a href="https://github.com/tangency-labs/solidcraft/stargazers"><img src="https://img.shields.io/github/stars/tangency-labs/solidcraft?style=flat" alt="Stars" /></a>
</p>

<p align="center">
  <a href="https://trendshift.io/repositories/296178?utm_source=trendshift-badge&utm_medium=badge&utm_campaign=badge-trendshift-296178" target="_blank" rel="noopener noreferrer"><img src="https://trendshift.io/api/badge/trendshift/repositories/296178/daily" alt="tangency-labs%2Fsolidcraft | Trendshift" width="250" height="55"/></a>
  <a href="https://trendshift.io/repositories/296178?utm_source=trendshift-badge&utm_medium=badge&utm_campaign=badge-trendshift-296178" target="_blank" rel="noopener noreferrer"><img src="https://trendshift.io/api/badge/trendshift/repositories/296178/daily?language=Rust" alt="tangency-labs%2Fsolidcraft | Trendshift (Rust)" width="250" height="55"/></a>
</p>

![SolidCraft: model a Raspberry Pi 4 case from an empty design: a sketch dimensioned with parameters, extrude, fillets and shell, a rim rounded in one pick, standoffs patterned on the board's mounting holes, port cut-outs, vent slots, "Pi 4" engraved in a font, a PETG material with its mass in Properties, and a height change in Parameters](docs/screenshots/demo.gif)

*A Raspberry Pi 4 case from an empty design: a sketch dimensioned with the board's parameters, extrude, fillet, shell, the rim rounded in one pick, an M2.5 standoff patterned on the 58 × 49 mm hole rectangle, port cut-outs, vent slots, "Pi 4" engraved in DejaVu Sans Bold and a PETG material defined in the design, with its mass in Properties, then height 24 → 32 mm in Parameters and it all rebuilds. 6× playback with the idle pauses cut. [Full video](https://github.com/tangency-labs/solidcraft/raw/refs/heads/main/docs/screenshots/demo.mp4), recorded with the [demo script](examples/demo/record_ui.sh).*

## Download

- **Windows:** the [installer][windows-msi].
- **macOS:** the [disk image][macos-dmg] (Apple silicon and Intel).
- **Linux:** the [AppImage][linux-appimage], or `sudo snap install solidcraft` ([Snap Store][snap-store]).
- **Web:** [try it in your browser][web-app], no install.

ARM64 and 32-bit builds, portable zip, .deb, .rpm, Flatpak and the other packages are on the
[downloads page](docs/downloads.md). The macOS app is signed with a Developer ID and notarized by
Apple. Windows signing: free code signing provided by SignPath.io, certificate by SignPath
Foundation ([code signing policy](https://docs.solidcraft.io/code-signing-policy.html)). Until the
production certificate is in place, SmartScreen may warn: choose **More info**, then **Run anyway**. On SolidCraft 0.11 to 0.14, download once by hand; see
[updating](https://docs.solidcraft.io/guide/updates.html#updating-from-011-to-014).

## What works

Early but usable for simple parts: 75 of 76 public reference parts match their analytic or
FreeCAD-computed volume, area, centre of mass and bounding box ([oracle](docs/oracle.md)); the
other is a tracked bug.

- **Sketches and constraints:** lines, arcs, splines, text and more, solved by our own constraint solver, with parameter-driven dimensions.
- **Features:** extrude, revolve, sweep, loft, fillet, chamfer, shell, draft, holes, threads, patterns, mirror, split and combine on an editable parametric timeline.
- **Parameters and configurations** with unit-aware expressions.
- **Assemblies** with joints, motion studies, contact sets, interference and mass properties.
- **Sheet metal** with flanges, hems, fold/unfold and flat patterns, and plastic enclosure features.
- **Files:** STEP, IGES, 3MF, STL, OBJ and glTF in and out where supported, URDF export, `.sc` designs (`.solidcraft` files from earlier releases still open).
- **Desktop app, browser app, headless CLI and MCP server**, all running the same commands.

The full list is in [docs/capabilities.md](docs/capabilities.md) and [docs/features.md](docs/features.md);
status and plans are in [ROADMAP.md](ROADMAP.md). See also [what people built](docs/showcase.md),
[3D mouse support](docs/3d-mouse.md) and the [architecture](docs/architecture.md).

## Build from source

```sh
cargo run --release -p solidcraft -- --sample      # desktop app with a sample part
cargo run --release -p solidcraft-cli -- commands  # the command registry
cargo run --release -p solidcraft-cli -- run examples/bracket.json --out bracket.step
cargo xtask ci                                     # fmt, clippy, tests, icons, assets, layers, wasm
```

More commands, the Windows cross-build and the browser build are in [docs/building.md](docs/building.md).

## AI agents (MCP)

`solidcraft-cli mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server. Headless it
runs its own design session, or with `--connect` it drives the running app so you can watch.
Claude Code: `claude mcp add solidcraft -- solidcraft-cli mcp`. If a terminal doesn't find
`solidcraft-cli` (older Windows installers don't put it on `PATH`), give its full path (by default
`"C:\\Program Files\\SolidCraft\\solidcraft-cli.exe"`). Tools, access rules and an example session:
[docs/mcp.md](docs/mcp.md).

## Scripts

A script is a JSON list of the same commands the UI runs, executed with `solidcraft-cli run`.
Format and example: [docs/scripts.md](docs/scripts.md). Pages can also be driven through the
[browser command API](docs/browser-api.md).

## Plugins (experimental desktop runtime)

Plugins add separately distributed behaviour without compiling that behaviour into SolidCraft.
The initial runtime loads **WebAssembly components**, explicitly requested by the user. Merely
installing a folder, enabling it in Preferences, or opening a design never executes plugin code.
The interface is experimental: WIT package `craft:plugin@0.1.0`, capability `commands@0.1`. While
it is 0.x a minor version may break the one before it; the host refuses a plugin that needs a
capability it does not have, in plain words. Review a plugin again after an update. The desktop app and headless CLI use the same commands. Browser
execution, automatic downloads and updates, custom panels, and plugin-defined parametric feature
types are future work.

### Install and inspect

A package is a folder named after its id, containing `plugin.json` and a `.wasm` component.
Preferences > Plugins lists packages under the per-user data folder: `%LOCALAPPDATA%/SolidCraft/plugins`
on Windows, `~/Library/Application Support/SolidCraft/plugins` on macOS, and
`${XDG_DATA_HOME:-~/.local/share}/solidcraft/plugins` on Linux. **Show in folder** opens this location.
You can also inspect and explicitly run a package in another folder. Nothing is downloaded by the host.

A minimal manifest looks like this. Plugin packages and examples are maintained outside this
repository, as specified in [the plugin RFC](docs/rfcs/0002-plugins.md).

```json
{
  "id": "example-box",
  "name": "Example box",
  "version": "0.1.0",
  "description": "Create an ordinary editable box from dimensions.",
  "capabilities": ["commands@0.1"],
  "runtime": {
    "module": "plugin.wasm",
    "entries": ["make"],
    "commands": ["solid.box"],
    "limits": {"fuel": 10000000, "memory_mib": 64, "deadline_ms": 5000}
  }
}
```

Run `plugin.inspect` with `{"path":"/path/to/example-box"}` through the CLI's JSON scripts, MCP
`execute`, or the desktop control channel's `engine.execute`. It reads the manifest and component
without executing them, returning the declared commands, limits and an `approval` SHA-256 hash.
Review the author, code and declared rights before using that hash. Inspection does not approve a
package. Existing metadata-only packages remain listable but cannot run until they declare a runtime.

### Run explicitly

After reviewing the inspected package, execute:

```json
{
  "command": "plugin.run",
  "params": {
    "path": "/path/to/example-box",
    "approval": "<exact approval hash returned by plugin.inspect>",
    "entry": "make",
    "params": {"length": 20, "width": 10, "height": 3}
  }
}
```

For example, save that command in a JSON array and use `solidcraft-cli run commands.json`.
For the running desktop app, send it as the `command` and `params` of a standalone
`engine.execute` request (see [control protocol](docs/control-protocol.md)). The native host runs
it on a worker and keeps the window responsive. Stop or a design change discards the pending
result; the host does not automatically rerun external code. Desktop batches and scripts must send
`plugin.run` separately. Headless CLI scripts may include it as a normal command.

The hash binds **both the exact manifest bytes and the component bytes**, so an updated package
requires a fresh inspection and permission review. It is a content identity, not a signature or an
author's reputation. Agent chat asks before executing external plugin code. MCP and the control
channel keep their existing file scopes: allow the package folder explicitly where needed. Naming
a package never grants access to another folder.

### User resource ceilings

The invocation defaults remain **10,000,000 fuel, 64 MiB of guest memory and 5000 ms**.
The default host ceilings remain **100,000,000,000 fuel and 128 MiB**. A package can declare
larger fuel or memory budgets, but execution is refused until you explicitly raise the host
ceilings and approve that exact package content. `plugin.inspect` is inert and reports the
requested limits, current `host_limits`, `within_host_limits` and any `limits_error` even when
a request is too large.

In the native GUI, open **Preferences > Plugins**, enter **Max plugin fuel** and
**Max plugin memory (MiB)**, then choose **Apply** or **OK**. Enter also accepts both values;
**Cancel** reverts changes since the last Apply and **Restore defaults** resets this page.
The settings persist in your user preferences and do not travel with saved designs.

For standalone CLI or headless MCP sessions, create a UTF-8 JSON file, for example
`plugin-limits.json` containing:

```json
{"max_fuel": 200000000000, "max_memory_mib": 512}
```

Pass it explicitly when starting the process:

```text
solidcraft-cli --plugin-limits plugin-limits.json run inspect-plugin.json --results
solidcraft-cli --plugin-limits plugin-limits.json mcp --headless --allow-path /path/to/plugins
```

Missing fields use the original host ceilings. Connected desktop sessions use the GUI's
preferences; CLI settings cannot override them. Positive integer fuel values up to `u64::MAX`
are accepted without rounding. Guest memory is limited to 4096 MiB by the current Wasm32 ABI;
that technical boundary requires a different runtime ABI to change. Neither setting changes
a package's declared budget, executes it, adds permissions, or retries a failed invocation.
Review its manifest and component, obtain the content approval hash, and explicitly invoke it.
A changed manifest budget requires a fresh approval hash. Keep the selected ceilings within your
machine's available resources; they are limits on guest execution, not total SolidCraft memory
or native CAD work. The independent maximum execution deadline stays 30 seconds.

### Permissions, results and saved designs

A plugin can call any command in the MCP catalog whose exact id its manifest lists. Commands that
reach past the open design are grouped: `file` (saving, exporting, importing, inserting from files,
libraries), `doc` (opening or switching designs), `app` (settings, window, diagnostics) and `plugin`
(inspecting or running plugins). Each group is refused unless the manifest requests it in
`runtime.permissions.command_groups` and the user grants it; this version has no grant UI yet.
Filesystem calls require matching manifest permissions and approved folder grants. The clock
requires its manifest permission; otherwise it returns a fixed epoch. No WASI, socket or process
API is linked. A design container reserves `plugins/<plugin-id>/` for a plugin's own data, kept
and saved back untouched when the plugin is missing; plugins cannot read or write it until the
`storage` capability arrives.
Inspection commands can disclose model data, so review them as permissions too. In particular, the optional
`model.mesh` permission exposes the requested body's evaluated vertices and triangles in its owning
component's coordinates (millimetres), with explicit transfer limits. It performs no conversion.

Edits run against an isolated copy of the design. Only a successful call is adopted, as **one Undo
step**. A trap, exhausted fuel, memory limit, invalid result or refused host call discards the
transaction, preserving the previous design and undo/redo history. The host validates produced
native features before accepting them. A plugin that catches a refused host call cannot commit an
earlier partial edit. CPU-heavy guest loops are bounded by fuel and a deadline; the guide explains
the separate cost of compiling a component and executing a granted native command.

The result is ordinary native features: they save, undo, redo and rebuild without the plugin.
Removing the package disables future invocations; it does not remove the features already created.
Changing the plugin's own parameters means another invocation in this first version; there is no
plugin-owned timeline evaluation hook yet. Turn an installed plugin off in Preferences > Plugins
to refuse subsequent desktop calls. Disabling it or changing resource ceilings during a pending
call prevents adoption. CLI
processes have their own session policy and do not read the desktop app's preferences.

See the [runtime and author guide](docs/plugin-runtime.md) for the complete manifest, supported
command boundary, limits, build options, tests and current restrictions.
For step-by-step package checks and expected results, see the [English test guide](docs/plugin-test-guide.md). The existing
[parametric plugin design](docs/plugins.md) remains a separate future phase.

## Contributing

Help is welcome. [CONTRIBUTING.md](CONTRIBUTING.md) covers how to build and test, the code map,
the clean-room and licence rules, and where help is wanted most. Open work is in
[GitHub issues](https://github.com/tangency-labs/solidcraft/issues); chat is on
[Discord](https://discord.gg/FtU4NQNVDH).

## Sponsors

[![CI powered by Blacksmith](docs/sponsors/blacksmith-ci-powered.png)](https://www.blacksmith.sh)

SolidCraft's continuous integration runs on [Blacksmith](https://www.blacksmith.sh), which
sponsors its CI minutes.

## Disclaimer

The files in this repository are provided as is, without warranty, liability or indemnity, and
Tangency Labs and the contributors accept no responsibility for any harmful or unlawful use of
them: read [DISCLAIMER.md](DISCLAIMER.md).

## Licence

MIT OR Apache-2.0, at your option. Developed under the
[clean-room contribution rules](AGENTS.md); third-party code and assets retain their
documented licences and attribution.

SolidCraft's brand and interface are licensed separately, under
[LICENSE-BRAND](LICENSE-BRAND): the UI kit, the look (interface icons, glyphs, built-in themes),
the theme's design tokens and palettes, and the SolidCraft name, logo and app icon. Use them freely
as part of SolidCraft. Using them in another program or product needs the maintainers' written
permission first.

SolidCraft is independent. Behaviour is designed from public documentation, general CAD
conventions and user feedback. Reference tests use analytic values and open-source tools.
SolidCraft is not affiliated with, sponsored by or endorsed by Autodesk, Dassault Systèmes, PTC,
Siemens or any other CAD vendor. Fusion, AutoCAD, Inventor, SolidWorks, Onshape, Creo, Blender and
Tinkercad are trademarks of their respective owners, named only to describe compatibility
(see [NOTICE](NOTICE)).

[badge-windows]: https://img.shields.io/badge/Windows-Download-0078D6?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxwYXRoIGQ9Ik0yIDJoOS41djkuNUgyek0xMi41IDJIMjJ2OS41aC05LjV6TTIgMTIuNWg5LjVWMjJIMnpNMTIuNSAxMi41SDIyVjIyaC05LjV6Ii8%2BPC9zdmc%2B
[badge-macos]: https://img.shields.io/badge/macOS-Download-333333?style=for-the-badge&logo=apple&logoColor=white
[badge-linux]: https://img.shields.io/badge/Linux-AppImage-E95420?style=for-the-badge&logo=linux&logoColor=white
[badge-snap]: https://img.shields.io/badge/Snap-Snap_Store-82BEA0?style=for-the-badge&logo=snapcraft&logoColor=white
[badge-web]: https://img.shields.io/badge/Web-Try_in_browser-2EA44F?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0ibm9uZSIgc3Ryb2tlPSJ3aGl0ZSIgc3Ryb2tlLXdpZHRoPSIyIj48Y2lyY2xlIGN4PSIxMiIgY3k9IjEyIiByPSIxMCIvPjxwYXRoIGQ9Ik0yIDEyaDIwTTEyIDJhMTUgMTUgMCAwIDEgMCAyME0xMiAyYTE1IDE1IDAgMCAwIDAgMjAiLz48L3N2Zz4%3D
[windows-msi]: https://github.com/tangency-labs/solidcraft/releases/latest/download/solidcraft-windows-x64.msi
[macos-dmg]: https://github.com/tangency-labs/solidcraft/releases/latest/download/solidcraft-macos-universal.dmg
[linux-appimage]: https://github.com/tangency-labs/solidcraft/releases/latest/download/solidcraft-linux-x86_64.AppImage
[snap-store]: https://snapcraft.io/solidcraft
[web-app]: https://solidcraft.io/app

