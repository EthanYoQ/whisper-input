# Conservative ASR Contextual Correction Contract

This reference defines how LLM polishing corrects likely ASR errors without changing the user's facts or the selected output mode. Prompt ownership lives in [`polish.rs`](../../../src-tauri/src/polish.rs), while deterministic rules are applied by [`coordinator/dictation.rs`](../../../src-tauri/src/coordinator/dictation.rs).

## Runtime Contract

- Contextual correction is part of the existing polish request, not a separate model pass.
- Deterministic user correction rules run before and after LLM polishing.
- User hotwords guide names, organizations, products, domain terms, English spellings, code names, branches, and versions.
- Raw and final text remain distinct in history and diagnostics.

## Safety Rules

- Correct only recognition errors that are clear from local sentence context, user hotwords, or common ASR confusion patterns.
- Keep the original wording when multiple corrections are plausible.
- Preserve numbers, dates, versions, percentages, dosages, paths, commands, URLs, entities, and mixed Chinese/English terms unless the correction is unambiguous.
- Never add external facts or invent missing content to improve fluency.
- Light, structured, and formal modes retain their existing transformation boundaries.

## Extension Boundary

- Audio-level rechecking, a second correction model call, and automatic global replacement are separate product decisions.
