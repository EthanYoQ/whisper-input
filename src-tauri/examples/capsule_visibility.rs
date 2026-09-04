//! Isolated native regression: no coordinator, microphone, credentials or user data.
//! --legacy replays the old raw-HWND show path and must fail the visibility check.
use std::time::Duration;
use tauri::{Emitter, Manager};

fn main() {
    let legacy = std::env::args().any(|arg| arg == "--legacy");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../.runtime/.cache")
        .join(format!("capsule-native-probe-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(".vibe-owner.json"), serde_json::json!({
        "owner":"capsule-native-regression", "sourceProject":"Whisper-input",
        "createdAt":chrono::Utc::now().to_rfc3339(), "ttlDays":1,
        "reason":"isolated synthetic WebView profile; no user data",
        "cleanupCommand":format!("Remove-Item -LiteralPath '{}' -Recurse -Force",root.display())
    }).to_string()).unwrap();
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
        let handle=app.handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            for cycle in 0..3 {
                let w=window.clone();
                handle.run_on_main_thread(move || {
                    w.set_ignore_cursor_events(false).unwrap();
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
                if !visible { handle.exit(1); return; }
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
