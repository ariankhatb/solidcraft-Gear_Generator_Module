# Troubleshooting

Every refusal and warning says what is wrong and what to do. This page collects the messages you
are most likely to see, word for word (numbers and names vary), grouped by where they appear.

In the dialog, the engine's "Item B" is shown as "Gear B". In messages from the gear planner a
gear of a multi-gear item is named with its number ("Gear B2"); in the dialog's tabs the same gear
may be called "B ring" or "B wheel".

## 1. A pick is refused (status line, warning colour)

The status line reads "That pick doesn't fit: …" followed by one of these.

| Message | Meaning and fix |
|---|---|
| already used by Gear B: click its card to change it | That entity belongs to another gear. Click Gear B's card first if you meant to change it. |
| already picked into this item: pick another | You clicked the same entity twice. |
| a construction line is a helper, not a shaft: pick a solid line, or turn this one off construction | Shafts must be normal lines. |
| pick a sketch circle, line or point, a model edge, a round face or an axis | The thing you clicked can't be read as a circle, line or point. |
| this Spur needs a pitch circle, not a shaft line: pick a pitch circle next | Wrong kind of entity for the next slot. Follow the status line. |
| this Spur has no room for another pitch circle: start a new item for it | The gear is full. Press New gear (or just click again with this gear complete). |
| … a bearing point goes on a Shaft item's line | A point with no shaft line to sit on. |
| A Spur gear can't start with that: … | No gear is active and the pick can't start a gear of the chosen type. Choose another type first. |
| …; it doesn't start a new Spur gear either: … | The pick fits neither the active gear nor a new one. |
| this gear has a bore and a hub circle already: click one of them to take it out first | A gear takes one bore and one hub circle. |

### Geometry checks while picking

