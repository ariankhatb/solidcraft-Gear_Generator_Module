// Walkthrough 3: a worm and wheel (05-worm-and-wheel): the wheel's Ø60 circle and the worm's
// axis line above it.
import * as w from './wt.mjs';
const sc = await w.begin('05-worm-and-wheel.json');
await w.gears(sc);
await w.chooseType(sc, 'Worm and Wheel');
await sc.shot('w3-01-type-worm');
await w.newGear(sc);
await w.pick(sc, [0, -30, 0]);
console.log('wheel', await w.status(sc));
await sc.shot('w3-02-wheel-picked');
await w.pick(sc, [20, 38, 0]);
console.log('worm', await w.status(sc));
await sc.shot('w3-03-worm-complete');
await w.ok(sc);
await w.frame(sc, 'iso', 1.1);
await sc.shot('w3-04-built');
console.log(JSON.stringify(await w.gearsFeature(sc)));
sc.close();
