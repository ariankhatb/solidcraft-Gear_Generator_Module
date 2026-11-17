# Gears dialog review (2026-11-16)

Screenshots are from my own build of `gear-drivetrain-fixes` (a4f9d2ea) on control port 7980,
driven over the control channel (`drive/*.mjs`).

## Before: what is confusing (before-*.png)

1. **The Create New button is off screen.** The dialog is laid out wider than its card: the
   type combo row (`Spur ▾` + Create New) and the guide line `① Pick a gear type ② press Create
   New ③ …` run past the right edge of the window (before-01-empty). A first-time user never sees
   the button the whole flow depends on; only a `…` stub shows.
2. **Two flows at once.** With no item made, clicking circles silently uses the *old* row mode:
   numbered rows with a role combo ("Auto"), Size By, Generate Shafts with a per-shaft row and
   Diameter field, Driver "Row 1", Module, Face Width (before-04-two-items). Nothing says these
   rows are a different, older way of making gears.
3. **Legacy type families** in the same list (Sprocket, Pulley, Differential, Spline,
   Elliptical, Cycloidal…) switch the dialog to yet another layout of value rows.
4. **No "what next".** The only guidance is a small one-line caption that is cut off. There is
   no status line saying which sketch piece to click.
5. **Geometry and Targets inputs at the top** look like the main thing to do ("Select"), but the
   Geometry chip is just where picks land; Targets matter only for Join/Cut.
6. **Item cards are cramped and unreadable** (before-05-editing): "1 · Spur ✓ pitch circle ○
   shaft line ⇄1" all on one wrapped line, a lone × on its own line, `⇄1/⇄2` chips that mean
   "pick for ratio" without saying so. Items are numbered 1, 2, 3 while the engine and the
   timeline call them A, C, D ("Gear C·1+2").
7. **The graph is the main view of links** but it is an empty box by default; links are only
   understandable by dragging pins. "Accept suggested (2)" shows even when both links already
   exist.
8. **Per-gear settings are hidden**: a card's settings only appear while it is active or
   incomplete, and there is no heading telling which gear they belong to.
9. **Messages are a wall of text** under the form (engine facts, warnings, link notes), not on
   the gear they are about.
10. **Edit from the viewport does not open the set.** Double-clicking a gear in the view (an
    assembly set) activates that gear's component instead (before-entry-viewport-dbl.png); the
    Gears dialog never opens. The timeline row and "body selected + Gears" do open the set.


## After: what changed (after-*.png, w1…w7-*.png)

| Before | After |
|---|---|
| 1. Create New off screen | The form fits its card: **Type** (icon list) + one wide primary **New gear** button at the top, with the type's picture and what it needs (after-01-empty). |
| 2. Two flows at once | One flow. The row/role mode, Generate Shafts rows and the old type families are gone from the dialog; old sets open as they are with **Convert to gears**. |
| 3. Legacy families | Only the 12 item types (Spur … Shaft). |
| 4. No "what next" | A status line, always: "Gear B · Compound — click: shaft line", "… is complete — optional: shaft line; or New gear", a refusal in the warning colour with the reason, a missing pick named. |
| 5. Geometry/Targets chips on top | Gone from the top; picks go to the active gear; Targets appear under Output only when Join/Cut/Intersect or Cut keep-out needs them. |
| 6. Cramped cards, 1/2/3 numbering | One card per gear: colour dot, icon, "Gear B", type, ×; while active a checklist (✓ with the sketch and size of the pick, → next, ○ optional); complete: one line "z20 m2". Names match the engine/timeline (A, B, C…); a compound's gears B1/B2, a planetary's sun/ring/planet. |
| 7. Graph as the main link view | Drivetrain rows "A ↔ B1 mesh 0.50 : 1 opposite", "B ═ C same shaft", suggestions listed with one Accept button, Add a link by hand (closed), Driver + rpm/torque, Ratio from/to with path; the graph only under "Show graph". |
| 8. Settings hidden | "Gear B settings" section under the cards: Size by Module/Teeth (the other shown as worked out), Face preset + sides, Helix (Hand and V once there is a helix), More (profile shift, bore, hub), type-specific rows; a gear in a Module Block edits the block's module ("Shared by gears A, B"). |
| 9. Wall of text | Warnings/errors on the card they belong to; one shared by several gears (an under-defined sketch) said once with an **Edit sketch** button. |
| 10. Viewport double-click | Opens the set (it activated the component); a feature in the set's folder and an active gear component + Gears open it too. |

Also: each picked sketch piece is drawn in its gear's colour and labelled in the view ("B · shaft line"), the active gear's side-A arrow, card hover lights its geometry; box-selecting several circles makes one gear each (w6).
