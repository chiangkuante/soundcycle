import { invoke } from "@tauri-apps/api/core";

// The backend picks the language (Traditional Chinese when Windows is in
// Chinese, English otherwise) so its messages and these always agree.

const zh = {
  tabPlayback: "播放",
  tabRecording: "錄音",
  tabSettings: "設定",
  switchNext: "切換到下一個裝置",
  hotkey: "熱鍵",
  hotkeyIdle: "點這裡後按下組合鍵",
  hotkeyRecording: "按下組合鍵，Backspace 清除，Esc 取消",
  hintEmpty: "勾選要輪流切換的裝置，再設定熱鍵。按下熱鍵就會切到下一個裝置。",
  startupTitle: "開機時自動啟動",
  startupDesc: "登入 Windows 後在系統匣執行，不會開啟視窗",
  backgroundTitle: "關閉視窗時縮到系統匣",
  backgroundDesc: "關閉後熱鍵仍可使用；取消勾選則關閉視窗會結束程式",
  commTitle: "同時切換預設通訊裝置",
  commDesc: "Discord、Teams 等通話軟體也會改用新裝置",
  stateDisabled: "已停用",
  stateUnplugged: "未插入",
  stateNotPresent: "未連接",
  inUse: "目前使用",
  noDevice: "沒有可切換的裝置",
  hotkeyCleared: "已清除熱鍵",
  saved: "已儲存",
  hotkeySet: (hotkey: string) => `熱鍵已設為 ${hotkey}`,
  skipped: (detail: string) => `⚠ 略過 ${detail}`,
};

export type Messages = typeof zh;

const en: Messages = {
  tabPlayback: "Playback",
  tabRecording: "Recording",
  tabSettings: "Settings",
  switchNext: "Switch to the next device",
  hotkey: "Hotkey",
  hotkeyIdle: "Click, then press a key combination",
  hotkeyRecording: "Press keys · Backspace clears · Esc cancels",
  hintEmpty: "Tick the devices to cycle through, then set a hotkey. Each press switches to the next device.",
  startupTitle: "Start with Windows",
  startupDesc: "Runs in the system tray after you sign in, without opening a window",
  backgroundTitle: "Keep running in the tray when closed",
  backgroundDesc: "Hotkeys keep working after closing; untick to exit when the window closes",
  commTitle: "Also switch the communication device",
  commDesc: "Call apps such as Discord and Teams follow the new device too",
  stateDisabled: "Disabled",
  stateUnplugged: "Unplugged",
  stateNotPresent: "Not connected",
  inUse: "In use",
  noDevice: "No device to switch to",
  hotkeyCleared: "Hotkey cleared",
  saved: "Saved",
  hotkeySet: (hotkey) => `Hotkey set to ${hotkey}`,
  skipped: (detail) => `⚠ Skipped: ${detail}`,
};

type TextKey = { [K in keyof Messages]: Messages[K] extends string ? K : never }[keyof Messages];

/** Current messages. A live ES module binding: importers see the update. */
export let m: Messages = zh;

/** Loads the language and fills in `data-i18n` (text), `data-i18n-title`
 *  and `data-i18n-placeholder` attributes in the page. */
export async function initI18n() {
  const lang = await invoke<string>("ui_language");
  m = lang === "zh" ? zh : en;
  document.documentElement.lang = lang === "zh" ? "zh-Hant" : "en";

  const fill = (attr: string, apply: (el: HTMLElement, text: string) => void) =>
    document.querySelectorAll<HTMLElement>(`[${attr}]`).forEach((el) => {
      apply(el, m[el.getAttribute(attr) as TextKey]);
    });

  fill("data-i18n", (el, text) => (el.textContent = text));
  fill("data-i18n-title", (el, text) => (el.title = text));
  fill("data-i18n-placeholder", (el, text) => ((el as HTMLInputElement).placeholder = text));
}
