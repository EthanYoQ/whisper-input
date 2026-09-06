//! Isolated native regression: no coordinator, microphone, credentials or user data.
//! --legacy replays the old raw-HWND show path and must fail the visibility check.
use std::time::Duration;
use tauri::Emitter;

#[cfg(target_os = "windows")]
fn hwnd(window: &tauri::WebviewWindow) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(window.hwnd().expect("window HWND").0)
}

#[cfg(target_os = "windows")]
fn demote_from_topmost(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_NOTOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    unsafe {
        SetWindowPos(
            hwnd(window),
            HWND_NOTOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .expect("demote synthetic capsule");
    }
}

#[cfg(target_os = "windows")]
fn topmost_without_focus(
    window: &tauri::WebviewWindow,
    foreground_window: &tauri::WebviewWindow,
) -> Result<(), String> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowRect, GWL_EXSTYLE, GW_HWNDNEXT,
        WS_EX_TOPMOST,
    };

    let capsule = hwnd(window);
    let foreground = hwnd(foreground_window);
    let extended_style = unsafe { GetWindowLongPtrW(capsule, GWL_EXSTYLE) } as u32;
    if extended_style & WS_EX_TOPMOST.0 == 0 {
        return Err("visible capsule did not reassert WS_EX_TOPMOST".into());
    }
    let foreground_style = unsafe { GetWindowLongPtrW(foreground, GWL_EXSTYLE) } as u32;
    if foreground_style & WS_EX_TOPMOST.0 != 0 {
        return Err("ordinary foreground window unexpectedly became TOPMOST".into());
    }
    if unsafe { GetForegroundWindow() } != foreground {
        return Err("showing the capsule changed keyboard focus".into());
    }

    let mut capsule_rect = RECT::default();
    let mut foreground_rect = RECT::default();
    unsafe { GetWindowRect(capsule, &mut capsule_rect) }
        .map_err(|error| format!("read capsule bounds: {error}"))?;
    unsafe { GetWindowRect(foreground, &mut foreground_rect) }
        .map_err(|error| format!("read foreground bounds: {error}"))?;
    let overlaps = capsule_rect.left < foreground_rect.right
        && capsule_rect.right > foreground_rect.left
        && capsule_rect.top < foreground_rect.bottom
        && capsule_rect.bottom > foreground_rect.top;
    if !overlaps {
        return Err("synthetic foreground window does not overlap the capsule".into());
    }

    let mut current = unsafe { GetWindow(capsule, GW_HWNDNEXT) }.ok();
    let mut foreground_is_below_capsule = false;
    for _ in 0..512 {
        let Some(current_window) = current else {
            break;
        };
        if current_window == foreground {
            foreground_is_below_capsule = true;
            break;
        }
        current = unsafe { GetWindow(current_window, GW_HWNDNEXT) }.ok();
    }
    if !foreground_is_below_capsule {
        return Err("ordinary foreground window is not below the capsule in z-order".into());
    }
    Ok(())
}

fn main() {
    let legacy = std::env::args().any(|arg| arg == "--legacy");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../.runtime/.cache")
        .join(format!("capsule-native-probe-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(".vibe-owner.json"),
        serde_json::json!({
            "owner":"capsule-native-regression", "sourceProject":"Whisper-input",
            "createdAt":chrono::Utc::now().to_rfc3339(), "ttlDays":1,
            "reason":"isolated synthetic WebView profile; no user data",
            "cleanupCommand":format!("Remove-Item -LiteralPath '{}' -Recurse -Force",root.display())
        })
        .to_string(),
    )
    .unwrap();
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    context.config_mut().identifier = "com.qingyu.capsule-regression".into();
    tauri::Builder::default().setup(move |app| {
        let window = tauri::WebviewWindowBuilder::new(app, "capsule",
            tauri::WebviewUrl::App("index.html?window=capsule".into()))
            .title("Capsule native regression — synthetic")
            .data_directory(root.join("webview"))
            .inner_size(220.0,84.0).position(500.0,500.0)
            .decorations(false).transparent(true).always_on_top(true)
            .skip_taskbar(true).focused(false).visible(false).build()?;
        window.set_ignore_cursor_events(true)?;
        #[cfg(target_os = "windows")]
        let foreground_windows = [
            tauri::WebviewWindowBuilder::new(
                app,
                "foreground-a",
                tauri::WebviewUrl::External("about:blank".parse().unwrap()),
            )
            .title("Capsule foreground control A")
            .data_directory(root.join("webview"))
            .inner_size(480.0, 280.0)
            .position(400.0, 400.0)
            .always_on_top(false)
            .visible(true)
            .focused(true)
            .build()?,
            tauri::WebviewWindowBuilder::new(
                app,
                "foreground-b",
                tauri::WebviewUrl::External("about:blank".parse().unwrap()),
            )
            .title("Capsule foreground control B")
            .data_directory(root.join("webview"))
            .inner_size(480.0, 280.0)
            .position(400.0, 400.0)
            .always_on_top(false)
            .visible(false)
            .focused(false)
            .build()?,
        ];
        let handle=app.handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            for cycle in 0..3 {
                #[cfg(target_os="windows")]
                let foreground = foreground_windows[cycle % foreground_windows.len()].clone();
                #[cfg(target_os="windows")]
                {
                    let focused = foreground.clone();
                    let hidden = foreground_windows[(cycle + 1) % foreground_windows.len()].clone();
                    handle.run_on_main_thread(move || {
                        hidden.hide().unwrap();
                        focused.show().unwrap();
                        focused.set_focus().unwrap();
                    }).unwrap();
                    std::thread::sleep(Duration::from_millis(300));
                }
                let w=window.clone();
                handle.run_on_main_thread(move || {
                    w.set_ignore_cursor_events(false).unwrap();
                    #[cfg(target_os="windows")]
                    demote_from_topmost(&w);
                    if legacy {
                        #[cfg(target_os="windows")]
                        unsafe {
                            use windows::Win32::UI::WindowsAndMessaging::{ShowWindow,SW_SHOWNOACTIVATE};
                            let hwnd=windows::Win32::Foundation::HWND(w.hwnd().unwrap().0);
                            let _=ShowWindow(hwnd,SW_SHOWNOACTIVATE);
                        }
                    } else {
                        openless_lib::capsule_window::set_visible(&w,true).unwrap();
                    }
                }).unwrap();
                std::thread::sleep(Duration::from_millis(300));
                let visible=window.is_visible().unwrap();
                println!("cycle={cycle} visible={visible} legacy={legacy}");
                if !visible { std::process::exit(1); }
                #[cfg(target_os="windows")]
                if let Err(error) = topmost_without_focus(&window, &foreground) {
                    eprintln!("FAIL cycle={cycle}: {error}");
                    std::process::exit(2);
                }
                handle.emit_to("capsule","capsule:state",serde_json::json!({
                    "state":"recording","level":0.6,"elapsedMs":1000,"translation":false
                })).unwrap();
                std::thread::sleep(Duration::from_millis(500));
                if cycle<2 {
                    let w=window.clone();
                    handle.run_on_main_thread(move || {
                        w.set_ignore_cursor_events(true).unwrap();
                        openless_lib::capsule_window::set_visible(&w,false).unwrap();
                    }).unwrap();
                    std::thread::sleep(Duration::from_millis(300));
                    assert!(!window.is_visible().unwrap());
                }
            }
            println!("PASS three hide/show cycles; synthetic capsule available for screenshot for 15 seconds");
            std::thread::sleep(Duration::from_secs(15));
            handle.exit(0);
        });
        Ok(())
    }).run(context).expect("native capsule regression");
}
