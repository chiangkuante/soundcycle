//! Windows Core Audio backend (MMDevice API over COM).

// COM method names mirror the Windows SDK.
#![allow(non_snake_case)]

use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use windows::core::{implement, interface, GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{PROPERTYKEY, RPC_E_CHANGED_MODE};
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eMultimedia, eRender, EDataFlow, ERole, IMMDevice, IMMDeviceEnumerator,
    IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, DEVICE_STATE, DEVICE_STATE_ACTIVE,
    DEVICE_STATE_DISABLED, DEVICE_STATE_UNPLUGGED,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};

use crate::config::Flow;
use crate::switcher::{AudioBackend, DeviceInfo, DeviceState};

/// Undocumented interface used by the Windows sound control panel to change
/// the default endpoint. Only `SetDefaultEndpoint` is called; the earlier
/// methods are declared to keep the vtable layout correct.
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn GetMixFormat(&self, device: PCWSTR, format: *mut *mut core::ffi::c_void) -> HRESULT;
    fn GetDeviceFormat(&self, device: PCWSTR, default: i32, format: *mut *mut core::ffi::c_void) -> HRESULT;
    fn ResetDeviceFormat(&self, device: PCWSTR) -> HRESULT;
    fn SetDeviceFormat(&self, device: PCWSTR, endpoint: *mut core::ffi::c_void, mix: *mut core::ffi::c_void) -> HRESULT;
    fn GetProcessingPeriod(&self, device: PCWSTR, default: i32, default_period: *mut i64, min_period: *mut i64) -> HRESULT;
    fn SetProcessingPeriod(&self, device: PCWSTR, period: *mut i64) -> HRESULT;
    fn GetShareMode(&self, device: PCWSTR, mode: *mut core::ffi::c_void) -> HRESULT;
    fn SetShareMode(&self, device: PCWSTR, mode: *mut core::ffi::c_void) -> HRESULT;
    fn GetPropertyValue(&self, device: PCWSTR, store: i32, key: *const core::ffi::c_void, value: *mut core::ffi::c_void) -> HRESULT;
    fn SetPropertyValue(&self, device: PCWSTR, store: i32, key: *const core::ffi::c_void, value: *mut core::ffi::c_void) -> HRESULT;
    fn SetDefaultEndpoint(&self, device: PCWSTR, role: ERole) -> HRESULT;
    fn SetEndpointVisibility(&self, device: PCWSTR, visible: i32) -> HRESULT;
}

const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

/// Initializes COM for the calling thread for the guard's lifetime. If the
/// thread already uses a different apartment (Tauri's UI thread is STA),
/// COM is usable as-is and nothing is torn down on drop.
struct ComGuard {
    owns: bool,
}

impl ComGuard {
    fn new() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            owns: hr.is_ok() && hr != RPC_E_CHANGED_MODE,
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owns {
            unsafe { CoUninitialize() };
        }
    }
}

fn data_flow(flow: Flow) -> EDataFlow {
    match flow {
        Flow::Playback => eRender,
        Flow::Recording => eCapture,
    }
}

fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn take_pwstr(value: PWSTR) -> String {
    let text = unsafe { value.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(value.0 as _)) };
    text
}

fn read_device(device: &IMMDevice) -> windows::core::Result<DeviceInfo> {
    unsafe {
        let id = take_pwstr(device.GetId()?);
        let state = match device.GetState()? {
            DEVICE_STATE_ACTIVE => DeviceState::Active,
            DEVICE_STATE_DISABLED => DeviceState::Disabled,
            DEVICE_STATE_UNPLUGGED => DeviceState::Unplugged,
            _ => DeviceState::NotPresent,
        };
        let name = device
            .OpenPropertyStore(STGM_READ)?
            .GetValue(&PKEY_Device_FriendlyName)?
            .to_string();
        Ok(DeviceInfo { id, name, state })
    }
}

pub struct WindowsAudio;

