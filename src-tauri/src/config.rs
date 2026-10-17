//! Persisted settings. This file is the single source of truth for which
//! devices belong to each flow: it only changes on explicit user actions,
//! never because a device was unplugged or re-plugged.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flow {
    Playback,
    Recording,
}

/// A device as remembered in the config. The endpoint id can change when a
/// device is plugged into another USB port or re-paired over Bluetooth, so the
/// name is kept as a fallback key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRef {
    pub id: String,
    pub name: String,
}

/// Each flow has exactly one command, stored under these names. (Earlier
/// versions allowed several named commands per flow; see `Config::normalize`.)
pub const PLAYBACK_KEY: &str = "@playback";
pub const RECORDING_KEY: &str = "@recording";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchCommand {
    pub name: String,
    pub flow: Flow,
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default)]
    pub devices: Vec<DeviceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub commands: Vec<SwitchCommand>,
    pub switch_communication_device: bool,
    pub run_in_background: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            commands: vec![SwitchCommand::new(Flow::Playback), SwitchCommand::new(Flow::Recording)],
            // Call apps (Discord, Teams) usually should follow the switch too.
            switch_communication_device: true,
            run_in_background: true,
        }
    }
}

impl Config {
    /// The command of a flow. `normalize` (run on load) guarantees it exists.
    pub fn flow(&self, flow: Flow) -> &SwitchCommand {
        self.commands.iter().find(|c| c.flow == flow).expect("one command per flow")
    }

    pub fn flow_mut(&mut self, flow: Flow) -> &mut SwitchCommand {
        self.commands.iter_mut().find(|c| c.flow == flow).expect("one command per flow")
    }

    /// Keeps exactly one command per flow. Configs from versions that allowed
    /// several keep the built-in one (or else the first) of each flow.
    fn normalize(&mut self) {
        let mut kept = Vec::new();
        for flow in [Flow::Playback, Flow::Recording] {
            let of_flow = |c: &&SwitchCommand| c.flow == flow;
            let mut command = self
                .commands
                .iter()
                .filter(of_flow)
                .find(|c| is_built_in_name(flow, &c.name))
                .or_else(|| self.commands.iter().find(of_flow))
                .cloned()
                .unwrap_or_else(|| SwitchCommand::new(flow));
            command.name = SwitchCommand::new(flow).name;
            kept.push(command);
        }
        self.commands = kept;
    }
}

/// The built-in key, or the text name older versions stored instead.
fn is_built_in_name(flow: Flow, name: &str) -> bool {
    match flow {
        Flow::Playback => matches!(name, PLAYBACK_KEY | "播放裝置" | "Playback"),
        Flow::Recording => matches!(name, RECORDING_KEY | "錄音裝置" | "Recording"),
    }
}

impl SwitchCommand {
    pub fn new(flow: Flow) -> Self {
        Self {
            name: match flow {
                Flow::Playback => PLAYBACK_KEY,
                Flow::Recording => RECORDING_KEY,
            }
            .into(),
            flow,
            hotkey: None,
            devices: Vec::new(),
        }
    }

