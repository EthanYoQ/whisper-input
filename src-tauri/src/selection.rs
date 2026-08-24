//! 跨平台选区捕获工具：为选区工作流读取当前前台 app 的选中文本。
//!
//! 三级 fallback：
//! 1. **macOS** AX：`AXUIElementCopyAttributeValue(focused, kAXSelectedTextAttribute)`
//!    走辅助功能 API 直读焦点元素的选区，**不**触碰剪贴板。
//! 2. **macOS / Windows** Cmd+C / Ctrl+C：snapshot 用户原剪贴板 → 模拟复制 → 80ms
//!    后读出新内容 → 还原原剪贴板。
//! 3. **Linux**：返回 `None`。
//!
//! 截断策略：超过 4000 字符的选区只保留首 2000 + 尾 2000 + `[…truncated…]` 标记，
//! 避免给 LLM 灌过长 context。
//!
use std::time::Duration;

const SELECTION_MAX_CHARS: usize = 4000;
const SELECTION_TRUNCATE_HEAD: usize = 2000;
const SELECTION_TRUNCATE_TAIL: usize = 2000;
const SELECTION_TRUNCATED_MARKER: &str = "\n[…truncated…]\n";

/// 从前台 app 读到的选区上下文。
/// `text` 已经过截断处理；`source_app` 是前台 app 的人类可读标签（可空）。
#[derive(Debug, Clone)]
pub struct SelectionContext {
    pub text: String,
    pub source_app: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectionReadPermission {
    Allowed,
    SecureInput,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectionPolishReadError {
    TargetUnavailable,
    SecureInput,
    UnknownTarget,
    NoSelection,
}

impl SelectionPolishReadError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::TargetUnavailable => "selectionPolishTargetUnavailable",
            Self::SecureInput => "selectionPolishSecureInput",
            Self::UnknownTarget => "selectionPolishUnknownTarget",
            Self::NoSelection => "selectionPolishNoSelection",
        }
    }
}

pub(crate) trait SelectionAccess {
    fn capture_target(&self) -> Option<SelectionInsertionTarget>;
    fn classify_target(&self, target: &SelectionInsertionTarget) -> SelectionReadPermission;
    fn read_full_selection(&self) -> Option<String>;
    fn source_app(&self) -> Option<String>;
}

pub(crate) struct SystemSelectionAccess;

impl SelectionAccess for SystemSelectionAccess {
    fn capture_target(&self) -> Option<SelectionInsertionTarget> {
        let target = capture_selection_insertion_target();
        selection_insertion_target_is_captured(&target).then_some(target)
    }

    fn classify_target(&self, target: &SelectionInsertionTarget) -> SelectionReadPermission {
        #[cfg(target_os = "windows")]
        {
            return classify_windows_selection_target(target);
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = target;
            SelectionReadPermission::Unknown
        }
    }

    fn read_full_selection(&self) -> Option<String> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            return simulate_copy_and_read();
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        None
    }

    fn source_app(&self) -> Option<String> {
        current_front_app()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SelectionPolishCapture {
    pub target: SelectionInsertionTarget,
    pub full_text: String,
    pub model_text: String,
    pub source_app: Option<String>,
}

pub(crate) struct SelectionPolishWorkflow<'a, A: SelectionAccess> {
    access: &'a A,
}

impl<'a, A: SelectionAccess> SelectionPolishWorkflow<'a, A> {
    pub(crate) fn new(access: &'a A) -> Self {
        Self { access }
    }

    pub(crate) fn capture(&self) -> Result<SelectionPolishCapture, SelectionPolishReadError> {
        let target = self
            .access
            .capture_target()
            .ok_or(SelectionPolishReadError::TargetUnavailable)?;
        match self.access.classify_target(&target) {
            SelectionReadPermission::Allowed => {}
            SelectionReadPermission::SecureInput => {
                return Err(SelectionPolishReadError::SecureInput);
            }
            SelectionReadPermission::Unknown => {
                return Err(SelectionPolishReadError::UnknownTarget);
            }
        }
        let full_text = self
            .access
            .read_full_selection()
            .filter(|text| !text.trim().is_empty())
            .ok_or(SelectionPolishReadError::NoSelection)?;
        let model_text = truncate_selection(full_text.trim());
        Ok(SelectionPolishCapture {
            target,
            full_text,
            model_text,
            source_app: self.access.source_app(),
        })
    }
}

