// A small client for SolidCraft's control channel, for driving the Gears dialog in a real
// window and taking screenshots of it (gear-ui-review). Usage from another module:
//   import { connect } from './sc.mjs';
//   const sc = await connect(7980);
//   await sc.call('ui.start', { command: 'gear.create' });
//   await sc.shot('01-empty');
// Never point it at the user's own app (7979).
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
export const shots = path.resolve(here, '..');
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function connect(port = 7980) {
  if (port === 7979) throw new Error('7979 is the user\'s app: never drive it');
  const tokenFile = path.join(process.env.LOCALAPPDATA, 'SolidCraft', 'control', `${port}.token`);
  const token = fs.readFileSync(tokenFile, 'utf8').trim();
  const sock = net.connect(port, '127.0.0.1');
  await new Promise((res, rej) => { sock.once('connect', res); sock.once('error', rej); });
  let buf = '';
  const waiting = new Map();
  let next = 1;
  sock.on('data', (d) => {
    buf += d.toString();
    let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i);
      buf = buf.slice(i + 1);
      try {
        const m = JSON.parse(line);
        if (waiting.has(m.id)) { waiting.get(m.id)(m); waiting.delete(m.id); }
      } catch { /* an event line */ }
    }
  });
  const raw = (method, params = {}) => new Promise((res) => {
    const id = next++;
    waiting.set(id, res);
    sock.write(JSON.stringify({ token, id, method, params }) + '\n');
  });
  const sc = {
    raw,
    async call(method, params = {}) {
      const m = await raw(method, params);
      if (!m.ok) throw new Error(`${method}: ${m.error}`);
      return m.result;
    },
    async exec(command, params = {}) { return sc.call('engine.execute', { command, params }); },
    async script(file) {
      const j = JSON.parse(fs.readFileSync(file, 'utf8'));
      return sc.call('engine.script', { commands: j.commands });
    },
    async shot(name) {
      await sleep(400);
      const file = path.join(shots, `${name}.png`);
      await sc.call('ui.screenshot', { path: file });
      return file;
    },
    async handles(prefix = '') { return (await sc.call('ui.handles', { prefix })) ?? []; },
    async handle(name) {
      const hs = await sc.handles(name);
      const list = Array.isArray(hs) ? hs : (hs.handles ?? []);
      return list.find((h) => h.name === name) ?? list.find((h) => h.name.startsWith(name));
    },
    async clickHandle(name, extra = {}) {
      let h = await sc.handle(name);
      if (!h) throw new Error(`no handle ${name}`);
      // A dialog control scrolled out of the dialog's view: scroll it in first.
      if (h.x > 1150 && (h.y < 250 || h.y > 780) && !name.startsWith('dialog:ok') && !name.startsWith('dialog:cancel')) {
        await sc.call('ui.scroll', { x: 1300, y: 600, delta: -(h.y - 520) });
        await sleep(500);
        h = await sc.handle(name);
      }
      await sc.call('ui.click', { x: h.x, y: h.y, ...extra });
      await sleep(250);
      return h;
    },
    async click(x, y, extra = {}) { await sc.call('ui.click', { x, y, ...extra }); await sleep(250); },
    async move(x, y) { await sc.call('ui.move', { x, y }); await sleep(150); },
    async toScreen(p) { return (await sc.call('ui.worldToScreen', { points: [p] }))[0]; },
    async clickWorld(p, extra = {}) {
      const s = await sc.toScreen(p);
      if (!s) throw new Error(`behind the camera: ${p}`);
      const [x, y] = Array.isArray(s) ? s : [s.x, s.y];
      await sc.click(x, y, extra);
      return [x, y];
    },
    async dialog() { return (await sc.call('ui.inspect', {})).dialog; },
    close() { sock.end(); },
  };
  return sc;
}
