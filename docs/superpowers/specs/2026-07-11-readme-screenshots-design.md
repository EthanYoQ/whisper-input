# README Screenshot Enhancement Design

**Date:** 2026-07-11  
**Scope:** `README.md`, `README.en.md`, and repository-tracked documentation images only.

## Goal

Make the GitHub landing pages show the real Windows desktop interface immediately, while preserving every existing README section, explanation, example, table, diagram, and link.

## Content structure

1. Keep the existing centered title, positioning, badges, and download link unchanged.
2. Add one primary interface screenshot immediately below the download call to action. It demonstrates the four output styles and establishes the visual identity before visitors read the long-form explanation.
3. Add a new "Interface Preview" / "界面预览" section after the existing product workflow. It contains:
   - Model settings screenshot, paired with a short description of selecting a cloud ASR + LLM solution and entering BYOK credentials.
   - Privacy and data screenshot, paired with a short description of audio, recognized text, local history, and locally stored configuration.
4. Preserve all existing sections after the inserted content in their current order and with their existing wording.
5. Write the Chinese and English additions independently so the tone is natural in each language while the facts, screenshot order, and links remain equivalent.

## Assets

Copy these verified existing screenshots into `docs/images/` with stable, descriptive names:

- `output/style-master-integrated-1240x800.png` -> `docs/images/whisper-input-output-styles.png`
- `output/settings-models-compact-1240x800.png` -> `docs/images/whisper-input-model-settings.png`
- `output/privacy-data-final-1240x800.png` -> `docs/images/whisper-input-privacy-data.png`

README image references use relative paths so GitHub renders them in the repository and forked copies.

## Constraints

- Do not remove, condense, reword, or reorder original README content.
- Do not claim offline processing or vendor behavior that the product does not provide.
- Do not use screenshots that expose credentials; the selected screenshots show masked fields.
- Keep each image accessible with descriptive Chinese or English alt text.

## Verification

1. Confirm the three copied files exist and are tracked candidates rather than relying on ignored or temporary output folders.
2. Compare the original README sections before and after the inserted blocks to confirm every prior heading remains, in the same order, with unchanged content.
3. Check both Markdown files for valid relative image paths and inspect the rendered images locally.
4. Review `git diff --check` and a focused diff of only the new documentation assets and README insertions.
