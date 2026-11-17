// Scrolls the open dialog so a section's header is near the top, then a screenshot: node shotsec.mjs <section title> <name>
import { connect, sleep } from './sc.mjs';
const sc = await connect(7980);
let h = await sc.handles(`dialog:section:${process.argv[2]}`); h = Array.isArray(h) ? h : h.handles ?? [];
for (let i = 0; i < 6 && !h[0]; i++) { await sc.call('ui.scroll', { x: 1300, y: 600, delta: -300 }); await sleep(300); h = await sc.handles(`dialog:section:${process.argv[2]}`); h = Array.isArray(h) ? h : h.handles ?? []; }
if (h[0]) { await sc.call('ui.scroll', { x: 1300, y: 600, delta: -(h[0].y - 330) }); await sleep(700); }
console.log(JSON.stringify(h[0]));
await sc.shot(process.argv[3]);
sc.close();