impl AudioBackend for WindowsAudio {
    /// Lists active, disabled and unplugged endpoints. Not-present endpoints
    /// are left out: Windows keeps many stale ones and none can be switched to.
    fn devices(&self, flow: Flow) -> Result<Vec<DeviceInfo>, String> {
        let _com = ComGuard::new();
        let mask = DEVICE_STATE(DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_DISABLED.0 | DEVICE_STATE_UNPLUGGED.0);

        let collection = unsafe { enumerator().and_then(|e| e.EnumAudioEndpoints(data_flow(flow), mask)) }
            .map_err(|e| e.message())?;
        let count = unsafe { collection.GetCount() }.map_err(|e| e.message())?;

        Ok((0..count)
            .filter_map(|i| unsafe { collection.Item(i) }.ok())
            .filter_map(|device| read_device(&device).ok())
            .collect())
    }

    fn default_id(&self, flow: Flow) -> Option<String> {
        let _com = ComGuard::new();
        let device = unsafe { enumerator().and_then(|e| e.GetDefaultAudioEndpoint(data_flow(flow), eConsole)) }.ok()?;
        unsafe { device.GetId() }.ok().map(take_pwstr)
    }

    fn set_default(&self, id: &str, include_communications: bool) -> Result<(), String> {
        let _com = ComGuard::new();
        let policy: IPolicyConfig =
            unsafe { CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL) }.map_err(|e| e.message())?;

        let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        let mut roles = vec![eConsole, eMultimedia];
        if include_communications {
            roles.push(eCommunications);
        }

        for role in roles {
            unsafe { policy.SetDefaultEndpoint(PCWSTR(wide.as_ptr()), role) }
                .ok()
                .map_err(|e| e.message())?;
        }

        Ok(())
    }
}

/// Receives Core Audio notifications. Windows calls these on its own threads
/// and they must return quickly, so each one only sends a signal.
#[implement(IMMNotificationClient)]
struct DeviceNotifier {
    changed: Sender<()>,
}

impl IMMNotificationClient_Impl for DeviceNotifier_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        let _ = self.changed.send(());
        Ok(())
    }

    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.changed.send(());
        Ok(())
    }

    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.changed.send(());
        Ok(())
    }

    fn OnDefaultDeviceChanged(&self, _: EDataFlow, _: ERole, _: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.changed.send(());
        Ok(())
    }

    /// Fires constantly for unrelated properties; only renames matter here.
    fn OnPropertyValueChanged(&self, _: &PCWSTR, key: &PROPERTYKEY) -> windows::core::Result<()> {
        if *key == PKEY_Device_FriendlyName {
            let _ = self.changed.send(());
        }
        Ok(())
    }
}

/// Calls `on_change` whenever devices are added, removed, plugged, unplugged,
/// renamed or the default changes. Plugging in one device fires several
/// notifications, so bursts within 250 ms are collapsed into one call.
pub fn watch_devices(on_change: impl Fn() + Send + 'static) {
    thread::spawn(move || {
        let _com = ComGuard::new();
        let (tx, rx) = mpsc::channel();
        let client: IMMNotificationClient = DeviceNotifier { changed: tx }.into();

        let registered = enumerator().and_then(|e| unsafe {
            e.RegisterEndpointNotificationCallback(&client)?;
            Ok(e)
        });
        // The enumerator must stay alive for notifications to keep arriving.
        let Ok(_enumerator) = registered else {
            eprintln!("Could not watch for device changes: {:?}", registered.err());
            return;
        };

        while rx.recv().is_ok() {
            while rx.recv_timeout(Duration::from_millis(250)).is_ok() {}
            on_change();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read-only smoke test against the real machine's devices:
    /// `cargo test -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn lists_real_devices() {
        for flow in [Flow::Playback, Flow::Recording] {
            let default = WindowsAudio.default_id(flow);
            println!("{flow:?} (default: {default:?})");
            for device in WindowsAudio.devices(flow).unwrap() {
                println!("  {:?} {} [{}]", device.state, device.name, device.id);
            }
        }
    }
}
