# Gear examples

Designs made with the Gears feature (`gear.create` with `items`). In each one, the sketches define the gears: circles are pitch circles, lines are shafts (or a rack's pitch line, or a worm's axis). Each design is one Gears feature that builds an assembly: components, revolute joints and ratio links. Every sketch is fully defined (0 degrees of freedom).

| File | What it shows |
|---|---|
| `01-spur-pair.sc` | Two touching pitch circles in one sketch: z20 / z30, module 2, 1.5 : 1 |
| `02-helical-pair-face-sides.sc` | Helix 20° with opposite hands. Face sides: A grows on side A only, B is custom (16 mm on side A, 4 mm on side B) |
| `03-compound-train.sc` | Compound gear: a big circle in layer 1 and a small one in layer 2, joined by a shaft line, driving a third gear |
| `05-worm-and-wheel.sc` | Wheel pitch circle plus the worm's axis line in one sketch, 1 start, 20 : 1 |
| `06-planetary.sc` | Sun, one planet and the ring (concentric), 4 planets on a generated carrier |
| `07-internal-ring.sc` | Ring gear with a pinion inside it |
| `08-herringbone-pair.sc` | Herringbone pair, 25° |
| `09-everything-drivetrain.sc` | One drivetrain: worm, rack, spur, herringbone, helical, planetary and internal stages, chained through shared shafts, driven from the worm |
| `10-hypoid-pair.sc` | Hypoid: ring pitch circle plus the pinion's axis line 15 mm off-centre, 37 : 9 |

Bevel gears are left out until a bevel can be drawn as one side-view sketch (an apex point and two shaft lines).

## Known issues

- `09`: the internal ring builds as a solid cup when its pinion shares a shaft with other gears (an engine bug, being fixed).
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
