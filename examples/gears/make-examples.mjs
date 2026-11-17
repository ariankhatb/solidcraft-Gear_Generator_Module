// Gear examples with fully defined sketches (every sketch ends at 0 degrees of freedom).
import fs from 'node:fs';
import path from 'node:path';

const out = process.argv[2];
fs.mkdirSync(path.join(out, 'scripts'), { recursive: true });

const XY = (z = 0) => ({ origin: [0, 0, z], x_dir: [1, 0, 0], y_dir: [0, 1, 0] });
const XZ = { origin: [0, 0, 0], x_dir: [1, 0, 0], y_dir: [0, 0, 1] };
const C = (command, params) => ({ command, params });
const sketch = (name, plane, ents) => [C('sketch.create', { plane, name, project_edges: false }), ...ents, C('sketch.finish', {})];
const circle = (id, c, r) => C('sketch.circle.center', { center: c, radius: r, id });
const line = (id, a, b) => C('sketch.line', { points: [a, b], ids: [id] });
const dia = (e, mm) => C('sketch.dimension', { entities: [e], type: 'diameter', value: `${mm} mm` });
const hdim = (a, b, mm) => C('sketch.dimension', { entities: [a, b], type: 'horizontal', value: `${mm} mm` });
const vdim = (a, b, mm) => C('sketch.dimension', { entities: [a, b], type: 'vertical', value: `${mm} mm` });
const len = (l, mm) => C('sketch.dimension', { entities: [l], value: `${mm} mm` });
const at0 = p => C('sketch.constraint.coincident', { a: p, b: 'origin' });
const tangent = (a, b) => C('sketch.constraint.tangent', { a, b });
const concentric = (a, b) => C('sketch.constraint.concentric', { a, b });
const hv = (l, mode) => C('sketch.constraint.horizontal_vertical', { line: l, mode });
const pick = (sketch, curve, slot) => ({ sketch, curve, slot });
const gears = (name, items, links, extra = {}) => C('gear.create', { name, items, links, assembly: true, ...extra });
const mesh = (a, ag, b, bg) => ({ a: { item: a, gear: ag }, b: { item: b, gear: bg }, kind: 'mesh' });
const ratio = (a, ag, b, bg) => C('gear.ratio', { a: { item: a, gear: ag }, b: { item: b, gear: bg } });

// Two touching pitch circles, the first at the origin, the second on the X axis.
const touchingPair = (a, da, b, db) => [
  circle(a, [0, 0], da / 2), circle(b, [(da + db) / 2, 0], db / 2),
  at0(`${a}.center`), dia(a, da), dia(b, db), tangent(a, b), vdim('origin', `${b}.center`, 0),
];

const ex = {};

ex['01-spur-pair'] = [
  ...sketch('Pitch circles', XY(), touchingPair('a', 40, 'b', 60)),
  gears('Spur pair', [
    { id: 'A', type: 'spur', components: [pick('Pitch circles', 'a', 'pitch')], module_block: 'M' },
    { id: 'B', type: 'spur', components: [pick('Pitch circles', 'b', 'pitch')], module_block: 'M' },
  ], [mesh('A', 0, 'B', 0)], { module_blocks: [{ id: 'M', module: 2 }], drive: { item: 'A', gear: 0, rpm: 1000, torque: 2 } }),
  ratio('A', 0, 'B', 0),
];

ex['02-helical-pair-face-sides'] = [
  ...sketch('Pitch circles', XY(), touchingPair('a', 36, 'b', 60)),
  gears('Helical pair', [
    { id: 'A', type: 'helical', components: [pick('Pitch circles', 'a', 'pitch')], module_block: 'M',
      gears: [{ gear: 0, helix_angle: 20, hand: 'right', face: { preset: 'a_only', a: 16, b: 0 } }] },
    { id: 'B', type: 'helical', components: [pick('Pitch circles', 'b', 'pitch')], module_block: 'M',
      gears: [{ gear: 0, helix_angle: 20, hand: 'left', face: { preset: 'custom', a: 16, b: 4 } }] },
  ], [mesh('A', 0, 'B', 0)], { module_blocks: [{ id: 'M', module: 1.5 }], drive: { item: 'A', gear: 0, rpm: 1000 } }),
  ratio('A', 0, 'B', 0),
];

