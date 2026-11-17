// Walkthrough 2: a compound train from two layer sketches and a shaft line (03-compound-train):
// Gear A spur (Ø30, layer 1) meshes the compound's big circle (Ø60, layer 1); the compound's small
// circle (Ø30, layer 2, 20 below) meshes Gear C (Ø50, layer 2).
import * as w from './wt.mjs';
const sc = await w.begin('03-compound-train.json', { view: 'iso', zoom: 1.25 });
await w.gears(sc);
await w.pick(sc, [-15, 0, 0]);                 // a: starts Gear A (Spur)
console.log('A', await w.status(sc));
await w.chooseType(sc, 'Compound');
await w.newGear(sc);
console.log('B new', await w.status(sc));
await sc.shot('w2-01-compound-new');
await w.pick(sc, [45, 0, 8]);                  // the shaft line, above layer 1
console.log('B shaft', await w.status(sc));
await sc.shot('w2-02-compound-shaft');
await w.pick(sc, [45, -30, 0]);                // c1
await w.pick(sc, [45, -15, -20]);              // c2
console.log('B done', await w.status(sc));
await sc.shot('w2-03-compound-complete');
await w.chooseType(sc, 'Spur');
await w.pick(sc, [85, -25, -20]);              // d: starts Gear C
console.log('C', await w.status(sc));
await sc.clickHandle('gear:accept');
await w.sleep(800);
await sc.shot('w2-04-train-linked');
console.log(JSON.stringify((await w.request(sc))?.params?.links));
await w.ok(sc);
await w.frame(sc, 'iso', 1.1);
await sc.shot('w2-05-built');
console.log(JSON.stringify(await w.gearsFeature(sc)));
sc.close();
