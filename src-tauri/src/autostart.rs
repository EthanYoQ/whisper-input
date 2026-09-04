use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    pub enabled: bool,
    pub stale: bool,
    pub registered_path: Option<String>,
    pub expected_path: String,
}

const APP_NAME: &str = crate::product::PRODUCT_NAME;

fn startup_approved_enabled(binary: bool, bytes: &[u8]) -> Result<bool, String> {
    if !binary || bytes.len() != 12 || bytes[1..4] != [0, 0, 0] {
        return Err("任务管理器启动状态格式未知，保留现有启动项".into());
    }
    match bytes[0] {
        2 if bytes[4..].iter().all(|byte| *byte == 0) => Ok(true),
        3 => Ok(false),
        _ => Err("任务管理器启动状态编码未知，保留现有启动项".into()),
    }
}

pub fn classify_windows_run_value(raw_value: Option<&str>, expected_path: &str) -> AutostartStatus {
    let registered_path = raw_value.and_then(extract_windows_run_exe_path);
    let stale = registered_path
        .as_deref()
        .map(|path| !paths_match(path, expected_path))
        .unwrap_or(false);
    AutostartStatus {
        enabled: raw_value.is_some(),
        stale,
        registered_path,
        expected_path: expected_path.to_string(),
    }
}

fn extract_windows_run_exe_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    let exe_end = lower.find(".exe").map(|index| index + 4)?;
    Some(trimmed[..exe_end].trim().to_string())
}

