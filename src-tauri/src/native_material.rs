//! Native material result, not a frontend guess based on a missing attribute.
use serde::Serialize;
use tauri::{Manager, Runtime};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    Acrylic,
    Mica,
    Vibrancy,
    Fallback,
}

pub struct MaterialState(tokio::sync::watch::Sender<Option<Material>>);

impl Default for MaterialState {
    fn default() -> Self {
        Self(tokio::sync::watch::channel(None).0)
    }
}

impl MaterialState {
    pub fn complete(&self, material: Material) {
        self.0.send_replace(Some(material));
    }
}

#[tauri::command]
pub async fn get_native_material_status<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, MaterialState>,
) -> Result<Material, String> {
    // Floating text windows deliberately do not request a native blur layer.
    if window.label() != "main" {
        return Ok(Material::Fallback);
    }
    let mut ready = state.0.subscribe();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Some(material) = *ready.borrow_and_update() {
                return Ok(material);
            }
            ready
                .changed()
                .await
                .map_err(|_| "material initialization unavailable".to_owned())?;
        }
    })
    .await
    .map_err(|_| "material initialization timed out".to_owned())?
}

pub fn complete<R: Runtime>(window: &tauri::WebviewWindow<R>, material: Material) {
    log::info!("[main] native material: {material:?}");
    window.state::<MaterialState>().complete(material);
}

fn choose_material(
    enabled: bool,
    acrylic: impl FnOnce() -> bool,
    mica: impl FnOnce() -> bool,
) -> Material {
    if !enabled {
        return Material::Fallback;
    }
    if acrylic() {
        Material::Acrylic
    } else if mica() {
        Material::Mica
    } else {
        Material::Fallback
    }
}

#[cfg(target_os = "windows")]
pub fn apply(window: &tauri::WebviewWindow) -> Material {
    windows_adapter::apply(window)
}

#[cfg(target_os = "windows")]
mod windows_adapter {
    use super::*;
    use std::ffi::c_void;
    use windows::core::{s, w};
    use windows::Win32::Foundation::{BOOL, HWND};
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    #[repr(C)]
    struct Version {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(version: *mut Version) -> i32;
    }

    fn build_number() -> Option<u32> {
        let mut version = Version {
            size: std::mem::size_of::<Version>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            service_pack: [0; 128],
        };
        (unsafe { RtlGetVersion(&mut version) } >= 0).then_some(version.build)
    }

    fn transparency_enabled() -> bool {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let disabled = matches!(
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
                .and_then(|key| key.get_value::<u32, _>("EnableTransparency")),
            Ok(0)
        );
        let mut contrast = HIGHCONTRASTW {
            cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        let high_contrast = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok()
            && (contrast.dwFlags & HCF_HIGHCONTRASTON).0 != 0;
        !disabled && !high_contrast
    }

    fn set_dwm(hwnd: HWND, attribute: i32, value: i32) -> bool {
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(attribute),
                (&value as *const i32).cast(),
                std::mem::size_of::<i32>() as u32,
            )
        };
        if let Err(error) = &result {
            log::warn!("[material] DWM attribute {attribute} failed: {error}");
        }
        result.is_ok()
    }

    // Same parameters as pinned window-vibrancy 0.7.1 (None tint). Unlike that
    // wrapper, every HRESULT/BOOL and missing dynamic symbol is checked here.
    fn acrylic(hwnd: HWND, build: u32) -> bool {
        if build >= 22523 {
            return set_dwm(hwnd, 38, 3);
        }
        if build < 17763 {
            return false;
        }
        #[repr(C)]
        struct Accent {
            state: i32,
            flags: u32,
            color: u32,
            animation: u32,
        }
        #[repr(C)]
        struct Data {
            attribute: i32,
            data: *mut c_void,
            size: usize,
        }
        unsafe {
            let Ok(module) = GetModuleHandleW(w!("user32.dll")) else {
                return false;
            };
            let Some(address) = GetProcAddress(module, s!("SetWindowCompositionAttribute")) else {
                return false;
            };
            let set: unsafe extern "system" fn(HWND, *mut Data) -> BOOL =
                std::mem::transmute(address);
            let mut accent = Accent {
                state: 4,
                flags: 0,
                color: 1 << 24,
                animation: 0,
            };
            let mut data = Data {
                attribute: 19,
                data: (&mut accent as *mut Accent).cast(),
                size: std::mem::size_of::<Accent>(),
            };
            set(hwnd, &mut data).as_bool()
        }
    }

    pub fn apply(window: &tauri::WebviewWindow) -> Material {
        let Ok(hwnd) = window.hwnd() else {
            return Material::Fallback;
        };
        let hwnd = HWND(hwnd.0);
        let Some(build) = build_number() else {
            return Material::Fallback;
        };
        choose_material(
            transparency_enabled(),
            || acrylic(hwnd, build),
            || {
                if build >= 22523 {
                    set_dwm(hwnd, 38, 2)
                } else if build >= 22000 {
                    set_dwm(hwnd, 1029, 1)
                } else {
                    false
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checks_failure_order_and_explicit_os_disable() {
        assert_eq!(
            choose_material(false, || panic!("disabled"), || panic!("disabled")),
            Material::Fallback
        );
        assert_eq!(
            choose_material(true, || true, || panic!("do not replace acrylic")),
            Material::Acrylic
        );
        assert_eq!(choose_material(true, || false, || true), Material::Mica);
        assert_eq!(
            choose_material(true, || false, || false),
            Material::Fallback
        );
    }
    #[tokio::test]
    async fn material_result_survives_publishing_before_subscription() {
        let state = MaterialState::default();
        state.complete(Material::Acrylic);
        assert_eq!(*state.0.subscribe().borrow(), Some(Material::Acrylic));
    }
}
