# Gear Generator: working notes (resume here)

Last updated 2026-11-14, before a PC restart. Integration branch: `gear-generator` (local commits
only, nothing pushed). Fork: github.com/ariankhatb/solidcraft, upstream tangency-labs/solidcraft.

## Where things are

| Piece | Location |
|---|---|
| Gear math + build plan (pure, fast tests) | `crates/gear` |
| Commands `gear.create`, `gear.calculate` | `crates/engine/src/cmd/gear.rs`, `schema/gear.rs` |
| Gear Generator window | `crates/ui-egui/src/gear_window.rs` (opened by `start("gear.create")`) |
| UI strings | `assets/locales/pending/gear-generator.json` |
| Built app / CLI | `target/release/solidcraft.exe`, `solidcraft-cli.exe` (built 15:34, includes everything merged below) |

Merged into `gear-generator`: base generator, bevel/ring fixes, `gear-strength` (ISO 6336 ratings,
no UI controls yet), `gear-robust` (sweep tests, exact fitted flanks, refusals), `gear-mesh2d`
(mesh phase sign fix, internal/rack/cycloidal fixes, 2D mesh checker). 48 gear-crate tests pass.
The engine's own gear tests and the UI test suite have never completed a run.

## User feedback from the first test build (2026-11-14)

Good: spur, helical, herringbone, internal, rack, pair, planetary, straight bevel, crossed
helical, cycloidal, cycloidal drive.

Broken (priority order):
1. **Spiral bevel**: gears don't mesh (46 mm^3 overlap measured on defaults).
2. **Hypoid**: "really messed up" (old straight-toothed hyperboloid version, 92 mm^3 overlap).
   Needs the real spiral hypoid (car-differential kind).
3. **Worm and wheel**: messed up (261 mm^3 overlap). User wants them to match "almost like one
   unit": a throated/enveloping wheel that wraps the worm, correct hand, phase and centre distance.
4. **Previews out of sync at times**: the 2D animated preview and/or the 3D view preview don't
   always agree (with each other, or between members). Investigate phases, speeds and stale
   3D preview after edits.

New requirements:
- **3D-print tooth profile** as a selectable standard next to ISO 53 / AGMA / stub, designed for
  printing and tunable: e.g. bigger root fillet, tip rounding, default backlash and clearance for
  FDM, horizontal-expansion (XY) compensation, minimum tip/tooth thickness, elephant-foot relief.
- **Chamfer settings**: tooth edge chamfers (both faces), bore/hub chamfers.
- **Shaft, not just bore**: a gear can carry an integral shaft.
- **Dual shaft mode**: a shaft on each side of the gear, each with its own dimensions
  (diameter, length, and likely end features such as D-flat/keyway/chamfer).
- **Interactive UI builder is the main workflow**: axis-first (pick the two shaft axes, the
  generator picks families that fit, sizes and places the gears on those axes).
- **Regenerate without breaking the model**: a created gear can be reopened in the builder and
  regenerated in place, keeping stable references (body names, feature ids/names, group) so
  features built on it don't break. This replaces the native parametric feature.

Added later the same day:
- **Clearance and more parameters**: expose clearance as its own setting (tip/root clearance
  coefficient c*, plus radial/axial clearance for printed sets), and other tooth parameters that
  are currently hidden or fixed (to be listed: tip shortening, root fillet radius, tip rounding,
  face-width offset between mates, per-gear backlash vs. total backlash).
- **Colour-code and label the gears**: each member (gear 1, gear 2, sun, planets, ring, worm,
  wheel...) gets its own colour and a label, used consistently in the 2D preview, the 3D view
  preview and that member's settings section in the window (coloured section header/marker).
  Hovering or editing a member's settings highlights that gear in the previews, and vice versa.

Batch 1, written but NOT compiled for the engine/UI (gear-crate tests pass, 48/48):
- `print_fdm` basic rack (dedendum 1.3, full root fillet) + "print preset" toggle in the window
  (25 deg pressure angle, that rack, backlash >= 0.15 mm) with a nozzle-width field and a
  warning when a tip land is under two nozzle widths; new "tip land" fact per gear.
- "Clearance x m" row (edits the dedendum).
- Member name labels in the 2D preview, in each member's colour.
- "Load and materials" section in the window feeding the merged strength rating.
First compile of these is still to do: expect small errors in `gear_window.rs`.

Suggestions the user liked enough to ask for "the easy ones" (the rest still open): mesh-check
badge in the window (the 2D checker `mesh2d.rs` is test-only; needs to be made public and told
which members mesh), print test coupon, printer presets, design-by-goal, saved presets and
design sheet/DXF, joints + housing features, hub hardware (set screws, nut traps, heat-set
inserts, bearing pockets), tip relief and crowning.