/// 选区润色开始时的 Windows 写入目标。异步润色完成后必须再次匹配此指纹，
/// 才允许覆盖原选区；普通 QA 只读取文本，不使用此目标。
#[derive(Debug, Clone, Default)]
pub(crate) struct SelectionInsertionTarget {
    #[cfg(target_os = "windows")]
    windows: Option<WindowsSelectionTarget>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowsSelectionTarget {
    foreground_window: usize,
    focused_window: usize,
    foreground_process_id: u32,
    foreground_thread_id: u32,
    focused_process_id: u32,
    focused_thread_id: u32,
    focused_control: WindowsFocusedControlFingerprint,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowsFocusedControlFingerprint {
    native_window: usize,
    automation_id: String,
    class_name: String,
    framework_id: String,
    control_type: i32,
    bounding_rectangle: (i32, i32, i32, i32),
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct WindowsFocusedControlObservation {
    process_id: u32,
    is_password: bool,
    fingerprint: WindowsFocusedControlFingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectionInsertionTargetValidation {
    Valid,
    TargetUnavailable,
    TargetChanged,
    SelectionChanged,
}

pub(crate) trait SelectionReplacementAccess {
    fn may_reactivate(&self, _target: &SelectionInsertionTarget) -> bool {
        true
    }
    fn reactivate(&self, target: &SelectionInsertionTarget) -> bool;
    fn classify_target(&self, _target: &SelectionInsertionTarget) -> SelectionReadPermission {
        SelectionReadPermission::Allowed
    }
    fn validate(
        &self,
        target: &SelectionInsertionTarget,
        expected_selection: &str,
    ) -> SelectionInsertionTargetValidation;
    fn insert(&self, replacement: &str) -> crate::types::InsertStatus;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelectionReplacementOutcome {
    Replaced(crate::types::InsertStatus),
    Rejected(&'static str),
}

pub(crate) struct SelectionReplacementWorkflow<'a, A: SelectionReplacementAccess> {
    access: &'a A,
}

impl<'a, A: SelectionReplacementAccess> SelectionReplacementWorkflow<'a, A> {
    pub(crate) fn new(access: &'a A) -> Self {
        Self { access }
    }

    pub(crate) fn replace(
        &self,
        target: &SelectionInsertionTarget,
        expected_selection: &str,
        replacement: &str,
    ) -> SelectionReplacementOutcome {
        if replacement.trim().is_empty() {
            return SelectionReplacementOutcome::Rejected("selectionPolishEmptyResult");
        }
        if !self.access.may_reactivate(target) {
            return SelectionReplacementOutcome::Rejected("selectionPolishTargetChanged");
        }
        if !self.access.reactivate(target) {
            return SelectionReplacementOutcome::Rejected("selectionPolishTargetUnavailable");
        }
        match self.access.classify_target(target) {
            SelectionReadPermission::Allowed => {}
            SelectionReadPermission::SecureInput => {
                return SelectionReplacementOutcome::Rejected("selectionPolishSecureInput");
            }
            SelectionReadPermission::Unknown => {
                return SelectionReplacementOutcome::Rejected("selectionPolishUnknownTarget");
            }
        }
        let validation = self.access.validate(target, expected_selection);
        if let Some(code) = validation.error_code() {
            return SelectionReplacementOutcome::Rejected(code);
        }
        match self.access.insert(replacement) {
            status @ (crate::types::InsertStatus::Inserted
            | crate::types::InsertStatus::PasteSent) => {
                SelectionReplacementOutcome::Replaced(status)
            }
            crate::types::InsertStatus::CopiedFallback | crate::types::InsertStatus::Failed => {
                SelectionReplacementOutcome::Rejected("selectionPolishInsertFailed")
            }
        }
    }
}

/// Only the exact captured window or the exact preview window may return focus to the captured
/// control. A different foreground window means the user deliberately moved on, even when it
/// belongs to the same process.
pub(crate) fn selection_insertion_target_may_reactivate(
    target: &SelectionInsertionTarget,
    preview_window: Option<usize>,
) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        let Some(captured) = target.windows.as_ref() else {
            return false;
        };
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground.0.is_null() {
                return false;
            }
            reactivation_foreground_is_allowed(
                captured.foreground_window,
                preview_window,
                foreground.0 as usize,
            )
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (target, preview_window);
        false
    }
}

fn reactivation_foreground_is_allowed(
    captured_window: usize,
    preview_window: Option<usize>,
    current_window: usize,
) -> bool {
    current_window == captured_window || preview_window == Some(current_window)
}

impl SelectionInsertionTargetValidation {
    pub(crate) const fn error_code(self) -> Option<&'static str> {
        match self {
            Self::Valid => None,
            Self::TargetUnavailable => Some("selectionPolishTargetUnavailable"),
            Self::TargetChanged => Some("selectionPolishTargetChanged"),
            Self::SelectionChanged => Some("selectionPolishSelectionChanged"),
        }
    }
}

pub(crate) fn capture_selection_insertion_target() -> SelectionInsertionTarget {
    #[cfg(target_os = "windows")]
    {
        return SelectionInsertionTarget {
            windows: capture_windows_selection_target(),
        };
    }

    #[cfg(not(target_os = "windows"))]
    SelectionInsertionTarget::default()
}

pub(crate) fn selection_insertion_target_is_captured(target: &SelectionInsertionTarget) -> bool {
    #[cfg(target_os = "windows")]
    {
        target.windows.is_some()
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = target;
        false
    }
}

pub(crate) fn classify_selection_insertion_target(
    target: &SelectionInsertionTarget,
) -> SelectionReadPermission {
    #[cfg(target_os = "windows")]
    {
        classify_windows_selection_target(target)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = target;
        SelectionReadPermission::Unknown
    }
}

/// 复核顺序为目标 → 选区文本 → 目标，夹住临时 Ctrl+C 的竞态窗口。
pub(crate) fn validate_selection_insertion_target(
    target: &SelectionInsertionTarget,
    expected_selection: &str,
) -> SelectionInsertionTargetValidation {
    #[cfg(target_os = "windows")]
    {
        let Some(captured) = target.windows.as_ref() else {
            return SelectionInsertionTargetValidation::TargetUnavailable;
        };
        let Some(before_copy) = capture_windows_selection_target() else {
            return SelectionInsertionTargetValidation::TargetUnavailable;
        };
        if captured != &before_copy {
            return SelectionInsertionTargetValidation::TargetChanged;
        }
        let current_selection = selected_text_for_validation();
        if !selection_text_matches(expected_selection, current_selection.as_deref()) {
            return SelectionInsertionTargetValidation::SelectionChanged;
        }
        let Some(after_copy) = capture_windows_selection_target() else {
            return SelectionInsertionTargetValidation::TargetUnavailable;
        };
        if captured != &after_copy {
            return SelectionInsertionTargetValidation::TargetChanged;
        }
        SelectionInsertionTargetValidation::Valid
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (target, expected_selection);
        SelectionInsertionTargetValidation::TargetUnavailable
    }
}

/// 预览窗口确认后先把焦点交还原前台窗口，再执行严格复核。
pub(crate) fn reactivate_selection_insertion_target(target: &SelectionInsertionTarget) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{BringWindowToTop, SetForegroundWindow};

        let Some(captured) = target.windows.as_ref() else {
            return false;
        };
        unsafe {
            let foreground = HWND(captured.foreground_window as *mut _);
            let _ = BringWindowToTop(foreground);
            let _ = SetForegroundWindow(foreground);
        }
        std::thread::sleep(Duration::from_millis(80));
        true
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = target;
        false
    }
}