fn paths_match(left: &str, right: &str) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn windows_run_command(exe_path: &str) -> String {
    format!(r#""{}""#, exe_path.trim().trim_matches('"'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsAutostartRepair {
    Preserve,
    ReplaceWithCurrent,
    DeleteInvalid,
}

fn is_self_contained_windows_exe(path: &str) -> bool {
    let normalized = normalize_path(path);
    normalized.ends_with(".exe") && !normalized.contains("\\target\\debug\\")
}

fn decide_windows_autostart_repair(
    raw_value: Option<&str>,
    current_exe: &str,
    current_is_dev: bool,
) -> WindowsAutostartRepair {
    let Some(raw_value) = raw_value else {
        return WindowsAutostartRepair::Preserve;
    };
    let registered_path = extract_windows_run_exe_path(raw_value);
    let current_is_self_contained = is_self_contained_windows_exe(current_exe);
    if current_is_dev {
        return if registered_path
            .as_deref()
            .is_some_and(|path| paths_match(path, current_exe))
        {
            WindowsAutostartRepair::DeleteInvalid
        } else {
            WindowsAutostartRepair::Preserve
        };
    }
    // An embedded debug build is runnable but must not take over an installed app.
    if !current_is_self_contained {
        return WindowsAutostartRepair::Preserve;
    }

    match registered_path {
        Some(path) if paths_match(&path, current_exe) => WindowsAutostartRepair::Preserve,
        _ => WindowsAutostartRepair::ReplaceWithCurrent,
    }
}

fn current_exe_path() -> Result<String, String> {
    std::env::current_exe()
        .map_err(|e| format!("读取当前程序路径失败: {e}"))
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub fn get_autostart_status() -> Result<AutostartStatus, String> {
    platform_get_autostart_status()
}

#[tauri::command]
pub fn set_autostart_enabled(enabled: bool) -> Result<AutostartStatus, String> {
    platform_set_autostart_enabled(enabled)
}

pub fn repair_stale_autostart_entry() {
    if let Err(err) = platform_repair_stale_autostart_entry() {
        log::warn!("[autostart] repair stale entry failed: {err}");
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use winreg::enums::RegType::REG_BINARY;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
    use winreg::{RegKey, RegValue};

    const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
    const STARTUP_APPROVED_RUN_KEY: &str =
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
    const STARTUP_APPROVED_ENABLED_VALUE: [u8; 12] = [
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    pub fn get_autostart_status() -> Result<AutostartStatus, String> {
        let expected_path = current_exe_path()?;
        let raw_value = read_run_value()?;
        let mut status = classify_windows_run_value(raw_value.as_deref(), &expected_path);
        let usable = status.registered_path.as_deref().is_some_and(|path| {
            is_self_contained_windows_exe(path)
                && std::path::Path::new(path).is_file()
                && !(tauri::is_dev() && paths_match(path, &expected_path))
        });
        status.enabled = raw_value.is_some() && usable;
        status.stale = raw_value.is_some() && !usable;
        if usable {
            status.expected_path = status.registered_path.clone().unwrap_or(expected_path);
        }
        if status.enabled && !task_manager_enabled()? {
            status.enabled = false;
        }
        Ok(status)
    }

    pub fn set_autostart_enabled(enabled: bool) -> Result<AutostartStatus, String> {
        if enabled {
            let expected_path = current_exe_path()?;
            if tauri::is_dev() || !is_self_contained_windows_exe(&expected_path) {
                return Err("开发调试版依赖本地前端服务，不能注册为 Windows 开机启动项".to_string());
            }
            write_run_value(&expected_path)?;
            write_startup_approved_enabled()?;
        } else {
            delete_run_value()?;
        }
        get_autostart_status()
    }

    pub fn repair_stale_autostart_entry() -> Result<(), String> {
        let expected_path = current_exe_path()?;
        let raw_value = read_run_value()?;
        let status = classify_windows_run_value(raw_value.as_deref(), &expected_path);
        // Running an embedded preview/portable build is not a request to replace
        // a working installed app's autostart ownership. Repair missing targets only.
        if status.registered_path.as_deref().is_some_and(|path| {
            !paths_match(path, &expected_path)
                && is_self_contained_windows_exe(path)
                && std::path::Path::new(path).is_file()
        }) {
            return Ok(());
        }
        match decide_windows_autostart_repair(raw_value.as_deref(), &expected_path, tauri::is_dev())
        {
            WindowsAutostartRepair::Preserve => {}
            WindowsAutostartRepair::ReplaceWithCurrent if task_manager_enabled()? => {
                write_run_value(&expected_path)?;
                write_startup_approved_enabled()?;
                log::info!(
                    "[autostart] repaired stale Windows Run entry: {:?} -> {}",
                    status.registered_path,
                    expected_path
                );
            }
            WindowsAutostartRepair::DeleteInvalid => {
                delete_run_value()?;
                log::warn!(
                    "[autostart] removed development-build Run entry that cannot load without Vite: {:?}",
                    status.registered_path
                );
            }
            WindowsAutostartRepair::ReplaceWithCurrent => {}
        }
        Ok(())
    }

    fn read_run_value() -> Result<Option<String>, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(RUN_KEY, KEY_READ)
            .map_err(|e| format!("读取启动注册表失败: {e}"))?;
        match key.get_value::<String, _>(APP_NAME) {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(format!("读取启动项失败: {err}")),
        }
    }

    fn write_run_value(exe_path: &str) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
            .map_err(|e| format!("打开启动注册表失败: {e}"))?
            .set_value(APP_NAME, &windows_run_command(exe_path))
            .map_err(|e| format!("写入启动项失败: {e}"))
    }

    fn delete_run_value() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
            .map_err(|e| format!("打开启动注册表失败: {e}"))?;
        match key.delete_value(APP_NAME) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("删除启动项失败: {err}")),
        }
    }

    fn write_startup_approved_enabled() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(STARTUP_APPROVED_RUN_KEY, KEY_SET_VALUE) {
            key.set_raw_value(
                APP_NAME,
                &RegValue {
                    vtype: REG_BINARY,
                    bytes: STARTUP_APPROVED_ENABLED_VALUE.to_vec(),
                },
            )
            .map_err(|e| format!("写入任务管理器启动状态失败: {e}"))?;
        }
        Ok(())
    }

    fn task_manager_enabled() -> Result<bool, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = match hkcu.open_subkey_with_flags(STARTUP_APPROVED_RUN_KEY, KEY_READ) {
            Ok(key) => key,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(err) => return Err(format!("读取任务管理器启动状态失败: {err}")),
        };
        let raw = match key.get_raw_value(APP_NAME) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(err) => return Err(format!("读取任务管理器启动项失败: {err}")),
        };
        startup_approved_enabled(raw.vtype == REG_BINARY, &raw.bytes)
    }
}

#[cfg(target_os = "windows")]
use platform::{
    get_autostart_status as platform_get_autostart_status,
    repair_stale_autostart_entry as platform_repair_stale_autostart_entry,
    set_autostart_enabled as platform_set_autostart_enabled,
};

#[cfg(not(target_os = "windows"))]
fn platform_get_autostart_status() -> Result<AutostartStatus, String> {
    let expected_path = current_exe_path()?;
    Ok(AutostartStatus {
        enabled: false,
        stale: false,
        registered_path: None,
        expected_path,
    })
}

