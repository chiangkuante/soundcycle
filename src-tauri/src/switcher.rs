//! Device cycling and the device list shown in the UI. Pure logic over an
//! `AudioBackend` so it can be tested without real audio hardware.

use serde::Serialize;

use crate::config::{DeviceRef, Flow, SwitchCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceState {
    Active,
    Disabled,
    Unplugged,
    NotPresent,
}

impl DeviceState {
    fn label(self) -> &'static str {
        match self {
            DeviceState::Active => tr!("可用", "available"),
            DeviceState::Disabled => tr!("已停用", "disabled"),
            DeviceState::Unplugged => tr!("未插入", "unplugged"),
            DeviceState::NotPresent => tr!("未連接", "not connected"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub state: DeviceState,
}

pub trait AudioBackend {
    fn devices(&self, flow: Flow) -> Result<Vec<DeviceInfo>, String>;
    fn default_id(&self, flow: Flow) -> Option<String>;
    fn set_default(&self, id: &str, include_communications: bool) -> Result<(), String>;
}

/// Finds the current device for `reference`, one of the command's `refs`.
///
/// An active device with the remembered id wins. Otherwise the name is used
/// as a fallback for endpoints whose id changed (other USB port, Bluetooth
/// re-pair), but only when it is unambiguous: exactly one active device has
/// that name and no other remembered device claims it by id. Two identical
/// headsets are never guessed between. Failing that, the remembered id is
/// shown in whatever state it is in (e.g. unplugged).
pub fn resolve<'a>(reference: &DeviceRef, refs: &[DeviceRef], devices: &'a [DeviceInfo]) -> Option<&'a DeviceInfo> {
    let active = |d: &&DeviceInfo| d.state == DeviceState::Active;

    if let Some(device) = devices.iter().filter(active).find(|d| d.id == reference.id) {
        return Some(device);
    }

    let claimed_by_other = |d: &DeviceInfo| refs.iter().any(|r| r.id == d.id && r != reference);
    let mut by_name = devices
        .iter()
        .filter(active)
        .filter(|d| d.name == reference.name && !claimed_by_other(d));
    if let (Some(device), None) = (by_name.next(), by_name.next()) {
        return Some(device);
    }

    devices.iter().find(|d| d.id == reference.id)
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct SwitchOutcome {
    pub switched_to: Option<String>,
    pub skipped: Vec<String>,
}

/// Switches to the next usable device after the current default, wrapping
/// around. Devices that are unplugged or disabled are skipped but stay in the
/// command, so they are used again as soon as they come back.
pub fn switch_next(
    backend: &impl AudioBackend,
    command: &SwitchCommand,
    include_communications: bool,
) -> Result<SwitchOutcome, String> {
    if command.devices.is_empty() {
        return Err(tr!("請先勾選至少一個裝置。", "Select at least one device first.").into());
    }

    let devices = backend.devices(command.flow)?;
    let current = backend.default_id(command.flow);
    let refs = &command.devices;

    let start = refs
        .iter()
        .position(|r| resolve(r, refs, &devices).map(|d| &d.id) == current.as_ref())
        .map_or(0, |i| i + 1);

    let mut outcome = SwitchOutcome::default();
    for reference in refs.iter().cycle().skip(start).take(refs.len()) {
        let Some(device) = resolve(reference, refs, &devices) else {
            outcome.skipped.push(tr!("「{}」未連接", "\"{}\" is not connected", reference.name));
            continue;
        };

        if Some(&device.id) == current.as_ref() {
            continue;
        }

        if device.state != DeviceState::Active {
            outcome.skipped.push(tr!("「{}」{}", "\"{}\" is {}", device.name, device.state.label()));
            continue;
        }

        match backend.set_default(&device.id, include_communications) {
            Ok(()) => {
                outcome.switched_to = Some(device.name.clone());
                break;
            }
            Err(e) => outcome.skipped.push(tr!("「{}」切換失敗：{}", "\"{}\" could not be switched to: {}", device.name, e)),
        }
    }

    Ok(outcome)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub state: DeviceState,
    pub is_default: bool,
    pub checked: bool,
}

/// The list the UI renders: every enumerated device, plus remembered devices
/// that are currently missing (shown as not connected, still checked).
pub fn device_rows(command: &SwitchCommand, devices: &[DeviceInfo], default_id: Option<&str>) -> Vec<DeviceRow> {
    let resolved: Vec<Option<&DeviceInfo>> = command.devices.iter().map(|r| resolve(r, &command.devices, devices)).collect();

    let mut rows: Vec<DeviceRow> = devices
        .iter()
        .map(|d| DeviceRow {
            id: d.id.clone(),
            name: d.name.clone(),
            state: d.state,
            is_default: Some(d.id.as_str()) == default_id,
            checked: resolved.iter().any(|r| r.is_some_and(|r| r.id == d.id)),
        })
        .collect();

    for (reference, _) in command.devices.iter().zip(&resolved).filter(|(_, r)| r.is_none()) {
        rows.push(DeviceRow {
            id: reference.id.clone(),
            name: reference.name.clone(),
            state: DeviceState::NotPresent,
            is_default: false,
            checked: true,
        });
    }

    rows
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    struct FakeBackend {
        devices: Vec<DeviceInfo>,
        default: RefCell<Option<String>>,
    }

    impl FakeBackend {
        fn new(devices: Vec<DeviceInfo>, default: Option<&str>) -> Self {
            Self {
                devices,
                default: RefCell::new(default.map(Into::into)),
            }
        }
    }

    impl AudioBackend for FakeBackend {
        fn devices(&self, _: Flow) -> Result<Vec<DeviceInfo>, String> {
            Ok(self.devices.clone())
        }

        fn default_id(&self, _: Flow) -> Option<String> {
            self.default.borrow().clone()
        }

        fn set_default(&self, id: &str, _: bool) -> Result<(), String> {
            *self.default.borrow_mut() = Some(id.into());
            Ok(())
        }
    }

    fn info(id: &str, name: &str, state: DeviceState) -> DeviceInfo {
        DeviceInfo {
            id: id.into(),
            name: name.into(),
            state,
        }
    }

    fn reference(id: &str, name: &str) -> DeviceRef {
        DeviceRef {
            id: id.into(),
            name: name.into(),
        }
    }

    fn command(refs: Vec<DeviceRef>) -> SwitchCommand {
        SwitchCommand {
            name: "test".into(),
            flow: Flow::Playback,
            hotkey: None,
            devices: refs,
        }
    }

    #[test]
    fn cycles_through_devices_and_wraps_around() {
        use DeviceState::Active;
        let backend = FakeBackend::new(
            vec![info("a", "A", Active), info("b", "B", Active), info("c", "C", Active)],
            Some("a"),
        );
        let cmd = command(vec![reference("a", "A"), reference("b", "B"), reference("c", "C")]);

        let names: Vec<_> = (0..4)
            .map(|_| switch_next(&backend, &cmd, false).unwrap().switched_to.unwrap())
            .collect();
        assert_eq!(names, ["B", "C", "A", "B"]);
    }

    #[test]
    fn skips_unplugged_device_but_keeps_it() {
        use DeviceState::{Active, Unplugged};
        let backend = FakeBackend::new(
            vec![info("a", "A", Active), info("b", "B", Unplugged), info("c", "C", Active)],
            Some("a"),
        );
        let cmd = command(vec![reference("a", "A"), reference("b", "B"), reference("c", "C")]);

        let outcome = switch_next(&backend, &cmd, false).unwrap();
        assert_eq!(outcome.switched_to.as_deref(), Some("C"));
        assert_eq!(outcome.skipped, ["「B」未插入"]);
        assert_eq!(cmd.devices.len(), 3);
    }

    #[test]
    fn replugged_device_with_new_id_is_found_by_name() {
        use DeviceState::{Active, Unplugged};
        let backend = FakeBackend::new(
            vec![
                info("a", "A", Active),
                info("old-g435", "耳機_G435", Unplugged),
                info("new-g435", "耳機_G435", Active),
            ],
            Some("a"),
        );
        let cmd = command(vec![reference("a", "A"), reference("old-g435", "耳機_G435")]);

        let outcome = switch_next(&backend, &cmd, false).unwrap();
        assert_eq!(outcome.switched_to.as_deref(), Some("耳機_G435"));
        assert_eq!(backend.default_id(Flow::Playback).as_deref(), Some("new-g435"));
    }

    #[test]
    fn does_not_guess_between_identically_named_devices() {
        use DeviceState::Active;
        let devices = vec![info("a", "耳機 (USB Headset)", Active), info("b", "耳機 (USB Headset)", Active)];
        let refs = vec![reference("gone", "耳機 (USB Headset)")];

        assert_eq!(resolve(&refs[0], &refs, &devices), None);
    }

    #[test]
    fn name_fallback_skips_device_claimed_by_another_reference() {
        use DeviceState::Active;
        let devices = vec![info("a", "耳機 (USB Headset)", Active)];
        let refs = vec![reference("a", "耳機 (USB Headset)"), reference("gone", "耳機 (USB Headset)")];

        assert_eq!(resolve(&refs[0], &refs, &devices).map(|d| d.id.as_str()), Some("a"));
        assert_eq!(resolve(&refs[1], &refs, &devices), None);
    }

    #[test]
    fn starts_at_first_device_when_current_default_is_not_in_command() {
        use DeviceState::Active;
        let backend = FakeBackend::new(
            vec![info("x", "X", Active), info("a", "A", Active), info("b", "B", Active)],
            Some("x"),
        );
        let cmd = command(vec![reference("a", "A"), reference("b", "B")]);

        let outcome = switch_next(&backend, &cmd, false).unwrap();
        assert_eq!(outcome.switched_to.as_deref(), Some("A"));
    }

    #[test]
    fn missing_remembered_device_stays_checked_in_rows() {
        use DeviceState::Active;
        let devices = vec![info("a", "A", Active)];
        let cmd = command(vec![reference("a", "A"), reference("gone", "耳機_EarPods")]);

        let rows = device_rows(&cmd, &devices, Some("a"));
        assert_eq!(rows.len(), 2);
        assert!(rows[0].checked && rows[0].is_default);
        assert_eq!(rows[1].name, "耳機_EarPods");
        assert_eq!(rows[1].state, DeviceState::NotPresent);
        assert!(rows[1].checked);
    }
}
