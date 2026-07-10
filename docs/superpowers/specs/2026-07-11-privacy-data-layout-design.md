# Privacy And Data Layout Design

## Scope

Redesign only the visual structure of `Settings > Privacy & Data`. Preserve all existing controls, labels, descriptions, state, async behavior, confirmations, IPC calls, and notification behavior.

## Approved Direction

The user selected the third generated concept: a clear data-lifecycle layout that separates privacy information, local retention, and maintenance actions while fitting the standard 1240 x 800 desktop window.

## Information Architecture

1. Keep the existing settings navigation and selected Privacy & Data tab unchanged.
2. Present the existing audio, recognized text, and local data explanations as a compact three-row information band.
3. Present local retention actions in one grouped surface:
   - Save history, with the existing toggle.
   - Clear history, with the existing button.
   - Clear vocabulary, with the existing button.
4. Present maintenance actions in a second grouped surface:
   - Clear configuration, with the existing button.
   - Export diagnostic bundle, with the existing button.

## Visual Rules

- Use the existing design tokens, components, typography, and icon system.
- Use spacing, alignment, subtle surface tint, and row separators before borders and shadows.
- Keep body text at the existing readable product scale.
- Keep interactive targets at least 40 px high.
- Keep button alignment consistent on the right side of each action row.
- Avoid nested cards and avoid introducing new icons or decorative assets.
- At narrow content widths, switch grouped action rows to a single-column layout without clipping text or controls.

## Functional Invariants

- `onHistoryEnabledChange` continues to call `updatePrefs` and refresh on failure.
- Clear-history, clear-vocabulary, and clear-configuration confirmations remain unchanged.
- `runAction`, busy states, saved notifications, and failure notifications remain unchanged.
- Diagnostic filename generation and `exportDiagnosticBundle` invocation remain unchanged.
- Existing localization keys and `privacyHelpItemsCopy` content remain unchanged.

## Acceptance Criteria

- All three privacy explanations and all five controls are visible at 1240 x 800 without page scrolling.
- Existing actions remain keyboard reachable and retain their disabled/busy states.
- The page has no new visible heavy border or nested-card treatment.
- The layout remains usable at a narrower desktop content width through a responsive single-column fallback.
- TypeScript tests and the production build pass.
- Same-viewport visual QA compares the selected generated reference with the running desktop application and records `final result: passed` in `design-qa.md`.
