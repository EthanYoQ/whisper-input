# Diagnostic Observability Contract

This reference defines the current privacy and evidence boundaries for local dictation diagnostics. Runtime ownership lives in [`diagnostics.rs`](../../../src-tauri/src/diagnostics.rs), and dictation capture lives in [`coordinator/dictation.rs`](../../../src-tauri/src/coordinator/dictation.rs).

## Session Evidence

- Each completed or terminally failed dictation produces one structured trace with a stable trace ID.
- Traces record facts at the hotkey, recorder, ASR, LLM, and insertion boundaries.
- The trace distinguishes front application, streaming eligibility, insertion method, focus restoration, and insertion result.
- Evidence flags describe observed conditions; they do not infer a provider or application root cause.
- `pasteSent` means the paste shortcut was sent. It never claims that the target control accepted the text.

## Storage And Retention

- Traces are stored locally as backward-compatible JSONL records.
- Retention keeps at most the latest 200 traces and removes traces older than seven days when appending.
- Diagnostics do not upload automatically. Export occurs only through an explicit user action.

## Export And Privacy

- The diagnostic bundle is a ZIP containing recent traces, selected history, a log excerpt, a redacted settings summary, and environment metadata.
- Traces and exported history may contain raw and final dictation text, front application names, provider identifiers, errors, and timing data.
- Local traces do not record microphone audio, raw PCM, credential files, authorization headers, or unrelated clipboard contents. They may persist raw and final text, application names, and provider error strings, so the local JSONL file is sensitive data.
- Export construction redacts secret-like settings and error text before writing the ZIP and never reads credential files. Local JSONL persistence does not apply that export redaction pass.

## Compatibility

- New trace fields are optional so existing JSONL records remain readable.
- Existing target-application and insertion fields remain the single representation of those facts; extensions add missing evidence instead of duplicating them.
