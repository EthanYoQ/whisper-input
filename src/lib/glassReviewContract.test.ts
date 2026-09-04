// @ts-ignore This repo intentionally runs contract tests under tsx without Node typings.
import { existsSync, readFileSync } from 'node:fs';

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

const qaSource = readFileSync(new URL('../pages/QaPanel.tsx', import.meta.url), 'utf8');
const selectionSource = readFileSync(new URL('../pages/SelectionPolishPanel.tsx', import.meta.url), 'utf8');
const shellSource = readFileSync(new URL('../components/FloatingShell.tsx', import.meta.url), 'utf8');
const mainSource = readFileSync(new URL('../main.tsx', import.meta.url), 'utf8');
const alphaSource = readFileSync(new URL('./glassAlpha.ts', import.meta.url), 'utf8');
const glassCss = readFileSync(new URL('../styles/glass.css', import.meta.url), 'utf8');
const previewCss = readFileSync(new URL('../styles/preview-replica.css', import.meta.url), 'utf8');
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
  tokensCss.includes('--ol-surface-2: rgb(255 255 255 / calc(0.45 * var(--lg-alpha-scale, 1)))') &&
    previewCss.includes('--wi-control: rgb(255 255 255 / calc(0.45 * var(--lg-alpha-scale, 1)))') &&
    previewCss.includes('--wi-control-active: rgb(255 255 255 / calc(0.78 * var(--lg-alpha-scale, 1)))') &&
    glassCss.includes('--lg-nav-active: rgb(255 255 255 / calc(0.68 * var(--lg-alpha-scale, 1)))'),
  'KIMI d724b2e control and navigation material strengths must remain the visual baseline',
);

assert(
  nativeSource.includes('apply_acrylic(&main, None)') &&
    nativeSource.includes('apply_mica(&main, None)') &&
    nativeSource.indexOf('apply_acrylic(&main, None)') < nativeSource.indexOf('apply_mica(&main, None)'),
  'Windows must preserve the approved Acrylic visual and use Mica only as its fallback',
);
assert(
  shellSource.includes('setThemePreference(nextTheme)') &&
    shellSource.includes("name={theme === 'dark' ? 'sun' : 'moon'}") &&
    shellSource.includes("t(theme === 'dark' ? 'shell.commandBar.themeLight' : 'shell.commandBar.themeDark')"),
  'the command bar must expose an accessible persisted light/dark toggle beside language',
);
assert(
  !nativeSource.includes('DwmEnableBlurBehindWindow'),
  'the unsupported Windows 8+ capsule blur-behind path must not return',
);
assert(
  !glassCss.includes("html[data-window-kind='main']:not([data-native-glass='on'])") &&
    !glassCss.includes('@media (prefers-reduced-transparency: reduce)') &&
    glassCss.includes('@media (forced-colors: active)'),
  'the main window must preserve KIMI transparency while forced-colors keeps its explicit accessibility fallback',
);

assert(
  cargoToml.includes('window-vibrancy = "0.6"') &&
    cargoToml.includes('window-vibrancy-win = { package = "window-vibrancy", version = "0.7" }') &&
    nativeSource.includes('use window_vibrancy_win::{apply_acrylic, apply_mica};'),
  'macOS must align with Tauri 0.6 while Windows retains KIMI window-vibrancy 0.7',
);
assert(
  (cargoLock.match(/name = "window-vibrancy"/g) ?? []).length === 2,
  'Cargo.lock must contain the platform-specific 0.6 and 0.7 window-vibrancy versions',
);

assert(existsSync(interLicenseUrl), 'the redistributed Inter font must include its license');
const interLicense = readFileSync(interLicenseUrl, 'utf8');
assert(
  interLicense.includes('SIL OPEN FONT LICENSE Version 1.1') &&
    interLicense.includes('Copyright (c) 2016 The Inter Project Authors'),
  'Inter LICENSE.txt must preserve the upstream copyright and OFL text',
);
