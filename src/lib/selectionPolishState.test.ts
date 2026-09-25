// @ts-ignore This repo runs contract tests under tsx without Node typings.
import { test } from 'node:test';
// @ts-ignore This repo runs contract tests under tsx without Node typings.
import assert from 'node:assert/strict';
import { applySelectionPolishEvent, canCopySelectionPolish, initialSelectionPolishPreviewState } from './selectionPolishState';

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

test('direct replacement rejection retains returned draft for copy without enabling replacement', () => {
  let state = applySelectionPolishEvent(initialSelectionPolishPreviewState, { kind: 'processing', requestId: 'direct' });
  state = applySelectionPolishEvent(state, { kind: 'error', requestId: 'direct', result: 'revised text', errorCode: 'targetChanged' });
  assert.equal(state.status, 'error');
  assert.equal(state.draft, 'revised text');
  assert.equal(canCopySelectionPolish(state), true);
});

test('preview replacement rejection keeps edited draft copyable for the current request only', () => {
  let state = applySelectionPolishEvent(initialSelectionPolishPreviewState, { kind: 'processing', requestId: 'preview' });
  state = applySelectionPolishEvent(state, { kind: 'ready', requestId: 'preview', result: 'initial' });
  state = { ...state, draft: 'edited' };
  state = applySelectionPolishEvent(state, { kind: 'error', requestId: 'preview', errorCode: 'targetChanged' });
  assert.equal(state.draft, 'edited');
  assert.equal(canCopySelectionPolish(state), true);
  assert.equal(canCopySelectionPolish({ ...state, busy: true }), false);
  assert.equal(applySelectionPolishEvent(state, { kind: 'error', requestId: 'stale', result: 'wrong' }).draft, 'edited');
});
