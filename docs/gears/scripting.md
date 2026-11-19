# Scripting: gear.create, gear.calculate, gear.update, gear.ratio

Everything the Gears dialog does is an ordinary SolidCraft command, so a drivetrain can be made
from the CLI, a script or the MCP server. The dialog itself sends exactly these requests.

A CLI script is JSON: `{"commands": [{"command": "...", "params": {...}}, ...]}` (a bare array
works too). Run it with:

```sh
solidcraft-cli run my-gears.json --save my-gears.sc --results
```

## A small example: a spur pair

```json
{
  "commands": [
    { "command": "sketch.create", "params": { "plane": { "origin": [0,0,0], "x_dir": [1,0,0], "y_dir": [0,1,0] }, "name": "Pitch circles" } },
    { "command": "sketch.circle.center", "params": { "center": [0,0],  "radius": 20, "id": "a" } },
    { "command": "sketch.circle.center", "params": { "center": [50,0], "radius": 30, "id": "b" } },
    { "command": "sketch.constraint.coincident", "params": { "a": "a.center", "b": "origin" } },
    { "command": "sketch.dimension", "params": { "entities": ["a"], "type": "diameter", "value": "40 mm" } },
    { "command": "sketch.dimension", "params": { "entities": ["b"], "type": "diameter", "value": "60 mm" } },
    { "command": "sketch.constraint.tangent", "params": { "a": "a", "b": "b" } },
    { "command": "sketch.dimension", "params": { "entities": ["origin", "b.center"], "type": "vertical", "value": "0 mm" } },
    { "command": "sketch.finish", "params": {} },
    { "command": "gear.create", "params": {
        "name": "Spur pair",
        "items": [
          { "id": "A", "type": "spur", "components": [ { "sketch": "Pitch circles", "curve": "a", "slot": "pitch" } ],
            "gears": [ { "gear": 0, "size_by": "module", "module": 2 } ] },
          { "id": "B", "type": "spur", "components": [ { "sketch": "Pitch circles", "curve": "b", "slot": "pitch" } ] }
        ],
        "links": [ { "a": { "item": "A", "gear": 0 }, "b": { "item": "B", "gear": 0 }, "kind": "mesh" } ],
        "drive": { "item": "A", "gear": 0, "rpm": 1000 },
        "assembly": true
    } },
    { "command": "gear.ratio", "params": { "a": { "item": "A", "gear": 0 }, "b": { "item": "B", "gear": 0 } } }
  ]
}
```

B gets module 2 from A (meshing gears share one module): z20 / z30. The full examples, with shaft
lines, bearing points, bores and the housing cutout, are in
[`examples/gears/scripts/`](../../examples/gears/scripts).

## gear.create with items

Top-level keys:

| Key | Type | Meaning |
|---|---|---|
| `items` | array | One entry per gear item (a card in the dialog). Required, at most 64. |
| `links` | array | `{a: {item, gear}, b: {item, gear}, kind: "mesh" \| "shaft"}`. Left out or empty: the links the drawing suggests are used. A Shaft item is linked as `gear: 0`, by Shaft only; a Shaft item no link names carries the gears on its line. |
| `module_blocks` | array | `{id, module?, members?}`. Items in a block share one module (none: Auto for the block). |
| `drive` | object | `{item, gear, rpm?, torque?}`: the driver; rpm and torque (N·m) fill the per-gear replies. |
| `assembly` | bool | Components, revolute joints and gear links. |
| `clearance_body` | object | The housing cutout: `{bearing: "608" \| {od, width, bore?}, bearing_fit?, radial?, axial?, shaft_extension?}`. Seats come from the bearing points. |
| `operation`, `targets` | string, [body names] | `new` (default), `join`, `cut`, `intersect` with the target bodies. |
| `keep_out_cuts`, `keep_out_gap` | [body names], mm | Cut the keep-out from these bodies, with this gap. |
| `pressure_angle`, `backlash`, `tip_round`, `xy_compensation` | numbers | The Advanced defaults for every gear (degrees, mm, × module, mm). |
| `tooth_chamfer` | object | `{size, faces?}`: a 45° chamfer of `size` mm off the tooth tips of every gear that takes one, on `faces` `"both"` (default), `"a"` or `"b"`. Absent or size 0: none. Too big for a gear's addendum: refused, naming the gear. |
| `name` | string | The feature's name. |

