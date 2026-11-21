# Housing cutout

The housing cutout is one extra body, named **Housing cutout**, that holds all the space the
drivetrain needs to turn:

- each gear's swept tooth envelope (its tip circle plus the radial gap, past its faces by the axial
  gap);
- the shafts, run on past their gears far enough to pass through a housing wall;
- a **bearing seat** at every bearing point you picked.

Model a housing block round the gears, then cut this body out of it (Combine › Cut) in one step.
The cutout follows the sketch like the gears do: move a bearing point and its seat moves.

It is a plain body: no component and no joint, so it doesn't take part in the motion.

## Turn it on

1. Pick **bearing points**: points on a gear's shaft line (spur, helical, herringbone, compound) or
   on a **Shaft** item's line. Pick the shaft line first, then the points on it.
2. On the **Housing & Output** tab, tick **Housing cutout**.
3. Choose the **Bearing**: one size for every seat, from the list (shown as "608 — 8 × 22 × 7":
   bore × OD × width, mm), or **Custom…** to type **Bearing OD**, **Bearing width** and, optionally,
   **Bearing bore**.

The line under the bearing says how many seats you will get: "No bearing points yet: pick points on
the gears' shaft lines (or a Shaft's) for the seats.", "1 bearing point: a seat there." or
"n bearing points: a seat at each."

While the dialog is open the view previews the envelopes as translucent grey rings and each seat as
rings in its gear's colour.

## Settings (Gaps and fit)

| Field | Default | What it does |
|---|---|---|
| **Radial gap** | 0.5 mm | Gap round every tip envelope and shaft. For printed housings 0.3–0.6 mm is typical. |
| **Axial gap** | 0.5 mm | Gap past each gear's faces along its axis. |
| **Seat fit** | 0 | How much larger than the bearing's OD each seat is, on the diameter: 0.1–0.2 for a printed slip fit, 0 or less for a press fit. |
| **Shaft run-on** | 2 × face width | How far each shaft's clearance runs past its gear on each side. |

Each seat is a cylinder of the bearing's OD + fit and the bearing's width, centred on the bearing
point and lying along the shaft line.

## Notes

- A ring gear (Ring Gear item, planetary ring) is taken as part of the housing and left out of the
  cutout.
- A custom bearing's bore is used only to check it against the shaft diameter; leave it empty to
  skip the check.
- Warnings about the cutout start with "clearance:" (a seat beyond its shaft, a seat on the teeth,
  a bearing bore that doesn't match the shaft). If the body can't be made at all, the set still
  builds and warns "the keep-out body can't be made: …".
- If a bearing point is deleted, the set fails and names it, like any missing pick (see
  [Editing](editing.md#deleted-or-renamed-references)).

## Housing cutout vs. "Cut keep-out from Targets"

| | Housing cutout | Cut keep-out from Targets (Housing & Output › Output) |
|---|---|---|
| Result | A separate body you cut yourself | The keep-out is cut straight out of the bodies you pick as Targets |
| Gap | Radial and axial gaps set separately | One **Keep-out gap**, used for both |
| When to use | You haven't modelled the housing yet, or want to keep the cut editable | The housing body already exists |

Both are made by the same code. The bearing size comes from the Housing cutout group, so for
seats in a keep-out cut, turn the housing cutout on as well (with Targets set, its space is cut
from them instead of being left as a separate body).

Example: `examples/gears/01-spur-pair.sc` (608 seats at two bearing points on gear A's shaft) and
`09-everything-drivetrain.sc`.