    /// The flow's name in the UI language, for messages.
    pub fn label(&self) -> &'static str {
        match self.flow {
            Flow::Playback => tr!("播放裝置", "Playback"),
            Flow::Recording => tr!("錄音裝置", "Recording"),
        }
    }

    /// Adds or removes a device. Matching is by id first; failing that, by
    /// name, but only against entries whose id is no longer present, so a
    /// device whose id changed replaces its stale entry while two present
    /// devices with the same name stay separate.
    pub fn set_checked(&mut self, device: DeviceRef, checked: bool, present_ids: &[String]) {
        let pos = self.devices.iter().position(|d| d.id == device.id).or_else(|| {
            self.devices
                .iter()
                .position(|d| d.name == device.name && !present_ids.contains(&d.id))
        });

        match (checked, pos) {
            (true, None) => self.devices.push(device),
            (true, Some(i)) => self.devices[i] = device,
            (false, Some(i)) => {
                self.devices.remove(i);
            }
            (false, None) => {}
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("config.json"),
        }
    }

    /// A missing file yields the defaults. A corrupt file is kept aside as
    /// `config.json.bak` so the user's selection is never silently lost.
    pub fn load(&self) -> Config {
        let Ok(text) = fs::read_to_string(&self.path) else {
            return Config::default();
        };

        match serde_json::from_str::<Config>(&text) {
            Ok(mut config) => {
                config.normalize();
                config
            }
            Err(_) => {
                let _ = fs::copy(&self.path, self.path.with_extension("json.bak"));
                Config::default()
            }
        }
    }

    /// Writes to a temp file and renames it over the old one, so a crash
    /// mid-write cannot leave a half-written config behind.
    pub fn save(&self, config: &Config) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }

        let tmp = self.path.with_extension("json.tmp");
        let mut file = fs::File::create(&tmp)?;
        file.write_all(serde_json::to_string_pretty(config)?.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, name: &str) -> DeviceRef {
        DeviceRef {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn set_checked_replaces_entry_whose_id_changed() {
        let mut cmd = Config::default().commands.remove(0);
        cmd.set_checked(dev("old", "耳機"), true, &[]);
        // "old" is gone, so the same-named "new" takes over its entry.
        cmd.set_checked(dev("new", "耳機"), true, &["new".into()]);
        assert_eq!(cmd.devices, vec![dev("new", "耳機")]);

        cmd.set_checked(dev("other", "耳機"), false, &["other".into()]);
        assert!(cmd.devices.is_empty());
    }

    #[test]
    fn set_checked_keeps_two_present_devices_with_the_same_name() {
        let mut cmd = Config::default().commands.remove(0);
        let present = ["a".to_string(), "b".to_string()];
        cmd.set_checked(dev("a", "耳機 (USB Headset)"), true, &present);
        cmd.set_checked(dev("b", "耳機 (USB Headset)"), true, &present);
        assert_eq!(cmd.devices, vec![dev("a", "耳機 (USB Headset)"), dev("b", "耳機 (USB Headset)")]);

        cmd.set_checked(dev("b", "耳機 (USB Headset)"), false, &present);
        assert_eq!(cmd.devices, vec![dev("a", "耳機 (USB Headset)")]);
    }

    #[test]
    fn normalize_keeps_the_built_in_command_of_each_flow() {
        let mut config = Config::default();
        config.commands = vec![
            SwitchCommand { name: "耳機組".into(), ..SwitchCommand::new(Flow::Playback) },
            SwitchCommand {
                name: "播放裝置".into(),
                hotkey: Some("Ctrl+F7".into()),
                ..SwitchCommand::new(Flow::Playback)
            },
        ];

        config.normalize();
        assert_eq!(config.commands.len(), 2);
        assert_eq!(config.flow(Flow::Playback).name, PLAYBACK_KEY);
        assert_eq!(config.flow(Flow::Playback).hotkey.as_deref(), Some("Ctrl+F7"));
        assert_eq!(config.flow(Flow::Recording), &SwitchCommand::new(Flow::Recording));
    }

    #[test]
    fn normalize_keeps_the_first_command_when_none_is_built_in() {
        let mut config = Config::default();
        config.commands = vec![
            SwitchCommand::new(Flow::Recording),
            SwitchCommand { name: "A".into(), hotkey: Some("F13".into()), ..SwitchCommand::new(Flow::Playback) },
            SwitchCommand { name: "B".into(), ..SwitchCommand::new(Flow::Playback) },
        ];

        config.normalize();
        assert_eq!(config.flow(Flow::Playback).hotkey.as_deref(), Some("F13"));
        assert_eq!(config.flow(Flow::Playback).label(), "播放裝置");
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("audio-switcher-test-{}", std::process::id()));
        let store = ConfigStore::new(&dir);
        let mut config = Config::default();
        config.commands[0].devices.push(dev("{0.0.0.00000000}.{abc}", "喇叭"));
        store.save(&config).unwrap();
        assert_eq!(store.load(), config);
        let _ = fs::remove_dir_all(dir);
    }
}