// Compound gear: big gear in layer 1, small gear in layer 2 (20 mm below), joined by a shaft line.
ex['03-compound-train'] = [
  ...sketch('Layer 1', XY(0), touchingPair('a', 30, 'c1', 60)),
  ...sketch('Layer 2', XY(-20), [
    circle('c2', [45, 0], 15), circle('d', [85, 0], 25),
    hdim('origin', 'c2.center', 45), vdim('origin', 'c2.center', 0), dia('c2', 30), dia('d', 50),
    tangent('c2', 'd'), vdim('origin', 'd.center', 0),
  ]),
  ...sketch('Compound shaft', XZ, [
    line('s', [45, -30], [45, 10]), hv('s', 'vertical'), hdim('origin', 's.start', 45), vdim('origin', 's.end', 10), len('s', 40),
  ]),
  gears('Compound train', [
    { id: 'A', type: 'spur', components: [pick('Layer 1', 'a', 'pitch')], module_block: 'M' },
    { id: 'C', type: 'compound', components: [pick('Compound shaft', 's', 'axis'), pick('Layer 1', 'c1', 'pitch'), pick('Layer 2', 'c2', 'pitch2')], module_block: 'M' },
    { id: 'D', type: 'spur', components: [pick('Layer 2', 'd', 'pitch')], module_block: 'M' },
  ], [mesh('A', 0, 'C', 0), mesh('C', 1, 'D', 0)], { module_blocks: [{ id: 'M', module: 1.5 }], drive: { item: 'A', gear: 0, rpm: 1000, torque: 1 } }),
  ratio('A', 0, 'D', 0),
];

// Worm and wheel: wheel pitch circle at the origin, worm axis a horizontal line above it.
ex['05-worm-and-wheel'] = [
  ...sketch('Worm stage', XY(), [
    circle('w', [0, 0], 30), line('axis', [-40, 38], [40, 38]),
    at0('w.center'), dia('w', 60), hv('axis', 'horizontal'), len('axis', 80),
    hdim('axis.start', 'origin', 40), vdim('origin', 'axis.start', 38),
  ]),
  gears('Worm drive', [
    { id: 'W', type: 'worm', starts: 1, components: [pick('Worm stage', 'w', 'pitch'), pick('Worm stage', 'axis', 'axis')] },
  ], [], { drive: { item: 'W', gear: 1, rpm: 1500 } }),
  ratio('W', 1, 'W', 0),
];

// Planetary: sun at the origin, ring concentric, one planet touching the sun on the X axis.
ex['06-planetary'] = [
  ...sketch('Planetary', XY(), [
    circle('sun', [0, 0], 15), circle('planet', [30, 0], 15), circle('ring', [0, 0], 45),
    at0('sun.center'), dia('sun', 30), dia('planet', 30), dia('ring', 90), concentric('ring', 'sun'),
    tangent('planet', 'sun'), vdim('origin', 'planet.center', 0),
  ]),
  gears('Planetary set', [
    { id: 'P', type: 'planetary', planets: 4, carrier: true,
      components: [pick('Planetary', 'sun', 'sun'), pick('Planetary', 'ring', 'ring'), pick('Planetary', 'planet', 'planet')] },
  ], [], { module_blocks: [{ id: 'M', module: 1.5, members: ['P'] }], drive: { item: 'P', gear: 0, rpm: 1000 } }),
];

// Internal: ring at the origin, pinion inside it touching the ring, on the X axis.
ex['07-internal-ring'] = [
  ...sketch('Ring and pinion', XY(), [
    circle('ring', [0, 0], 40), circle('pin', [25, 0], 15),
    at0('ring.center'), dia('ring', 80), dia('pin', 30), tangent('pin', 'ring'), vdim('origin', 'pin.center', 0),
  ]),
  gears('Internal pair', [
    { id: 'R', type: 'internal', rim: 6, components: [pick('Ring and pinion', 'ring', 'pitch'), pick('Ring and pinion', 'pin', 'pitch2')] },
  ], [], { module_blocks: [{ id: 'M', module: 1.5, members: ['R'] }], drive: { item: 'R', gear: 1, rpm: 1000 } }),
  ratio('R', 1, 'R', 0),
];

for (const [name, cmds] of Object.entries(ex)) {
  fs.writeFileSync(path.join(out, 'scripts', name + '.json'), JSON.stringify({ commands: cmds }, null, 1));
}
console.log(Object.keys(ex).join('\n'));