Don't mix `items` with the older single-set keys `sketch`, `selection`, `axis1`, `axis2` or
`placement`: "give `items`, or `sketch`, `selection`, `axis1` or `placement`: one of them".

### An item

| Key | Meaning |
|---|---|
| `id` | Its name (default A, B, … by place). Links, blocks and the drive use it. |
| `type` | `spur`, `helical`, `herringbone`, `internal`, `rack`, `compound`, `planetary`, `bevel` (also `spiral_bevel`, `chevron_bevel`), `hypoid`, `worm`, `crossed_helical`, `shaft`. |
| `components` | The picks, in order. Each is exactly one of `{sketch, curve}`, `{sketch, point}`, `{edge: [x,y,z]}`, `{face: [x,y,z]}`, `{axis}`, plus an optional `slot`. `sketch` is the sketch's id or name. |
| `gears` | Per-gear settings (below). The first one fills any gear left out. |
| `module_block` | The id of a block in `module_blocks`. |
| `planets`, `carrier` | Planetary: number of planets (Auto: as many as fit, at most 3) and whether a carrier is made (default true). |
| `diameter` | Shaft: its diameter (mm; Auto: half the smallest root diameter of the gears it carries). |
| `differential` | Hypoid or bevel pair: its ring gear (the hypoid's ring, the bevel pair's larger gear) becomes a differential, `{style?, module?, side_teeth?, …, axle?: {length?, diameter?}}`; every key in [the table below](#a-differential-on-a-ring-gear). On any other type: refused. |
| `rim`, `starts`, `throated`, `bevel_style`, `spiral_angle`, `back_height`, `keyway` | Type-specific settings. They are read and checked, but in this build the drivetrain planner doesn't apply them yet (see [UI_TEXT_GAPS.md](UI_TEXT_GAPS.md)); `bevel_style` (straight, spiral, chevron) is applied. |

**Slots.** The order of the components decides what each is, as in the dialog: the first circle of
a spur gear is its pitch circle, a line its shaft, points its bearings. `slot` names are labels used
in error messages (`pitch`, `pitch2`, `axis`, `axis2`, `bearing`, `sun`, `ring`, `planet`, `apex`,
`contact`), with two exceptions that matter: `slot: "bore"` and `slot: "hub"` mark a circle as the
bore or hub of the gear it is concentric with.

**Gear numbers** within an item (0-based, `gear` in links, the drive and `gears[]`):

| Type | 0 | 1 | 2 |
|---|---|---|---|
| spur, helical, herringbone | the gear | | |
| compound | first circle | second circle | … |
| internal | ring | pinion | |
| rack | rack | pinion | |
| planetary | sun | ring | planet |
| bevel, crossed_helical | first | second | |
| hypoid | ring | pinion | |
| worm | wheel | worm | |
| shaft | the shaft itself (links only: it has no gears) | | |

### A gear's settings (`gears[]`)

| Key | Values | Default |
|---|---|---|
| `gear` | 0-based index | its place in the list |
| `size_by` | `module` \| `teeth` | `teeth` if only teeth is given, else `module` |
| `module` | mm, > 0 | Auto |
| `teeth` | 1 … 10000 | from the drawn circle |
| `face` | `{preset: symmetric \| a_only \| b_only \| custom, a, b}`; `a`, `b` in mm or null (Auto) | symmetric, Auto (5 × module each side) |
| `helix_angle` | degrees, −89 … 89 | 0 |
| `hand` | `left` \| `right` \| null | null: opposite its mate |
| `herringbone` | bool (V teeth) | false |
| `profile_shift` | −5 … 5, or `"auto"` | `"auto"` |
| `bore` | `"auto"`, `null` or `"none"` (no hole), or a diameter | `"auto"` |
| `hub` | `{diameter, length_a, length_b}` | none |

### A differential on a ring gear

`differential` on a hypoid or bevel pair turns its ring gear (the hypoid's ring, the bevel pair's
larger gear) into a differential's ring: the whole differential is designed with the drivetrain,
as the single-set `differential` type designs it (`crate::diff`), with the drivetrain's ring as its
drive. It is its own design: every setting below can be shown and edited (a front end's
differential tab), every one optional.

```json
{"id": "F", "type": "hypoid",
 "components": [{"sketch": "Sketch4", "curve": "c1", "slot": "ring"}, {"sketch": "Sketch4", "curve": "l2", "slot": "axis"}],
 "gears": [{"gear": 0, "size_by": "teeth", "teeth": 37}, {"gear": 1, "size_by": "teeth", "teeth": 9}],
 "differential": {"style": "open", "pinions": 2, "axle": {"length": 75}}}
```

| Key | Values | Default (open / biased / torsen_b / torsen_a) |
|---|---|---|
| `style` | `open` (equal bevel side gears, spider pinions) \| `biased` (uneven bevel side gears: a fixed torque split) \| `torsen_b` (helical side gears, paired helical elements) \| `torsen_a` (worm side gears, worm-wheel elements) | `open`. A style starts from its own defaults; the keys given are laid over them. |
| `module` | mm, > 0 (bevel: large end; Torsen: normal) | Auto: the largest (in 0.05 steps) whose case is at most 0.6 × the ring's pitch Ø |
| `side_teeth` | side gear A, 6 … 200 (Torsen A: worm starts, 1 … 200) | 16 / 12 / 18 / 6 |
| `side_teeth2` | side gear B, 0 (as A) or 6 … 200; `biased` needs it different from A | 0 / 18 / 0 / 0 |
| `pinion_teeth` | spider pinions or element gears, 6 … 200 | 10 / 12 / 11 / 15 |
| `pinions` | spider pinions 1 … 6 (bevel styles), element pairs 1 … 8 (Torsen) | 2 / 2 / 3 / 2 |
| `face_width` | side gears' face, mm, ≥ 0 (0: 0.3 × cone distance; Torsen 8 modules) | 0 |
| `spiral_angle` | bevel side gears and pinions, degrees, −45 … 45 | 0 (straight) |
| `helix_angle` | Torsen, degrees, 0 … 80 | 30 / 30 / 30 / 60 |
| `pressure_angle` | degrees, 0 (20) or 14.5 … 30 | 0 |
| `backlash` | mm, ≥ 0 | 0 |
| `friction` | Torsen bias estimate's coefficient, 0 … 0.5 | 0.1 |
| `pin_diameter` | mm, ≥ 0 (0: about half the pinions' small-end roots) | 0 |
| `wall` | case wall, mm, ≥ 0 (0: 15 % of the gear sphere, at least 3) | 0 |
| `washer` | thrust washer allowance, mm, ≥ 0 (0: from the size) | 0 |
| `clearance` | running clearance round the gears in the case, mm, ≥ 0 | 0.5 |
| `bearing` | case journal bearings by name (`6800`, `6001`, `30205`, …) | the smallest whose bore takes the journal round the axle |
| `windows` | assembly windows in the case between the pinions | false (plain stock) |
| `ring_bolts` | the ring bolted to a flange on the case: tapped holes in the ring's back, through holes in the flange | true (false: the ring sits on the case, fixed all the same) |
| `bolts` | ring bolts, 0 … 64 (0: from the bolt circle, 6 … 16) | 0 |
| `bolt_diameter` | bolt size M, mm (0: the largest that fits between the case and the ring's roots, M3 … M16) | 0 |
| `axle.length` | each half-shaft's reach from the differential's middle along the ring's axis, mm | 1.5 × the ring's pitch Ø, or to the ends of a Shaft item drawn on that axis |
| `axle.diameter` | the half-shafts' (and the side gears' bores') diameter, mm | that Shaft item's, else 0.4 × side gear A's pitch Ø (in half mm) |

Values outside these ranges are refused when the request is read, with the generator's own words
(`items[6].differential: pinions must be between 1 and 6`); what only shows once the gears are
sized comes back as the reply's `error` (`… a torque-biasing differential needs side gear B with a
different tooth count …`). The differential's own `drive`, `mount`, `parts` and `keep_out` (single-set keys) don't
apply: the drivetrain's ring is the drive, every part is built, and the housing cutout is the
drivetrain's.

**Where and what.** Its middle sits on the ring's axis where the pinion's axis passes it (a
bevel's apex, a hypoid pinion's closest approach). The bodies (item F):

| Body | Joint |
|---|---|
| `Diff F case` (with its pinion pins and flange) | `Diff F case fixed`: rigid on the ring gear |
| `Diff F side A + axle`, `Diff F side B + axle` | `… shaft`: revolute on the case, on the ring's axis |
| `Diff F pinion 1`, `Diff F pinion 2`, … (`element` for Torsen) | `… shaft`: revolute on the case, on its pin |

The ring's bore is opened to the case's outside (or the case grows out to the ring's bore), and
with `ring_bolts` a flange on the case sits against the ring's back face. Side B is the side the
ring's teeth face. A Shaft item drawn on the ring's axis becomes the half-shafts (it is not made
as a bar of its own).

**Motion.** Side gear A turns each pinion and pinion 1 turns side gear B, all relative to the
case. An open differential has two degrees of freedom; side gear A is left free (as the single-set
differential leaves it), so driving the drivetrain turns both half-shafts with the case: driving
straight. Driving side gear A's joint turns side gear B the other way relative to the case.

**The reply.** `gear.calculate` and `gear.create` reply with `differentials`, one per item with a
differential, for a differential tab:

```json
{"ring": {"item": "F", "gear": 0},
 "facts": [{"label": "differential module", "value": 1.05, "unit": "mm"},
           {"label": "side gear A teeth", "value": 16, "unit": ""},
           {"label": "case outside diameter", "value": 29.6, "unit": "mm"},
           {"label": "ring bolts", "value": 8, "unit": ""}, "…"],
 "warnings": ["…"],
 "note": "Gear F carries an open differential: module 1.05, 2 pinions, case Ø29.6, …"}
```

The facts are the generator's (gear sizes, pin and hub sizes, case outside diameter and length,
wall, journal bearings and their spread, axle bore, torque share per side, speeds with one side
held, the biased style's torque split, Torsen bias and lead angles) plus the module and teeth
chosen, the pinions, the ring bolts and flange, the half-shaft diameter, reach and length. The
same facts are in the set's `facts` as `differential Gear F: …`. A ring too small for any
differential is refused: "Gear F's ring gear (Ø…) is too small for a differential (…): use a
bigger ring, or give the differential a module". With `clearance_body`, the housing cutout holds
the case's turning envelope (pinions going round with it) and both half-shafts' channels.

## gear.calculate

The same parameters as `gear.create`. Builds nothing. Returns what the set would be:

- `gears`: per gear `{item, gear, teeth, module, pitch_d, ratio?, rpm?, torque?, contact_ratio?,
  profile_shift?, warnings, side_a?, bore?, hub?}`;
- `links`: per link used `{a, b, kind, ratio?, error?, hint?}`;
- `suggestions`: links the drawing suggests that aren't given `{a, b, kind, why}`;
- `detail_suggestions`: circles that could be a gear's bore or hub;
- `error` when the drivetrain can't be built (with the replies above still filled), `warnings`,
  `notes`, `facts`.

The dialog calls it on every change to fill the cards, the link rows and Results.

## gear.update

`{group, …}`: `group` is the set's timeline folder or Gears feature id (or give `body`: one of its
bodies). Any `gear.create` parameters given are laid over the ones the set was made with; `null`
removes one; `reset: true` starts from scratch with only what you give (the dialog sends the whole
request with `reset: true`). The set is updated in place: bodies keep their names, components and
joints. Returns as `gear.create`, plus `updated: "in_place" | "rebuilt"`.

A Gears feature also re-plans by itself whenever a sketch it reads changes: no command needed.

## gear.ratio

The speed ratio between two gears of one set.

| Key | Meaning |
|---|---|
| `a`, `b` | From and to: a body's name (`"Gear A·1"`), a component id, or `{item, gear}` |
| `group` | The set (optional: found from `a` or `b`, or the design's one set made of items) |
| `request` | Instead of `group`: an unbuilt set's `gear.calculate` parameters (the dialog uses this) |

Returns `ratio` (b's motion per unit of a's, signed), `direction` (`same` \| `opposite`),
`reduction` (|ω a ÷ ω b|: above 1 slows down), `path` (each step `{from, to, from_gear?, to_gear?,
kind, teeth?, ratio, efficiency, self_locking}`), `efficiency` (0–1), and for a rack `mm_per_rev` or
`rev_per_mm`.

Example: for a 20 : 1 worm, `gear.ratio {a: worm, b: wheel}` gives `ratio` −0.05 (or +0.05,
depending on hand) and `reduction` 20.

## Driving the assembly

```json
{ "command": "joint.drive", "params": { "joint": "Gear A·1 shaft", "value": 30 } }
```

Turns gear A by 30° and every linked gear with it. `joint.list` lists the joints.
