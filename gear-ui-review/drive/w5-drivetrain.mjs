// Walkthrough 5: the big drivetrain (09-everything-drivetrain): seven layer sketches stacked in
// Z, ten gears made one by one through the dialog, then the suggested links accepted.
import * as w from './wt.mjs';
const sc = await w.begin('09-everything-drivetrain.json', { view: 'iso', zoom: 1.15 });
await w.gears(sc);
const plan = [
  ['Worm and Wheel', [[0, -30, 60], [20, 38, 60]]],
  ['Rack', [[25, -15, 30], [60, 0, 30]]],
  ['Spur', [[0, -15, 0]]],
  ['Spur', [[45, -30, 0]]],
  ['Herringbone', [[45, -15, -30]]],
  ['Herringbone', [[87, -27, -30]]],
  ['Helical', [[87, -12, -60]]],
  ['Helical', [[123, -24, -60]]],
  ['Planetary', [[0, -13.5, 90], [0, -40.5, 90], [27, 13.5, 90]]],
  ['Ring Gear', [[153, -50, -90], [123, -20, -90]]],
];
let n = 0;
for (const [type, picks] of plan) {
  await w.chooseType(sc, type);
  await w.newGear(sc);
  for (const p of picks) await w.pick(sc, p);
  console.log(++n, type, await w.status(sc));
  if (n === 2 || n === 9) await sc.shot(`w5-0${n === 2 ? 1 : 2}-gear-${n}`);
}
await sc.shot('w5-03-ten-gears');
try { await sc.clickHandle('gear:accept'); } catch (e) { console.log(e.message); }
await w.sleep(1500);
await sc.shot('w5-04-links');
const r = await w.request(sc);
console.log('links', JSON.stringify(r?.params?.links));
console.log('error', JSON.stringify((await sc.dialog())?.error));
sc.close();
