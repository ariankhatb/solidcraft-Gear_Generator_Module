import { connect, sleep } from './sc.mjs';
const sc = await connect(7980);
await sc.call('ui.scroll', { x: 1300, y: 650, delta: -(+process.argv[2] || 400) });
await sleep(600);
await sc.shot(process.argv[3] ?? 'probe');
let h = await sc.handles('gear:'); console.log(JSON.stringify(h).slice(0, 800));
sc.close();
