# Privacy And Data Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the Privacy & Data settings surface to match the selected compact data-lifecycle layout without changing any behavior or copy.

**Architecture:** Keep `PrivacySection` and all of its existing handlers in `Settings.tsx`. Replace only its JSX grouping with semantic layout classes, then implement the visual hierarchy in the existing replica stylesheet. Extend the existing source-contract test so future edits cannot silently remove the approved groups or restore the oversized privacy card width.

**Tech Stack:** React 18, TypeScript, Vite, Tauri 2, Node built-in test runner, existing CSS tokens and UI components.

## Global Constraints

- Change only the Privacy & Data page structure and visual styling.
- Preserve every existing label, description, button, toggle, handler, confirmation, IPC call, busy state, and notification.
- Do not add dependencies, icons, routes, API calls, or new settings.
- Fit the standard 1240 x 800 desktop window without page scrolling.
- Keep a responsive single-column fallback for narrow content areas.

---

### Task 1: Privacy Layout Contract

**Files:**
- Modify: `src/lib/frontendReplicaContract.test.ts`
- Test: `src/lib/frontendReplicaContract.test.ts`

**Interfaces:**
- Consumes: source text from `src/pages/Settings.tsx` and `src/styles/preview-replica.css`
- Produces: regression assertions for `wi-privacy-data-flow`, `wi-privacy-retention`, `wi-privacy-maintenance`, and the responsive privacy layout

- [ ] **Step 1: Write the failing test**

Add assertions that require the three approved semantic groups and their responsive CSS selectors.

- [ ] **Step 2: Run the focused test and verify failure**

Run: `node --test src/lib/frontendReplicaContract.test.ts`

Expected: FAIL because the new privacy layout classes do not exist yet.

- [ ] **Step 3: Keep the failing output as the implementation baseline**

Confirm the failure names the missing privacy layout class rather than an unrelated test or runtime error.

### Task 2: Selected Layout Implementation

**Files:**
- Modify: `src/pages/Settings.tsx`
- Modify: `src/styles/preview-replica.css`
- Test: `src/lib/frontendReplicaContract.test.ts`

**Interfaces:**
- Consumes: existing `Card`, `Toggle`, `Btn`, translations, handlers, and `privacyHelpItemsCopy`
- Produces: the compact privacy information band and grouped retention/maintenance actions

- [ ] **Step 1: Restructure only the `PrivacySection` JSX**

Wrap the existing help items in `wi-privacy-data-flow`, the history toggle and local clear actions in `wi-privacy-retention`, and configuration/diagnostics actions in `wi-privacy-maintenance`. Keep each existing control expression and callback unchanged.

- [ ] **Step 2: Add the layout CSS**

Use compact grid rows, lightweight separators, right-aligned controls, and a narrow-width media/container fallback. Remove the privacy card's restrictive 860 px width so the selected layout can use the available content area.

- [ ] **Step 3: Run the focused test and verify it passes**

Run: `node --test src/lib/frontendReplicaContract.test.ts`

Expected: all source-contract tests pass.

- [ ] **Step 4: Run production compilation**

Run: `npm run build`

Expected: TypeScript and Vite complete without errors.

### Task 3: Desktop Runtime And Visual QA

**Files:**
- Modify: `design-qa.md`

**Interfaces:**
- Consumes: selected generated option 3 and the locally running Tauri application
- Produces: a same-state visual comparison and final QA result

- [ ] **Step 1: Restart the Tauri desktop application**

Stop only the running `src-tauri\\target\\debug\\whisper-input.exe` process and relaunch the existing Tauri dev command so current local configuration remains in use.

- [ ] **Step 2: Capture the Privacy & Data page at 1240 x 800**

Open Settings, select Privacy & Data, and capture the running desktop window in the same state as the selected reference.

- [ ] **Step 3: Compare the selected reference and implementation together**

Check content fit, hierarchy, spacing, row alignment, button visibility, clipping, borders, and scrollbar state. Fix P0-P2 issues and repeat the capture.

- [ ] **Step 4: Record the result**

Update `design-qa.md` with the reference path, implementation capture path, findings table, and the exact line `final result: passed` only after all blocking issues are resolved.

- [ ] **Step 5: Keep the local application running for user inspection**

Report the running executable path and local frontend URL used by the desktop application.
