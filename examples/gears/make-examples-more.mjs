// Herringbone example + one big drivetrain with every parallel/worm/rack/planetary/internal stage.
// Every sketch is fully defined (dimensions + constraints to its own origin).
import fs from 'node:fs';
import path from 'node:path';

const out = process.argv[2];
fs.mkdirSync(path.join(out, 'scripts'), { recursive: true });

const XY = z => ({ origin: [0, 0, z], x_dir: [1, 0, 0], y_dir: [0, 1, 0] });
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
// A bore circle: drawn on a pitch circle's centre (Concentric) with its diameter (0 DOF).
const bore = (id, of, x, d) => [circle(id, [x, 0], d / 2), concentric(id, of), dia(id, d)];
const XZ = { origin: [0, 0, 0], x_dir: [1, 0, 0], y_dir: [0, 0, 1] };
const point = (sketch, id) => ({ sketch, point: id, slot: 'bearing' });
// An upright shaft line at x (from z0 to z1, in an XZ sketch) with a bearing point on it at z:
// the line held vertical, placed and sized; the point on the line, its height dimensioned (0 DOF).
const shaftWithBearing = (l, x, z0, z1, p, z) => [
  line(l, [x, z0], [x, z1]), hv(l, 'vertical'), hdim('origin', `${l}.start`, x), vdim('origin', `${l}.start`, Math.abs(z0)), len(l, z1 - z0),
  C('sketch.point', { point: [x, z], id: p }), C('sketch.constraint.coincident', { a: p, b: l }), vdim('origin', p, Math.abs(z)),
];
const link = (kind, a, ag, b, bg) => ({ a: { item: a, gear: ag }, b: { item: b, gear: bg }, kind });
const ratio = (a, ag, b, bg) => C('gear.ratio', { a: { item: a, gear: ag }, b: { item: b, gear: bg } });
// A circle whose centre sits on the X axis at x (fully placed).
const onX = (id, x, d) => [circle(id, [x, 0], d / 2), x === 0 ? at0(`${id}.center`) : hdim('origin', `${id}.center`, x), ...(x === 0 ? [] : [vdim('origin', `${id}.center`, 0)]), dia(id, d)];

const ex = {};

ex['08-herringbone-pair'] = [
  ...sketch('Pitch circles', XY(0), [...onX('a', 0, 36), circle('b', [48, 0], 30), dia('b', 60), tangent('a', 'b'), vdim('origin', 'b.center', 0)]),
  C('gear.create', { name: 'Herringbone pair', assembly: true,
    items: [
      { id: 'A', type: 'herringbone', components: [pick('Pitch circles', 'a', 'pitch')], module_block: 'M', gears: [{ gear: 0, helix_angle: 25, hand: 'right', herringbone: true, face: { preset: 'symmetric', a: 10, b: 10 } }] },
      { id: 'B', type: 'herringbone', components: [pick('Pitch circles', 'b', 'pitch')], module_block: 'M', gears: [{ gear: 0, helix_angle: 25, hand: 'left', herringbone: true, face: { preset: 'symmetric', a: 10, b: 10 } }] },
    ],
    links: [link('mesh', 'A', 0, 'B', 0)], module_blocks: [{ id: 'M', module: 1.5 }], drive: { item: 'A', gear: 0, rpm: 1000, torque: 2 } }),
  ratio('A', 0, 'B', 0),
];

