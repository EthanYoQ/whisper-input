# ASR Contextual Correction Design

## Goal

Improve dictation quality by making the LLM polish stage correct obvious ASR word errors from context while preserving the user's original facts, order, and selected output mode.

## Background

Community and product references point to the same pattern:

- Contextual ASR correction is usually a second analysis pass over the transcript using user-provided context, hotwords, names, and domain vocabulary.
- LLM post-ASR correction can help, but over-correction and hallucination are real risks.
- Reliable systems constrain correction to obvious recognition errors and keep the original transcript available for audit.

The current app already has the right surface for this:

- `src-tauri/src/polish.rs` builds the shared system prompt for raw, light, structured, and formal modes.
- `compose_system_prompt` already appends user hotwords and tells the model to prefer them when the transcript contains homophone or near-shape errors.
- `src-tauri/src/correction.rs` and `coordinator/dictation.rs` already apply deterministic user correction rules before and after LLM polish.
- Diagnostics already store both raw transcript and final text.

## Scope

This change strengthens the existing LLM prompt contract. It does not add a new API call, UI screen, model, or setting.

Included:

- Add a dedicated shared `# ASR 纠错` prompt block used by all polish modes.
- Define safe correction categories: homophone, near-shape, domain term, English/product spelling, common grammar word choice, and numeric/version preservation.
- Define hard limits: do not use external knowledge, do not infer missing facts, do not change entities unless context or hotwords make the correction clear, and keep uncertain text unchanged.
- Keep mode boundaries: light mode stays light; structured mode still structures; formal mode still uses formal document shape.
- Add Rust tests that fail if the correction block is removed or if it invites over-correction.

Excluded:

- No independent LLM correction pass before polish. That would add latency and duplicate failure modes.
- No automatic global replacements beyond existing user correction rules.
- No audio-level re-check. The app has only ASR text at this stage.
- No UI changes in this release.

## Prompt Contract

The shared prompt should tell the model:

1. Treat the transcript as ASR output, not as perfect source text.
2. Correct only errors that are obvious from local sentence context, user hotwords, or common ASR confusion patterns.
3. Prefer user hotwords for product names, hospital names, people, drugs, English terms, code names, branches, and versions.
4. Keep the original wording when multiple corrections are plausible.
5. Preserve numbers, dates, version strings, percentages, dosages, paths, commands, URLs, and mixed Chinese/English terms.
6. Never add facts to make a sentence more fluent.

## Data Flow

The runtime flow remains unchanged:

1. ASR provider returns raw transcript.
2. Existing deterministic correction rules run on raw transcript.
3. LLM receives raw transcript inside `<raw_transcript>`.
4. Shared prompt instructs LLM to perform conservative contextual correction as part of the selected mode.
5. Existing deterministic correction rules run on final text.
6. Diagnostics/history keep raw and final text.

## Testing

Add prompt-level tests in `src-tauri/src/polish.rs`:

- Every mode includes the dedicated `# ASR 纠错` block.
- The block includes conservative correction language and hotword/domain examples.
- The block explicitly preserves uncertain text and forbids external facts.
- Light mode still says it does not restructure.
- Structured and formal mode still retain their formatting requirements.

Run:

- `cargo test --lib asr_contextual_correction`
- `cargo test --lib`
- `npm run build`
- `cargo check --lib`
- `scripts/windows-preflight.ps1`
- `scripts/windows-package-msvc.ps1 -CleanArtifacts -SkipNpmCi`

## Release

After verification, bump version from `1.3.5` to `1.3.6`, package Windows artifacts, launch the new portable build locally, push `main`, create tag `v1.3.6`, and publish the release assets.
