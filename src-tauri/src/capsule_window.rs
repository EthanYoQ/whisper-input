//! Keep framework visibility synchronized with the native capsule window.
#[cfg(target_os = "windows")]
fn show_topmost_without_activating<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> tauri::Result<()> {
    use std::io;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    // `alwaysOnTop` in tauri.conf.json only establishes the initial state. A hidden
    // window can later be demoted in the native z-order, so every visible transition
    // must restore both Tauri's state and the Win32 TOPMOST band. Set NOACTIVATE first
    // so neither show operation can take focus from the dictation target.
    window.set_focusable(false)?;
    window.show()?;
    window.set_always_on_top(true)?;

    let hwnd = HWND(window.hwnd()?.0);
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
        .map_err(|error| io::Error::other(format!("failed to restore capsule TOPMOST: {error}")))?;
    }
    Ok(())
}

pub fn set_visible<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    visible: bool,
) -> tauri::Result<()> {
    if visible {
        #[cfg(target_os = "windows")]
        return show_topmost_without_activating(window);
        #[cfg(not(target_os = "windows"))]
        window.show()?;
    } else {
        #[cfg(target_os = "windows")]
        window.set_focusable(false)?;
        window.hide()?;
    }
    Ok(())
}
