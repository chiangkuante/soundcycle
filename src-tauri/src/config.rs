//! Persisted settings. This file is the single source of truth for which
//! devices belong to a command: it only changes on explicit user actions,
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

/// Names of the two built-in commands. They are stored as these keys and
/// shown with a label in the current UI language, so they follow a change of
/// Windows language. Renaming one replaces the key with the user's name.
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
            commands: vec![
                SwitchCommand {
                    name: PLAYBACK_KEY.into(),
                    flow: Flow::Playback,
                    hotkey: None,
                    devices: Vec::new(),
                },
                SwitchCommand {
                    name: RECORDING_KEY.into(),
                    flow: Flow::Recording,
                    hotkey: None,
                    devices: Vec::new(),
                },
            ],
            switch_communication_device: false,
            run_in_background: true,
        }
    }
}

impl Config {
    pub fn command(&self, name: &str) -> Option<&SwitchCommand> {
        self.commands.iter().find(|c| c.name == name)
    }

    pub fn command_mut(&mut self, name: &str) -> Option<&mut SwitchCommand> {
        self.commands.iter_mut().find(|c| c.name == name)
    }

    pub fn add_command(&mut self, name: &str, flow: Flow) -> Result<String, String> {
        let name = self.available_name(name, None)?;
        self.commands.push(SwitchCommand {
            name: name.clone(),
            flow,
            hotkey: None,
            devices: Vec::new(),
        });
        Ok(name)
    }

    pub fn rename_command(&mut self, old: &str, new: &str) -> Result<String, String> {
        let new = self.available_name(new, Some(old))?;
        self.command_mut(old).ok_or(tr!("找不到指令", "Command not found"))?.name = new.clone();
        Ok(new)
    }

    /// Each flow keeps at least one command so its tab is never empty.
    pub fn delete_command(&mut self, name: &str) -> Result<(), String> {
        let flow = self.command(name).ok_or(tr!("找不到指令", "Command not found"))?.flow;
        if self.commands.iter().filter(|c| c.flow == flow).count() <= 1 {
            return Err(tr!("每個分頁至少要保留一個指令", "Each tab needs at least one command").into());
        }
        self.commands.retain(|c| c.name != name);
        Ok(())
    }

    /// Names are trimmed and must be unique ignoring case, across both flows,
    /// because commands are looked up by name alone. A name may not match
    /// another command's displayed label either, and `@` is reserved for the
    /// built-in keys.
    fn available_name(&self, name: &str, renaming: Option<&str>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr!("名稱不能是空白", "The name can't be empty").into());
        }
        if name.starts_with('@') {
            return Err(tr!("名稱不能以 @ 開頭", "The name can't start with @").into());
        }

        let lower = name.to_lowercase();
        let taken = self.commands.iter().any(|c| {
            Some(c.name.as_str()) != renaming && (c.name.to_lowercase() == lower || c.label().to_lowercase() == lower)
        });
        if taken {
            return Err(tr!("名稱「{}」已經被使用", "The name \"{}\" is already in use", name));
        }

        Ok(name.to_string())
    }

    /// Configs written before the built-in keys existed stored the default
    /// names as text; turn those back into keys.
    fn migrate_default_names(&mut self) {
        for c in &mut self.commands {
            let key = match (c.flow, c.name.as_str()) {
                (Flow::Playback, "播放裝置" | "Playback") => PLAYBACK_KEY,
                (Flow::Recording, "錄音裝置" | "Recording") => RECORDING_KEY,
                _ => continue,
            };
            c.name = key.into();
        }
    }
}

impl SwitchCommand {
    /// The name to show: built-in keys become a label in the UI language.
    pub fn label(&self) -> String {
        match self.name.as_str() {
            PLAYBACK_KEY => tr!("播放裝置", "Playback").into(),
            RECORDING_KEY => tr!("錄音裝置", "Recording").into(),
            name => name.into(),
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
                config.migrate_default_names();
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
    fn command_names_are_trimmed_and_unique_ignoring_case() {
        let mut config = Config::default();
        assert_eq!(config.add_command("  耳機組 ", Flow::Playback).unwrap(), "耳機組");
        assert!(config.add_command("耳機組", Flow::Recording).is_err());
        assert!(config.add_command("   ", Flow::Playback).is_err());

        config.add_command("Speakers", Flow::Playback).unwrap();
        assert!(config.add_command("speakers", Flow::Playback).is_err());
    }

    #[test]
    fn rename_allows_changing_case_of_own_name_only() {
        let mut config = Config::default();
        config.add_command("Speakers", Flow::Playback).unwrap();

        assert_eq!(config.rename_command("Speakers", "speakers").unwrap(), "speakers");
        assert!(config.rename_command("speakers", "播放裝置").is_err());
        assert!(config.rename_command("不存在", "新名稱").is_err());
    }

    #[test]
    fn delete_keeps_at_least_one_command_per_flow() {
        let mut config = Config::default();
        assert!(config.delete_command(PLAYBACK_KEY).is_err());

        config.add_command("耳機組", Flow::Playback).unwrap();
        config.delete_command(PLAYBACK_KEY).unwrap();
        assert!(config.command(PLAYBACK_KEY).is_none());
        assert!(config.delete_command("耳機組").is_err());
    }

    #[test]
    fn built_in_commands_show_a_label_and_reserve_it() {
        let mut config = Config::default();
        assert_eq!(config.commands[0].name, PLAYBACK_KEY);
        assert_eq!(config.commands[0].label(), "播放裝置");
        // Another command may not take the built-in's label, nor an @ name.
        assert!(config.add_command("播放裝置", Flow::Recording).is_err());
        assert!(config.add_command("@playback", Flow::Playback).is_err());

        let renamed = config.rename_command(PLAYBACK_KEY, "耳機組").unwrap();
        assert_eq!(config.command(&renamed).unwrap().label(), "耳機組");
    }

    #[test]
    fn load_migrates_old_default_names_to_keys() {
        let mut config = Config::default();
        config.commands[0].name = "Playback".into();
        config.commands[1].name = "錄音裝置".into();
        config.commands.push(SwitchCommand {
            name: "Playback".into(),
            flow: Flow::Recording,
            hotkey: None,
            devices: vec![],
        });

        config.migrate_default_names();
        let names: Vec<_> = config.commands.iter().map(|c| c.name.as_str()).collect();
        // Only names on their own flow are built-ins; the rest are user names.
        assert_eq!(names, [PLAYBACK_KEY, RECORDING_KEY, "Playback"]);
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