/// 尝试捕获当前选区文本。所有 IO 都在调用线程完成（短小、阻塞但 < 200ms）。
///
/// 返回 `None` 表示真的没拿到东西（用户没选 / 平台不支持 / 权限缺失）。
/// 返回 `Some(ctx)` 时 `ctx.text` **保证非空**。
pub fn capture_selection() -> Option<SelectionContext> {
    let source_app = current_front_app();

    // 1. macOS AX 直读
    #[cfg(target_os = "macos")]
    if let Some(text) = macos_ax::read_selected_text() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            log::info!(
                "[selection] AX read OK ({} chars){}",
                trimmed.chars().count(),
                source_app
                    .as_deref()
                    .map(|a| format!(" front_app={a}"))
                    .unwrap_or_default()
            );
            return Some(SelectionContext {
                text: truncate_selection(trimmed),
                source_app,
            });
        }
    }

    // 2. 模拟复制 fallback（macOS / Windows）
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(text) = simulate_copy_and_read() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            log::info!(
                "[selection] simulate-copy fallback OK ({} chars){}",
                trimmed.chars().count(),
                source_app
                    .as_deref()
                    .map(|a| format!(" front_app={a}"))
                    .unwrap_or_default()
            );
            return Some(SelectionContext {
                text: truncate_selection(trimmed),
                source_app,
            });
        }
    }

    // 3. Linux：best-effort 读 PRIMARY selection（wl-paste / xclip / xsel）。
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    if let Some(text) = linux_selection::read_selected_text() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            log::info!(
                "[selection] linux primary selection OK ({} chars){}",
                trimmed.chars().count(),
                source_app
                    .as_deref()
                    .map(|a| format!(" front_app={a}"))
                    .unwrap_or_default()
            );
            return Some(SelectionContext {
                text: truncate_selection(trimmed),
                source_app,
            });
        }
    }

    None
}

/// 长度截断到首 + 尾 + 标记。
fn truncate_selection(text: &str) -> String {
    let total: usize = text.chars().count();
    if total <= SELECTION_MAX_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(SELECTION_TRUNCATE_HEAD).collect();
    let tail_start = total.saturating_sub(SELECTION_TRUNCATE_TAIL);
    let tail: String = text.chars().skip(tail_start).collect();
    format!("{head}{SELECTION_TRUNCATED_MARKER}{tail}")
}

