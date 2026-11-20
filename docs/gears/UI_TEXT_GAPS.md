# Gears dialog: UI text gaps

Places where the dialog's explanation, tooltip or message is missing, confusing, or doesn't match
what the code does (and so doesn't match the guide). Each entry gives the file, the **current
text** and a **suggested text**, ready for whoever edits the dialog. File paths are under
`crates/ui-egui/src/` unless given in full.

These are from reading the code at `2ee452a9` (branch `gear-drivetrain-final`). Nothing was changed
in the code. Where the code and `GEAR_UI_SPEC.md` disagree, the guide follows the code.

Priorities: **P1** says something that isn't true, **P2** missing or confusing, **P3** wording.

> **Status:** the dialog's second round (merged after `2ee452a9`) applied these tooltips and
> wording, and made the "settings that have no effect yet" either work (starts, spiral angle, rim,
> back height, auto hub Ø) or go (throated, keyway; profile shift is now read-only). This file is
> kept as the record of what was found.

## Settings that have no effect yet

The fields below are typed, checked and sent with the request, but the drivetrain planner
(`crates/gear/src/plan/system.rs` and `drivetrain.rs`'s `Ask`) never reads them for a set made of
items. Either wire them up or hide them / mark them "(not applied yet)". The guide says so for each.

| Field | Where it's dropped | Suggestion |
|---|---|---|
| More › **Profile shift** (per gear) | `items::GearSettings::profile_shift` is never read; the shift is always worked out (`sketch_rules::size_train`, `size_stage`) | **P1.** Remove the field, or show the worked-out shift read-only: label "Profile shift", caption "0.32 (worked out to hold the drawn centre distance)". |
| More › **Bore: None** | `gear_items.rs::gear_details` only reads `Bore::Diameter`; Auto and None both give no hole from the setting, and a built shaft still sets the bore | **P1.** Keep Auto and Ø only, or make None suppress the shaft bore. |
| More › **Hub** with **Hub Ø** left on auto | `gear_details` takes `hub.diameter`; `None` → no hub is built | **P1.** Either give Auto a value (e.g. bore + 2 × module, or 1.6 × bore) or make the field required: hint "mm" instead of "auto", and refuse "type the hub Ø, or draw a hub circle". |
| **Hand** Left / Right (trains) | `Ask` has no hand; `size_train` sets hands itself (opposite across a mesh). Only a planetary set reads `hand` | **P2.** Hide Hand except on a planetary item, or apply it to the train's gear 1. |
| Ring Gear **Rim** | `GearSettings::rim` not read by `plan_items` | **P2.** Wire into the internal stage's `Spec::rim`, or hide. |
| Worm **Starts**, **Throated wheel** | `select.rs` sets `starts_free: true`; starts are chosen from the ratio (`axes.rs::worm`) | **P1.** Wire `starts` into the worm pair's `Drive`/`Spec`, or hide. Example `05`/`09` set `starts` and it's not used. |
| Bevel/Hypoid **Spiral angle** | `GearSettings::spiral_angle` not read; `Spec::spiral_angle` comes from defaults | **P2.** Wire it, or hide. (Style Straight/Spiral/Chevron *is* applied.) |
| Rack **Back height** | not read | **P2.** Wire into the rack's `Spec`, or hide. |
| Shaft **Keyway** | not read | **P3.** Hide until it works. |

## Tooltips that say something the code doesn't do

| # | File / constant | Current text | Suggested text |
|---|---|---|---|
| 1 (P1) | `dialogs_gear_view.rs` `TIP_FACE` | "How far the teeth reach each side of the sketch plane, along the axis. Auto ≈ 10 × module, limited by the room on the shaft." | "How far the teeth reach from the sketch plane along the axis, on this side. Auto: 5 × module each side (10 × module in all); A only / B only: 10 × module on that side." (`items::face_extent` has no shaft-room limit.) |
| 2 (P2) | `gear_rows`, label "Each side" | label "Each side", hint "auto" | Keep the label; tooltip add: "The value is per side: 10 mm gives a 20 mm face." |
| 3 (P1) | `TIP_SHIFT` | "Moves the tooth out to avoid undercut on small gears. Auto does it below 17 teeth." | If the field stays: "Worked out for you: it holds the drawn centre distance and clears undercut on small gears." (see the table above) |
| 4 (P1) | `TIP_BORE` | "Hole through the gear. Auto fits its shaft." | "Hole through the gear. Auto: fits the shaft when a shaft line is picked; otherwise none, unless a bore circle is drawn. Ø: this diameter. A drawn bore circle wins." |
| 5 (P2) | `TIP_MODULE` | "Tooth size in mm (pitch Ø ÷ teeth). Auto picks a standard module so the smallest gear has ≥17 teeth." | "Tooth size in mm (pitch Ø ÷ teeth). Meshing gears share one module: set it on one gear and its mates follow. Auto: the largest standard module giving the smallest meshing gear 17 teeth or more." |
| 6 (P2) | `TIP_TEETH` | "Number of teeth. The pitch circle stays as drawn, so the module follows." | add: "Its mates take the same module." |
| 7 (P2) | `TIP_ACCEPT` | "Make every suggested link real. Suggested = circles the drawing shows as touching or on one line." | "Make every suggested link real. Suggested: circles that touch in the drawing (mesh) or gears on one axis (same shaft)." ("on one line" reads like a sketch line.) |
| 8 (P2) | `dialogs_gear_graph.rs` `ACCEPT_TIP` | "Make every dashed link real. Dashed = circles the drawing shows as touching or on one line." | Same as #7. |
| 9 (P2) | `dialogs_gear_graph.rs` `RATIO_TIP` | "Click two gear rows to see the ratio between them (Ratio, under the graph)." | "Click two gear rows to see the ratio between them (Ratio from / to, above the graph)." The ratio rows are above "Show graph" now. |
| 10 (P3) | `dialogs_gear_graph.rs` `EMPTY` | "Items appear here. Drag from a ● Mesh pin to another to make them mesh." | "Gears appear here. Drag from a ● Mesh pin to another gear's to make them mesh." |
| 11 (P2) | `TIP_TYPE` vs spec | "What the next gear will be. Each type lists the sketch pieces it needs." | OK. (Spec §5 still says "item".) |
| 12 (P2) | Bevel/Hypoid **Style** row uses `TIP_V_BEVEL` | "Chevron bevel: the spiral changes hand at mid-face." | "Straight: straight teeth. Spiral: curved teeth, smoother and quieter. Chevron: the spiral changes hand at mid-face, so the axial thrust cancels." |
| 13 (P2) | `TIP_EQUATIONS` | "Exact involute curves instead of fitted splines: slower, smoother." | "Exact involute curves instead of fitted splines: slower, smoother. The set is then made as formula features, not one Gears feature, and an existing Gears feature can't be switched to it." (`gear_feature.rs::wanted` / `update` refuse it: "this set is one Gears feature: its teeth can't be drawn as formulas, and it can't go back to a folder of features".) Better: disable the checkbox when editing a Gears feature. |

## Missing tooltips

| Row | Suggested tooltip |
|---|---|
| **Rim** (Ring Gear), hint "auto: 2.5 × module" | "Rim thickness outside the tooth roots. Auto: 2.5 × module." |
| **Throated wheel** | "The wheel's rim wraps round the worm for more contact." |
| **Back height** (Rack) | "Material behind the rack's tooth roots. Auto from the module." |
| **Carrier** (Planetary) | "Make a carrier body: the planets turn on its pins. Off: the planets have no carrier." |
| **Shaft Ø** | "Shaft diameter. Auto: from the smallest gear's root." |
| **Keyway** | "A keyway along the shaft." |
| **Hub** checkbox | "A boss on the gear's face round its bore. A drawn hub circle sets its Ø." |
| **Hub Ø** | "Hub diameter (mm). Smaller than the tooth roots." |
| **Hub length A / B**, hint "0" | hint "auto"; tooltip "How far the hub reaches past the face on side A / B. Both empty: half the face width on side A." |
| **Bore Ø** (uses `TIP_BORE`) | "Bore diameter (mm). Must stay inside the tooth roots." |
| **Input torque** (reuses `TIP_RPM`) | "Optional: the driver's torque in N·m. Fills torque for every gear in Results (losses included)." |
| **Input rpm**, hint "optional" | hint "rpm, optional". |
| **Driver** "—" option | add to `TIP_DRIVER`: "— : none; the assembly is driven from the first gear." |
| **Gear** tabs (multi-gear items) | Current: "Each gear of this item has its own size, face and helix." OK. |

## Ratio readout

| # | Where | Current | Suggested |
|---|---|---|---|
| R1 (P1) | `dialogs_gear.rs::ratio_lines` | Shows only `ratio` ("0.05 : 1, opposite direction" for a 20 : 1 worm). Users read "20 : 1" as a reduction. | Add a line from the reply's `reduction`: "Reduction  20 : 1" (or show "1 : 20"). Tooltip on the Ratio row: "Speed of the second gear ÷ speed of the first." |
| R2 (P2) | `ratio_lines`, Path | Path names gears as "2·1 → 3·1" (`item + 1 · gear + 1`). Everything else in the dialog uses letters. | Use `Gear::gear_name`: "A → B1 → B2 → C". |
| R3 (P3) | Drivetrain link rows | "A ⟷ B mesh 0.67 : 1 opposite": the direction words are "opposite" / "same way", the ratio readout says "opposite direction" / "same direction". | Use one pair everywhere: "opposite" / "same way". |
| R4 (P3) | Results row "z30 · m2 · Ø60.00 · 0.67 : 1 · …" | The ratio has no label. | "z30 · m2 · Ø60 · speed 0.67 × driver · …". |

## Messages

| # | Where | Current | Suggested |
|---|---|---|---|
| M1 (P2) | `dialogs_gear.rs::refresh` near-miss | "A and B are 0.4 mm apart, so they won't mesh. Make them tangent in the sketch to mesh them." | "Gears A and B are 0.4 mm apart, so they won't mesh. Make the circles Tangent in the sketch, or leave them unlinked." (other messages say "Gear A") |
| M2 (P2) | Gear names in planner messages (`items::who`) | "Gear B2", "Gear W2" for a pair's second gear | The dialog names these "B ring", "W worm" (`dialogs_gear_items::gear_part`). Translate in `dialogs_gear::worded` (e.g. "Gear W2" → "Gear W worm"), or give `who` the part names. |
| M3 (P2) | Engine missing-pick errors (`gear_items.rs::thing_of`) | "item B, pitch: curve `b` is gone from sketch 'Pitch circles': pick the pitch again, …" | Use the slot's label (`Slot::what`): "Gear B, pitch circle: curve b is gone from sketch 'Pitch circles': click Gear B's card, then its replacement in the view." Raw slot names (`pitch`, `axis2`) and backticks leak into the UI. |
| M4 (P2) | Engine bore/hub messages | "item A, gear 1: the bore (Ø18) reaches the teeth's root (Ø17.5): it is left out" | "Gear A: the bore circle (Ø18) reaches the tooth roots (Ø17.5), so no bore is cut. Draw a smaller bore circle." |
| M5 (P3) | `dialogs_gear.rs::output` | "Cut Keep-Out cuts the gears' room out of the Targets: pick the housing bodies (Output › Targets)" | "Cut keep-out from Targets needs the housing bodies: pick them under Output › Targets." (matches the checkbox label) |
| M6 (P3) | `items::no_room` | "this Spur has no room for another pitch circle: start a new item for it" | "Gear A (Spur) has no room for another pitch circle: press New gear for it." ("item" is the engine's word) |
| M7 (P3) | `items::no_room` point case | "a bearing point goes on a Shaft item's line" | "a bearing point goes on a shaft line: pick the gear's shaft line first, or make a Shaft gear." |
| M8 (P3) | `plan_items` empty | "add a gear item: pick a pitch circle" | "Add a gear: press New gear and click a pitch circle." |
| M9 (P2) | `ratio` errors pass through `worded()` | `worded` turns every "item " into "gear ", so "item B has no gear 2" becomes "gear B has no gear 2". | Replace only the "Item X"/"item X" pattern (a capital-letter id after it). |

## Labels and layout

| # | Where | Current | Suggested |
|---|---|---|---|
| L1 (P3) | Panel title | "Gear Generator" (the toolbar button says "Gears", `Gear::title()` returns "GEARS") | "Gears", one name everywhere. |
| L2 (P3) | Output checkbox | "Assembly (components, joints and gear ratios)": truncated to "Assembly (components, joints and …" in a normal-width panel | "Assembly (joints and gear links)". |
| L3 (P2) | Bore/hub circles | Taken automatically on a gear made in this session, only offered with **Use** on a reopened set (`auto_details` is false for loaded items). A user who adds a bore circle later doesn't see it used. | Take new concentric circles automatically on reopened gears too (keep the `dropped` list so a removed one stays out), or add a line: "Circles drawn on a gear's centre: click Use to make them its bore or hub." |
| L4 (P2) | Status line, Targets mode | "Click the bodies the gears act on (Targets). Click a gear's card to go back to picking gears." | OK. |

## Where `GEAR_UI_SPEC.md` is out of date (follow the code)

- §1/§2: "Create New", "Item 3 · Compound", the numbered items: the code says **New gear**, "Gear B · Compound", letters.
- §2: the **Shafts** section with Generate Shafts, Ø and length rows doesn't exist; a shaft is built where a shaft line is picked.
- §2: "Remove the Clearance Body checkbox": the code has the **Housing cutout** section instead.
- §2: Results is a list of rows ("z20 · m2 · Ø40 …"), not a table with columns.
- §3: Bore "Auto | none | Ø" and Hub "none | Ø + length (per side)" match the fields, but see the "no effect" table above.
- §4: The graph is optional ("Show graph"); the link rows are the main view.
- §5: Several hover texts changed (Accept suggested, Face preset, Mesh/Shaft); the tables above give the current ones.

## Tooth chamfer

Done: the set-wide tooth chamfer is the **All gears** section (**Tooth chamfer**, **Chamfer faces**),
sent as `tooth_chamfer {size, faces}`; its texts are in [The dialog](dialog.md#teeth) and
[Scripting](scripting.md).