#[cfg(not(target_os = "windows"))]
fn platform_set_autostart_enabled(_enabled: bool) -> Result<AutostartStatus, String> {
    platform_get_autostart_status()
}

#[cfg(not(target_os = "windows"))]
fn platform_repair_stale_autostart_entry() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_approved_unknown_records_fail_closed() {
        let mut bytes = [0; 12];
        bytes[0] = 2;
        assert_eq!(startup_approved_enabled(true, &bytes), Ok(true));
        assert!(startup_approved_enabled(false, &bytes).is_err());
        for length in 0..12 {
            assert!(startup_approved_enabled(true, &bytes[..length]).is_err());
        }
        bytes[4] = 1;
        assert!(startup_approved_enabled(true, &bytes).is_err());
        bytes[0] = 3;
        assert_eq!(startup_approved_enabled(true, &bytes), Ok(false));
        bytes[0] = 6;
        assert!(startup_approved_enabled(true, &bytes).is_err());
        bytes[0] = 2;
        bytes[1] = 1;
        assert!(startup_approved_enabled(true, &bytes).is_err());
    }

    #[test]
    fn debug_build_cannot_replace_a_self_contained_autostart_target() {
        let installed = r#""C:\Program Files\Whisper Input\whisper-input.exe""#;
        let debug = r"C:\Repo\src-tauri\target\debug\whisper-input.exe";

        assert_eq!(
            decide_windows_autostart_repair(Some(installed), debug, true),
            WindowsAutostartRepair::Preserve
        );
        assert_eq!(
            decide_windows_autostart_repair(Some(&windows_run_command(debug)), debug, true),
            WindowsAutostartRepair::DeleteInvalid
        );
    }

    #[test]
    fn release_build_can_repair_an_older_release_autostart_target() {
        let old = r#""C:\Program Files\Whisper Input\whisper-input.exe""#;
        let current = r"C:\Users\User\AppData\Local\Programs\Whisper Input\whisper-input.exe";

        assert_eq!(
            decide_windows_autostart_repair(Some(old), current, false),
            WindowsAutostartRepair::ReplaceWithCurrent
        );
    }

    #[test]
    fn stale_windows_run_value_is_not_usable_autostart() {
        let current = r"C:\App\Whisper Input.exe";
        let stale = r#""C:\Old\Whisper Input.exe""#;

        let state = classify_windows_run_value(Some(stale), current);

        assert!(state.enabled);
        assert!(state.stale);
        assert_eq!(
            state.registered_path.as_deref(),
            Some(r"C:\Old\Whisper Input.exe")
        );
        assert_eq!(state.expected_path, current);
    }

    #[test]
    fn current_windows_run_value_is_enabled_and_not_stale() {
        let current = r"C:\App\Whisper Input.exe";
        let state = classify_windows_run_value(Some(r#""C:\App\Whisper Input.exe""#), current);

        assert!(state.enabled);
        assert!(!state.stale);
        assert_eq!(state.registered_path.as_deref(), Some(current));
    }

    #[test]
    fn missing_windows_run_value_is_disabled() {
        let current = r"C:\App\Whisper Input.exe";
        let state = classify_windows_run_value(None, current);

        assert!(!state.enabled);
        assert!(!state.stale);
        assert_eq!(state.registered_path, None);
    }

    #[test]
    fn dev_protocol_not_path_determines_autostart_safety() {
        let installed = r#""C:\Apps\Whisper Input\whisper-input.exe""#;
        for dev in [
            r"D:\custom-target\release\whisper-input.exe",
            r"C:\Preview\whisper-input.exe",
        ] {
            assert_eq!(
                decide_windows_autostart_repair(Some(installed), dev, true),
                WindowsAutostartRepair::Preserve
            );
            assert_eq!(
                decide_windows_autostart_repair(Some("invalid unrelated entry"), dev, true),
                WindowsAutostartRepair::Preserve
            );
            assert_eq!(
                decide_windows_autostart_repair(None, dev, true),
                WindowsAutostartRepair::Preserve
            );
            assert_eq!(
                decide_windows_autostart_repair(Some(&windows_run_command(dev)), dev, true),
                WindowsAutostartRepair::DeleteInvalid
            );
        }
        assert_eq!(
            decide_windows_autostart_repair(
                Some(installed),
                r"C:\repo\target\debug\whisper-input.exe",
                false
            ),
            WindowsAutostartRepair::Preserve
        );
    }
}
