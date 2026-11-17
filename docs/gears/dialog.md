# The Gears dialog, section by section

The dialog is a panel on the right titled **Gear Generator**. From top to bottom:

1. [Type and New gear](#type-and-new-gear)
2. [The status line](#the-status-line)
3. [Gear cards](#gear-cards)
4. [Gear settings](#gear-settings) (for the active gear)
5. [Drivetrain](#drivetrain)
6. [Housing cutout](#housing-cutout)
7. [Output](#output)
8. [Results](#results) (closed)
9. [Advanced](#advanced) (closed)

**Typing values.** Every number field takes a plain number or an expression with units, as other
SolidCraft fields do (`2`, `2 mm`, `20 deg`). An empty field, or `auto`, means Auto. Lengths are in
millimetres unless you type a unit. A value that doesn't read gives a message such as "Gear A gear 1:
module: … type a number, or leave it empty for Auto".

## Type and New gear

| Control | What it does |
|---|---|
| **Type** | The type the next gear is made as. 12 types: Spur, Helical, Herringbone, Ring Gear, Rack, Compound, Planetary, Bevel Pair, Hypoid Pair, Worm and Wheel, Crossed Helical, Shaft. When the dialog opens on an existing set, it starts on the first gear's type. |
| *(picture and "Needs:" line)* | What the type needs you to pick, e.g. "Needs: a pitch circle. Optional: its shaft line." |
| **+ New gear** | Starts a new, empty gear of the chosen type and makes it the active one. Your next picks go to it. |

You don't have to press New gear. When no gear is waiting for a pick (none is active, or the active
one is complete and the pick doesn't fit it), clicking geometry starts a new gear of the chosen type.
Box-selecting several circles makes one gear per circle.

With no gears yet, a guide box shows three steps and the hint "Then link them under Drivetrain:
touching circles are suggested for you."

## The status line

Always one line saying what to do next, for example:

- "Press New gear (or click a circle in the view) to start a Spur gear"
- "Gear B · Compound — click: shaft line"
- "Gear A · Spur is complete — optional: shaft line; or New gear for the next one"
- "Gear B · Bevel Pair is complete — sized by its teeth and module (Gear B settings below); or click its contact line to size it from the drawing"
- "Click a gear's card to change it, or New gear for another"

A refused pick turns it into a warning (⚠, warning colour): "That pick doesn't fit: …" with the
reason and what to do. A missing reference also shows here: "Gear B · Spur — its curve b of sketch
'Pitch circles' is missing: click its replacement in the view".

## Gear cards

One card per gear item, named **Gear A**, **Gear B**, … (after Z: A2, B2, …). Each card has:

- a **colour dot** (the same colour is used for its geometry and outline in the view);
- the type icon, the name and the type;
- **▶ driver** when it is the driver;
- **×** to remove it (its links go too).

**Click a card** to make it the active gear: your picks go to it and its settings show below.
**Hover** a card to light its geometry in the view.

While a card is active (or incomplete) it shows a **checklist**:

| Mark | Meaning |
|---|---|
| ✓ | filled; the pick is named by its sketch and size ("Pitch circles · Ø40.00", "Shaft · 120.00 long") |
| → | the next slot to fill ("click one in the view") |
| ● | still required |
| ○ | optional ("shaft line (optional)", "bearing point (optional, repeats)") |

A compound's circles are listed as "B1 pitch circle (first layer)", "B2 pitch circle (second
layer)", …

**Bore and hub rows.** For spur, helical, herringbone and compound gears, circles drawn on the
gear's centre inside its pitch circle are listed as **bore circle** / **hub circle**:

- on a gear made in this session they are taken automatically (✓ with **×** to take one out; one
  taken out stays out);
- on a reopened set they are offered with their diameter and a **Use** button.

A complete, inactive card folds to one line: its teeth and module, and bore and hub if any
("z20 m2 · bore 8 · hub 16"; a compound lists each gear).

**Messages** sit on the card they are about: ⚠ warnings and × errors, with what to do. A message
several gears share (one under-defined sketch) is said once above the cards, naming the gears, with
an **Edit sketch** button. See [Troubleshooting](troubleshooting.md).

## Gear settings

The section **Gear X settings** shows the active gear's settings. An item that makes several gears
(compound, pair, planetary, ring with pinion, rack with pinion) has a **Gear** row of tabs (B1, B2;
wheel, worm; sun, ring, planet; …): each gear has its own size, face and helix.

### Type-specific rows (shown first)

| Type | Row | Default | Meaning |
|---|---|---|---|
| Ring Gear | **Rim** | auto: 2.5 × module | Rim thickness outside the tooth roots (mm). |
| Worm and Wheel | **Starts** | 1 | Thread starts on the worm. |
| | **Throated wheel** | off | A wheel whose rim wraps round the worm. |
| Bevel Pair, Hypoid Pair | **Style** | Straight | Straight, Spiral or Chevron (the spiral changes hand at mid-face). Hypoid has a spiral regardless. |
| | **Spiral angle** | 35 deg | Tooth angle at mid-face (shown for Spiral, Chevron and Hypoid). |
| Rack | **Back height** | auto | Material behind the rack's tooth roots (mm). |
| Planetary | **Planets** | auto | How many planets: Auto fits as many as there is room for, 3 at most. |
| | **Carrier** | on | Generates a carrier body that the planets turn on. |
| Shaft | **Shaft Ø** | auto | Shaft diameter. |
| | **Keyway** | off | A keyway along the shaft. |

> In this build several of these rows are sent with the request but not yet applied by the
> planner for sets made in this dialog (rim, starts, throated, spiral angle, back height, keyway).
> See [UI_TEXT_GAPS.md](UI_TEXT_GAPS.md#settings-that-have-no-effect-yet). Planets, Carrier and
> Shaft Ø are applied.

A Shaft item has only Shaft Ø and Keyway; it has no teeth.

### Size by, Module, Teeth

| Row | Default | Meaning |
|---|---|---|
| **Size by** | Module | **Module**: you set the tooth size; the tooth count follows from the drawn circle. **Teeth**: you set the count; the module follows (pitch Ø ÷ teeth). |
| **Module** | auto | Tooth size in mm. Auto: the largest standard module that gives the smallest gear of the meshing group 17 teeth or more. The caption shows the count worked out: "Teeth: 20 (worked out)". |
| **Teeth** | from the circle | The tooth count. The caption shows "Module: … (worked out)". |

Meshing gears share one module: setting it on one gear sets it for its mates. If the gear is in a
**Module Block** (from the graph), the Module row edits the block's module, and the caption says
"Shared by gears A, B · teeth: …".

Until the gear is complete, the caption reads "— until the gear is complete".

### Face

| Preset | Fields | Auto |
|---|---|---|
| **Symmetric** (default) | **Each side** | 5 × module each side (10 × module in all) |
| **A only** | **Side A** | 10 × module, all on side A |
| **B only** | **Side B** | 10 × module, all on side B |
| **Custom** | **Side A**, **Side B** | both empty: 5 × module each side; one given: the other is 0 |

The face is how far the teeth reach from the sketch plane along the axis, on each side.
Note that **Each side** is per side: `10` gives a 20 mm face. Side A is the green arrow in the view
(see [Sketch conventions › Side A](sketch-conventions.md#side-a-and-side-b)). Use A only / B only
to grow a gear out of one face of its sketch, for example two gears on one shaft drawn in one
plane.

Meshing gears' faces must overlap along the axis; gears on one shaft must not overlap.

### Helix

Hidden for Bevel, Hypoid and Worm.

| Row | Default | Meaning |
|---|---|---|
| **Helix angle** | 0 = straight (Helical and Herringbone start at 20 deg) | Tooth angle to the axis. Meshing gears share one helix angle. |
| **Hand** | Auto | Shown when the helix isn't 0. Auto: opposite its mate across an outside mesh, the same inside a ring and on one shaft. |
| **V teeth (herringbone)** | off (on for Herringbone) | Two opposite helices meeting in the middle. Meshing gears both have V teeth, or neither. Needs a helix angle. |

### More (closed)

| Row | Default | Meaning |
|---|---|---|
| **Profile shift** | auto | Moves the tooth outward. The planner works the shift out itself to hold the drawn centre distance and clear undercut. *(A typed value is not applied in this build; see UI_TEXT_GAPS.)* |
| **Bore** | Auto | Auto, None or Ø. **Ø** shows **Bore Ø** (mm) and cuts that hole. With a shaft line picked, the gear sits on a built shaft and its bore fits the shaft. A drawn bore circle wins over this setting. |
| **Hub** | off | A boss on the gear's face. **Hub Ø** (mm), **Hub length A** and **Hub length B** (mm past the face on each side; both 0: half the face width on side A). A drawn hub circle gives the Ø. *(With Hub Ø left on auto and no hub circle, no hub is built in this build.)* |

A bore at or past the tooth roots is left out with a warning; a hub wider than the roots gets a
warning.

## Drivetrain

Appears once there are two gears (or any link). Its title counts the links: "Drivetrain · 2 links".

- **Link rows**: "A ⟷ B mesh 0.67 : 1 opposite" or "B ═ C same shaft", each with **×**. The ratio
  is the second gear's speed over the first's, with its direction. A link that can't be built
  shows its error under it in red.
- **Suggested links** (italic, "· suggested") and one **Accept suggested (n)** button. Suggested =
  circles the drawing shows as touching, or gears on one axis. With no links and no suggestions the
  section says "No links yet: add one below."
- **Add a link by hand** (closed unless there is nothing yet): **Gear**, **Link** (Mesh or Same
  shaft), **With gear**, **Add link**.
- **Driver**: the gear you turn. Every other gear's speed and direction follow from it. "—" means
  none; the assembly is then driven from the first gear.
- **Input rpm** and **Input torque** (N·m), both optional, shown once a driver is chosen. They fill
  speed and torque per gear in Results.
- **Ratio from** / **to**: pick two gears to read the ratio between them: **Ratio** (to's speed over
  from's, and the direction), **Path** (the chain of meshes and shared shafts), **Efficiency** and,
  for a rack, **Rack travel** per turn. See [Motion and ratios](motion-and-ratios.md).
- Notes from the planner in small text ("2 bodies from 1 stage", "module block M1: module 2", …).
- **Show graph**: the same links as a node graph. Drag from a ● Mesh pin to another gear's to mesh
  them, from a ■ Shaft pin for a shared shaft; right-click for **Add Module Block**, **Make
  Driver**, **Remove link**, **Accept suggested**. Gears inside a Module Block share one module.

## Housing cutout

Closed and off by default. See [Housing cutout](housing-cutout.md) for the full page.

| Row | Default | Meaning |
|---|---|---|
| **Make a housing cutout** | off | Adds one body, "Housing cutout": the space the drivetrain needs to turn plus a bearing seat at every bearing point. |
| **Bearing** | 608 — 8 × 22 × 7 | One bearing size for every seat (bore × OD × width, mm), or **Custom…** with **Bearing OD**, **Bearing width** and optional **Bearing bore**. |
| *(count line)* | | "No bearing points yet: …", "1 bearing point: a seat there.", "3 bearing points: a seat at each." |
| More › **Radial gap** | 0.5 mm | Gap round every tip envelope and shaft. |
| More › **Axial gap** | 0.5 mm | Gap past each gear's faces along its axis. |
| More › **Seat fit** | 0 | How much larger than the bearing OD each seat is, on the diameter. |
| More › **Shaft run-on** | 2 × face | How far each shaft's clearance runs past its gear on each side. |

## Output

| Row | Default | Meaning |
|---|---|---|
| **Assembly (components, joints and gear ratios)** | on | Each rigid part (a gear, a compound, a gear with its shaft) becomes a component on a revolute joint, linked to its mates by the gear ratio. |
| **Operation** | New Body | New Body: separate bodies. Join, Cut, Intersect: combine the gears with the **Targets**. |
| **Cut keep-out from Targets** | off | Cut the space the gears sweep (plus the gap) out of the Targets, e.g. a housing you already modelled. |
| **Keep-out gap** | auto | Extra room round the swept teeth when cutting (used as both the radial and the axial gap). |
| **Targets** | | Shown when the operation isn't New Body or keep-out is on. Click the chip, then click bodies. Click a gear card to go back to picking gears. |

Join, Cut and Intersect need Targets: "Join, Cut and Intersect act on the Targets: pick the bodies
(Output › Targets)".

## Results

Closed. One row per gear once the set is worked out: **z** (teeth), **m** (module), **Ø** (pitch
diameter), the ratio to the driver, rpm and N·m (with an input), and the contact ratio. Hover a row
for torque, contact ratio, profile shift and its warnings.

## Advanced

Closed. The defaults every gear takes.

| Row | Default | Meaning |
|---|---|---|
| **Pressure Angle** | 20 deg | Tooth flank angle. |
| **Backlash** | 0 | Play between meshing teeth, taken from each tooth's thickness (mm). |
| **Tip Rounding** | × module | Round the tooth tips, as a fraction of the module. |
| **XY Compensation** | 0 | Shrink (or grow) every tooth flank for a printer's over-extrusion (mm). |
| **Equation teeth** | off | Exact involute curves instead of fitted splines: slower, smoother. *(See the note in [Troubleshooting](troubleshooting.md#equation-teeth).)* |

<!-- TODO: tooth chamfer. A set-wide tooth chamfer setting is being added in a parallel change.
     Document its row (label, default, units) here when it lands. -->

## OK and Cancel

- **OK** on a new set runs `gear.create`: one Gears feature (in a folder with its joints when
  Assembly is on).
- **OK** on an existing set runs `gear.update` in place. If nothing changed, OK just closes.
- **Cancel** leaves the design as it was.

The view shows only outlines while the dialog is open (pitch, tip and root circles at both faces,
each gear's picks in its colour and labelled "B · shaft line", each mesh's contact in yellow, the
active gear's side-A arrow). The teeth are built on OK.
