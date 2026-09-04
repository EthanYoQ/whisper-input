import { asyncSubscription } from './asyncSubscription';

function assert(value: boolean, message: string) { if (!value) throw new Error(message); }
async function run() {
  let resolve!: (stop: () => void) => void;
  let stops = 0;
  const stop = asyncSubscription(() => new Promise(r => { resolve = r; }), () => { throw new Error('unexpected'); });
  await Promise.resolve();
  stop(); stop();
  resolve(() => { stops++; });
  await new Promise(r => setTimeout(r, 0));
  assert(stops === 1, 'late subscription must be disposed exactly once');
  let errors = 0;
  asyncSubscription(async () => { throw new Error('denied'); }, () => { errors++; });
  await new Promise(r => setTimeout(r, 0));
  assert(errors === 1, 'active listener failure must reach visible error handler');
  const cancel = asyncSubscription(async () => { throw new Error('denied'); }, () => { errors++; });
  cancel();
  await new Promise(r => setTimeout(r, 0));
  assert(errors === 1, 'unmounted listener must not update React state');
}
void run();