// The big drivetrain (each spur, herringbone and helical gear with a Ø8 bore circle on its centre). Shafts (world X): 0, 45, 87, 123, 153. Layers (world Z) step 30 mm.
//  L+60  worm W (wheel on shaft 0) driven by its worm along a line at y = 38
//  L+30  rack pinion G (shaft 45) on a rack line below it
//  L 0   spur A (shaft 0) -> spur B (shaft 45)
//  L-30  herringbone C (shaft 45) -> herringbone D (shaft 87)
//  L-60  helical E (shaft 87) -> helical F (shaft 123)
//  L+90  planetary P: sun on shaft 0 (above the worm wheel), ring concentric, 3 planets on a carrier
//  L-90  internal R: pinion on shaft 123 inside a ring centred on shaft 153
const S = n => `L${n}`;
const big = [
  ...sketch(S('+60 worm'), XY(60), [...onX('w', 0, 60), line('worm', [-40, 38], [40, 38]), hv('worm', 'horizontal'), len('worm', 80), hdim('worm.start', 'origin', 40), vdim('origin', 'worm.start', 38)]),
  ...sketch(S('+30 rack'), XY(30), [...onX('g', 45, 30), line('rack', [5, -15], [85, -15]), hv('rack', 'horizontal'), len('rack', 80), tangent('rack', 'g'), hdim('origin', 'rack.start', 5)]),
  ...sketch(S('0 spur'), XY(0), [...onX('a', 0, 30), circle('b', [45, 0], 30), dia('b', 60), tangent('a', 'b'), vdim('origin', 'b.center', 0), ...bore('abore', 'a', 0, 8), ...bore('bbore', 'b', 45, 8)]),
  ...sketch(S('-30 herringbone'), XY(-30), [...onX('c', 45, 30), circle('d', [87, 0], 27), dia('d', 54), tangent('c', 'd'), vdim('origin', 'd.center', 0), ...bore('cbore', 'c', 45, 8), ...bore('dbore', 'd', 87, 8)]),
  ...sketch(S('-60 helical'), XY(-60), [...onX('e', 87, 24), circle('f', [123, 0], 24), dia('f', 48), tangent('e', 'f'), vdim('origin', 'f.center', 0), ...bore('ebore', 'e', 87, 8), ...bore('fbore', 'f', 123, 8)]),
  ...sketch(S('+90 planetary'), XY(90), [...onX('sun', 0, 27), circle('planet', [27, 0], 13.5), circle('pring', [0, 0], 40.5), dia('planet', 27), dia('pring', 81), concentric('pring', 'sun'), tangent('planet', 'sun'), vdim('origin', 'planet.center', 0)]),
  ...sketch(S('-90 internal'), XY(-90), [...onX('rp', 123, 40), circle('ring', [153, 0], 50), dia('ring', 100), tangent('rp', 'ring'), vdim('origin', 'ring.center', 0)]),
  // Shaft lines of A, B (layer 0), E and F (layer -60), each with a bearing point 14 mm below its gear.
  ...sketch('Shafts', XZ, [
    ...shaftWithBearing('sa', 0, -18, 18, 'pa', -14), ...shaftWithBearing('sb', 45, -18, 18, 'pb', -14),
    ...shaftWithBearing('se', 87, -78, -42, 'pe', -74), ...shaftWithBearing('sf', 123, -78, -42, 'pf', -74),
  ]),
  C('gear.create', { name: 'Everything drivetrain', assembly: true,
    items: [
      { id: 'W', type: 'worm', starts: 2, gears: [{ gear: 0, face: { preset: 'symmetric', a: 3.8, b: 3.8 } }], components: [pick(S('+60 worm'), 'w', 'pitch'), pick(S('+60 worm'), 'worm', 'axis')] },
      { id: 'G', type: 'rack', components: [pick(S('+30 rack'), 'rack', 'axis'), pick(S('+30 rack'), 'g', 'pitch')], module_block: 'M' },
      { id: 'A', type: 'spur', components: [pick(S('0 spur'), 'a', 'pitch'), pick(S('0 spur'), 'abore', 'bore'), pick('Shafts', 'sa', 'axis'), point('Shafts', 'pa')], module_block: 'M' },
      { id: 'B', type: 'spur', components: [pick(S('0 spur'), 'b', 'pitch'), pick(S('0 spur'), 'bbore', 'bore'), pick('Shafts', 'sb', 'axis'), point('Shafts', 'pb')], module_block: 'M' },
      { id: 'C', type: 'herringbone', components: [pick(S('-30 herringbone'), 'c', 'pitch'), pick(S('-30 herringbone'), 'cbore', 'bore')], module_block: 'M', gears: [{ gear: 0, helix_angle: 25, hand: 'right', herringbone: true }] },
      { id: 'D', type: 'herringbone', components: [pick(S('-30 herringbone'), 'd', 'pitch'), pick(S('-30 herringbone'), 'dbore', 'bore')], module_block: 'M', gears: [{ gear: 0, helix_angle: 25, hand: 'left', herringbone: true }] },
      { id: 'E', type: 'helical', components: [pick(S('-60 helical'), 'e', 'pitch'), pick(S('-60 helical'), 'ebore', 'bore'), pick('Shafts', 'se', 'axis'), point('Shafts', 'pe')], module_block: 'M', gears: [{ gear: 0, helix_angle: 20, hand: 'right' }] },
      { id: 'F', type: 'helical', components: [pick(S('-60 helical'), 'f', 'pitch'), pick(S('-60 helical'), 'fbore', 'bore'), pick('Shafts', 'sf', 'axis'), point('Shafts', 'pf')], module_block: 'M', gears: [{ gear: 0, helix_angle: 20, hand: 'left' }] },
      { id: 'P', type: 'planetary', planets: 3, carrier: true, components: [pick(S('+90 planetary'), 'sun', 'sun'), pick(S('+90 planetary'), 'pring', 'ring'), pick(S('+90 planetary'), 'planet', 'planet')], module_block: 'M' },
      { id: 'R', type: 'internal', rim: 6, components: [pick(S('-90 internal'), 'ring', 'pitch'), pick(S('-90 internal'), 'rp', 'pitch2')], module_block: 'M2' },
    ],
    links: [
      link('shaft', 'W', 0, 'A', 0),
      link('mesh', 'A', 0, 'B', 0),
      link('shaft', 'B', 0, 'G', 1),
      link('shaft', 'B', 0, 'C', 0),
      link('mesh', 'C', 0, 'D', 0),
      link('shaft', 'D', 0, 'E', 0),
      link('mesh', 'E', 0, 'F', 0),
      link('shaft', 'W', 0, 'P', 0),
      link('shaft', 'F', 0, 'R', 1),
    ],
    module_blocks: [{ id: 'M', module: 1.5 }, { id: 'M2', module: 2 }],
    drive: { item: 'W', gear: 1, rpm: 3000, torque: 0.5 },
    // The housing cutout: the space it all turns in, with a 608 seat at each bearing point.
    clearance_body: { bearing: '608', bearing_fit: 0.15 } }),
  ratio('W', 1, 'A', 0),
  ratio('W', 1, 'F', 0),
  ratio('W', 1, 'R', 0),
  ratio('W', 1, 'P', 1),
  ratio('A', 0, 'G', 0),
  C('inspect.interference', {}),
];
ex['09-everything-drivetrain'] = big;

ex['10-hypoid-pair'] = [
  ...sketch('Hypoid', XY(0), [...onX('ring', 0, 90), line('pinion', [-60, -15], [60, -15]), hv('pinion', 'horizontal'), len('pinion', 120), hdim('pinion.start', 'origin', 60), vdim('pinion.start', 'origin', 15)]),
  C('gear.create', { name: 'Hypoid pair', assembly: true,
    items: [{ id: 'H', type: 'hypoid', components: [pick('Hypoid', 'ring', 'pitch'), pick('Hypoid', 'pinion', 'axis')], gears: [{ gear: 0, teeth: 37 }, { gear: 1, teeth: 9 }] }],
    links: [], drive: { item: 'H', gear: 1, rpm: 2000, torque: 5 } }),
  ratio('H', 1, 'H', 0),
];

for (const [name, cmds] of Object.entries(ex)) {
  fs.writeFileSync(path.join(out, 'scripts', name + '.json'), JSON.stringify({ commands: cmds }, null, 1));
}
console.log(Object.keys(ex).join('\n'));
