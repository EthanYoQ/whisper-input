# ASR Contextual Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Strengthen the existing LLM polish prompts so ASR misrecognitions are conservatively corrected from context without changing facts or output-mode behavior.

**Architecture:** Add one shared prompt block in `src-tauri/src/polish.rs` and wire it into the existing `prompts::system_prompt` composition. Keep the current ASR -> correction rules -> LLM polish -> correction rules -> insertion flow unchanged.

**Tech Stack:** Rust, Tauri 2, existing `PolishMode` prompt tests, npm/Vite build, Windows MSVC packaging.

---

## File Structure

- Modify `src-tauri/src/polish.rs`: add the shared ASR correction prompt block, include it in `prompts::system_prompt`, and add prompt contract tests.
- Modify version files after tests pass: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`.
- No frontend code changes.

## Task 1: Add Failing Prompt Contract Tests

**Files:**
- Modify: `src-tauri/src/polish.rs`

- [x] **Step 1: Add failing tests**

Add prompt contract tests for:

- `asr_contextual_correction_prompt_is_present_for_all_polish_modes`
- `asr_contextual_correction_prompt_covers_domain_and_product_terms`
- `asr_contextual_correction_does_not_relax_mode_boundaries`

- [x] **Step 2: Run tests to verify RED**

Run:

```powershell
cargo test --lib asr_contextual_correction
```

Observed: two tests failed because `# ASR 纠错` and domain/product examples were not present yet.

## Task 2: Implement Shared ASR Correction Prompt Block

**Files:**
- Modify: `src-tauri/src/polish.rs`

- [x] **Step 1: Add shared prompt block**

Add `ASR_CORRECTION_BLOCK` next to `COMMON_RULES`. It covers homophone, near-shape, domain term, English/product spelling, numeric/version preservation, and hard anti-hallucination limits.

- [x] **Step 2: Include it in prompt composition**

Change `prompts::system_prompt` composition from:

```rust
format!("{}\n\n{}\n\n{}\n\n{}", ROLE_BLOCK, task_and_example, COMMON_RULES, OUTPUT_BLOCK)
```

to include `ASR_CORRECTION_BLOCK` before `OUTPUT_BLOCK`.

- [x] **Step 3: Run tests to verify GREEN**

Run:

```powershell
cargo test --lib asr_contextual_correction
```

Observed: three ASR contextual correction prompt tests passed.

## Task 3: Full Verification and Version Bump

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`

- [x] **Step 1: Run full Rust tests**

Run:

```powershell
cargo test --lib
```

Observed: 417 passed.

- [x] **Step 2: Run frontend build**

Run:

```powershell
npm run build
```

Observed: TypeScript and Vite production build passed.

- [x] **Step 3: Bump version to 1.3.6**

Run:

```powershell
npm version 1.3.6 --no-git-tag-version
```

Patch `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` from `1.3.5` to `1.3.6`, then run:

```powershell
cargo check --lib
```

Observed: `src-tauri/Cargo.lock` records `whisper-input` version `1.3.6`.

## Task 4: Package, Install Locally, Push, and Release

**Files:**
- Release artifacts under `.artifacts/windows-msvc`

- [ ] **Step 1: Run preflight**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\windows-preflight.ps1
```

Expected: preflight passes.

- [ ] **Step 2: Package Windows artifacts**

Stop any running portable build if it locks `.artifacts`, then run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\windows-package-msvc.ps1 -CleanArtifacts -SkipNpmCi
```

Expected: setup EXE, MSI, and portable ZIP are generated for `1.3.6`.

- [ ] **Step 3: Launch local current app from new portable build**

Run:

```powershell
Start-Process -FilePath '.artifacts\windows-msvc\Whisper_Input_1.3.6_x64_portable\Whisper Input.exe'
```

Expected: running `Whisper Input` process path points to the `1.3.6` portable directory.

- [ ] **Step 4: Commit and push main**

Run:

```powershell
git add docs/superpowers/specs/2026-05-24-asr-contextual-correction-design.md docs/superpowers/plans/2026-05-24-asr-contextual-correction.md package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/src/polish.rs
git commit -m "feat: add contextual ASR correction prompt"
git push origin main
```

Expected: `main` pushes successfully.

- [ ] **Step 5: Create GitHub release**

Run:

```powershell
git tag -a v1.3.6 -m "Whisper Input v1.3.6"
git push origin v1.3.6
gh release create v1.3.6 .artifacts\windows-msvc\Whisper_Input_1.3.6_x64_setup.exe .artifacts\windows-msvc\Whisper_Input_1.3.6_x64_en-US.msi .artifacts\windows-msvc\Whisper_Input_1.3.6_x64_portable.zip --title "Whisper Input v1.3.6" --notes "<release notes with verification and SHA256>"
```

Expected: release is latest and contains all three assets.
