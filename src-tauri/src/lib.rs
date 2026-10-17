// First, so the `tr!` macro is in scope for every module below.
#[macro_use]
mod i18n;

mod audio;
mod config;
mod hotkeys;
mod osd;
mod startup;
mod switcher;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use audio::WindowsAudio;
use config::{Config, ConfigStore, DeviceRef, Flow};
use switcher::{AudioBackend, DeviceRow, SwitchOutcome};

struct AppState {
    config: Mutex<Config>,
    store: ConfigStore,
    /// True while the UI's hotkey field has focus, so pressing an existing
    /// hotkey reaches the field instead of switching devices.
    hotkeys_paused: AtomicBool,
}

#[tauri::command]
fn ui_language() -> &'static str {
    i18n::code()
}

#[tauri::command]
fn get_hotkey(state: State<AppState>, flow: Flow) -> Option<String> {
    state.config.lock().unwrap().flow(flow).hotkey.clone()
}

#[tauri::command]
fn device_rows(state: State<AppState>, flow: Flow) -> Result<Vec<DeviceRow>, String> {
    let config = state.config.lock().unwrap();
    let backend = WindowsAudio;
    let devices = backend.devices(flow)?;
    let default_id = backend.default_id(flow);
    Ok(switcher::device_rows(config.flow(flow), &devices, default_id.as_deref()))
}

#[tauri::command]
fn set_device_checked(
    state: State<AppState>,
    flow: Flow,
    id: String,
    name: String,
    checked: bool,
) -> Result<(), String> {
    let present_ids: Vec<String> = WindowsAudio.devices(flow)?.into_iter().map(|d| d.id).collect();
    let mut config = state.config.lock().unwrap();
    config.flow_mut(flow).set_checked(DeviceRef { id, name }, checked, &present_ids);
    state.store.save(&config).map_err(|e| e.to_string())
}

#[tauri::command]
fn switch_next(state: State<AppState>, flow: Flow) -> Result<SwitchOutcome, String> {
    let config = state.config.lock().unwrap();
    switcher::switch_next(&WindowsAudio, config.flow(flow), config.switch_communication_device)
}

/// Sets or clears a flow's hotkey. The hotkey is test-registered first, so
/// one already taken by another program is rejected with a clear message
/// instead of silently never firing. Playback and recording may share one;
/// pressing it switches both.
#[tauri::command]
fn set_hotkey(app: AppHandle, state: State<AppState>, flow: Flow, hotkey: Option<String>) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();

    if let Some(hotkey) = &hotkey {
        let shortcut = hotkeys::parse(hotkey)?;
        let shortcuts = app.global_shortcut();
        if !shortcuts.is_registered(shortcut) {
            shortcuts
                .register(shortcut)
                .map_err(|_| tr!("「{}」無法註冊，可能已被其他程式佔用", "\"{}\" could not be registered; another program may be using it", hotkey))?;
            let _ = shortcuts.unregister(shortcut);
        }
    }

    config.flow_mut(flow).hotkey = hotkey;
    state.store.save(&config).map_err(|e| e.to_string())?;

    if !state.hotkeys_paused.load(Ordering::SeqCst) {
        hotkeys::register_all(&app, &config);
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Settings {
    switch_communication_device: bool,
    run_in_background: bool,
    /// Lives in the Windows Run registry key, not in the config file.
    run_at_startup: bool,
}

#[tauri::command]
fn get_settings(app: AppHandle, state: State<AppState>) -> Settings {
    let config = state.config.lock().unwrap();
    Settings {
        switch_communication_device: config.switch_communication_device,
        run_in_background: config.run_in_background,
        run_at_startup: startup::is_enabled(&app),
    }
}

#[tauri::command]
fn update_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<Settings, String> {
    {
        let mut config = state.config.lock().unwrap();
        config.switch_communication_device = settings.switch_communication_device;
        config.run_in_background = settings.run_in_background;
        state.store.save(&config).map_err(|e| e.to_string())?;
    }

    if startup::is_enabled(&app) != settings.run_at_startup {
        startup::set_enabled(&app, settings.run_at_startup)?;
    }

    Ok(get_settings(app.clone(), state))
}

#[tauri::command]
fn pause_hotkeys(app: AppHandle, state: State<AppState>, paused: bool) -> Result<(), String> {
    state.hotkeys_paused.store(paused, Ordering::SeqCst);

    if paused {
        hotkeys::unregister_all(&app);
        return Ok(());
    }

    let errors = hotkeys::register_all(&app, &state.config.lock().unwrap());
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just brings the running one forward.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| tray::show_main_window(app)))
        .plugin(tauri_plugin_autostart::Builder::new().args([startup::MINIMIZED_ARG]).build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        hotkeys::on_pressed(app, shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let store = ConfigStore::new(&app.path().app_config_dir()?);
            let config = store.load();
            let handle = app.handle().clone();

            osd::create(&handle)?;

            startup::refresh_run_key(&handle);

            // Shown after a delay: the overlay page must finish loading before
            // it can receive the message.
            let errors = hotkeys::register_all(&handle, &config);
            if !errors.is_empty() {
                let osd_handle = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    osd::show(&osd_handle, errors.into_iter().map(osd::Line::error).collect());
                });
            }

            app.manage(AppState {
                config: Mutex::new(config),
                store,
                hotkeys_paused: AtomicBool::new(false),
            });

            // Created here rather than in tauri.conf.json: the page's first
            // commands need AppState, and a config window can load (instantly,
            // from embedded assets) before setup has registered it. Starts
            // hidden so a launch at sign-in never flashes it on screen.
            WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("SoundCycle")
                .inner_size(400.0, 640.0)
                .min_inner_size(340.0, 400.0)
                .visible(false)
                .build()?;

            tray::create(&handle)?;

            if !startup::launched_at_sign_in() {
                tray::show_main_window(&handle);
            }

            audio::watch_devices(move || {
                let _ = handle.emit("devices-changed", ());
                let tray_handle = handle.clone();
                let _ = handle.run_on_main_thread(move || tray::refresh(&tray_handle));
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let run_in_background = window.state::<AppState>().config.lock().unwrap().run_in_background;
                if run_in_background {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            ui_language,
            get_hotkey,
            device_rows,
            set_device_checked,
            switch_next,
            set_hotkey,
            pause_hotkeys,
            get_settings,
            update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
