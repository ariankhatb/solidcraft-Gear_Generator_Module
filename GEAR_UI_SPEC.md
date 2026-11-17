# Gears dialog: settings, flow and hover guides

The source of truth for the Create New / item / node-graph dialog. The audience is someone who
knows gears but has never used this dialog. Inputs use engineering terms, results use plain
language.

## 1. The flow a user follows

0. **Draw** with the native sketch tools. Use any number of sketches (or a 3D sketch): a circle for
   each pitch circle, a line for each shaft, and a point on a shaft line for each bearing.
   Construction geometry is ignored.
1. **Open Gears.** With no items yet, the dialog shows one guide block instead of empty rows:
   > ① Pick a gear type ② press **Create New** ③ click the geometry it asks for ④ connect items in the graph (tangent circles are suggested for you)
2. **Pick a type, press Create New.** A new item card opens and becomes the *active* item (accent
   border). The selection input is now scoped to that item. The status line reads
   "Item 3 · Compound — pick: pitch circle 1 of 2+". The next slot to fill is highlighted.
3. **Click the geometry.** Each pick fills the next slot of the item's checklist (see §3).
   - A pick that doesn't fit the slot is rejected with the reason:
     - "a construction line is a helper, not a shaft"
     - "this point is not on a shaft line"
     - "already used by Item 2"
   - When every required slot is ✓, the card collapses to a one-line summary, and Create New gets
     focus again for the next item.
4. **Connect.** The graph shows each item as a node.
   - Links the drawing implies (circles tangent at a sensible distance, circles on one line) appear
     **dashed**, under a single **Accept suggested (n)** button.
   - The user can also drag links themselves.
   - Every link shows its ratio, or its error in red: "centre distance 31.2, needs 30.0 ±0.05".
5. **Module blocks (optional).** A Module Block frame is placed from the graph's right-click menu.
   - Items dragged inside it share one module; the block's module is a value or Auto.
   - An item in no block gets its own module.
6. **Driver.** One item (or one of its gears) is the driver; a ▶ badge marks it in the graph.
   Optional input rpm and torque fill the results table.
7. **Check the results** (Results section) and the preview: outlines, contacts and face widths only.
   **OK** builds the teeth and the single Gears timeline feature.
8. **Edit** (from the timeline, the folder or the feature) reopens the same set, with its items,
   links and blocks. OK updates the set in place.

## 2. Dialog layout (top to bottom)

| Section | Contents |
|---|---|
| Header | Type combo + **Create New** (one type list; the old separate "Type" row is removed for sketched sets) |
| Items | One card per item: number, icon, type, checklist of slots, per-item settings (§3), ×; click the card to make it active |
| Graph | The node graph, docked under the items, with a **⤢ Pop out** button for a larger window; empty state: "Items appear here. Drag from a ● Mesh pin to another to make them mesh." |
| Shafts | Generate Shafts ☑; per shaft: Ø (Auto), length (from line / Auto), bearings found |
| Drive | Driver; input rpm; input torque (both optional) |
| Output | Operation (New Body / Join / Cut / Intersect) + Targets; Cut Keep-Out from Targets ☐ + keep-out gap; Assembly (joints + ratio links) ☑ |
| Advanced (closed) | Pressure angle, backlash, tip rounding, XY compensation, equation teeth (defaults for every item; an item can override them) |
| Results | Table, one row per gear: Item·gear, z, m, Ø pitch, ratio from driver, rpm, torque, contact ratio, warnings (undercut, thin rim, interference) |

**Remove the Clearance Body checkbox.** The user cancelled that feature.

## 3. Item types: slots and settings

Common to every gear in an item:
- **Size by:** Module | Teeth (a segmented toggle). The other value is shown greyed as the
  computed value.
- **Face:** two fields, **Side A** and **Side B**: the distance the face extends each side of the
  sketch plane, along the axis.
  - A preset toggle: **Symmetric | Side A only | Side B only | Custom**.
  - Auto = about 10 × module, capped by the room on the shaft.
  - A small diagram arrow in the viewport shows which side is A.
- **Helix:** angle (0 = straight), **Hand** L/R (the mate is set to the opposite hand
  automatically), **V** (herringbone) checkbox.
- **Profile shift:** Auto (avoids undercut below 17 teeth) or a value.
- **Bore:** Auto (from its shaft) | none | Ø.
- **Hub:** none | Ø + length (per side).

| Type | Slots (required ●, optional ○) | Type-specific settings |
|---|---|---|
| Spur / Helical / Herringbone | ● pitch circle; ○ shaft line | the common set; Helical and Herringbone preset the helix angle |
| Ring gear (internal) | ● pitch circle | rim thickness (Auto = 2.5 m); meshes with a pinion inside |
| Rack | ● pitch line | length (from the line), back height |
| Compound | ● shaft line; ● pitch circle × 2+ (on that line) | per circle: Size by / Face A·B / Helix / V; axial gap between gears comes from the drawing; one body |
| Bevel pair | ● circle 1, ● circle 2 (on intersecting axes) or ● 2 axis lines + ● 1 circle | teeth 1 & 2 or ratio; type Straight / Spiral / V (chevron); spiral angle + hand; face width (Auto ≤ ⅓ cone distance); shaft angle (read from the sketch, shown) |
| Hypoid pair | ● ring circle; ● pinion axis line (skew) | pinion teeth, spiral angle + hand, offset (read from the sketch, shown), face width |
| Worm and wheel | ● wheel circle; ● worm axis line | starts, worm length (from the line / Auto), lead angle (shown), throated wheel ☐ |
| Crossed helical | ● circle 1, ● circle 2 (skew axes) | helix 1 (Auto = half the shaft angle), hand |
| Shaft (as an item) | ● line; ○ bearing points | Ø (Auto), length from the line, keyway ☐ |

