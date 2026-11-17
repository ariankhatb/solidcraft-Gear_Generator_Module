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

## After

See after-*.png and walkthrough-*.png and the list in the commit messages.