// ─────────────────────────── 模拟复制 fallback (mac/win) ───────────────────────────

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn simulate_copy_and_read() -> Option<String> {
    // a) snapshot 当前剪贴板（用作还原原状态的备份）
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[selection] clipboard init failed: {e}");
            return None;
        }
    };
    let original = match clipboard.get_text() {
        Ok(t) => t,
        Err(e) => {
            // The text API cannot distinguish an empty clipboard from an unreadable non-text
            // payload. Do not overwrite data we cannot restore.
            log::info!("[selection] clipboard text unavailable; preserving existing contents: {e}");
            return None;
        }
    };

    // b) 写一个 sentinel 进剪贴板 — 之后用来检查模拟复制是否真的有覆盖（如果还是
    //    sentinel 说明 Cmd+C 没生效或目标 app 没选区）。
    let sentinel = format!("__openless_qa_sentinel_{}__", uuid_like_token());
    if let Err(e) = clipboard.set_text(sentinel.clone()) {
        log::warn!("[selection] clipboard set_text(sentinel) failed: {e}");
        // 即使设置 sentinel 失败，也尝试发 Cmd+C 看能不能直接拿到东西
    }

    // c) 模拟 Cmd+C / Ctrl+C
    let post_ok = post_copy_shortcut();
    if !post_ok {
        log::warn!("[selection] post_copy_shortcut failed");
        // 不立刻 return：剪贴板可能已经被某些路径污染，按下方还原流程恢复。
    }

    // d) 等剪贴板更新（macOS / Windows 都需要少量时间让目标 app 把数据 put 进去）
    std::thread::sleep(Duration::from_millis(80));

    // e) 读新值
    let captured = clipboard.get_text().ok();

    // f) 还原原剪贴板
    if let Err(e) = clipboard.set_text(original) {
        log::warn!("[selection] clipboard restore failed: {e}");
    }

    let captured = captured?;
    if captured == sentinel || captured.is_empty() {
        return None;
    }
    Some(captured)
}

#[cfg(target_os = "windows")]
fn selected_text_for_validation() -> Option<String> {
    selection_text_for_fingerprint(simulate_copy_and_read()?)
}

fn selection_text_for_fingerprint(text: String) -> Option<String> {
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(any(target_os = "windows", test))]
fn selection_text_matches(expected: &str, actual: Option<&str>) -> bool {
    actual.is_some_and(|actual| actual == expected)
}

#[cfg(target_os = "windows")]
fn capture_windows_selection_target() -> Option<WindowsSelectionTarget> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };

    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return None;
        }
        let mut foreground_process_id = 0;
        let foreground_thread_id =
            GetWindowThreadProcessId(foreground, Some(&mut foreground_process_id));
        if foreground_process_id == 0 || foreground_thread_id == 0 {
            return None;
        }

        let mut gui_info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let focused = if GetGUIThreadInfo(foreground_thread_id, &mut gui_info).is_ok()
            && !gui_info.hwndFocus.0.is_null()
        {
            gui_info.hwndFocus
        } else {
            foreground
        };
        let mut focused_process_id = 0;
        let focused_thread_id = GetWindowThreadProcessId(focused, Some(&mut focused_process_id));
        if focused_process_id == 0 || focused_thread_id == 0 {
            return None;
        }

        let focused_control = observe_windows_focused_control()?;
        if focused_control.process_id != focused_process_id {
            return None;
        }

        Some(WindowsSelectionTarget {
            foreground_window: foreground.0 as usize,
            focused_window: focused.0 as usize,
            foreground_process_id,
            foreground_thread_id,
            focused_process_id,
            focused_thread_id,
            focused_control: focused_control.fingerprint,
        })
    }
}

#[cfg(target_os = "windows")]
fn classify_windows_target_identity(
    target: &WindowsSelectionTarget,
    observed_process_id: Option<u32>,
    observed_is_password: Option<bool>,
) -> SelectionReadPermission {
    if observed_process_id != Some(target.focused_process_id) {
        return SelectionReadPermission::Unknown;
    }
    match observed_is_password {
        Some(true) => SelectionReadPermission::SecureInput,
        Some(false) => SelectionReadPermission::Allowed,
        None => SelectionReadPermission::Unknown,
    }
}

#[cfg(target_os = "windows")]
fn classify_windows_selection_target(target: &SelectionInsertionTarget) -> SelectionReadPermission {
    let Some(captured) = target.windows.as_ref() else {
        return SelectionReadPermission::Unknown;
    };
    let Some(observed) = observe_windows_focused_control() else {
        return SelectionReadPermission::Unknown;
    };
    if observed.fingerprint != captured.focused_control {
        return SelectionReadPermission::Unknown;
    }
    classify_windows_target_identity(
        captured,
        Some(observed.process_id),
        Some(observed.is_password),
    )
}

