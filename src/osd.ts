import { listen } from "@tauri-apps/api/event";

type Flow = "playback" | "recording";

interface Line {
  kind: "switched" | "error";
  text: string;
  flow: Flow | null;
}

const icons: Record<Flow, string> = {
  playback:
    '<svg viewBox="0 0 24 24"><path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3A4.5 4.5 0 0 0 14 8v8a4.5 4.5 0 0 0 2.5-4zM14 3.2v2.1a7 7 0 0 1 0 13.4v2.1a9 9 0 0 0 0-17.6z"/></svg>',
  recording:
    '<svg viewBox="0 0 24 24"><path d="M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5.3-3a5.3 5.3 0 0 1-10.6 0H5a7 7 0 0 0 6 6.9V21h2v-3.1a7 7 0 0 0 6-6.9h-1.7z"/></svg>',
};

const card = document.querySelector<HTMLDivElement>("#card")!;

function renderLine(line: Line): HTMLElement {
  const el = document.createElement("div");
  el.className = line.kind;

  if (line.kind !== "switched") {
    el.textContent = `✕ ${line.text}`;
    return el;
  }

  // "耳機_G435 (G435 Wireless Gaming Headset)" → big endpoint name, small hardware name.
  const [, device = line.text, hardware = ""] =
    line.text.match(/^(.+) \((.+)\)$/) ?? [];

  const names = document.createElement("div");
  names.className = "names";
  const deviceEl = document.createElement("span");
  deviceEl.className = "device";
  deviceEl.textContent = device;
  names.append(deviceEl);
  if (hardware) {
    const hardwareEl = document.createElement("span");
    hardwareEl.className = "hardware";
    hardwareEl.textContent = hardware;
    names.append(hardwareEl);
  }

  el.innerHTML = line.flow ? icons[line.flow] : "";
  el.append(names);
  return el;
}

listen<Line[]>("osd-show", ({ payload }) => {
  card.replaceChildren(...payload.map(renderLine));
  card.classList.remove("hiding");

  // Replay the pop on every message so back-to-back switches are visible.
  card.classList.remove("pop");
  void card.offsetWidth;
  card.classList.add("visible", "pop");
});

listen("osd-hide", () => {
  card.classList.add("hiding");
  card.classList.remove("visible");
});
