//! Run at sign-in. Packaged (MSIX / Microsoft Store) builds must use a
//! `StartupTask` declared in Package.appxmanifest: registry writes from a
//! package are redirected to private storage, so a Run key would never fire.
//! Unpackaged builds use the Run key via tauri-plugin-autostart.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use windows::core::HSTRING;
use windows::ApplicationModel::Activation::ActivationKind;
use windows::ApplicationModel::{AppInstance, Package, StartupTask, StartupTaskState};

/// Must match `TaskId` of the `windows.startupTask` extension in the manifest.
const TASK_ID: &str = "AudioSwitcherStartup";

/// Passed by the Run key entry so a launch at sign-in stays in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

pub fn is_packaged() -> bool {
    Package::Current().is_ok()
}

/// True when this process was started by sign-in rather than by the user.
pub fn launched_at_sign_in() -> bool {
    if std::env::args().any(|arg| arg == MINIMIZED_ARG) {
        return true;
    }
    is_packaged()
        && AppInstance::GetActivatedEventArgs()
            .and_then(|args| args.Kind())
            .is_ok_and(|kind| kind == ActivationKind::StartupTask)
}

fn startup_task() -> windows::core::Result<StartupTask> {
    StartupTask::GetAsync(&HSTRING::from(TASK_ID))?.join()
}

pub fn is_enabled(app: &AppHandle) -> bool {
    if is_packaged() {
        return startup_task()
            .and_then(|task| task.State())
            .is_ok_and(|state| state == StartupTaskState::Enabled || state == StartupTaskState::EnabledByPolicy);
    }
    app.autolaunch().is_enabled().unwrap_or(false)
}

pub fn set_enabled(app: &AppHandle, enabled: bool) -> Result<(), String> {
    if is_packaged() {
        let task = startup_task().map_err(|e| e.message())?;
        if !enabled {
            return task.Disable().map_err(|e| e.message());
        }

        // Windows never re-enables a task the user turned off in Task Manager;
        // only the user can, so explain where.
        return match task.RequestEnableAsync().and_then(|op| op.join()).map_err(|e| e.message())? {
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
            StartupTaskState::DisabledByUser => {
                Err(tr!("開機啟動已在工作管理員中被停用，請到「工作管理員 → 啟動應用程式」重新啟用", "Startup was turned off in Task Manager; turn it back on under Task Manager → Startup apps").into())
            }
            StartupTaskState::DisabledByPolicy => Err(tr!("開機啟動已被系統管理原則停用", "Startup is turned off by a system policy").into()),
            _ => Err(tr!("無法開啟開機自動啟動", "Could not turn on startup").into()),
        };
    }

    // A dev build opens a console window at sign-in (closing it kills the
    // app) and its UI needs the Vite dev server, so it can't autostart.
    if cfg!(debug_assertions) && enabled {
        return Err(tr!("開發版不支援開機自動啟動，請在安裝版中開啟", "Dev builds can't start with Windows; turn this on in the installed app").into());
    }

    let autolaunch = app.autolaunch();
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|e| tr!("無法變更開機自動啟動：{}", "Could not change startup: {}", e))
}

/// The Run key stores an exe path; re-registering points it at this exe,
/// e.g. after an update or an entry left by a dev build.
pub fn refresh_run_key(app: &AppHandle) {
    if cfg!(not(debug_assertions)) && !is_packaged() && app.autolaunch().is_enabled().unwrap_or(false) {
        let _ = app.autolaunch().enable();
    }
}
