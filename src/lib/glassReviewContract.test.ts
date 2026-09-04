// @ts-ignore This repo intentionally runs contract tests under tsx without Node typings.
import { existsSync, readFileSync } from 'node:fs';

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

const qaSource = readFileSync(new URL('../pages/QaPanel.tsx', import.meta.url), 'utf8');
const selectionSource = readFileSync(new URL('../pages/SelectionPolishPanel.tsx', import.meta.url), 'utf8');
const shellSource = readFileSync(new URL('../components/FloatingShell.tsx', import.meta.url), 'utf8');
const windowChromeSource = readFileSync(new URL('../components/WindowChrome.tsx', import.meta.url), 'utf8');
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
    previewCss.includes('--wi-control-active: rgb(255 255 255 / calc(0.78 * var(--lg-alpha-scale, 1)))'),
  'KIMI d724b2e control material strengths must remain the visual baseline',
);
assert(
  glassCss.includes('--lg-sidebar-top: rgb(255 255 255 / calc(0.26 * var(--lg-alpha-scale, 1)))') &&
    glassCss.includes('--lg-sidebar-bottom: rgb(255 255 255 / calc(0.16 * var(--lg-alpha-scale, 1)))') &&
    glassCss.includes('--lg-sheet-top: rgb(253 254 255 / calc(0.60 * var(--lg-alpha-scale, 1)))') &&
    glassCss.includes('--lg-sheet-bottom: rgb(244 247 252 / calc(0.52 * var(--lg-alpha-scale, 1)))') &&
    glassCss.includes('rgb(255 255 255 / calc(0.28 * var(--lg-alpha-scale, 1)))') &&
    glassCss.includes('rgb(179 179 179 / calc(0.16 * var(--lg-alpha-scale, 1)))') &&
    glassCss.includes("--lg-nav-active: rgb(255 255 255 / calc(0.10 * var(--lg-alpha-scale, 1)))"),
  'Light must preserve the KIMI parent glass while keeping the nested navigation chip translucent and the approved dark material unchanged',
);
assert(
  !previewCss.includes('--wi-overview-line:') &&
    previewCss.includes('Light Overview uses one glass surface only') &&
    previewCss.includes('border: 0;\n  border-radius: 0;\n  background: transparent;') &&
    previewCss.includes('gap: 22px;\n  border: 0;\n  border-radius: 0;\n  background: transparent;') &&
    previewCss.includes('gap: 16px;\n  border: 0;\n  border-radius: 0;\n  background: transparent;') &&
    previewCss.includes(':root[data-theme=\'dark\'] .wi-overview-page .wi-model-grid'),
  'Light Overview children must paint no nested surface or border while preserving dark overrides',
);

assert(
  nativeSource.includes('apply_acrylic(&main, None)') &&
    nativeSource.includes('apply_mica(&main, None)') &&
    nativeSource.indexOf('apply_acrylic(&main, None)') < nativeSource.indexOf('apply_mica(&main, None)'),
  'Windows must preserve the approved Acrylic visual and use Mica only as its fallback',
);
assert(
  nativeSource.includes('(style | WS_THICKFRAME.0 as i32) & !(WS_CAPTION.0 as i32)') &&
    nativeSource.includes('ClientToScreen(hwnd, &mut client_origin)') &&
    nativeSource.includes('client_origin.x - rect.left') &&
    nativeSource.includes('if window.is_minimized().unwrap_or(false)') &&
    nativeSource.includes('tauri::WindowEvent::Focused(true)') &&
    nativeSource.includes('schedule_windows_rounded_frame(app)') &&
    nativeSource.includes('app.run_on_main_thread(move ||') &&
    windowChromeSource.includes('getCurrentWindow().startResizeDragging(direction)'),
  'the Windows main window must retain Acrylic-compatible resizing while clipping its native frame outside the client surface and restoring that crop after activation',
);
assert(
  !nativeSource.includes('SetWinEventHook(') &&
    !nativeSource.includes('EVENT_SYSTEM_FOREGROUND') &&
    nativeSource.includes('tauri::WindowEvent::Focused(false)') &&
    nativeSource.includes('schedule_main_minimize_after_focus_loss(app)') &&
    nativeSource.includes('Duration::from_millis(75)') &&
    nativeSource.includes('main.minimize()') &&
    !nativeSource.includes('hide_main_window(&app_for_main_thread)') &&
    nativeSource.includes('suppress_main_auto_minimize_for_explicit_show()') &&
    nativeSource.includes('main_auto_minimize_is_suppressed()'),
  'the Windows desktop main window must minimize from its own settled focus-loss event, preserving its taskbar entry without a global foreground hook racing explicit restore',
);
assert(
  nativeSource.indexOf('if let Err(err) = w.unminimize()') <
    nativeSource.indexOf('if let Err(err) = w.show()'),
  'a minimized or tray-hidden main window must be restored before it is shown again',
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