| Message | Fix |
|---|---|
| the pitch circle is tilted 12° to the shaft line: draw it in a sketch plane square to the line | Redraw the circle in a plane square to the shaft. |
| the pitch circle's centre is 0.8 mm off the shaft line: put its centre on the line (Coincident) | Constrain the line through the centre. |
| this point is 1.2 mm off the shaft line: a bearing sits on the shaft; put the point on the line (Coincident) | Make the bearing point coincident with the shaft line. |
| pick the shaft line first, then the bearing points on it | Order matters for bearing points. |
| two circles sit at the same place on the shaft: a compound's gears lie in different sketch planes along it; draw this one in another plane | Compound: one circle per sketch plane. |
| this circle is not parallel to the first: a compound's circles lie in parallel sketch planes, square to its shaft | Compound circles must be in parallel planes. |
| the pinion is not inside the ring: move its centre to 15 mm from the ring's centre (ring radius − pinion radius) | Ring Gear: make the pinion touch the ring from inside (Tangent). |
| the two circles are the same size: a pinion inside a ring is smaller than it | |
| the rack's pitch line doesn't touch the pinion's pitch circle: it is 21 mm from the centre, the radius is 20 mm; make it Tangent | Rack: line tangent to the pinion circle. |
| the sun and ring are not concentric: their centres are 0.5 mm apart; put both centres on one point (Concentric) | Planetary. |
| this planet's radius is 9 mm: between this sun and ring it must be 10 mm ((ring − sun) / 2) | Planetary: resize the planet. |
| this planet doesn't touch the sun and ring: … make it Tangent to both | |
| these circles' axes are parallel: a bevel pair's shafts meet at an angle (parallel shafts are a spur pair) | Use Spur gears and a Mesh link. |
| these circles' axes don't meet: they pass 4 mm apart; make them meet, or use a Hypoid or Crossed Helical pair for offset shafts | |
| the pitch circles don't touch: a bevel pair's circles meet at one point, the same distance from where the shafts meet (40 and 42 mm); move or resize one | |
| this line doesn't run through the apex point (2 mm off): draw the shaft lines from the apex | Side-view bevel. |
| the contact line must start at the apex point: draw it from the apex, between the two shaft lines | |
| the pinion's axis meets the ring's: that is a bevel pair; a hypoid pinion is offset, so move its line off the ring's axis (less than the ring's radius) | Hypoid. |
| the worm's axis passes through the wheel's axis: move it out to the wheel's radius plus the worm's pitch radius (more than 30 mm from the wheel's axis) | Worm. |
| the worm's line is 25 mm from the wheel's axis, inside the wheel's pitch radius 30 mm: … | Worm line must be outside the pitch circle. |
| the worm's axis crosses the wheel's at 80°: a worm crosses square (90°); draw its line in the wheel's plane, tangent-wise | |
| the circles don't touch: their axes are 50 mm apart and their radii add up to 48 mm; move one circle's shaft or resize a circle | Crossed helical. |

## 2. The set can't be built (red × on a card, or above the cards)

| Message | Fix |
|---|---|
| Add a gear: pick a type, press New gear and click the sketch pieces it lists | OK with no gears. |
| Gear B · Compound still needs a pitch circle: click one in the view (or remove the gear) | An incomplete gear. Finish it or press × on its card. |
| Gear A and Gear B are 0.6 mm apart: their centres are 50.6 mm apart and their radii add up to 50 mm; make the circles Tangent or remove the link | A Mesh link between circles that don't touch. |
| Gear A and Gear B overlap by 2 mm: … | Same, overlapping. |
| Gear A and Gear B are on one axis, so they can't mesh: link them by Shaft instead | |
| Gear A and Gear C share a shaft but are not on one axis: put their centres on one line, square to both circles, or remove the link | A Same shaft link between gears on different axes. |
| gear A asks module 2, but gear B it meshes with is module 1.5: meshing gears share one module | Set the same module, or leave one on Auto. |
| gear A asks a 20° helix, but gear B it meshes with asks 15°: meshing gears share one helix angle (of opposite hands) | |
| gear A and gear B mesh: both have V (herringbone) teeth, or neither | |
| gear A: V (herringbone) teeth need a helix angle | |
| gear C touches no other gear: make it tangent to the gear it meshes with (Tangent constraint); it is 0.4 mm from gear B's circle | A gear in a train with nothing to mesh with. |
| gears A and B share a shaft but their faces overlap along it by 3 mm …: move one along the shaft, or put one's face on side A and the other's on side B | Use the Face presets. |
| gears A and B mesh but their faces don't overlap along the shafts …: put both on one sketch plane, or change their face sides … | |
| Gear D and Gear E collide (near 10, 25, 0): they are in different stages and turn into each other; move one (along its shaft or apart), make it smaller, or link them if they share a shaft | Two stages sweep into each other. |
| Gear W2 is a worm gear: it meshes only at an angle to its mate; put a spur gear on its shaft (a Compound, or a Shaft link) to mesh on parallel shafts | One gear, one kind of teeth. |
| Gear P1 is in a planetary set: its gears mesh only within the set …; link another gear to its sun or ring by Shaft instead | |
| Gear G1 meshes with two pinions: a rack with several pinions is not made yet; give each pinion its own rack | |
| the drive gear (Gear C) doesn't exist: make another gear the driver | |
| item B, gear 1: the bore (Ø12) must be smaller than the hub (Ø10) | Swap or resize the circles. |
| Join, Cut and Intersect act on the Targets: pick the bodies (Output › Targets) | |
| Cut Keep-Out cuts the gears' room out of the Targets: pick the housing bodies (Output › Targets) | |
| Housing cutout: type the custom bearing's OD and width | Custom bearing needs OD and width. |

Tooth-limit refusals name sizes that would work, e.g. a profile shift that would undercut or point
the teeth at the drawn centre distance. Try a smaller module (more teeth), change a diameter, or
use Size by Teeth.

## 3. Missing references

See [Editing › Deleted or renamed references](editing.md#deleted-or-renamed-references):

- "Missing: curve b of sketch 'Pitch circles' (deleted or renamed?). Click this card, then its replacement in the view."
- "item B, pitch: curve `b` is gone from sketch 'Pitch circles': pick the pitch again, or draw it again with id `b`"
- "item A, axis: sketch 'Shafts' is gone (deleted): pick the axis again"
- "… isn't there before the Gears feature (it comes after it in the timeline, or it is suppressed or failed): move it before the Gears feature"
- "… is construction geometry now: a pitch circle must be drawn geometry"

## 4. Warnings (⚠, the set still builds)

| Message | Meaning |
|---|---|
| sketch 'Pitch circles' isn't fully defined (2 degrees of freedom): dimension it so the gears can't drift | Use **Edit sketch** and add dimensions. |
| A and B are 0.4 mm apart, so they won't mesh. Make them tangent in the sketch to mesh them. | Two nearly touching circles with no link. |
| item A, gear 1: the bore (Ø18) reaches the teeth's root (Ø17.5): it is left out | Smaller bore circle or larger gear. |
| item A, gear 1: the hub (Ø40) is wider than the teeth's root (Ø35) | |
| …: a helical pinion's rack is built with straight teeth (not made yet): use a spur pinion on a rack | |
| the keep-out body can't be made: … | The housing cutout failed; the gears still build. |
| clearance: … | Housing cutout checks (seat past a shaft, seat on the teeth, bearing bore ≠ shaft). |
| undercut, pointed teeth, low contact ratio … | Tooth checks; hover the Results row for the gear's warnings. |

## 5. Notes (small text under Drivetrain)

- "no links given: the gears mesh and share shafts as the drawing suggests"
- "module block M1: module 2"
- "gears 1 and 2: no standard module gives the smallest gear 17 teeth: module 0.5 gives it 12, cleared of undercut by profile shift"
- "Gear A meshes nothing: 21 teeth at module 2 make it Ø42 (Ø41 drawn)"
- "Gear H: the pinion's size is not drawn: taken as a quarter of the ring's (4:1); give both tooth counts to set it"

## Other problems

**Nothing happens when I click a circle.** Check the status line: the click may have been refused
(warning colour). If the Targets chip is active (Output), picks go to Targets; click a gear's card
to go back to picking gears.

**The gears don't mesh.** Make sure there is a link in Drivetrain. Accept the suggestion, or add a
Mesh link by hand. Circles must be exactly tangent (constrain them).

**The bore circle I drew isn't used.** On a reopened set, bore and hub circles are offered with a
**Use** button on the active card instead of being taken automatically. Click the gear's card, then
**Use**.

**Two gears on one shaft overlap.** Draw them in different sketch planes, or use the Face presets
(A only on one, B only on the other) so their faces don't overlap.

<a id="equation-teeth"></a>
**Equation teeth.** A set with Equation teeth is made as a folder of formula-driven features, not as
one Gears feature. An existing Gears feature can't be switched to equation teeth: the update fails
with "this set is one Gears feature: its teeth can't be drawn as formulas, and it can't go back to a
folder of features". Leave Equation teeth off unless you need formula teeth.
