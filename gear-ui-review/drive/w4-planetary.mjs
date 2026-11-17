// Walkthrough 4: a planetary set (06-planetary): sun Ø30, ring Ø90, one planet Ø30.
import * as w from './wt.mjs';
const sc = await w.begin('06-planetary.json');
await w.gears(sc);
await w.chooseType(sc, 'Planetary');
await w.newGear(sc);
await w.pick(sc, [0, -15, 0]);
console.log('sun', await w.status(sc));
await sc.shot('w4-01-sun');
await w.pick(sc, [-45, 0, 0]);
console.log('ring', await w.status(sc));
await w.pick(sc, [30, 15, 0]);
console.log('planet', await w.status(sc));
await sc.shot('w4-02-complete');
await w.ok(sc);
await w.frame(sc, 'iso', 1.1);
await sc.shot('w4-03-built');
console.log(JSON.stringify(await w.gearsFeature(sc)));
sc.close();
