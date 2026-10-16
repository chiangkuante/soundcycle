# SoundCycle

Cycle through your audio devices with a hotkey on Windows. Built with Rust and Tauri v2.

用熱鍵在多個音訊裝置之間輪流切換的 Windows 工具，以 Rust + Tauri v2 打造。

- Pick the playback or recording devices to cycle through; each hotkey press switches the Windows default to the next one.
- Devices you unplug stay selected and are used again once they're back, even if Windows gives them a new ID.
- A focus-safe overlay shows the device you switched to; a tray menu lists devices for direct switching.
- Optional: also switch the default communication device, start with Windows.

[Privacy policy / 隱私權政策](PRIVACY.md)

## Development

Requirements: Windows 10 1903+ or 11, Node.js, Rust (MSVC toolchain) and the Visual Studio Build Tools C++ workload.

```powershell
npm install
npm run tauri dev        # run with hot reload
cd src-tauri; cargo test # unit tests
```

The UI follows the Windows display language (Traditional Chinese or English). Set `SOUNDCYCLE_LANG=en` or `zh` to override it.

## Packaging (MSIX / Microsoft Store)

Uses Microsoft's [winapp CLI](https://learn.microsoft.com/windows/apps/dev-tools/winapp-cli/guides/tauri) (`winget install Microsoft.WinAppCli`).

```powershell
npm run pack:msix    # local test package signed with a dev certificate
npm run pack:store   # unsigned package for Partner Center (the Store signs it)
```

The package identity in `msix/Package.appxmanifest` comes from Partner Center. Raise its `Version` for every new package. Store listing text lives in `store/listing.md`.

## Project layout

| Path | What |
|---|---|
| `src-tauri/src/audio.rs` | Core Audio: device list, default device (IPolicyConfig), change notifications |
| `src-tauri/src/switcher.rs` | Cycling logic and the device list shown in the UI |
| `src-tauri/src/config.rs` | Settings file (the single source of truth for selected devices) |
| `src-tauri/src/hotkeys.rs` | Global hotkeys |
| `src-tauri/src/osd.rs` | Switch overlay window (shown without taking focus) |
| `src-tauri/src/startup.rs` | Start with Windows (StartupTask when packaged, Run key otherwise) |
| `src-tauri/src/tray.rs` | Tray icon and menu |
| `src/` | Settings window and overlay frontend |

## License

[MIT](LICENSE)