#[cfg(target_os = "windows")]
fn observe_windows_focused_control() -> Option<WindowsFocusedControlObservation> {
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let uninitialize = init.is_ok();
        if !init.is_ok() && init != RPC_E_CHANGED_MODE {
            return None;
        }

        let observed = (|| {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let focused = automation.GetFocusedElement().ok()?;
            let process_id = u32::try_from(focused.CurrentProcessId().ok()?).ok()?;
            let is_password = focused.CurrentIsPassword().ok()?.as_bool();
            let native_window = focused.CurrentNativeWindowHandle().ok()?.0 as usize;
            let automation_id = focused.CurrentAutomationId().ok()?.to_string();
            let class_name = focused.CurrentClassName().ok()?.to_string();
            let framework_id = focused.CurrentFrameworkId().ok()?.to_string();
            let control_type = focused.CurrentControlType().ok()?.0;
            let rect = focused.CurrentBoundingRectangle().ok()?;
            Some(WindowsFocusedControlObservation {
                process_id,
                is_password,
                fingerprint: WindowsFocusedControlFingerprint {
                    native_window,
                    automation_id,
                    class_name,
                    framework_id,
                    control_type,
                    bounding_rectangle: (rect.left, rect.top, rect.right, rect.bottom),
                },
            })
        })();

        if uninitialize {
            CoUninitialize();
        }

        observed
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn uuid_like_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

#[cfg(target_os = "macos")]
fn post_copy_shortcut() -> bool {
    macos_paste::post_cmd_c().is_ok()
}

#[cfg(target_os = "windows")]
fn post_copy_shortcut() -> bool {
    windows_paste::send_ctrl_c().is_ok()
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
mod linux_selection {
    use std::process::Command;

    const PRIMARY_SELECTION_COMMANDS: &[(&str, &[&str])] = &[
        ("wl-paste", &["--primary", "--no-newline"]),
        ("xclip", &["-o", "-selection", "primary"]),
        ("xsel", &["--primary", "--output"]),
    ];

    pub fn read_selected_text() -> Option<String> {
        for (bin, args) in PRIMARY_SELECTION_COMMANDS {
            if let Some(text) = run_capture(bin, args) {
                return Some(text);
            }
        }
        log::info!(
            "[selection] linux primary selection unavailable (wl-paste/xclip/xsel all failed)"
        );
        None
    }

    fn run_capture(bin: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(bin).args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_string())
    }
}

// ─────────────────────────── macOS AX read ───────────────────────────

#[cfg(target_os = "macos")]
mod macos_ax {
    use std::ffi::{c_void, CStr};
    use std::os::raw::c_char;

    #[repr(C)]
    struct OpaqueAxRef(c_void);
    type AxUiElementRef = *mut OpaqueAxRef;
    type CFStringRef = *const c_void;
    type CFTypeRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type AxError = i32;

    const AX_ERROR_SUCCESS: AxError = 0;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> AxUiElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AxUiElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AxError;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFTypeRef);
        fn CFStringCreateWithCString(
            allocator: CFAllocatorRef,
            cstr: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFStringGetCStringPtr(s: CFStringRef, encoding: u32) -> *const c_char;
        fn CFStringGetCString(
            s: CFStringRef,
            buffer: *mut c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> bool;
        fn CFStringGetLength(s: CFStringRef) -> isize;
        fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    /// 调 system-wide AX 树拿 focused element，再读它的 selected text。
    /// 失败（权限缺失 / 没焦点 / 该控件不支持选区属性）时返回 None。
    pub fn read_selected_text() -> Option<String> {
        unsafe {
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                return None;
            }
            // 注意：这里不能直接用 CFSTR 宏（Rust 没有），改用 CFStringCreateWithCString
            // 临时构造 attribute key。
            let focused_attr =
                cfstring_from_static(b"AXFocusedUIElement\0").unwrap_or(std::ptr::null());
            let selected_attr =
                cfstring_from_static(b"AXSelectedText\0").unwrap_or(std::ptr::null());
            if focused_attr.is_null() || selected_attr.is_null() {
                if !system.is_null() {
                    CFRelease(system as CFTypeRef);
                }
                if !focused_attr.is_null() {
                    CFRelease(focused_attr);
                }
                if !selected_attr.is_null() {
                    CFRelease(selected_attr);
                }
                return None;
            }

            let mut focused: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(system, focused_attr, &mut focused);
            CFRelease(system as CFTypeRef);
            CFRelease(focused_attr);
            if err != AX_ERROR_SUCCESS || focused.is_null() {
                CFRelease(selected_attr);
                return None;
            }

            let mut selected: CFTypeRef = std::ptr::null();
            let err2 = AXUIElementCopyAttributeValue(
                focused as AxUiElementRef,
                selected_attr,
                &mut selected,
            );
            CFRelease(focused);
            CFRelease(selected_attr);
            if err2 != AX_ERROR_SUCCESS || selected.is_null() {
                return None;
            }

            let result = cfstring_to_rust(selected);
            CFRelease(selected);
            result
        }
    }

    unsafe fn cfstring_from_static(bytes_with_nul: &[u8]) -> Option<CFStringRef> {
        let cstr = CStr::from_bytes_with_nul(bytes_with_nul).ok()?;
        let s =
            CFStringCreateWithCString(std::ptr::null(), cstr.as_ptr(), K_CF_STRING_ENCODING_UTF8);
        if s.is_null() {
            None
        } else {
            Some(s)
        }
    }

    unsafe fn cfstring_to_rust(s: CFStringRef) -> Option<String> {
        let direct = CFStringGetCStringPtr(s, K_CF_STRING_ENCODING_UTF8);
        if !direct.is_null() {
            let cstr = CStr::from_ptr(direct);
            return cstr.to_str().ok().map(|s| s.to_string());
        }
        let length = CFStringGetLength(s);
        if length <= 0 {
            return Some(String::new());
        }
        let max_bytes = CFStringGetMaximumSizeForEncoding(length, K_CF_STRING_ENCODING_UTF8) + 1;
        let mut buf: Vec<u8> = vec![0; max_bytes as usize];
        let ok = CFStringGetCString(
            s,
            buf.as_mut_ptr() as *mut c_char,
            max_bytes,
            K_CF_STRING_ENCODING_UTF8,
        );
        if !ok {
            return None;
        }
        let cstr = CStr::from_ptr(buf.as_ptr() as *const c_char);
        cstr.to_str().ok().map(|s| s.to_string())
    }
}

// ─────────────────────────── macOS Cmd+C post ───────────────────────────

#[cfg(target_os = "macos")]
mod macos_paste {
    use std::ffi::c_void;

    #[repr(C)]
    struct OpaqueCGEvent(c_void);
    type CGEventRef = *mut OpaqueCGEvent;

    #[repr(C)]
    struct OpaqueCGEventSource(c_void);
    type CGEventSourceRef = *mut OpaqueCGEventSource;

    type CGEventTapLocation = u32;
    type CGEventSourceStateID = i32;
    type CGKeyCode = u16;
    type CGEventFlags = u64;

    const KCG_HID_EVENT_TAP: CGEventTapLocation = 0;
    const KCG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE: CGEventSourceStateID = 1;
    const KCG_EVENT_FLAG_MASK_COMMAND: CGEventFlags = 0x0010_0000;
    /// kVK_ANSI_C
    const KEY_C: CGKeyCode = 8;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceCreate(state_id: CGEventSourceStateID) -> CGEventSourceRef;
        fn CGEventCreateKeyboardEvent(
            source: CGEventSourceRef,
            virtual_key: CGKeyCode,
            key_down: bool,
        ) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
        fn CGEventPost(tap: CGEventTapLocation, event: CGEventRef);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    pub fn post_cmd_c() -> Result<(), String> {
        unsafe {
            let source = CGEventSourceCreate(KCG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE);
            let down = CGEventCreateKeyboardEvent(source, KEY_C, true);
            let up = CGEventCreateKeyboardEvent(source, KEY_C, false);
            if down.is_null() || up.is_null() {
                if !source.is_null() {
                    CFRelease(source as *const c_void);
                }
                if !down.is_null() {
                    CFRelease(down as *const c_void);
                }
                if !up.is_null() {
                    CFRelease(up as *const c_void);
                }
                return Err("CGEventCreateKeyboardEvent returned null".into());
            }
            CGEventSetFlags(down, KCG_EVENT_FLAG_MASK_COMMAND);
            CGEventSetFlags(up, KCG_EVENT_FLAG_MASK_COMMAND);
            CGEventPost(KCG_HID_EVENT_TAP, down);
            CGEventPost(KCG_HID_EVENT_TAP, up);
            CFRelease(down as *const c_void);
            CFRelease(up as *const c_void);
            if !source.is_null() {
                CFRelease(source as *const c_void);
            }
        }
        Ok(())
    }
}

// ─────────────────────────── Windows Ctrl+C send ───────────────────────────

#[cfg(target_os = "windows")]
mod windows_paste {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_C, VK_CONTROL,
    };

    pub fn send_ctrl_c() -> Result<(), String> {
        let mut inputs = [
            keyboard_event(VK_CONTROL, false),
            keyboard_event(VK_C, false),
            keyboard_event(VK_C, true),
            keyboard_event(VK_CONTROL, true),
        ];

        let sent = unsafe { SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32) };
        if (sent as usize) != inputs.len() {
            return Err(format!("SendInput sent {sent}/{}", inputs.len()));
        }
        Ok(())
    }

    fn keyboard_event(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
}

