# Sketch conventions

You draw the drivetrain first, with the ordinary sketch tools, in as many sketches as you like
(flat sketches on any plane, or a 3D sketch). The Gears dialog then reads what you pick. Nothing
is copied: each gear keeps a reference to the sketch entity (by its sketch and its id), so the gear
follows when you change the sketch.

## What each entity means

| You draw | It means | Notes |
|---|---|---|
| **Circle** (or arc) | A **pitch circle**. Centre = gear centre, diameter = pitch diameter, the sketch plane's normal = the gear's axis. | The drawn diameter is kept. Must be normal (not construction) geometry. |
| **Line** | A **shaft line**: the gear's axis. Also a rack's pitch line, a worm's axis, a hypoid pinion's axis, or a bevel's shaft in a side view. | A construction line is a helper. Spur, helical, herringbone, compound and shaft gears refuse it as a shaft: "a construction line is a helper, not a shaft". |
| **Point on a shaft line** | A **bearing point**: a bearing seat in the [housing cutout](housing-cutout.md). | Taken by spur, helical, herringbone, compound and Shaft gears, after their shaft line. |
| **Point** where two shaft lines meet | A bevel pair's **apex** (side-view bevel). | Optional: two shaft lines that start at one common end are read as meeting there. |
| **Smaller circle on a gear's centre**, inside its pitch circle, in the same plane | The gear's **bore** (the smaller) or **hub** (the larger). | Spur, helical, herringbone and compound gears only. At most one bore and one hub per gear. |

Other things you can pick instead of sketch geometry: a round model edge (read as a circle), a
straight model edge, a round face or a construction axis (read as a line). A projected copy of a
curve counts as the curve it was projected from.

## Side A and side B

Every gear has a direction along its axis called **side A**:

- for a gear defined by a circle alone: the sketch plane's normal;
- when a shaft line is picked for it: the line's direction from its **first point** to its second.

Side B is the other way. Face width is set per side (see [The dialog › Face](dialog.md#face)). The
active gear shows a green arrow marked **A** in the view.

## How the size is decided

- **The drawn pitch diameter is kept.** With a module, each gear takes the whole tooth count
  nearest its drawn diameter (Ø41 at module 2 → 21 teeth, not 20.5).
- **Drawn centre distances are held exactly by profile shift.** So two tangent circles always mesh
  at the distance you drew, even when the counts are rounded. If the shift needed would undercut or
  point the teeth, the set is refused and the message names sizes that would work.
- **Meshing gears share one module.** Type the module on one gear and its mates follow. Two meshing
  gears that ask different modules are an error.
- **Auto module:** the largest standard module (ISO 54 series I) that gives the smallest gear of
  the meshing group 17 teeth or more. Tiny gears that can't reach 17 get the largest module giving 8
  or more, with a note, and profile shift against undercut.
- **Size by Teeth** instead: the tooth count you type is kept and the module follows (pitch Ø ÷
  teeth).
- A gear that meshes nothing, at a fixed module, gets the count nearest its circle, so its built
  pitch circle can differ slightly from the drawn one. A note says so: "Gear A meshes nothing: 21
  teeth at module 2 make it Ø42 (Ø41 drawn)".

## Which circles mesh

The dialog suggests links from the drawing:

- **Mesh:** two circles in one plane that touch from outside; a circle touching a ring from inside;
  a rack line touching a circle.
- **Same shaft:** gears on one axis, at different places along it (different sketch planes).

Accept the suggestions, or add links by hand. If you build with no links at all, the suggested ones
are used (the reply notes "no links given: the gears mesh and share shafts as the drawing
suggests").

Two circles in one plane that **almost** touch (a gap up to two modules, or a fifth of their
radii added up) and aren't linked get a
warning: "A and B are 0.4 mm apart, so they won't mesh. Make them tangent in the sketch to mesh
them."

## Fully define the sketch

The gears follow the sketch, so a sketch that is free to move lets the gears drift. Each gear whose
sketch has degrees of freedom gets a warning:

> sketch 'Pitch circles' isn't fully defined (2 degrees of freedom): dimension it so the gears can't drift

The card (or the line above the cards, when several gears share the sketch) has an **Edit sketch**
button. It opens the sketch; when you finish it, the Gears dialog comes back as you left it.

A checklist for a fully defined gear sketch:

- every pitch circle has a **diameter** dimension (or equal / tangent relations that fix it);
- the first gear's centre is fixed (on the origin, or dimensioned from it);
- each meshing pair is **Tangent** (or its centre distance dimensioned);
- the direction from one centre to the next is fixed (horizontal, vertical, or an angle);
- shaft lines are **Coincident** with the circle centre they pass through, and have a fixed length;
- bearing points are **Coincident** with their shaft line and dimensioned along it;
- bore and hub circles are **Concentric** with their pitch circle and have a diameter.

## Several sketches

- Gears on parallel shafts at **different heights** (a compound gear, a stacked train) are drawn
  in **parallel sketch planes**, one per layer.
- Shaft lines usually go in a sketch at right angles to the gear sketch (an upright sketch through
  the gear's centre), so they can run along the axis.
- A bevel pair, worm or hypoid is drawn in one sketch (see [Gear types](gear-types.md)).
- Every sketch a Gears feature reads must come **before** it in the timeline. A sketch drawn after
  the feature is moved before it when you OK an edit.
