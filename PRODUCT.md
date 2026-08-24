# Whisper Input Product Context

## Product Register

- Register: Product UI
- Platform: Windows-first desktop application built with Tauri and React; release assets also cover macOS arm64
- Primary users: Chinese-speaking workplace users who use voice input for daily writing
- Core purpose: turn speech into text, optionally polish or structure it with a configured LLM, and insert the result into the active application

## Design Direction

- Personality: quiet, trustworthy, efficient, and familiar
- Density: compact enough for a 1240 x 800 desktop window without sacrificing readable type or 40 px control targets
- Hierarchy: use spacing, alignment, typography, and lightweight separators before borders or shadows
- Component language: preserve the existing sidebar, settings tabs, cards, buttons, toggles, icon system, tokens, and Chinese/English localization
- Motion: restrained and functional; respect reduced-motion preferences

## Product Contracts

- Selection polish, history-derived actions, streaming diagnostics, and style packs are supported product capabilities; their runtime contracts live in the owning modules and tests.
- Structured diagnostics follow the [diagnostic observability contract](docs/superpowers/specs/2026-05-21-diagnostic-observability-design.md).
- LLM correction follows the [conservative ASR correction contract](docs/superpowers/specs/2026-05-24-asr-contextual-correction-design.md).
- Iteration-specific restrictions belong in their versioned spec, not in this stable product context.

## Anti-References

- Avoid one oversized vertical list with large empty areas
- Avoid nested cards, heavy borders, decorative shadows, loud danger styling, and unnecessary new labels
- Extend the existing icon system, navigation, controls, tokens, and localization instead of introducing a parallel visual language
