# Glass review fixes — 2026-09-05

Scope: follow-up to `17d8f831a13e5810af9a5d372f7efb4e61d844ea` on
`codex/glass-review-fixes`, Draft PR #17. Do not merge or publish installers
until native/manual acceptance is complete. Work in the existing checkout;
do not modify the separate `source` checkout or create another source copy.

## Preserved design and repaired behavior

- Preserve the Kimi sidebar and sheet material tokens and the four window roles.
  Light child cards remain transparent; the empty chart no longer adds a white
  overlay or renders empty axes and points. Do not increase parent opacity to
  mask a failed native backdrop.
- Synchronize light/dark preference across preloaded windows. Read before first
  paint, subscribe before rereading persisted state, and do not re-emit received
  events. The main toggle also reflects received changes.
- Keep Settings tabs and help/theme/language controls in wrapping normal flow.
- QA and selection text surfaces have a 90% alpha floor independent of the
  transparency slider. This is a readability fallback, not a new native blur.
  Capsule material is unchanged.
- Selection has only its event and dragging capabilities. Late subscriptions
  unsubscribe after unmount; registration errors produce a visible message.
- Only main receives resize-dragging permission. Its eight hit areas sit above
  the custom titlebar. Close is intercepted by Rust: cancel delayed blur work,
  then hide to tray without exiting.
- Explicit show starts a 750 ms grace period and must acquire actual focus
  before blur can minimize. Blur checks wait at least 75 ms; show, refocus and
  close invalidate old jobs. Auxiliary-window blur rechecks main after a
  same-process focus transfer. External focus minimizes rather than hides.
- Native Windows material uses checked Acrylic, then Mica, then explicit
  fallback results. Disabled system transparency/high contrast skips material
  attempts. Unknown or failed status delivery must not force opaque CSS or
  block React rendering. No global focus hook is introduced.
- Development protocol builds cannot register themselves or replace another
  installation's autostart entry. Valid other installations and user-disabled
  entries are preserved. Registry errors and unknown approval encodings fail
  closed rather than silently enabling startup.

## Repeatable checks

From this checkout, with dependencies installed:

```powershell
npm run build
rg --files src -g '*.test.ts' | ForEach-Object {
    npx tsx $_
    if ($LASTEXITCODE -ne 0) { throw "Test failed: $_" }
}
npm run check:hotkey-injection
$env:GITHUB_ACTIONS = 'true'
npm run check:hotkey-injection
Remove-Item Env:GITHUB_ACTIONS
cargo test --manifest-path src-tauri/Cargo.toml --lib --quiet
```

Browser regression: start Vite on port 1433, then run
`node scripts/check-glass-review.mjs`. Set `PLAYWRIGHT_MODULE` to an installed
Playwright package if it is outside the checkout, and `GLASS_TEST_URL` if using
another port. This uses an isolated Edge browser context and synthetic data.
It is not proof of Tauri IPC, native material or taskbar behavior.

Recorded results: frontend build passed; 17 TypeScript files passed; hotkey
checks passed (45 native unit tests / 4 CI-safe tests); Rust library tests
passed (500 passed, 4 ignored); all nine browser regression groups passed,
including five locales across six header widths and light/dark floating alpha
at slider minimum and maximum. Existing compiler/bundle warnings remain.

Rust unit tests now use process/thread-isolated synthetic directories under
`.runtime/.cache` and a shared mock credential backend, never user data paths
or the OS vault. Keep this isolation: the old tests could overwrite live
preferences and vocabulary presets. Ignored live provider probes require
explicit `WHISPER_INPUT_SMOKE_CREDENTIALS` JSON input (credential account names,
or `llm:<provider-id>`); they no longer discover keys from the user's vault.
Do not log this input or run live probes without authorization.

## Native acceptance

Use the self-contained release executable, not a copied development binary or
a Vite page. Stop the development server before final desktop acceptance.
Verify the running executable path; installed Preview and source builds may
share a single-instance identifier. Do not replace the user's autostart target
as part of this check.

Pending acceptance includes real tray and taskbar clicks, all four windows'
native theme/event behavior, auxiliary-window-to-external focus transfer,
Windows 10 material behavior, forced native failure/high contrast, mixed DPI,
and macOS. Passing unit or browser tests does not close these gates.
Keep local screenshots containing user history and recovery snapshots private;
never attach them to the public PR.

Latest local executable SHA-256:
`D41AA1712813F5E3E5F952E68DB59E9CF6EDD7778DD0286E41A54968AA275760`.
With Vite stopped, this build opened its embedded frontend. Light/dark material
and maximize/restore were visually inspected without the previous system
caption/white frame. Eight-direction native dragging is **not accepted**:
automated drag results were inconsistent and require an uninterrupted desktop
recheck. Do not report all native gates passed or push while this check remains
unresolved. The Draft PR has not yet received these follow-up changes.
