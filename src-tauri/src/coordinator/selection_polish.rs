use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, Manager};
use uuid::Uuid;

use super::{active_style, enabled_phrases, polish_selection_text_with_style, Inner};
use crate::selection::{
    reactivate_selection_insertion_target, selection_insertion_target_may_reactivate,
    validate_selection_insertion_target, SelectionPolishCapture, SelectionPolishWorkflow,
    SelectionReplacementAccess, SelectionReplacementOutcome, SelectionReplacementWorkflow,
    SystemSelectionAccess,
};
use crate::types::{InsertStatus, SelectionPolishOutputMode};

pub(super) struct SelectionPolishSession {
    request_id: String,
    capture: SelectionPolishCapture,
    result: Option<String>,
    preview_window: Option<usize>,
}

fn is_ready_for_confirmation(session: &SelectionPolishSession, request_id: &str) -> bool {
    session.request_id == request_id && session.result.is_some()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionPolishPayload<'a> {
    kind: &'a str,
    request_id: Option<&'a str>,
    result: Option<&'a str>,
    source_app: Option<&'a str>,
    error_code: Option<&'a str>,
    insert_status: Option<InsertStatus>,
}

fn emit_state(inner: &Arc<Inner>, payload: SelectionPolishPayload<'_>) {
    let Some(app) = inner.app.lock().clone() else {
        return;
    };
    if let Some(window) = app.get_webview_window("selection-polish") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit_to("selection-polish", "selection-polish:state", payload);
}

fn emit_if_current(
    inner: &Arc<Inner>,
    request_id: &str,
    payload: SelectionPolishPayload<'_>,
) -> bool {
    let state = inner.selection_polish_state.lock();
    if !state
        .as_ref()
        .is_some_and(|session| session.request_id == request_id)
    {
        return false;
    }
    emit_state(inner, payload);
    true
}

fn hide_window(inner: &Arc<Inner>) {
    if let Some(app) = inner.app.lock().clone() {
        if let Some(window) = app.get_webview_window("selection-polish") {
            let _ = window.hide();
        }
    }
}

#[cfg(target_os = "windows")]
fn selection_polish_window_handle(inner: &Arc<Inner>) -> Option<usize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let app = inner.app.lock().clone()?;
    let window = app.get_webview_window("selection-polish")?;
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return None;
    };
    Some(raw.hwnd.get() as usize)
}

#[cfg(not(target_os = "windows"))]
fn selection_polish_window_handle(_inner: &Arc<Inner>) -> Option<usize> {
    None
}

pub(super) async fn begin(inner: &Arc<Inner>) -> Result<(), String> {
    let request_id = Uuid::new_v4().to_string();
    {
        let mut active = inner.selection_polish_active_request.lock();
        *active = Some(request_id.clone());
        inner.selection_polish_state.lock().take();
    }
    let capture = SelectionPolishWorkflow::new(&SystemSelectionAccess)
        .capture()
        .map_err(|error| {
            if inner.selection_polish_active_request.lock().as_deref() == Some(request_id.as_str())
            {
                emit_state(
                    inner,
                    SelectionPolishPayload {
                        kind: "error",
                        request_id: None,
                        result: None,
                        source_app: None,
                        error_code: Some(error.code()),
                        insert_status: None,
                    },
                );
                error.code().to_string()
            } else {
                "selectionPolishCancelled".to_string()
            }
        })?;

    let source_app = capture.source_app.clone();
    let preview_window = selection_polish_window_handle(inner);
    {
        let active = inner.selection_polish_active_request.lock();
        if active.as_deref() != Some(request_id.as_str()) {
            return Err("selectionPolishCancelled".into());
        }
        *inner.selection_polish_state.lock() = Some(SelectionPolishSession {
            request_id: request_id.clone(),
            capture,
            result: None,
            preview_window,
        });
    }
    emit_if_current(
        inner,
        &request_id,
        SelectionPolishPayload {
            kind: "processing",
            request_id: Some(&request_id),
            result: None,
            source_app: source_app.as_deref(),
            error_code: None,
            insert_status: None,
        },
    );

    let prefs = inner.prefs.get();
    let style = active_style(inner);
    let model_text = inner
        .selection_polish_state
        .lock()
        .as_ref()
        .filter(|session| session.request_id == request_id)
        .map(|session| session.capture.model_text.clone())
        .ok_or_else(|| "selectionPolishCancelled".to_string())?;
    let hotwords = enabled_phrases(inner);
    let result = polish_selection_text_with_style(
        &model_text,
        style.base_mode,
        &hotwords,
        &prefs.working_languages,
        prefs.chinese_script_preference,
        prefs.effective_output_language_preference(),
        prefs.llm_thinking_enabled,
        source_app.as_deref(),
        Some(&style.selection_prompt),
    )
    .await
    .map_err(|error| {
        let code = "selectionPolishProviderFailed";
        log::warn!("[selection-polish] provider failed: {error:#}");
        if !emit_if_current(
            inner,
            &request_id,
            SelectionPolishPayload {
                kind: "error",
                request_id: Some(&request_id),
                result: None,
                source_app: source_app.as_deref(),
                error_code: Some(code),
                insert_status: None,
            },
        ) {
            return "selectionPolishCancelled".to_string();
        }
        code.to_string()
    })?;

    {
        let mut state = inner.selection_polish_state.lock();
        let Some(session) = state
            .as_mut()
            .filter(|session| session.request_id == request_id)
        else {
            return Err("selectionPolishCancelled".into());
        };
        session.result = Some(result.clone());
    }

    if prefs.selection_polish_output_mode == SelectionPolishOutputMode::DirectReplace {
        return replace(inner, request_id, result).map(|_| ());
    }

    emit_if_current(
        inner,
        &request_id,
        SelectionPolishPayload {
            kind: "ready",
            request_id: Some(&request_id),
            result: Some(&result),
            source_app: source_app.as_deref(),
            error_code: None,
            insert_status: None,
        },
    );
    Ok(())
}

