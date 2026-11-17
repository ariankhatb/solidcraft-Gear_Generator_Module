# Gears: user guide

The **Gears** command turns sketch geometry into a working drivetrain. You draw circles, lines and
points with the normal sketch tools. The dialog makes each circle a gear, keeps the drawn size, and
builds the whole set as **one Gears feature**: gear bodies, revolute joints and gear links, so
turning one gear turns the others. Change the sketch and the gears follow.

This guide is for people who know CAD and gears in general, but not this tool.

## Pages

| Page | What it covers |
|---|---|
| [Quick start](quick-start.md) | A spur pair in two minutes, with screenshots |
| [Sketch conventions](sketch-conventions.md) | What each circle, line and point means; how to fully define the sketch |
| [The dialog](dialog.md) | Every section and field: what it does, its default and its units |
| [Gear types](gear-types.md) | All 12 types: how to sketch each one and what you get |
| [Editing and sketch changes](editing.md) | Reopening a set, what follows the sketch, deleted references |
| [Housing cutout](housing-cutout.md) | One body holding the space the drivetrain needs, with bearing seats |
| [Motion and ratios](motion-and-ratios.md) | The assembly, driving joints, reading ratios |
| [Troubleshooting](troubleshooting.md) | The real error and warning messages, what they mean and how to fix them |
| [FAQ and glossary](faq-glossary.md) | Short answers and gear terms (module, pitch circle, face side, …) |
| [Scripting](scripting.md) | `gear.create`, `gear.calculate`, `gear.update`, `gear.ratio` parameters, with an example |

Worked examples: [`examples/gears/`](../../examples/gears/README.md) (ten `.sc` files, from a
spur pair to a full mixed drivetrain).

## Where to find it

**Model › Create › Gears** in the toolbar. The dialog opens in the right-hand panel under the title
**Gear Generator**. In the timeline the result is one row (one Gears feature, in a folder with its
joints when Assembly is on).

## The idea in five lines

1. A **circle** is a pitch circle. Its drawn diameter is kept.
2. A **line** is a shaft (or a rack's pitch line, or a worm's or pinion's axis).
3. A **point on a shaft line** is a bearing point (a seat in the housing cutout).
4. A **smaller circle on a gear's centre** is its bore; the next larger one its hub.
5. Each gear picks the tooth count nearest its drawn diameter for the module; the drawn centre
   distances are held exactly by profile shift.

> Note for this build: a set-wide **tooth chamfer** setting is being added in a parallel change.
> TODO: document it in [The dialog](dialog.md) once it lands.
