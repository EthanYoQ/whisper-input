// @ts-ignore This repo intentionally runs contract tests under tsx without Node typings.
import { existsSync, readFileSync } from 'node:fs';

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

const qaSource = readFileSync(new URL('../pages/QaPanel.tsx', import.meta.url), 'utf8');
const selectionSource = readFileSync(new URL('../pages/SelectionPolishPanel.tsx', import.meta.url), 'utf8');
const mainSource = readFileSync(new URL('../main.tsx', import.meta.url), 'utf8');
const alphaSource = readFileSync(new URL('./glassAlpha.ts', import.meta.url), 'utf8');
const glassCss = readFileSync(new URL('../styles/glass.css', import.meta.url), 'utf8');
const tokensCss = readFileSync(new URL('../styles/tokens.css', import.meta.url), 'utf8');
const nativeSource = readFileSync(new URL('../../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const cargoToml = readFileSync(new URL('../../src-tauri/Cargo.toml', import.meta.url), 'utf8');
const cargoLock = readFileSync(new URL('../../src-tauri/Cargo.lock', import.meta.url), 'utf8');
const interLicenseUrl = new URL('../assets/fonts/LICENSE.txt', import.meta.url);

for (const source of [qaSource, selectionSource]) {
  assert(source.includes('var(--lg-float-top)'), 'floating surfaces must use the shared light/dark tokens');
  assert(source.includes('var(--lg-float-bottom)'), 'floating surfaces must use the shared light/dark tokens');
}

for (const removedToken of ['--ol-ink-1', '--ol-surface-1', '--ol-danger']) {
  assert(!selectionSource.includes(removedToken), `selection polish must not use undefined ${removedToken}`);
}
assert(
  selectionSource.includes("width: '100%'") && selectionSource.includes("height: '100vh'"),
  'selection polish must fill its native window instead of exposing a transparent strip',
);

assert(
  alphaSource.includes("GLASS_ALPHA_EVENT = 'ui:glass-alpha-changed'") &&
    alphaSource.includes('emit(GLASS_ALPHA_EVENT') &&
    alphaSource.includes('listen<number>(GLASS_ALPHA_EVENT'),
  'glass alpha changes must broadcast and be consumed across Tauri windows',
);
assert(mainSource.includes('startGlassAlphaSync()'), 'every frontend window must start glass-alpha sync');
assert(
  tokensCss.includes('--ol-surface-2: rgb(255 255 255 / calc(0.18 * var(--lg-alpha-scale, 1)))'),
  'ordinary light-theme controls must not return to the stacked 45% white veil',
);

assert(
  nativeSource.includes('apply_mica(&main, None)') &&
    nativeSource.includes('apply_acrylic(&main, None)') &&
    nativeSource.includes('mark_native_glass_enabled(&main)'),
  'Windows must prefer Mica, retain an Acrylic fallback, and expose native-glass success',
);
assert(
  !nativeSource.includes('DwmEnableBlurBehindWindow'),
  'the unsupported Windows 8+ capsule blur-behind path must not return',
);
assert(
  glassCss.includes("html[data-window-kind='main']:not([data-native-glass='on'])") &&
    glassCss.includes('@media (forced-colors: active)') &&
    glassCss.includes('@media (prefers-reduced-transparency: reduce)'),
  'native-glass failure and accessibility modes must retain an opaque readable fallback',
);

assert(
  cargoToml.includes('window-vibrancy = "0.6"'),
  'the direct window-vibrancy dependency must align with Tauri to avoid duplicate macOS symbols',
);
assert(
  (cargoLock.match(/name = "window-vibrancy"/g) ?? []).length === 1,
  'Cargo.lock must contain exactly one window-vibrancy version',
);

assert(existsSync(interLicenseUrl), 'the redistributed Inter font must include its license');
const interLicense = readFileSync(interLicenseUrl, 'utf8');
assert(
  interLicense.includes('SIL OPEN FONT LICENSE Version 1.1') &&
    interLicense.includes('Copyright (c) 2016 The Inter Project Authors'),
  'Inter LICENSE.txt must preserve the upstream copyright and OFL text',
);
