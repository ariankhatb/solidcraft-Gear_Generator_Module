// A refusal's suggestions as one-click fixes on the card: two Ø20 circles touching at module 3.
import * as w from './wt.mjs';
const sc = await w.begin(null);
await sc.call('engine.script', { commands: [
  { command: 'sketch.create', params: { plane: 'XY', name: 'Pair' } },
  { command: 'sketch.circle.center', params: { center: [0, 0], radius: 10, id: 'a' } },
  { command: 'sketch.circle.center', params: { center: [20, 0], radius: 10, id: 'b' } },
  { command: 'sketch.finish', params: {} },
] });
await w.frame(sc, 'top', 2.2);
await w.gears(sc);
await w.pick(sc, [0, -10, 0]);
await w.pick(sc, [20, -10, 0]);
await w.sleep(1500);
const into = async (name, lo, hi) => { for (let k = 0; k < 14; k++) { let a = await sc.handles(name); a = Array.isArray(a) ? a : a.handles ?? []; const h = a[0]; if (h && h.y > lo && h.y < hi) return h; await sc.call('ui.scroll', { x: 1300, y: 700, delta: h ? -(h.y - (lo + hi) / 2) / 2 : -250 }); await w.sleep(400); } };
await into('dialog:row:Module', 300, 700);
await w.typeInto(sc, 'Module', '3 mm');
await w.sleep(3000);
await into('gear:card:A', 330, 420);
await w.sleep(800);
await sc.shot('final-r2-one-click-fixes');
let hs = await sc.handles('dialog:'); hs = Array.isArray(hs) ? hs : hs.handles ?? [];
console.log((await w.status(sc)) ?? '');
sc.close();
