use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use super::{active_style, capture_frontmost_app, enabled_phrases, polish_text_with_style, Inner};
use crate::persistence::CredentialsVault;
use crate::selection::{
    classify_selection_insertion_target, reactivate_selection_insertion_target,
    selection_insertion_focus_matches, selection_insertion_target_accepts_text,
    selection_insertion_target_may_reactivate, SelectionInsertionTarget, SelectionReadPermission,
};
use crate::types::{DictationSession, HistoryAction, InsertStatus};

fn find_source(inner: &Arc<Inner>, id: &str) -> Result<DictationSession, String> {
    inner
        .history
        .list()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|session| session.id == id)
        .ok_or_else(|| "historyEntryNotFound".to_string())
}

fn derived_session(
    source: &DictationSession,
    final_text: String,
    action: HistoryAction,
    status: InsertStatus,
    mode: crate::types::PolishMode,
    inner: &Arc<Inner>,
) -> DictationSession {
    DictationSession {
        id: Uuid::new_v4().to_string(),
        created_at: Utc::now().to_rfc3339(),
        raw_transcript: source.raw_transcript.clone(),
        final_text,
        mode,
        app_bundle_id: None,
        app_name: capture_frontmost_app(),
        insert_status: status,
        error_code: None,
        duration_ms: None,
        dictionary_entry_count: None,
        asr_provider_id: Some(CredentialsVault::get_active_asr()),
        llm_provider_id: Some(CredentialsVault::get_active_llm()),
        history_action: Some(action),
        source_session_id: Some(source.id.clone()),
    }
}

fn persist_if_enabled(inner: &Arc<Inner>, session: &DictationSession) -> Result<(), String> {
    let prefs = inner.prefs.get();
    if prefs.history_enabled {
        inner
            .history
            .append_with_retention(session.clone(), prefs.history_retention_days)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) async fn repolish(
    inner: &Arc<Inner>,
    history_id: String,
) -> Result<DictationSession, String> {
    let source = find_source(inner, &history_id)?;
    let prefs = inner.prefs.get();
    let style = active_style(inner);
    let final_text = polish_text_with_style(
        &source.raw_transcript,
        style.base_mode,
        &enabled_phrases(inner),
        &prefs.working_languages,
        prefs.chinese_script_preference,
        prefs.effective_output_language_preference(),
        prefs.llm_thinking_enabled,
        capture_frontmost_app().as_deref(),
        &[],
        Some(&style.dictation_prompt),
    )
    .await
    .map_err(|error| error.to_string())?;
    let session = derived_session(
        &source,
        final_text,
        HistoryAction::Repolish,
        InsertStatus::CopiedFallback,
        style.base_mode,
        inner,
    );
    persist_if_enabled(inner, &session)?;
    Ok(session)
}

pub(super) fn reinsert(inner: &Arc<Inner>, history_id: String) -> Result<DictationSession, String> {
    let source = find_source(inner, &history_id)?;
    let prefs = inner.prefs.get();
    let style = active_style(inner);
    let target = inner.history_reinsert_target.lock().take();
    let status = if target
        .as_ref()
        .is_some_and(|target| history_target_is_safe(inner, target))
    {
        inner.inserter.insert(
            &source.final_text,
            prefs.restore_clipboard_after_paste,
            prefs.paste_shortcut,
        )
    } else {
        inner.inserter.copy_fallback(&source.final_text)
    };
    let mut session = derived_session(
        &source,
        source.final_text.clone(),
        HistoryAction::Reinsert,
        status,
        style.base_mode,
        inner,
    );
    if matches!(status, InsertStatus::Failed) {
        session.error_code = Some("historyReinsertFailed".into());
    }
    persist_if_enabled(inner, &session)?;
    Ok(session)
}

fn history_target_is_safe(inner: &Arc<Inner>, target: &SelectionInsertionTarget) -> bool {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use tauri::Manager;
        let Some(app) = inner.app.lock().clone() else {
            return false;
        };
        let Some(window) = app.get_webview_window("main") else {
            return false;
        };
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::Win32(raw) = handle.as_raw() else {
            return false;
        };
        let main_window = Some(raw.hwnd.get() as usize);
        if !selection_insertion_target_may_reactivate(target, main_window) {
            return false;
        }
        reactivate_selection_insertion_target(target)
            && selection_insertion_focus_matches(target)
            && selection_insertion_target_accepts_text(target)
            && classify_selection_insertion_target(target) == SelectionReadPermission::Allowed
            && selection_insertion_focus_matches(target)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = inner;
        selection_insertion_target_may_reactivate(target, None)
            && reactivate_selection_insertion_target(target)
            && selection_insertion_focus_matches(target)
            && selection_insertion_target_accepts_text(target)
            && classify_selection_insertion_target(target) == SelectionReadPermission::Allowed
            && selection_insertion_focus_matches(target)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (inner, target);
        false
    }
}
