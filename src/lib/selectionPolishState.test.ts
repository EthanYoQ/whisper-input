// @ts-ignore This repo runs contract tests under tsx without Node typings.
import { test } from 'node:test';
// @ts-ignore This repo runs contract tests under tsx without Node typings.
import assert from 'node:assert/strict';
import { applySelectionPolishEvent, initialSelectionPolishPreviewState } from './selectionPolishState';

test('new selection clears old draft and stale completion cannot alter current request', () => {
  let state = applySelectionPolishEvent(initialSelectionPolishPreviewState, {
    kind: 'processing', requestId: 'A',
  });
  state = applySelectionPolishEvent(state, { kind: 'ready', requestId: 'A', result: 'A result' });
  state = applySelectionPolishEvent(state, { kind: 'processing', requestId: 'B' });
  assert.equal(state.draft, '');
  assert.equal(state.status, 'processing');
  state = applySelectionPolishEvent(state, { kind: 'error', requestId: 'A', errorCode: 'late' });
  assert.equal(state.status, 'processing');
  state = applySelectionPolishEvent(state, { kind: 'ready', requestId: 'B', result: 'B result' });
  assert.equal(state.draft, 'B result');
});
