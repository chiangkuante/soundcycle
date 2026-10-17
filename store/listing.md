# Microsoft Store listing draft

Store ID 9N486C0C2CGP — https://apps.microsoft.com/detail/9N486C0C2CGP

---

## 繁體中文 (zh-TW)

### 簡短描述（最多 100 字元建議）

用熱鍵在耳機、喇叭、麥克風之間一鍵切換。拔掉再插回的裝置也會記住。

### 描述

SoundCycle 讓你用一組熱鍵，在你挑選的音訊裝置之間輪流切換，不必再打開音效設定一層層點選。

● 一鍵輪切：選好要輪流使用的裝置（例如耳機、喇叭、外接 DAC），按下熱鍵就切到下一個。
● 記得每一個裝置：無線耳機沒電、USB 裝置拔掉，勾選都會保留；插回去就能直接切換，不必重新設定。換了 USB 孔或重新配對藍牙，只要同名的裝置只有一個，也會用名稱自動找回。
● 播放與錄音分開設定：喇叭和麥克風各自有一個熱鍵，也可以設成同一個熱鍵一次切換兩者。
● 浮出提示：切換時在螢幕下方顯示目前的裝置，連按多次也只顯示最新的那一個；不會搶走遊戲或其他視窗的焦點。
● 系統匣常駐：從系統匣選單直接選擇播放或錄音裝置。
● 同時切換通訊裝置：Discord、Teams 等通話軟體也會一起改用新裝置（預設開啟，可關閉）。
● 開機自動啟動：登入後在背景待命，不會跳出視窗（預設開啟，可關閉）。
● 熱鍵衝突提示：熱鍵若已被其他程式使用，會直接告訴你。

不收集任何個人資料，不需要網路連線。

### 功能（Product features，每項最多 200 字元）

- 用熱鍵在多個播放或錄音裝置之間輪流切換
- 拔掉再插回的裝置會保留設定，插回即可切換
- 切換時顯示不搶焦點的浮出提示
- 系統匣選單快速選擇裝置
- 同時切換預設通訊裝置
- 開機自動在背景啟動

### 搜尋關鍵字（Store 上限 7 組）

音訊切換, 聲音輸出, 預設音訊裝置, 耳機, 喇叭, 麥克風, 熱鍵

---

## English (en-US)

### Short description

Switch between headphones, speakers and microphones with one hotkey. Unplugged devices are remembered.

### Description

SoundCycle cycles through the audio devices you pick with a single hotkey — no more digging through Sound settings.

● One-key cycling: choose the devices to rotate through (headset, speakers, external DAC…) and press the hotkey to move to the next one.
● Remembers every device: a headset that runs out of battery or a USB device you unplug stays selected, and is used again as soon as it is back. Plugged into another USB port or re-paired over Bluetooth? If only one device has that name, it is found again by name.
● Separate playback and recording: give speakers and microphones their own hotkey, or use the same one to switch both at once.
● On-screen overlay: shows the device you switched to at the bottom of the screen, always the latest one when you press repeatedly, without stealing focus from your game.
● Lives in the system tray: pick a playback or recording device straight from the tray menu.
● Communication device too: also switches the default communication device used by Discord, Teams and other call apps (on by default; can be turned off).
● Starts with Windows: waits in the background after you sign in, without opening a window (on by default; can be turned off).
● Hotkey conflict warnings: tells you when another program already owns a hotkey.

Collects no personal data and needs no internet connection.

### Product features

- Cycle through playback or recording devices with a hotkey
- Unplugged devices stay selected and work again when plugged back in
- Focus-safe on-screen overlay shows the device you switched to
- Pick devices straight from the system tray menu
- Switches the default communication device too
- Starts in the background when you sign in

### Search terms (Store limit: 7)

audio switcher, sound output, default audio device, headphones, speakers, microphone, hotkey

---

## Certification notes (for the "Notes for certification" field)

runFullTrust: SoundCycle is a desktop (Win32) app. It enumerates audio
endpoints with the documented Core Audio API (IMMDeviceEnumerator,
IMMNotificationClient) and registers global hotkeys with RegisterHotKey.

Policy 10.2.8 disclosure: Windows has no documented API for changing the
default audio endpoint, so SoundCycle calls IPolicyConfig::SetDefaultEndpoint,
the undocumented COM interface the Windows Sound control panel itself uses
(also used by other Store apps such as EarTrumpet). It is called only when the
user explicitly asks for a switch (their hotkey, the switch button, or a tray
menu item), only for the default playback/recording/communication endpoint,
and the user's own choice in Windows Sound settings stays in effect until they
switch again. SoundCycle changes no other Windows setting. It runs at sign-in
through a manifest StartupTask that is on by default and can be turned off
in the app's settings or in Task Manager.

To test: open the app, tick two or more playback devices, set a hotkey
(e.g. Ctrl+F7) and press it. The default device changes and an overlay at the
bottom of the screen shows the new device.

## Screenshots and captions (each caption ≤ 200 characters)

Files in `store/screenshots/` (1920×1080, the real UI with sample device
names; no added logos or text). App tile icon: `store/app-tile-300.png`.

| File | 繁體中文說明 | English caption |
|---|---|---|
| `*-1-playback.png` | 勾選要輪流切換的裝置並設定熱鍵；拔掉的裝置（例如 AirPods）會保留勾選，插回就能用。 | Tick the devices to cycle through and set a hotkey. Unplugged devices, like the AirPods here, stay selected. |
| `*-2-overlay.png` | 切換時在螢幕下方顯示目前的裝置，不會搶走遊戲或其他視窗的焦點。 | An overlay shows the device you switched to, without taking focus from your game. |
| `*-3-recording.png` | 麥克風等錄音裝置也能用自己的熱鍵輪流切換。 | Microphones and other recording devices get their own hotkey too. |
| `*-4-settings.png` | 開機時自動在系統匣待命，並同時切換通話軟體使用的通訊裝置；兩者都能在設定中關閉。 | Starts with Windows in the tray and also switches the communication device used by call apps; both can be turned off in Settings. |
