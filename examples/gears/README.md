# Gear examples

Designs made with the Gears feature (`gear.create` with `items`). In each one, the sketches define the gears: circles are pitch circles, lines are shafts (or a rack's pitch line, or a worm's axis). Each design is one Gears feature that builds an assembly: components, revolute joints and ratio links. Every sketch is fully defined (0 degrees of freedom).

| File | What it shows |
|---|---|
| `01-spur-pair.sc` | Two touching pitch circles in one sketch: z20 / z30, module 2, 1.5 : 1. Ø8 bore circles on both gears and a Ø16 hub circle on A (concentric, dimensioned). Gear A's shaft line in an upright sketch with bearing points 20 mm below and 28 mm above the gear, and the **housing cutout** on: one body "Housing cutout" with a 608 seat at each bearing point |
| `02-helical-pair-face-sides.sc` | Helix 20° with opposite hands. Face sides: A grows on side A only, B is custom (16 mm on side A, 4 mm on side B) |
| `03-compound-train.sc` | Compound gear: a big circle in layer 1 and a small one in layer 2, joined by a shaft line, driving a third gear. Ø10 bore circles on both of the compound's gears |
| `04-bevel-side-view.sc` | Bevel drawn as one side-view sketch: an apex point, the two shaft lines, and a contact line whose angle (33.7°) sets the ratio (1.5 : 1) and whose length (54 mm) sets the cone distance. z30 / z45, module 2. |
| `05-worm-and-wheel.sc` | Wheel pitch circle plus the worm's axis line in one sketch, 1 start, 20 : 1 |
| `06-planetary.sc` | Sun, one planet and the ring (concentric), 4 planets on a generated carrier |
| `07-internal-ring.sc` | Ring gear with a pinion inside it |
| `08-herringbone-pair.sc` | Herringbone pair, 25° |
| `09-everything-drivetrain.sc` | One drivetrain: worm, rack, spur, herringbone, helical, planetary and internal stages, chained through shared shafts, driven from the worm. Ø8 bore circles on the spur, herringbone and helical gears; shaft lines with bearing points on A, B, E and F, and the housing cutout with 608 seats |
| `10-hypoid-pair.sc` | Hypoid: ring pitch circle plus the pinion's axis line 15 mm off-centre, 37 : 9 |

## Bores, hubs, bearing points and the housing cutout

- A circle drawn on a gear's centre, inside its pitch circle, is its bore (the smaller) or hub: pick it with `slot: bore` / `hub` (the Gears dialog takes the drawing's circles by itself; × on the card leaves one out). A bored gear goes on a shaft of your own, so no shaft stub is built from its shaft line (a compound keeps a sleeve between its layers, bored through).
- A point on a gear's shaft line (or on a Shaft item's line) is a bearing point (`slot: bearing`). With `clearance_body` on, each is a seat of the set's bearing in the **Housing cutout** body: the space the drivetrain needs to turn, to cut out of a housing block. The cutout is a plain body (no component, no joint) and follows the sketch: move a point and its seat moves; delete it and the set fails, naming it. In 01 and 09 the cutout is hidden so the files open on the gears: show it with its eye in the Browser (Bodies › Housing cutout).
- `tooth_chamfer: {size, faces}` puts one 45° chamfer on the tooth tips of every gear in the set (faces `both`, `a` or `b`); the dialog has it under All gears. Bevel, hypoid, worm, crossed helical, rack and planetary gears stay plain, with a note.

## Known issues

- `09`: `inspect.interference` reports a large overlap between the internal ring and its pinion. The ring is built correctly (an annulus, 56,804 mm³); the interference check gives a false positive when a pinion sits inside a toothed hole.
- `10`: SolidCraft's own geometry check flags the hypoid pinion's trimmed surfaces ("Bad CurveCusp"), the known kernel issue. The gears themselves are correct.

## Rebuilding

`scripts/*.json` are CLI scripts that draw the sketches and make the gears. To rebuild a design:

```sh
solidcraft-cli run examples/gears/scripts/01-spur-pair.json --save 01-spur-pair.sc --results
```

`make-examples.mjs` (01–07) and `make-examples-more.mjs` (08–10) regenerate the scripts:

```sh
node examples/gears/make-examples.mjs examples/gears
```

## What each example teaches

Open an example, double-click the Gears row in the timeline, and click each gear's card to see how
it is set up. The full guide is in [docs/gears/](../../docs/gears/README.md).

- **01 spur pair**: the basics. Two tangent circles in one sketch become two gears; one Mesh link;
  one module set once in a Module Block (both gears share it). Also: bore and hub circles drawn on a
  gear's centre (Ø8 bore, Ø16 hub), a shaft line in a second, upright sketch, two bearing points on
  it, and the housing cutout with a 608 seat at each. Try changing the Ø60 dimension: B regrows and
  stays meshed.
- **02 helical pair, face sides**: helix angle and hand per gear (20°, right and left), and the face
  presets: A only (16 mm on side A) and Custom (16 mm on A, 4 mm on B). Watch the green side-A arrow
  on the active gear.
- **03 compound train**: two sketch planes ("Layer 1", "Layer 2") and a shaft line through both. The
  Compound item C is one body with a gear in each layer; A meshes C's first gear, C's second gear
  meshes D. Each compound gear has its own bore circle. Read the ratio A → D under Ratio from / to.
- **04 bevel side view**: a bevel pair from one side-view sketch: the apex point, two shaft lines
  from it and a contact line whose angle sets the ratio (1.5 : 1) and whose length sets the cone
  distance. Without a contact line, the pair needs both gears sized by Teeth, plus a module.
- **05 worm and wheel**: the wheel circle and the worm's axis line in the wheel's plane, outside the
  circle. The driver is the worm (gear 1 of the item); the dialog's ratio reads as a speed ratio
  (worm → wheel 0.05 : 1).
- **06 planetary**: sun and ring concentric, one planet circle touching both. Planets: 4, Carrier on:
  the carrier body is generated and the planets ride on it.
- **07 internal ring**: a Ring Gear item with its pinion: two circles, the smaller touching the
  larger from inside. Rim 6 mm.
- **08 herringbone pair**: V teeth (25°, opposite hands) with a symmetric face of 10 mm each side
  (20 mm in all).
- **09 everything drivetrain**: many stage types in one Gears feature, joined by Same shaft links
  across seven sketch planes; two Module Blocks; a Shaft-line sketch with bearing points on A, B, E
  and F; the housing cutout. Drive the worm and every stage turns.
- **10 hypoid pair**: the ring circle and the pinion's axis line offset 15 mm from the ring's axis.
  The pinion isn't drawn, so both tooth counts are given (37 and 9) to set the ratio.
