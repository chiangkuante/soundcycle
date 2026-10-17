import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { initI18n, m } from "./i18n";

type Flow = "playback" | "recording";
type DeviceState = "active" | "disabled" | "unplugged" | "notpresent";

interface DeviceRow {
  id: string;
  name: string;
  state: DeviceState;
  is_default: boolean;
  checked: boolean;
}

interface SwitchOutcome {
  switched_to: string | null;
  skipped: string[];
}

const stateLabel = (state: DeviceState): string =>
  ({ active: "", disabled: m.stateDisabled, unplugged: m.stateUnplugged, notpresent: m.stateNotPresent })[state];

const deviceList = document.querySelector<HTMLUListElement>("#devices")!;
const statusEl = document.querySelector<HTMLParagraphElement>("#status")!;
const hotkeyInput = document.querySelector<HTMLInputElement>("#hotkey")!;
const hint = document.querySelector<HTMLParagraphElement>("#hint")!;
let flow: Flow = "playback";

function showStatus(message: string, isError = false) {
  statusEl.textContent = message;
  statusEl.classList.toggle("error", isError);
}

/** Loads the current tab: its hotkey and its device list. */
async function loadFlow() {
  hotkeyInput.value = formatHotkey(await invoke<string | null>("get_hotkey", { flow }));
  await loadDevices();
}

// Hotkeys are stored with KeyboardEvent.code names ("Ctrl+Alt+KeyA");
// show them the way keys are labelled ("Ctrl + Alt + A").
function formatHotkey(hotkey: string | null): string {
  if (!hotkey) return "";
  return hotkey
    .split("+")
    .map((part) =>
      part
        .replace(/^Key|^Digit/, "")
        .replace(/^Numpad/, "Num ")
        .replace(/^Super$/, "Win"),
    )
    .join(" + ");
}

const modifierCodes = /^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/;

function hotkeyFromEvent(e: KeyboardEvent): string | null {
  if (modifierCodes.test(e.code)) return null;
  const parts = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Super");
  parts.push(e.code);
  return parts.join("+");
}

async function setHotkey(hotkey: string | null) {
  try {
    await invoke("set_hotkey", { flow, hotkey });
    hotkeyInput.value = formatHotkey(hotkey);
    showStatus(hotkey ? m.hotkeySet(formatHotkey(hotkey)) : m.hotkeyCleared);
  } catch (e) {
    showStatus(String(e), true);
  }
}

// While the field has focus, global hotkeys are paused so that pressing an
// existing one is recorded instead of switching devices.
hotkeyInput.addEventListener("focus", () => {
  invoke("pause_hotkeys", { paused: true });
  hotkeyInput.placeholder = m.hotkeyRecording;
});

hotkeyInput.addEventListener("blur", async () => {
  hotkeyInput.placeholder = m.hotkeyIdle;
  try {
    await invoke("pause_hotkeys", { paused: false });
  } catch (e) {
    showStatus(String(e), true);
  }
});

hotkeyInput.addEventListener("keydown", async (e) => {
  e.preventDefault();
  if (e.code === "Escape") {
    hotkeyInput.blur();
  } else if (e.code === "Backspace" || e.code === "Delete") {
    await setHotkey(null);
  } else {
    const hotkey = hotkeyFromEvent(e);
    if (hotkey) await setHotkey(hotkey);
  }
});

async function loadDevices() {
  try {
    const rows = await invoke<DeviceRow[]>("device_rows", { flow });
    const visible = rows.filter((r) => r.state !== "disabled" || r.checked);
    deviceList.replaceChildren(...visible.map(renderRow));
    hint.hidden = rows.some((r) => r.checked);
  } catch (e) {
    showStatus(String(e), true);
  }
}

function renderRow(row: DeviceRow): HTMLLIElement {
  const li = document.createElement("li");
  li.className = `device ${row.state}`;

  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.checked = row.checked;
  checkbox.addEventListener("change", async () => {
    try {
      await invoke("set_device_checked", {
        flow,
        id: row.id,
        name: row.name,
        checked: checkbox.checked,
      });
      await loadDevices();
    } catch (e) {
      showStatus(String(e), true);
    }
  });

  const name = document.createElement("span");
  name.className = "name";
  name.textContent = row.name;

  const badge = document.createElement("span");
  badge.className = "badge";
  badge.textContent = row.is_default ? m.inUse : stateLabel(row.state);

  const label = document.createElement("label");
  label.append(checkbox, name, badge);
  li.append(label);
  return li;
}

async function switchNext() {
  try {
    const outcome = await invoke<SwitchOutcome>("switch_next", { flow });
    const lines = outcome.switched_to ? [`✔ ${outcome.switched_to}`] : [];
    lines.push(...outcome.skipped.map(m.skipped));
    showStatus(lines.join("\n") || m.noDevice);
    await loadDevices();
  } catch (e) {
    showStatus(String(e), true);
  }
}

// --- Settings ---------------------------------------------------------------

interface Settings {
  switch_communication_device: boolean;
  run_in_background: boolean;
  run_at_startup: boolean;
}

const settingKeys: (keyof Settings)[] = [
  "run_at_startup",
  "run_in_background",
  "switch_communication_device",
];
const settingInput = (key: keyof Settings) =>
  document.querySelector<HTMLInputElement>(`#${key}`)!;

function renderSettings(settings: Settings) {
  for (const key of settingKeys) settingInput(key).checked = settings[key];
}

async function loadSettings() {
  renderSettings(await invoke<Settings>("get_settings"));
}

for (const key of settingKeys) {
  settingInput(key).addEventListener("change", async () => {
    const settings = Object.fromEntries(
      settingKeys.map((k) => [k, settingInput(k).checked]),
    ) as unknown as Settings;
    try {
      renderSettings(await invoke<Settings>("update_settings", { settings }));
      showStatus(m.saved);
    } catch (e) {
      showStatus(String(e), true);
      await loadSettings();
    }
  });
}

// --- Tabs -------------------------------------------------------------------

const audioView = document.querySelector<HTMLElement>("#audio-view")!;
const settingsView = document.querySelector<HTMLElement>("#settings-view")!;

document.querySelectorAll<HTMLButtonElement>(".tab").forEach((tab) => {
  tab.addEventListener("click", async () => {
    document.querySelector(".tab.active")?.classList.remove("active");
    tab.classList.add("active");
    showStatus("");

    const view = tab.dataset.view!;
    audioView.hidden = view === "settings";
    settingsView.hidden = view !== "settings";

    if (view === "settings") {
      await loadSettings();
    } else {
      flow = view as Flow;
      await loadFlow();
    }
  });
});

document.querySelector("#switch")!.addEventListener("click", switchNext);

// The backend emits this when devices are plugged, unplugged, renamed or the
// default changes (including switches made by hotkey or from the tray).
listen("devices-changed", () => {
  if (!audioView.hidden) loadDevices();
});

initI18n()
  .then(() => loadFlow())
  .catch((e) => showStatus(String(e), true));
