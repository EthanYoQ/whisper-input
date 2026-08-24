import type { StylePack } from './types';

export interface StyleDemo {
  source: 'builtin' | 'saved' | 'transient' | 'empty';
  input: string;
  output: string | null;
}

export interface TransientStylePreview {
  input: string;
  output: string;
}

export interface StylePackActionAvailability {
  canEdit: boolean;
  canToggle: boolean;
  canDelete: boolean;
}

export function stylePackActionAvailability(
  kind: StylePack['kind'],
): StylePackActionAvailability {
  return {
    canEdit: kind === 'custom',
    canToggle: true,
    canDelete: kind === 'custom',
  };
}

export function resolveStyleDemo(
  pack: StylePack,
  builtinInput: string,
  builtinOutput: string,
  transientPreview: TransientStylePreview | null,
  customInput?: string,
): StyleDemo {
  if (pack.kind === 'builtin') {
    return { source: 'builtin', input: builtinInput, output: builtinOutput };
  }

  if (transientPreview?.output.trim()) {
    return {
      source: 'transient',
      input: transientPreview.input,
      output: transientPreview.output,
    };
  }

  const savedExample = pack.examples[0];
  if (savedExample && (customInput === undefined || customInput === savedExample.input)) {
    return { source: 'saved', input: savedExample.input, output: savedExample.output };
  }

  return { source: 'empty', input: customInput ?? builtinInput, output: null };
}