struct SystemReplacementAccess<'a> {
    inner: &'a Arc<Inner>,
    preview_window: Option<usize>,
}

impl SelectionReplacementAccess for SystemReplacementAccess<'_> {
    fn may_reactivate(&self, target: &crate::selection::SelectionInsertionTarget) -> bool {
        selection_insertion_target_may_reactivate(target, self.preview_window)
    }

    fn reactivate(&self, target: &crate::selection::SelectionInsertionTarget) -> bool {
        reactivate_selection_insertion_target(target)
    }

    fn classify_target(
        &self,
        target: &crate::selection::SelectionInsertionTarget,
    ) -> crate::selection::SelectionReadPermission {
        crate::selection::classify_selection_insertion_target(target)
    }

    fn validate(
        &self,
        target: &crate::selection::SelectionInsertionTarget,
        expected_selection: &str,
    ) -> crate::selection::SelectionInsertionTargetValidation {
        validate_selection_insertion_target(target, expected_selection)
    }

    fn insert(&self, replacement: &str) -> InsertStatus {
        let prefs = self.inner.prefs.get();
        self.inner.inserter.insert(
            replacement,
            prefs.restore_clipboard_after_paste,
            prefs.paste_shortcut,
        )
    }
}

pub(super) fn replace(
    inner: &Arc<Inner>,
    request_id: String,
    replacement: String,
) -> Result<InsertStatus, String> {
    let outcome = {
        let mut state = inner.selection_polish_state.lock();
        let session = state
            .as_ref()
            .filter(|session| is_ready_for_confirmation(session, &request_id))
            .ok_or_else(|| "selectionPolishPreviewUnavailable".to_string())?;
        let outcome = SelectionReplacementWorkflow::new(&SystemReplacementAccess {
            inner,
            preview_window: session.preview_window,
        })
        .replace(
            &session.capture.target,
            &session.capture.full_text,
            &replacement,
        );
        if matches!(outcome, SelectionReplacementOutcome::Replaced(_)) {
            state.take();
            hide_window(inner);
        }
        outcome
    };

    match outcome {
        SelectionReplacementOutcome::Replaced(status) => Ok(status),
        SelectionReplacementOutcome::Rejected(code) => {
            let source_app = inner
                .selection_polish_state
                .lock()
                .as_ref()
                .filter(|session| session.request_id == request_id)
                .and_then(|session| session.capture.source_app.clone());
            emit_if_current(
                inner,
                &request_id,
                SelectionPolishPayload {
                    kind: "error",
                    request_id: Some(&request_id),
                    result: Some(&replacement),
                    source_app: source_app.as_deref(),
                    error_code: Some(code),
                    insert_status: None,
                },
            );
            Err(code.into())
        }
    }
}

pub(super) fn copy(inner: &Arc<Inner>, text: String) -> Result<(), String> {
    match inner.inserter.copy_fallback(text.trim()) {
        InsertStatus::CopiedFallback => Ok(()),
        _ => Err("selectionPolishCopyFailed".into()),
    }
}

pub(super) fn cancel(inner: &Arc<Inner>) {
    let mut active = inner.selection_polish_active_request.lock();
    active.take();
    inner.selection_polish_state.lock().take();
    hide_window(inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_requires_current_ready_request() {
        let mut session = SelectionPolishSession {
            request_id: "B".into(),
            capture: SelectionPolishCapture {
                target: Default::default(),
                full_text: "B".into(),
                model_text: "B".into(),
                source_app: None,
            },
            result: None,
            preview_window: None,
        };
        assert!(!is_ready_for_confirmation(&session, "A"));
        assert!(!is_ready_for_confirmation(&session, "B"));
        session.result = Some("B result".into());
        assert!(is_ready_for_confirmation(&session, "B"));
        assert!(!is_ready_for_confirmation(&session, "A"));
    }

    #[test]
    fn frontend_payload_never_contains_the_original_selection_or_fingerprint() {
        let payload = SelectionPolishPayload {
            kind: "ready",
            request_id: Some("request"),
            result: Some("polished"),
            source_app: Some("Editor"),
            error_code: None,
            insert_status: None,
        };

        let json = serde_json::to_string(&payload).unwrap();

        assert!(!json.contains("original"));
        assert!(!json.contains("fingerprint"));
        assert!(!json.contains("selected secret"));
    }
}
