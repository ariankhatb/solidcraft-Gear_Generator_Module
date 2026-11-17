// Walkthrough 5, end: Driver W's worm at 3000 rpm, ratio A wheel → J ring, then OK on the big drivetrain.
import * as w from './wt.mjs';
import { connect } from './sc.mjs';
const sc = await connect(7980);
try { await sc.clickHandle('dialog:Driver'); await w.sleep(400); await sc.clickHandle('dialog:Driver:A worm'); } catch (e) { console.log('driver', e.message); }
await w.sleep(500);
try { await w.typeInto(sc, 'Input rpm', '3000'); } catch (e) { console.log('rpm', e.message); }
try { await sc.clickHandle('dialog:Ratio from'); await w.sleep(400); await sc.clickHandle('dialog:Ratio from:A worm'); await w.sleep(300); await sc.clickHandle('dialog:to'); await w.sleep(400); await sc.clickHandle('dialog:to:J ring'); } catch (e) { console.log('ratio', e.message); }
await w.sleep(1500);
await sc.shot('w5-05-driver-and-ratio');
const t0 = Date.now();
await w.ok(sc);
console.log('ok took', (Date.now() - t0) / 1000, 's');
await w.frame(sc, 'iso', 1.0);
await sc.shot('w5-06-built');
const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
console.log((doc.timeline ?? []).filter((f) => f.type === 'GearsFeature'), (doc.timeline ?? []).filter((f) => f.error).map((f) => f.name + ': ' + f.error));
sc.close();
