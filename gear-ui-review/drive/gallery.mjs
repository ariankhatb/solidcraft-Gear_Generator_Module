// Gallery renders of the types round 2 fixed (800 × 600, ui.render, no grid or axes), into
// docs/gears/img/types: crossed-helical.png and bevel-chevron.png; plus the tooth chamfer.
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import * as w from './wt.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const out = path.resolve(here, '..', '..', 'docs', 'gears', 'img', 'types');
fs.mkdirSync(out, { recursive: true });
const which = process.argv[2] ?? 'all';

async function render(sc, name, view = 'iso', zoom = 1.1, cam = null) {
  // As the gallery: each gear in its own colour, no sketches or joint glyphs.
  for (const folder of ['sketches', 'joints']) {
    try { await sc.exec('browser.visibility', { visible: false, folder }); } catch (e) { console.log('hide', folder, e.message); }
  }
  try { await sc.call('ui.start', { command: 'inspect.colored_components' }); } catch (e) { console.log('colours', e.message); }
  await w.frame(sc, view, zoom);
  if (cam) {
    await sc.call('ui.camera', cam);
    const c = await sc.call('ui.camera', { fit: true });
    if (c?.distance) await sc.call('ui.camera', { distance: c.distance * zoom });
  }
  await w.sleep(1500);
  const file = path.join(out, `${name}.png`);
  await sc.call('ui.render', { path: file, width: 800, height: 600, grid: false, axes: false });
  console.log('rendered', file);
}

async function build(commands) {
  const sc = await w.begin(null);
  const r = await sc.call('engine.script', { commands });
  console.log(JSON.stringify(r).slice(0, 300));
  await w.sleep(4000);
  return sc;
}

if (which === 'all' || which === 'crossed') {
  // Gear A round Z (Ø40), gear B (Ø20) round X through (0, 30, 0): skew at 90°, 30 mm apart.
  const sc = await build([
    { command: 'sketch.create', params: { plane: 'XY', name: 'Gear A' } },
    { command: 'sketch.circle.center', params: { center: [0, 0], radius: 20, id: 'a' } },
    { command: 'sketch.finish', params: {} },
    { command: 'sketch.create', params: { plane: { origin: [0, 30, 0], x_dir: [0, 1, 0], y_dir: [0, 0, 1] }, name: 'Gear B' } },
    { command: 'sketch.circle.center', params: { center: [0, 0], radius: 10, id: 'b' } },
    { command: 'sketch.finish', params: {} },
    { command: 'gear.create', params: { name: 'Crossed helical', assembly: true, items: [{ id: 'A', type: 'crossed_helical', components: [{ sketch: 'Gear A', curve: 'a', slot: 'pitch' }, { sketch: 'Gear B', curve: 'b', slot: 'pitch2' }], gears: [{ gear: 0, size_by: 'module', module: 1 }] }] } },
  ]);
  await sc.call('ui.view', { view: 'iso' });
  await render(sc, 'crossed-helical');
  sc.close();
}

if (which === 'all' || which === 'chevron') {
  const j = JSON.parse(fs.readFileSync(path.resolve(here, '..', '..', 'examples', 'gears', 'scripts', '04-bevel-side-view.json'), 'utf8'));
  const cmds = j.commands.map((c) => {
    if (c.command !== 'gear.create') return c;
    const p = JSON.parse(JSON.stringify(c.params));
    p.name = 'Chevron bevel pair';
    p.items[0].bevel_style = 'chevron';
    p.items[0].gears = p.items[0].gears.map((g) => ({ ...g, spiral_angle: 30 }));
    return { ...c, params: p };
  });
  const sc = await build(cmds);
  await render(sc, 'bevel-chevron', 'iso', Number(process.argv[5] ?? 0.8), { yaw: Number(process.argv[3] ?? 2.4), pitch: Number(process.argv[4] ?? 0.35) });
  sc.close();
}