Decisions:
- **Do not rebuild the app until a large batch of features is ready to test at once** (user,
  2026-11-14). Fast gear-crate tests are fine; batch engine/UI compiles.
- The native parametric `FeatureKind::Gear` work (branch `gear-parametric`) is **paused, not
  needed**. Re-point that agent ("timeline rebuilder") at regenerate-in-place with stable
  references.
- Don't run many full builds at once: 32 GB RAM, each engine compile takes 3-5 GB and 15-30 min.
  At most two engine-building agents at a time; gear-crate-only agents are cheap.

## Branches and worktrees (all under `C:\Users\Arian\workspace\projects\`)

| Worktree | Branch | State |
|---|---|---|
| `solidcraft` | `gear-generator` | integration branch, clean |
| `solidcraft-mesh3d` | `gear-mesh3d` | 3D meshing fixes (worm, bevel, spiral bevel, crossed); WIP commit, unfinished |
| `solidcraft-hypoid` | `gear-hypoid` | spiral hypoid rewrite; WIP commit, unfinished |
| `solidcraft-axes` | `gear-axes` | axis-first workflow; WIP commit, unfinished |
| `solidcraft-extras` | `gear-extras` | sprockets, timing pulleys, splines, face gears; unfinished |
| `solidcraft-parametric` | `gear-parametric` | native parametric feature; PAUSED, to be re-pointed |
| `solidcraft-strength`, `-robust`, `-mesh2d` | merged | worktrees can be removed |

All of these branched before the strength/robust/mesh2d merges, so each needs
`git merge gear-generator` (expect conflicts in `plan.rs`, `involute.rs`, `cmd/gear.rs`,
`gear_window.rs`; `PreviewGear` gained `orbit`, `Step` gained `Taper`, `Plan` gained `refuse`).

## Next steps after restart

1. Fix the three broken sets (spiral bevel, worm + enveloping wheel, hypoid) with a 3D mesh proof:
   zero overlap AND real contact, through several mesh positions.
2. Fix preview sync.
3. 3D-print profile, chamfers, shafts and dual shafts (gear crate + window).
4. Axis-first builder as the main window flow; regenerate-in-place with stable references.
5. Then: extras (sprockets, pulleys, splines), strength controls in the window.

## Useful commands

- Fast math tests: `cargo test --release -p solidcraft-gear` (about 45 s).
- Build: `cargo build --release -p solidcraft -p solidcraft-cli` (about 30 min when the machine is busy, less alone).
- Overlap check: script `[{"command":"gear.create","params":{"type":"worm"}},{"command":"inspect.interference","params":{"budget_s":120}}]`
  run with `target\release\solidcraft-cli.exe run script.json --results`.
