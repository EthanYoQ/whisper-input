import {
  applyGlassAlpha,
  clampGlassAlpha,
  GLASS_ALPHA_DEFAULT,
  GLASS_ALPHA_MAX,
  GLASS_ALPHA_MIN,
  readGlassAlpha,
  setGlassAlpha,
} from './glassAlpha';

function assert(condition: boolean, message: string) {
  if (!condition) throw new Error(message);
}

function setGlobalProperty(name: 'window' | 'document', value: unknown): void {
  Object.defineProperty(globalThis, name, {
    value,
    configurable: true,
    writable: true,
  });
}

function removeGlobalProperty(name: 'window' | 'document'): void {
  delete (globalThis as Record<string, unknown>)[name];
}

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
const originalDocument = Object.getOwnPropertyDescriptor(globalThis, 'document');

function restoreGlobals(): void {
  removeGlobalProperty('window');
  removeGlobalProperty('document');
  if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow);
  if (originalDocument) Object.defineProperty(globalThis, 'document', originalDocument);
}

try {
  assert(clampGlassAlpha(0.2) === GLASS_ALPHA_MIN, 'alpha below the floor should clamp to the minimum');
  assert(clampGlassAlpha(2) === GLASS_ALPHA_MAX, 'alpha above the ceiling should clamp to the maximum');
  assert(
    clampGlassAlpha(Number.NaN) === GLASS_ALPHA_DEFAULT,
    'non-finite alpha should fall back to the default',
  );
  assert(clampGlassAlpha(0.8) === 0.8, 'in-range alpha should pass through unchanged');

  removeGlobalProperty('window');
  removeGlobalProperty('document');
  assert(readGlassAlpha() === GLASS_ALPHA_DEFAULT, 'missing window should fall back to the default');
  applyGlassAlpha(0.6);
  setGlassAlpha(0.6);

  setGlobalProperty('window', {
    localStorage: {
      getItem: () => null,
      setItem: () => undefined,
      removeItem: () => undefined,
    },
  });
  assert(readGlassAlpha() === GLASS_ALPHA_DEFAULT, 'missing storage value should fall back to the default');

  setGlobalProperty('window', {
    localStorage: {
      getItem: () => 'unexpected',
      setItem: () => undefined,
      removeItem: () => undefined,
    },
  });
  assert(readGlassAlpha() === GLASS_ALPHA_DEFAULT, 'invalid storage should fall back safely');

  setGlobalProperty('window', {
    localStorage: {
      getItem: () => '0.05',
      setItem: () => undefined,
      removeItem: () => undefined,
    },
  });
  assert(readGlassAlpha() === GLASS_ALPHA_MIN, 'stored alpha below the floor should clamp to the minimum');

  setGlobalProperty('window', {
    localStorage: {
      getItem: () => { throw new Error('blocked'); },
      setItem: () => undefined,
      removeItem: () => undefined,
    },
  });
  assert(readGlassAlpha() === GLASS_ALPHA_DEFAULT, 'storage read failures should return a safe default');

  let written: string | null = null;
  const writtenValues: string[] = [];
  let removed = false;
  setGlobalProperty('window', {
    localStorage: {
      getItem: () => written,
      setItem: (_key: string, value: string) => { writtenValues.push(value); written = value; },
      removeItem: () => { removed = true; written = null; },
    },
  });
  setGlobalProperty('document', {
    documentElement: {
      style: {
        setProperty: (_name: string, value: string) => { writtenValues.push(`css:${value}`); },
      },
    },
  });
  setGlassAlpha(0.75);
  assert(writtenValues.includes('0.75'), 'setGlassAlpha should persist the clamped value');
  assert(writtenValues.includes('css:0.75'), 'setGlassAlpha should write the CSS variable');
  setGlassAlpha(GLASS_ALPHA_DEFAULT);
  assert(removed, 'resetting to the default should clear the stored override');

  removeGlobalProperty('document');
  applyGlassAlpha(0.6);
  setGlassAlpha(0.6);
  assert(true, 'document absence should not throw');
} finally {
  restoreGlobals();
}
