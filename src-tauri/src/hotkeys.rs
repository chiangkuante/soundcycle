//! Global hotkeys. Hotkey strings use `KeyboardEvent.code` names joined by
//! `+`, e.g. `Ctrl+Alt+KeyA` or `F13`.

use std::collections::HashSet;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::audio::WindowsAudio;
use crate::config::Config;
use crate::osd::{self, Line};
use crate::switcher;
use crate::AppState;

/// Parses a hotkey. A modifier is required, except for F13–F24, which no
/// regular key on a keyboard produces and so cannot steal normal typing.
pub fn parse(hotkey: &str) -> Result<Shortcut, String> {
    let shortcut: Shortcut = hotkey.parse().map_err(|_| tr!("無效的熱鍵「{}」", "Invalid hotkey \"{}\"", hotkey))?;

    let key = format!("{:?}", shortcut.key);
    let is_extra_function_key = key
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (13..=24).contains(&n));

    if shortcut.mods.is_empty() && !is_extra_function_key {
        return Err(tr!("熱鍵需要搭配 Ctrl、Alt、Shift 或 Win（F13–F24 除外）", "Hotkeys need Ctrl, Alt, Shift or Win (except F13–F24)").into());
    }

    Ok(shortcut)
}

/// Re-registers every configured hotkey. Returns one message per hotkey that
/// Windows refused, usually because another program already owns it.
pub fn register_all(app: &AppHandle, config: &Config) -> Vec<String> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();

    let mut seen = HashSet::new();
    let mut errors = Vec::new();
    for hotkey in config.commands.iter().filter_map(|c| c.hotkey.as_deref()) {
        let Ok(shortcut) = parse(hotkey) else { continue };
        if seen.insert(shortcut.id()) && shortcuts.register(shortcut).is_err() {
            errors.push(tr!("「{}」無法註冊，可能已被其他程式佔用", "\"{}\" could not be registered; another program may be using it", hotkey));
        }
    }
    errors
}

pub fn unregister_all(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

/// Runs every command bound to the pressed hotkey and shows the result.
pub fn on_pressed(app: &AppHandle, shortcut: &Shortcut) {
    let state = app.state::<AppState>();
    let lines: Vec<Line> = {
        let config = state.config.lock().unwrap();
        config
            .commands
            .iter()
            .filter(|c| c.hotkey.as_deref().and_then(|h| parse(h).ok()).is_some_and(|s| s.id() == shortcut.id()))
            // Skipped devices are left out; only the device switched to matters.
            .map(|c| match switcher::switch_next(&WindowsAudio, c, config.switch_communication_device) {
                Ok(outcome) => match outcome.switched_to {
                    Some(name) => Line::switched(c.flow, &name),
                    None => Line::error(tr!("{}：沒有可切換的裝置", "{}: no device to switch to", c.label())),
                },
                Err(e) => Line::error(tr!("{}：{}", "{}: {}", c.label(), e)),
            })
            .collect()
    };

    osd::show(app, lines);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Modifiers;

    #[test]
    fn requires_modifier_except_for_f13_to_f24() {
        assert!(parse("Ctrl+F7").is_ok());
        assert!(parse("Super+KeyA").is_ok());
        assert!(parse("F13").is_ok());
        assert!(parse("F24").is_ok());
        assert!(parse("F7").is_err());
        assert!(parse("KeyA").is_err());
        assert!(parse("Ctrl+").is_err());
    }

    #[test]
    fn modifiers_parse_regardless_of_order() {
        let a = parse("Ctrl+Shift+KeyM").unwrap();
        let b = parse("Shift+Ctrl+KeyM").unwrap();
        assert_eq!(a.id(), b.id());
        assert_eq!(a.mods, Modifiers::CONTROL | Modifiers::SHIFT);
    }
}
