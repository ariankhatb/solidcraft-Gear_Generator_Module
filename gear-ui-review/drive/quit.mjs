import { connect } from './sc.mjs';
const sc = await connect(7980); try { await sc.call('app.quit', { force: true }); } catch (e) { console.log(e.message); } sc.close();
