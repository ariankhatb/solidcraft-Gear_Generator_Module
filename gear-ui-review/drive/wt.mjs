// Walkthrough helpers: drive the Gears dialog the way a person does (clicks on its buttons and
// on the sketch geometry in the view), screenshotting each step.
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { connect, sleep } from './sc.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));

export async function begin(sketchScript, { view = 'top', zoom = 1.35 } = {}) {
  const sc = await connect(7980);
  try { await sc.call('ui.recover', { action: 'later' }); } catch { /* none */ }
  if (await sc.dialog()) await sc.call('ui.key', { key: 'Escape' });
  await sc.call('ui.documents', { action: 'new' });
  await sleep(1200);
  if (sketchScript) await sc.script(path.join(here, 'sketches', sketchScript));
  await frame(sc, view, zoom);
  return sc;
}

/// The view framed on everything, pulled back so the dialog on the right doesn't cover it.
export async function frame(sc, view = 'top', zoom = 1.35) {
  await sc.call('ui.view', { view });
  const cam = await sc.call('ui.camera', { fit: true });
  if (cam?.distance) await sc.call('ui.camera', { distance: cam.distance * zoom });
  await sleep(500);
}

export async function gears(sc) {
  await sc.call('ui.start', { command: 'gear.create' });
  await sleep(1200);
}

export async function chooseType(sc, label) {
  await sc.clickHandle('dialog:Type');
  await sleep(500);
  await sc.clickHandle(`dialog:Type:${label}`);
  await sleep(400);
}

export async function newGear(sc) { await sc.clickHandle('gear:new'); await sleep(500); }

/// A pick in the view at a world point (on a circle, a line or a point of the sketch).
export async function pick(sc, p) { await sc.clickWorld(p); await sleep(700); }

export async function status(sc) {
  let h = await sc.handles('gear:status:');
  h = Array.isArray(h) ? h : h.handles ?? [];
  return h.map((x) => x.name.slice('gear:status:'.length)).join(' | ');
}

export async function ok(sc) {
  await sc.clickHandle('dialog:ok');
  for (let i = 0; i < 120; i++) {
    await sleep(1000);
    const d = await sc.dialog();
    const ops = await sc.call('ui.operations', {});
    if (!d && !(ops?.operations ?? ops)?.length) break;
  }
  await sleep(1500);
}

/// Types a value into a dialog row's field (clicks the field, selects all, types, Tab away).
export async function typeInto(sc, row, text) {
  const h = await sc.handle(`dialog:row:${row}`);
  if (!h) throw new Error(`no row ${row}`);
  await sc.click(h.x + 90, h.y);
  await sc.call('ui.key', { key: 'A', ctrl: true });
  await sc.call('ui.text', { text });
  await sc.call('ui.key', { key: 'Tab' });
  await sleep(600);
}

export async function gearsFeature(sc) {
  const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
  return (doc.timeline ?? []).filter((f) => f.type === 'GearsFeature').pop();
}

export async function request(sc) {
  const d = await sc.dialog();
  return d?.commands?.[0];
}

export { sleep };