Bearings: a point on a shaft line, picked into a Shaft item or a gear's optional shaft slot. Size:
the standard list, or Auto (the smallest bearing that fits the shaft Ø).

## 4. Graph

- **Item node.** Its title is "3 · Compound" with the type icon. It has one row per gear
  (z / m filled in once calculated).
  - Each gear row has a **● Mesh** pin and a **■ Shaft** pin.
  - A pair (bevel, hypoid, worm) has a row for each of its two gears, with their mesh already
    made internally (drawn as a solid inner link).
- **Mesh link** (solid): the engine checks the centre distance and the angle. The label shows the
  ratio, e.g. "2.00 : 1"; a red label shows the error.
- **Shaft link** (thick, grey): the two gears turn together. They must be coaxial, otherwise:
  "not on one axis (4.1 mm apart)".
- **Module Block:** a group frame. Its header holds a Module value or Auto; the members are drawn
  inside the frame. Dragging an item out of the frame removes it from the block.
- **Hover** a node: its geometry lights up in the viewport. Hover a link: both contacts light up.
- **Right-click:** Add Module Block · Make Driver · Remove link · Accept suggested.
- The node graph and the item cards stay in sync: deleting one deletes the other.

## 5. Hover guides (exact texts)

| Control | Hover text |
|---|---|
| Type combo | "What the next item will be. Each type lists the geometry it needs." |
| Create New | "Start a new gear item of this type, then click the geometry it asks for in the view." |
| Item card | "Click to make this the item your picks go to. Hover to light its geometry." |
| Slot ○ | "Still needed: {slot}. Click one in the view." |
| Size by | "Set the module (tooth size) or the tooth count; the other follows from the pitch circle." |
| Module | "Tooth size in mm (pitch Ø ÷ teeth). Auto picks a standard module so the smallest gear has ≥17 teeth." |
| Teeth | "Number of teeth. The pitch circle stays as drawn, so the module follows." |
| Face Side A / B | "How far the teeth reach each side of the sketch plane, along the axis. Auto ≈ 10 × module, limited by the room on the shaft." |
| Face preset | "Symmetric: equal both sides. Side A/B only: the gear grows out of one face of the sketch." |
| Helix angle | "Tooth angle to the axis. 0 = straight. Meshing gears get equal angles of opposite hand." |
| Hand | "Left or right helix. The mating gear is set to the other hand for you." |
| V | "Herringbone: two opposite helices meeting in the middle — no axial thrust." |
| V (bevel) | "Chevron bevel: the spiral changes hand at mid-face." |
| Profile shift | "Moves the tooth out to avoid undercut on small gears. Auto does it below 17 teeth." |
| Bore | "Hole through the gear. Auto fits its shaft." |
| Spiral angle | "Spiral bevel/hypoid tooth angle at mid-face. 35° is typical." |
| Starts | "Thread starts on the worm. More starts = faster, less self-locking." |
| Generate Shafts | "A shaft on every gear axis, made one body with its gears. A picked line sets its length." |
| Driver | "The gear you turn. Every other gear's speed and direction follow from it." |
| Input rpm / torque | "Optional: fills speed and torque for every gear in Results." |
| Module Block | "Gears in this block share one module, so they can mesh with each other." |
| Mesh pin | "Drag to another gear's Mesh pin: they mesh. The distance between them is checked." |
| Shaft pin | "Drag to another gear's Shaft pin: they share a shaft and turn together." |
| Accept suggested | "Make every dashed link real. Dashed = circles the drawing shows as touching or on one line." |
| Operation | "New Body: separate bodies. Join/Cut/Intersect: combine with the Targets." |
| Cut Keep-Out | "Cut the space the gears sweep (plus the gap) out of the Targets, e.g. a housing." |
| Keep-out gap | "Extra room around the swept teeth when cutting the housing." |
| Assembly | "Each shaft and its gears become a component on a revolve joint, linked to its mates by the gear ratio." |
| Pressure angle | "Tooth flank angle. 20° is standard." |
| Backlash | "Play between meshing teeth, taken from each tooth's thickness." |
| Equation teeth | "Exact involute curves instead of fitted splines: slower, smoother." |

Every refusal or warning carries **what is wrong + what to do**, e.g.
"Item 4: Ø40 circle at z8 undercuts — raise teeth to 17+ or turn on Profile Shift Auto."

## 6. Contract additions (engine, gear-items)

Per gear inside an item, in addition to module/teeth/face/helix/herringbone:
- `face: {a, b}` (mm or null = auto)
- `hand: "left"|"right"|null`
- `profile_shift: number|"auto"`
- `bore: number|"auto"|null`
- `hub: {diameter, length_a, length_b}|null`
- `rim` (ring gear)
- `starts`, `throated` (worm)
- `spiral_angle`, `bevel_style: straight|spiral|chevron`

Set-level: `drive: {item, gear, rpm?, torque?}`, `keep_out_gap`.

The reply needs, per gear: `{item, gear, teeth, module, pitch_d, ratio, rpm?, torque?,
contact_ratio, warnings[]}`. For each link: `{a, b, kind, ratio?, error?, hint?}`.
