// This is a tray-first desktop app. Keeping the GUI subsystem in debug builds
// too prevents Windows from showing a console window behind the app whenever a
// locally built executable is launched. Runtime logs still go to openless.log.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    openless_lib::run();
}
