//! Keep framework visibility synchronized with the native capsule window.
pub fn set_visible<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    visible: bool,
) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    window.set_focusable(false)?;
    if visible {
        window.show()?;
    } else {
        window.hide()?;
    }
    Ok(())
}