// ─────────────────────────── front-app label ───────────────────────────

#[cfg(target_os = "macos")]
fn current_front_app() -> Option<String> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    unsafe {
        let cls = AnyClass::get("NSWorkspace")?;
        let workspace: *mut AnyObject = msg_send![cls, sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let name_obj: *mut AnyObject = msg_send![app, localizedName];
        let name = ns_string_to_rust(name_obj);
        let bundle_obj: *mut AnyObject = msg_send![app, bundleIdentifier];
        let bundle = ns_string_to_rust(bundle_obj);
        match (name, bundle) {
            (Some(n), Some(b)) => Some(format!("{n} ({b})")),
            (Some(n), None) => Some(n),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn ns_string_to_rust(ns_string: *mut objc2::runtime::AnyObject) -> Option<String> {
    use objc2::msg_send;
    if ns_string.is_null() {
        return None;
    }
    let utf8: *const std::os::raw::c_char = unsafe { msg_send![ns_string, UTF8String] };
    if utf8.is_null() {
        return None;
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(utf8) };
    let s = cstr.to_string_lossy().into_owned();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(target_os = "windows")]
fn current_front_app() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied <= 0 {
            return None;
        }
        let title = String::from_utf16_lossy(&buf[..copied as usize]);
        if title.is_empty() {
            None
        } else {
            Some(title)
        }
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn current_front_app() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct FakeReplacementAccess {
        reactivation_allowed: bool,
        reactivation_count: Cell<usize>,
        permission: SelectionReadPermission,
        validation: SelectionInsertionTargetValidation,
        insert_count: Cell<usize>,
    }

    impl SelectionReplacementAccess for FakeReplacementAccess {
        fn may_reactivate(&self, _target: &SelectionInsertionTarget) -> bool {
            self.reactivation_allowed
        }

        fn reactivate(&self, _target: &SelectionInsertionTarget) -> bool {
            self.reactivation_count
                .set(self.reactivation_count.get() + 1);
            true
        }

        fn classify_target(&self, _target: &SelectionInsertionTarget) -> SelectionReadPermission {
            self.permission
        }

        fn validate(
            &self,
            _target: &SelectionInsertionTarget,
            _expected_selection: &str,
        ) -> SelectionInsertionTargetValidation {
            self.validation
        }

        fn insert(&self, _replacement: &str) -> crate::types::InsertStatus {
            self.insert_count.set(self.insert_count.get() + 1);
            crate::types::InsertStatus::PasteSent
        }
    }

    struct FakeSelectionAccess {
        permission: SelectionReadPermission,
        read_count: Cell<usize>,
        text: Option<String>,
    }

    impl SelectionAccess for FakeSelectionAccess {
        fn capture_target(&self) -> Option<SelectionInsertionTarget> {
            Some(SelectionInsertionTarget::default())
        }

        fn classify_target(&self, _target: &SelectionInsertionTarget) -> SelectionReadPermission {
            self.permission
        }

        fn read_full_selection(&self) -> Option<String> {
            self.read_count.set(self.read_count.get() + 1);
            self.text.clone()
        }

        fn source_app(&self) -> Option<String> {
            Some("Editor".into())
        }
    }

    #[test]
    fn selection_polish_workflow_blocks_unknown_targets_before_reading() {
        let access = FakeSelectionAccess {
            permission: SelectionReadPermission::Unknown,
            read_count: Cell::new(0),
            text: Some("secret".into()),
        };

        let result = SelectionPolishWorkflow::new(&access).capture();

        assert_eq!(result.unwrap_err(), SelectionPolishReadError::UnknownTarget);
        assert_eq!(access.read_count.get(), 0);
    }

    #[test]
    fn selection_polish_workflow_blocks_secure_inputs_before_reading() {
        let access = FakeSelectionAccess {
            permission: SelectionReadPermission::SecureInput,
            read_count: Cell::new(0),
            text: Some("secret".into()),
        };

        let result = SelectionPolishWorkflow::new(&access).capture();

        assert_eq!(result.unwrap_err(), SelectionPolishReadError::SecureInput);
        assert_eq!(access.read_count.get(), 0);
    }

    #[test]
    fn selection_polish_capture_keeps_the_full_fingerprint_but_limits_model_input() {
        let full_text = format!("  {}  ", "a".repeat(SELECTION_MAX_CHARS + 10));
        let access = FakeSelectionAccess {
            permission: SelectionReadPermission::Allowed,
            read_count: Cell::new(0),
            text: Some(full_text.clone()),
        };

        let capture = SelectionPolishWorkflow::new(&access).capture().unwrap();

        assert_eq!(capture.full_text, full_text);
        assert!(capture.model_text.contains(SELECTION_TRUNCATED_MARKER));
        assert_eq!(capture.source_app.as_deref(), Some("Editor"));
    }

    #[test]
    fn selection_replacement_refuses_changed_targets_without_inserting() {
        let access = FakeReplacementAccess {
            reactivation_allowed: true,
            reactivation_count: Cell::new(0),
            permission: SelectionReadPermission::Allowed,
            validation: SelectionInsertionTargetValidation::TargetChanged,
            insert_count: Cell::new(0),
        };

        let outcome = SelectionReplacementWorkflow::new(&access).replace(
            &SelectionInsertionTarget::default(),
            "before",
            "after",
        );

        assert_eq!(
            outcome,
            SelectionReplacementOutcome::Rejected("selectionPolishTargetChanged")
        );
        assert_eq!(access.insert_count.get(), 0);
    }

    #[test]
    fn selection_replacement_refuses_blank_results_without_inserting() {
        let access = FakeReplacementAccess {
            reactivation_allowed: true,
            reactivation_count: Cell::new(0),
            permission: SelectionReadPermission::Allowed,
            validation: SelectionInsertionTargetValidation::Valid,
            insert_count: Cell::new(0),
        };

        let outcome = SelectionReplacementWorkflow::new(&access).replace(
            &SelectionInsertionTarget::default(),
            "before",
            "   ",
        );

        assert_eq!(
            outcome,
            SelectionReplacementOutcome::Rejected("selectionPolishEmptyResult")
        );
        assert_eq!(access.insert_count.get(), 0);
    }

    #[test]
    fn selection_replacement_rechecks_secure_input_before_reading_selection() {
        let access = FakeReplacementAccess {
            reactivation_allowed: true,
            reactivation_count: Cell::new(0),
            permission: SelectionReadPermission::SecureInput,
            validation: SelectionInsertionTargetValidation::Valid,
            insert_count: Cell::new(0),
        };

        let outcome = SelectionReplacementWorkflow::new(&access).replace(
            &SelectionInsertionTarget::default(),
            "before",
            "after",
        );

        assert_eq!(
            outcome,
            SelectionReplacementOutcome::Rejected("selectionPolishSecureInput")
        );
        assert_eq!(access.insert_count.get(), 0);
    }

    #[test]
    fn selection_replacement_refuses_third_party_focus_before_reactivation() {
        let access = FakeReplacementAccess {
            reactivation_allowed: false,
            reactivation_count: Cell::new(0),
            permission: SelectionReadPermission::Allowed,
            validation: SelectionInsertionTargetValidation::Valid,
            insert_count: Cell::new(0),
        };

        let outcome = SelectionReplacementWorkflow::new(&access).replace(
            &SelectionInsertionTarget::default(),
            "before",
            "after",
        );

        assert_eq!(
            outcome,
            SelectionReplacementOutcome::Rejected("selectionPolishTargetChanged")
        );
        assert_eq!(access.reactivation_count.get(), 0);
        assert_eq!(access.insert_count.get(), 0);
    }

    #[test]
    fn focus_reactivation_allows_only_the_exact_target_or_preview_window() {
        assert!(reactivation_foreground_is_allowed(11, Some(22), 11));
        assert!(reactivation_foreground_is_allowed(11, Some(22), 22));
        assert!(!reactivation_foreground_is_allowed(11, Some(22), 33));
        assert!(!reactivation_foreground_is_allowed(11, None, 22));
    }

    #[test]
    fn selection_polish_read_errors_expose_stable_user_codes() {
        assert_eq!(
            SelectionPolishReadError::TargetUnavailable.code(),
            "selectionPolishTargetUnavailable"
        );
        assert_eq!(
            SelectionPolishReadError::SecureInput.code(),
            "selectionPolishSecureInput"
        );
        assert_eq!(
            SelectionPolishReadError::UnknownTarget.code(),
            "selectionPolishUnknownTarget"
        );
        assert_eq!(
            SelectionPolishReadError::NoSelection.code(),
            "selectionPolishNoSelection"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_target_permission_is_fail_closed_and_process_bound() {
        let target = WindowsSelectionTarget {
            foreground_window: 1,
            focused_window: 2,
            foreground_process_id: 41,
            foreground_thread_id: 42,
            focused_process_id: 41,
            focused_thread_id: 43,
            focused_control: WindowsFocusedControlFingerprint {
                native_window: 2,
                automation_id: "editor".into(),
                class_name: "Edit".into(),
                framework_id: "Win32".into(),
                control_type: 50004,
                bounding_rectangle: (0, 0, 100, 20),
            },
        };

        assert_eq!(
            classify_windows_target_identity(&target, Some(41), Some(false)),
            SelectionReadPermission::Allowed
        );
        assert_eq!(
            classify_windows_target_identity(&target, Some(41), Some(true)),
            SelectionReadPermission::SecureInput
        );
        assert_eq!(
            classify_windows_target_identity(&target, Some(99), Some(false)),
            SelectionReadPermission::Unknown
        );
        assert_eq!(
            classify_windows_target_identity(&target, None, None),
            SelectionReadPermission::Unknown
        );
    }

    #[test]
    fn truncate_short_passes_through() {
        let text = "hello world";
        assert_eq!(truncate_selection(text), text);
    }

    #[test]
    fn truncate_long_keeps_head_and_tail() {
        let head: String = "a".repeat(SELECTION_TRUNCATE_HEAD);
        let middle: String = "b".repeat(2_000);
        let tail: String = "c".repeat(SELECTION_TRUNCATE_TAIL);
        let combined = format!("{head}{middle}{tail}");
        let out = truncate_selection(&combined);
        assert!(out.contains("[…truncated…]"));
        assert!(out.starts_with(&"a".repeat(50)));
        assert!(out.ends_with(&"c".repeat(50)));
        // 中段 b 应被裁掉
        assert!(!out.contains(&"b".repeat(20)));
    }

    #[test]
    fn selection_validation_requires_an_exact_nonempty_fingerprint() {
        assert!(selection_text_matches(
            "selected text",
            Some("selected text")
        ));
        assert!(!selection_text_matches(
            "selected text",
            Some("selected text ")
        ));
        assert!(!selection_text_matches("selected text", None));
    }

    #[test]
    fn selection_fingerprint_preserves_whitespace_and_full_length() {
        let raw = format!("  {}  ", "x".repeat(SELECTION_MAX_CHARS + 50));

        assert_eq!(selection_text_for_fingerprint(raw.clone()), Some(raw));
        assert_eq!(selection_text_for_fingerprint("   ".into()), None);
    }
}
