# FAQ and glossary

## FAQ

**Why does my gear have 21 teeth when I drew Ø41 at module 2?**
The drawn diameter is kept and the tooth count is the whole number nearest Ø ÷ module (20.5 → 21).
The centre distance you drew is held by profile shift, so the gears still mesh where you drew them.
Use Size by Teeth if you want a set count.

**Do I have to type the module on every gear?**
No. Meshing gears share one module: type it on one gear and its mates follow. Leave it on Auto
everywhere and the dialog picks the largest standard module that gives the smallest gear 17 teeth
or more.

**Why are two of my gears not meshing?**
They need a Mesh link (Drivetrain). Touching circles are suggested: press Accept suggested. Circles
that only nearly touch are not suggested; make them Tangent in the sketch.

**What's the difference between a Compound and two gears with a Same shaft link?**
A Compound is one body holding all its gears (one part to print). Two gears linked Same shaft are
separate bodies that turn together in the assembly.

**Can I mix bevel, worm and spur stages in one feature?**
Yes. Connect the stages through shared shafts: a worm wheel and a spur gear on one shaft are two
gears linked Same shaft (or a Compound). One gear can't have two kinds of teeth.

**Where did the sprockets, pulleys and differentials go?**
They were part of the earlier Gear Generator window. In this build they are made with the
`gear.create` command only (CLI, scripts, MCP). See [Scripting](scripting.md).

**My gears moved when I edited something else in the sketch.**
The sketch isn't fully defined. Use the card's **Edit sketch** button and add dimensions until the
warning goes away.

**I deleted a circle and the feature failed.**
That's on purpose: a reference is never swapped for another silently. Reopen the set, click the
gear named on the card, then click the replacement circle. See [Editing](editing.md).

**How do I make a gear grow out of one side of its sketch plane?**
Face: **A only** or **B only**. The green arrow in the view is side A.

**How do I get bearing seats?**
Draw points on the shaft lines, pick them into the gear (after its shaft line), and turn on the
[housing cutout](housing-cutout.md).

**Why does the ratio say 0.05 : 1 for my 20 : 1 worm?**
The dialog shows speed ratios (output speed over input speed). Swap Ratio from / to to read it the
other way. See [Motion and ratios](motion-and-ratios.md).

**Does OK on Edit with no changes rebuild anything?**
No. If nothing changed, OK just closes.

## Glossary

| Term | Meaning |
|---|---|
| **Pitch circle** | The circle on which two meshing gears roll on each other without slipping. Pitch diameter d = module × teeth (÷ cos helix for a helical gear). In this tool, the circle you draw. |
| **Module (m)** | Tooth size in mm: pitch diameter ÷ teeth. Meshing gears have the same module. Standard values follow ISO 54 (1, 1.25, 1.5, 2, 2.5, 3, …). |
| **Teeth (z)** | The number of teeth. |
| **Addendum** | How far a tooth reaches outside the pitch circle: 1 × module for a standard tooth. Tip diameter = d + 2m. |
| **Dedendum** | How far the tooth space reaches inside the pitch circle: 1.25 × module. Root diameter = d − 2.5m. |
| **Pressure angle** | The angle of the tooth flank's line of action; 20° is standard (Advanced). |
| **Profile shift (x)** | Moving the cutting rack out from the gear by x × module. Used here to hold the drawn centre distance and to avoid undercut on small gears. |
| **Undercut** | On gears with few teeth (under about 17 at 20°) the cutter removes part of the tooth root, weakening it. Profile shift prevents it. |
| **Backlash** | Play between meshing teeth (Advanced). Taken from each tooth's thickness. |
| **Contact ratio** | How many tooth pairs are in contact on average. Above about 1.2 is good; under 1 the gears don't turn smoothly. Shown in Results. |
| **Face width** | The length of the teeth along the axis. |
| **Face side A / B** | The two sides of a gear's sketch plane along its axis. Side A is the sketch normal (or its shaft line's direction from its first point). The face is set per side. |
| **Helix angle** | The angle of helical teeth to the axis. Meshing gears on parallel shafts have equal angles of opposite hand. |
| **Hand** | Left or right helix. |
| **Herringbone / V teeth** | Two opposite helices meeting in the middle of the face, so the axial thrusts cancel. |
| **Spur** | Straight teeth parallel to the axis. |
| **Ring gear (internal)** | Teeth on the inside of a ring; its pinion runs inside it in the same direction. |
| **Rack** | A straight gear: a pinion turning on it makes it slide. |
| **Compound gear** | Two or more gears on one shaft made as one part, turning together. Used to chain reductions. |
| **Planetary set** | A sun gear, a ring gear round it and planets between them on a carrier. |
| **Carrier** | The part that holds the planets' pins. |
| **Bevel gear** | A conical gear for shafts that meet at an angle. **Straight**, **spiral** (curved teeth) or **chevron** (the spiral changes hand mid-face). |
| **Apex** | The point where a bevel pair's shafts meet; the tip of both pitch cones. |
| **Cone distance** | The distance from the apex to the outer end of the teeth along the pitch cone. |
| **Hypoid** | A bevel-like pair whose pinion axis is offset from the ring's axis (car axles). |
| **Worm and wheel** | A screw (worm) driving a wheel at 90°. Large reductions; often self-locking. |
| **Starts** | The number of threads on a worm. Ratio = wheel teeth ÷ starts. |
| **Crossed helical** | Two helical gears on shafts that pass each other without meeting. |
| **Stage** | One mesh or set in the drivetrain (a spur pair, a worm stage, a planetary set). |
| **Mesh link** | The two gears' teeth engage. |
| **Same shaft link** | The two gears turn together as one. |
| **Module Block** | A group (in the graph) whose gears share one module. |
| **Driver** | The gear you turn; all ratios, speeds and torques are worked out from it. |
| **Speed ratio** | Output speed ÷ input speed. A reduction is below 1. |
| **Reduction** | Input speed ÷ output speed (above 1 slows down). |
| **Bore** | The hole through a gear for its shaft. |
| **Hub** | A boss on a gear's face, round the bore. |
| **Bearing point** | A point on a shaft line where a bearing sits; a seat in the housing cutout. |
| **Housing cutout** | One body holding the space the drivetrain needs to turn, with bearing seats, to cut from a housing. |
| **Gear item** | One card in the dialog. It can make one gear (spur), several (compound, planetary) or none (shaft). |
| **Gears feature** | The one timeline feature that holds and builds the whole drivetrain. |