- Picture: `solidcraft-cli.exe snapshot script.json --out x.png --view iso --no-grid`.
- Drive the app: start with `--control 7979`; token in `%LOCALAPPDATA%\SolidCraft\control\7979.token`
  (Store Python can't read it: read it in PowerShell and pass it in); `ui.start {command:"gear.create"}`, `ui.screenshot {path}`.

## After the restart (2026-11-14, evening)

Rule from the user: **one rebuild at a time**. No agent builds or checks the engine, UI or app;
agents only run `cargo test --release -p solidcraft-gear`. The only exception is the builder
window agent, which may run a single `cargo check -p solidcraft-ui-egui` when nothing else is
compiling. The integrator does one full build when a large batch is ready.

Agents running (fresh agents; branches merge `gear-generator` first):

| Worktree / branch | Job |
|---|---|
| `solidcraft-mesh3d` / `gear-mesh3d` | pure-math 3D mesh checker (`mesh3d.rs`); fix spiral bevel, worm + throated wheel, verify bevel and crossed helical |
| `solidcraft-hypoid` / `gear-hypoid` | real spiral hypoid from existing step kinds, proved with a 3D check |
| `solidcraft-body` / `gear-body` | shafts and dual shafts, chamfers, tunable 3D-print profile, clearance and tooth parameters, hub hardware |
| `solidcraft-axes` / `gear-axes` | owns `gear_window.rs`: axis-first builder, regenerate in place (`gear.update`), colour-coding and highlight, preview sync |
| `solidcraft-extras` / `gear-extras` | sprockets, timing pulleys, splines, face gear |
| `solidcraft-tools` / `gear-tools` | public mesh-check API for a window badge, print test coupon, design-by-goal, design sheet and DXF |

`gear-parametric` stays paused. Merged worktrees (strength, robust, mesh2d) were removed.
Integration order when they report: body, tools, extras (gear crate, additive) -> mesh3d ->
hypoid -> axes last (it wires the UI for everyone's new fields), then one build, kernel overlap
check on every set, screenshots, launch.

## Equation-driven teeth (user decision, 2026-11-14 evening)

User: "I want the calculations to be parametric, not meshes if possible" and picked
equation-driven teeth: flanks drawn with SolidCraft's `sketch.equation_curve` (the involute and
trochoid formulas in the design's expression language) instead of splines through computed
points, optionally driven by design parameters so Change Parameters reshapes the teeth natively.
Planar tooth kinds (spur, helical, herringbone, internal, pair, planetary, rack) first; bevel,
spiral bevel, hypoid and worm wheel keep fitted splines (no closed-form planar section).
Agent worktree: `solidcraft-eqn` / `gear-equations`.

## Status 2026-11-14, 18:30

Build of 18:03 (`target/release`) contains everything except equation-driven teeth. Kernel
results on that build (`inspect.interference`, defaults): pair, planetary, bevel, spiral bevel,
crossed helical, worm, cycloidal: no overlap; cycloidal drive 0.001 mm^3 and face gear
0.014 mm^3 (touching contact); **hypoid 12.4 mm^3 (about 0.04 mm deep over 290 mm^2), being
fixed on `gear-hypoid`** (its math checker said 3.6 um, so the kernel's loft differs from the
checker's model). All single-gear kinds build.

Merged since that build (needs the next build): `gear-equations` — `tooth_curves:
fitted|equations`, `parametric`, `parameter_prefix`; planar kinds only; default still fitted.
Verified on the kernel with scripts (volume and a teeth 20->24 parameter change), but the engine
path and its new engine tests have never been compiled into a build or run.

Next build: after the hypoid fix lands. Also run the engine gear tests once
(`cargo test --release -p solidcraft-engine gear` — compiles all engine tests; do it when nothing
else is compiling).

## User feedback 2026-11-14, ~18:45 — UI redesign (priority)

"I want the UI to be a little faster. We don't want to generate the gears immediately. We should
do the preview first, more realistically though, even including the 3D aspect … use a drawing
setup where you're able to change the angles … just input the angle and it'll happen … design by
outputs … I don't want the 0.3 whatever 3D printed thing in the settings; it makes it look too
complicated. Have a simple mode where you just say the diameters you want, or the angle of the
shaft … there is no actual 3D preview; the one that generates is too slow, but we don't need to
generate everything. Generating the angles between the gears, the shafts, all of that is easy
compute, so we can do that without the teeth."

Plan:
1. No kernel builds while designing: the live `gear.create` view preview becomes an on-demand
   button; only Create builds solids.
2. An instant, orbitable 3D preview inside the window drawn from the math (shafts/axes, angles,
   pitch cylinders/cones/hyperboloids, gear blanks, optional quick toothed extrusions).
3. A schematic drawing of the set with editable dimensions (shaft angle, offset, centre distance,
   diameters): click a dimension, type a value, everything updates.
4. Simple mode (default): design by outputs — diameters, ratio, shaft angle, centre distance or
   offset; the solver picks module/teeth/shifts. Advanced mode holds everything else (basic rack,
   print tuning, hardware, chamfers, tooth tuning).
Agents: `gear-simple` (outputs solver, gear crate) and `gear-ui2` (window: modes, 3D view,
drawing, no auto kernel preview).

### Refinement (same evening): the shafts decide the type

User: "Instead of choosing between hypoid [and the rest], we want an automatic mode selector …
we are given two shafts. One is coming through the plane of the screen. The other one is placed
by hand — draw it, 2 points 1 and 2, roughly, then dimension it … and that will give you exactly
what you need, whether it's going to be a bevel or a hypoid or whatever in between. … I should be
able to draw this out first using sketches — a sketch with a dot and an angled shaft — and that's
what you select in the program, and that will generate the gear. … Apply this logic to the other
ones as well."
- Simple mode input = the drawing: shaft 1 a dot, shaft 2 two points, then dimensions (angle,
  offset, position). Type is a label (override only when several fit).
- Same axis → single gear; parallel → pair; intersecting → bevel; skew → hypoid (small offset;
  offset 0 = spiral bevel) / worm or crossed helical (large offset).
- Axis slots accept sketch points (axis normal to the sketch plane) and sketch lines.
Sent to `gear-ui2` (window, engine axis references) and `gear-simple` (`auto_kind`, two-shaft
`Simple`).

### Clearance body for housings (same evening)

User: "this should also output … the cutout of the gear without the tooth … the surface of the
gear if you were to Boolean cut it out of a block. And it all has to be just one geometry, so
it's easy to make a housing: you make whatever shape you want and then Boolean-cut whatever this
gear set requires out of it. … Add one bearing [size] … load bearings as a library … select it
… cut that out for all the places you want, which are dots on the shaft … add that to the
housing-side design. But it's mostly the Boolean part of all of the clearance needed for the
assembly."
- One body: every member's swept tip envelope (cylinder/cone/hyperboloid/worm tube) plus
  radial and axial clearance, the shafts, and bearing seats (one bearing size from the library,
  placed at dots on the shafts). Other housing features stay manual for now.
Agent worktree: `solidcraft-keepout` / `gear-keepout`.

### Sketch rules (confirmed by the user) — circles are pitch diameters, lines are shafts

One sketch = gear 1's plane. A circle is a gear's pitch circle (Ø = pitch Ø), its shaft through
the centre normal to the sketch unless a line says otherwise.

| Drawn | Set | Sizes |
|---|---|---|
| one circle | single gear (spur/helical by helix field) | pitch Ø |
| two circles touching outside | parallel pair (helix field: helical/herringbone) | both pitch Ø; centre distance |
| circle touching inside a bigger one | internal pair | both pitch Ø |
| centre circle + circles around it inside a big circle | planetary | sun, planets, ring |
| circle + tangent line | rack and pinion OR worm and wheel (one choice; worm Ø a field or a second parallel line) | line = pitch line |
| circle + line through its centre | bevel, 90° | line = shaft 2 seen from above; meets shaft 1 at the apex |
| circle + line crossing the circle off-centre | hypoid | offset = line's distance from centre |
| circle + line missing the circle | crossed helical | offset = line's distance from centre |
| second circle in another sketch touching the first | bevel at any angle | both pitch Ø; shaft angle = angle between sketch planes |

Face width: a field per gear (default ≈ 10×module; 30% of cone distance for bevel/hypoid),
shown as a dimension. Mate size for bevel/hypoid with one circle: ratio field (or the side-sketch
circle). Non-90° bevel/hypoid: angle field or the two-sketch form.
- Bevels at any shaft angle (user's example: 80°) must be easy in Simple mode: a shaft-angle
  dimension on the bevel's side-view drawing (default 90°), side-view sketch input (two lines
  meeting at the apex), cone angles and apex drawn. The generator already supports 5–175°.

### Mode first, then pick from the sketch; new mode: bevel planetary (user, same evening)

User: keep the mode selection for now; after choosing the mode, select the components from the
sketch that the mode needs. New set: "something similar to a sun gear and planet gears around
it, but the planets are made at a cone shape, so each one goes in a different direction, not
perpendicular or parallel to each other."
- **Bevel planetary**: a bevel sun; N bevel planets whose axes meet the sun axis at one apex at a
  cone angle (90° = differential spider gears); optional second sun opposite (full
  differential) or an outer bevel ring. Sketch: sun circle + one line per planet from the centre
  (azimuths) + cone angle dimension (or a side-view sketch: two lines meeting at the apex);
  planet size from a second circle on a spoke or a ratio.
- Window: mode picker stays; each mode lists its pick slots (circle / line / point / second
  sketch) and validates picks; auto-detection only as a hint. Per-mode slots:
  single gear: 1 circle; pair: 2 touching circles; internal: circle inside circle; planetary:
  sun/planet/ring circles; rack/worm: circle + tangent line; bevel: circle + line through centre
  (or 2 side-view lines); hypoid: circle + off-centre crossing line; crossed helical: circle +
  missing line; bevel planetary: sun circle + planet lines + cone angle.
Agent worktree for the new kind: `solidcraft-bevelplanet` / `gear-bevelplanet`.
- **Correction: ONE sketch only**, basic geometry (circles, lines). No second sketch, no side-view
  sketch. Out-of-plane angles (bevel/hypoid/crossed shaft angle, bevel planetary cone angle) are
  settings (default 90°). Counts are settings: draw one planet (one line from the sun's centre
  for bevel planetary, one planet circle for classic planetary) and set the planet count.
- **Origin dot, always**: every mode's first pick is a sketch point = origin of the set; default
  orientation follows the sketch (circle modes: shafts normal to the sketch; line modes: shafts in
  the plane); rotate about the origin in 90° steps (+ free angle) to align to the wanted plane.
- **Bevel and bevel planetary drawn as a side view** in the one sketch: a line (sun/gear 1 shaft),
  the dot on it (apex = origin), a second line from the dot at an angle (planet/gear 2 shaft);
  the angle between the lines is the meeting angle (drawn). Sizes, planet count, face width are
  settings. Hypoid stays circle + off-centre line with the out-of-plane offset as a setting.
- Optimization agent (user request: "faster and smoother"): worktree `solidcraft-perf` /
  `gear-perf` — kernel build path per kind, plan/check_mesh/solver costs; measure before/after.

## Integrator plan while the user is away (from ~19:15, back in a couple of hours)

User: "whenever that is done, rebuild everything yourself … if the first rebuild goes wrong, redo
it … if it finishes and I'm not around, double check everything and rebuild again if there are
bugs." Steps: merge each agent branch as it lands (gear tests + cargo check after each) → full
release build → kernel overlap check on every kind, engine gear tests, renders, window
screenshots (Simple and Advanced) via the control channel → fix and rebuild until clean →
summary for the user.
