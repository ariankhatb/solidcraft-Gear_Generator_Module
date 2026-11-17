# Gear examples

Designs made with the Gears feature (`gear.create` with `items`). In each one, the sketches define the gears: circles are pitch circles, lines are shafts (or a rack's pitch line, or a worm's axis). Each design is one Gears feature that builds an assembly: components, revolute joints and ratio links. Every sketch is fully defined (0 degrees of freedom).

| File | What it shows |
|---|---|
| `01-spur-pair.sc` | Two touching pitch circles in one sketch: z20 / z30, module 2, 1.5 : 1. Ø8 bore circles on both gears and a Ø16 hub circle on A (concentric, dimensioned). Gear A's shaft line in an upright sketch with bearing points 20 mm below and 28 mm above the gear, and the **housing cutout** on: one body "Housing cutout" with a 608 seat at each bearing point |
| `02-helical-pair-face-sides.sc` | Helix 20° with opposite hands. Face sides: A grows on side A only, B is custom (16 mm on side A, 4 mm on side B) |
| `03-compound-train.sc` | Compound gear: a big circle in layer 1 and a small one in layer 2, joined by a shaft line, driving a third gear. Ø10 bore circles on both of the compound's gears |
| `04-bevel-side-view.sc` | Bevel drawn as one side-view sketch: an apex point, the two shaft lines, and a contact line whose angle (33.7°) sets the ratio (1.5 : 1) and whose length (54 mm) sets the cone distance. z30 / z45, module 2. Needs a build with `gear-drivetrain-fixes` or later |
| `05-worm-and-wheel.sc` | Wheel pitch circle plus the worm's axis line in one sketch, 1 start, 20 : 1 |
| `06-planetary.sc` | Sun, one planet and the ring (concentric), 4 planets on a generated carrier |
| `07-internal-ring.sc` | Ring gear with a pinion inside it |
| `08-herringbone-pair.sc` | Herringbone pair, 25° |
| `09-everything-drivetrain.sc` | One drivetrain: worm, rack, spur, herringbone, helical, planetary and internal stages, chained through shared shafts, driven from the worm. Ø8 bore circles on the spur, herringbone and helical gears; shaft lines with bearing points on A, B, E and F, and the housing cutout with 608 seats |
| `10-hypoid-pair.sc` | Hypoid: ring pitch circle plus the pinion's axis line 15 mm off-centre, 37 : 9 |

## Bores, hubs, bearing points and the housing cutout

- A circle drawn on a gear's centre, inside its pitch circle, is its bore (the smaller) or hub: pick it with `slot: bore` / `hub` (the Gears dialog takes the drawing's circles by itself; × on the card leaves one out).
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
