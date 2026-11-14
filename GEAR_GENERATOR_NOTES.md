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
