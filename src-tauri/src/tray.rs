//! System tray icon. Its menu lists the active devices so any of them can be
//! made the default directly, and is rebuilt whenever devices change.

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::audio::WindowsAudio;
use crate::config::Flow;
use crate::switcher::{AudioBackend, DeviceState};
use crate::AppState;

const TRAY_ID: &str = "main";
const DEVICE_PREFIX: &str = "device:";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("SoundCycle")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn device_items(app: &AppHandle, flow: Flow) -> tauri::Result<Vec<CheckMenuItem<Wry>>> {
    let backend = WindowsAudio;
    let default_id = backend.default_id(flow);
    let devices = backend.devices(flow).unwrap_or_default();

    devices
        .iter()
        .filter(|d| d.state == DeviceState::Active)
        .map(|d| {
            let id = format!("{DEVICE_PREFIX}{}", d.id);
            let is_default = Some(&d.id) == default_id.as_ref();
            CheckMenuItem::with_id(app, id, &d.name, true, is_default, None::<&str>)
        })
        .collect()
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let playback = device_items(app, Flow::Playback)?;
    let recording = device_items(app, Flow::Recording)?;
    let recording_refs: Vec<&dyn IsMenuItem<Wry>> = recording.iter().map(|i| i as _).collect();

    let menu = Menu::new(app)?;
    for item in &playback {
        menu.append(item)?;
    }
    if !playback.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&Submenu::with_items(app, tr!("錄音裝置", "Recording devices"), !recording.is_empty(), &recording_refs)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "show", tr!("開啟設定", "Open settings"), true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "quit", tr!("結束", "Exit"), true, None::<&str>)?)?;
    Ok(menu)
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => show_main_window(app),
        "quit" => app.exit(0),
        id => {
            if let Some(device_id) = id.strip_prefix(DEVICE_PREFIX) {
                let include_communications = app.state::<AppState>().config.lock().unwrap().switch_communication_device;
                let _ = WindowsAudio.set_default(device_id, include_communications);
                // Clicking toggles the check mark locally; rebuild from the
                // real state in case the switch failed.
                refresh(app);
            }
        }
    }
}
