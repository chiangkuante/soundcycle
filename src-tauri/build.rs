// App commands are declared here so they need an explicit permission:
// capabilities/main.json grants them to the main window only, and the
// overlay window (capabilities/osd.json) cannot call any of them.
const COMMANDS: &[&str] = &[
    "ui_language",
    "get_hotkey",
    "device_rows",
    "set_device_checked",
    "switch_next",
    "set_hotkey",
    "pause_hotkeys",
    "get_settings",
    "update_settings",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
