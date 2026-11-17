// Walkthrough 1: a spur pair from 01-spur-pair's sketch (a Ø40 at the origin, a Ø60 tangent).
import * as w from './wt.mjs';
const sc = await w.begin('01-spur-pair.json');
await w.gears(sc);
await sc.shot('w1-01-empty');
await w.newGear(sc);
console.log('1', await w.status(sc));
await sc.shot('w1-02-new-gear');
await w.pick(sc, [0, 20, 0]);
console.log('2', await w.status(sc));
await sc.shot('w1-03-gear-a-picked');
await w.pick(sc, [50, 30, 0]);
console.log('3', await w.status(sc));
await sc.shot('w1-04-gear-b-started-by-pick');
await sc.clickHandle('gear:accept');
try { await sc.clickHandle('dialog:Driver'); await w.sleep(400); await sc.clickHandle('dialog:Driver:A'); } catch (e) { console.log('driver', e.message); }
await w.sleep(800);
await sc.shot('w1-05-linked');
console.log(JSON.stringify(await w.request(sc)).slice(0, 400));
await w.ok(sc);
await w.frame(sc, 'iso', 1.2);
await sc.shot('w1-06-built');
console.log(JSON.stringify(await w.gearsFeature(sc)));
sc.close();
