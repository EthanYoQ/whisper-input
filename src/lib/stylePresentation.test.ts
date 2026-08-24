import { resolveStyleDemo, stylePackActionAvailability } from './stylePresentation';
import type { StylePack } from './types';

function assertDeepEqual(actual: unknown, expected: unknown) {
  const actualJson = JSON.stringify(actual);
  const expectedJson = JSON.stringify(expected);
  if (actualJson !== expectedJson) {
    throw new Error(`Expected ${expectedJson}, received ${actualJson}`);
  }
}

const builtinPack: StylePack = {
  id: 'builtin.light',
  name: 'Light',
  description: '',
  kind: 'builtin',
  baseMode: 'light',
  dictationPrompt: '',
  selectionPrompt: '',
  examples: [],
  createdAt: '',
  updatedAt: '',
};

const demo = resolveStyleDemo(
  builtinPack,
  'raw spoken input',
  'trusted built-in output',
  null,
);

assertDeepEqual(demo, {
  source: 'builtin',
  input: 'raw spoken input',
  output: 'trusted built-in output',
});

assertDeepEqual(stylePackActionAvailability('builtin'), {
  canEdit: false,
  canToggle: true,
  canDelete: false,
});

assertDeepEqual(stylePackActionAvailability('custom'), {
  canEdit: true,
  canToggle: true,
  canDelete: true,
});

const savedCustomPack: StylePack = {
  ...builtinPack,
  id: 'custom.meeting-notes',
  name: 'Meeting notes',
  kind: 'custom',
  examples: [{ title: 'Confirmed example', input: 'meeting input', output: 'meeting output' }],
};

assertDeepEqual(
  resolveStyleDemo(savedCustomPack, 'fallback input', 'fallback output', null),
  { source: 'saved', input: 'meeting input', output: 'meeting output' },
);

assertDeepEqual(
  resolveStyleDemo(savedCustomPack, 'fallback input', 'fallback output', null, 'edited input'),
  { source: 'empty', input: 'edited input', output: null },
);

assertDeepEqual(
  resolveStyleDemo(
    savedCustomPack,
    'fallback input',
    'fallback output',
    { input: 'new preview input', output: 'new preview output' },
  ),
  { source: 'transient', input: 'new preview input', output: 'new preview output' },
);

const emptyCustomPack: StylePack = {
  ...savedCustomPack,
  id: 'custom.empty',
  examples: [],
};

assertDeepEqual(
  resolveStyleDemo(
    emptyCustomPack,
    'default preview input',
    'unused built-in output',
    { input: 'default preview input', output: '   ' },
  ),
  { source: 'empty', input: 'default preview input', output: null },
);
