# Motion and ratios

## The assembly

With **Output › Assembly (components, joints and gear ratios)** on (the default), OK builds:

- one **component** per rigid part: a gear, a compound gear (all its gears in one body), a gear with
  the shaft built along its line, a planetary carrier, each planet;
- one **revolute joint** per part on its axis, named after the body: **Gear A·1 shaft**;
- one **gear link** per mesh, with the ratio and direction of that mesh (a rack and pinion gets a
  rack-and-pinion link, the rack a slider; a worm drives its wheel by starts ÷ teeth);
- gears joined by a **Same shaft** link turn as one.

The joints and links live in the set's timeline folder. An update keeps them on the same bodies,
moves them onto the new shafts and gives the links the new ratios. Joints you add to the gears
yourself are kept too.

The **Driver** (Drivetrain section) is the gear the assembly is rooted on. Without one, the first
gear is used.

## Turning the gears

- **Assemble › Motion › Drive Joints**: pick a gear's joint (e.g. *Gear A·1 shaft*) and set its
  angle. Every linked gear turns with it.
- From a script: `joint.drive {"joint": "Gear A·1 shaft", "value": 30}` (degrees).
- **Motion Study** (Assemble › Motion) animates a joint over time and can check interference.

The housing cutout is a plain body and doesn't move.

## Reading ratios

All ratios in the dialog are **speed ratios: the second gear's speed over the first's**, with a
direction (*same way* or *opposite*).

| Where | Shows | Example (A z20 driving B z30) |
|---|---|---|
| Drivetrain link row | the speed of the link's second gear over its first's | A ⟷ B mesh **0.67 : 1** opposite |
| Results (per gear) | the gear's speed over the driver's | B: 0.67 : 1 |
| Ratio from / to | to's speed over from's, the path between them, efficiency | Ratio 0.67 : 1, opposite direction · Path … · Efficiency … % |

So a **reduction shows as a ratio below 1**. A 20 : 1 worm reduction (worm to wheel) reads
**0.05 : 1**. To read it the other way round, swap **Ratio from** and **to**.

- **Path** lists the gears from one to the other through meshes and shared shafts.
- **Efficiency** multiplies each mesh's typical efficiency (a worm's depends on its lead angle; a
  self-locking worm driven from its wheel can't turn back).
- **Rack travel**: for a rack, mm of travel per turn of the first gear.

## Speed and torque

Set a **Driver**, then **Input rpm** and/or **Input torque** (N·m). Results then lists each gear's
rpm and torque (torque reduced by the efficiencies along the path). Hover a Results row for its
torque, contact ratio and profile shift.

## Selecting two gears in the model

With two gear bodies of one set selected in the model, the quick readout shows the ratio between
them (`gear.ratio` with the two bodies' names).

See also: [Scripting › gear.ratio](scripting.md#gearratio).
