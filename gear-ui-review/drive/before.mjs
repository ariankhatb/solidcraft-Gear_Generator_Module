// Screenshots of the Gears dialog as it was before the redesign (old row-based/Create New flow):
// empty, picking, a complete set, editing. Run against an app on port 7980:
//   node before.mjs
import path from 'node:path';
import { connect, sleep } from './sc.mjs';

const sc = await connect(7980);
const sk = (f) => path.join(path.dirname(new URL(import.meta.url).pathname).replace(/^\/(\w:)/, '$1'), 'sketches', f);

await sc.call('ui.documents', { action: 'new' });
await sleep(1500);
console.log('script', JSON.stringify(await sc.script(sk('01-spur-pair.json'))).slice(0, 200));
await sc.call('ui.view', { view: 'top' });
await sc.call('ui.camera', { fit: true });
await sleep(800);
console.log(JSON.stringify(await sc.call('ui.start', { command: 'gear.create' })).slice(0, 300));
await sleep(1200);
await sc.shot('before-01-empty');
const buttons = await sc.handles('dialog:button');
console.log(JSON.stringify(buttons).slice(0, 1500));
const create = (Array.isArray(buttons) ? buttons : buttons.handles ?? []).find((h) => h.name.endsWith('Create New'));
if (create) {
  await sc.click(create.x, create.y);
  await sleep(600);
  await sc.shot('before-02-new-item');
  await sc.clickWorld([0, 20, 0]);
  await sleep(1000);
  await sc.shot('before-03-picked-one');
  await sc.click(create.x, create.y);
  await sleep(600);
  await sc.clickWorld([50, 30, 0]);
  await sleep(1200);
  await sc.shot('before-04-two-items');
}
console.log(JSON.stringify(await sc.dialog()).slice(0, 1500));
sc.close();
