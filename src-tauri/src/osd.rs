//! On-screen overlay that shows the result of a switch, replacing Windows
//! toasts. Each new message replaces the previous one immediately, so rapid
//! switching always shows the device that is current now.
//!
//! The window is shown and hidden with raw Win32 calls: tao's `show()` uses
//! `SW_SHOW`, which would activate the overlay and pull focus out of a game.

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE,
};

use crate::config::Flow;

const LABEL: &str = "osd";
/// Logical size of the overlay window. The card inside sits at the bottom
/// and the rest is transparent and click-through.
const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 200.0;
/// Gap between the overlay window and the bottom of the work area (above the
/// taskbar). The page adds its own padding so the card's shadow isn't clipped.
const BOTTOM_MARGIN: f64 = 44.0;
const SWITCHED_MS: u64 = 1000;
/// Errors stay longer so there is time to read them.
const ERROR_MS: u64 = 3000;
const FADE_MS: u64 = 200;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Switched,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
    pub flow: Option<Flow>,
}

impl Line {
    pub fn switched(flow: Flow, device: &str) -> Self {
        Self {
            kind: Kind::Switched,
            text: device.to_string(),
            flow: Some(flow),
        }
    }

    pub fn error(text: String) -> Self {
        Self {
            kind: Kind::Error,
            text,
            flow: None,
        }
    }
}

/// Bumped on every `show`; a pending hide only runs if no newer message
/// arrived in the meantime.
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("osd.html".into()))
        .title("SoundCycle OSD")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

pub fn show(app: &AppHandle, lines: Vec<Line>) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    if lines.is_empty() {
        return;
    }

    let has_error = lines.iter().any(|l| matches!(l.kind, Kind::Error));
    let visible_ms = if has_error { ERROR_MS } else { SWITCHED_MS };
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    let _ = window.emit_to(LABEL, "osd-show", &lines);
    show_without_focus(&window);

    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(visible_ms));
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        let _ = app.emit_to(LABEL, "osd-hide", ());

        thread::sleep(Duration::from_millis(FADE_MS));
        if GENERATION.load(Ordering::SeqCst) == generation {
            if let Some(window) = app.get_webview_window(LABEL) {
                if let Ok(hwnd) = window.hwnd() {
                    unsafe {
                        let _ = ShowWindow(HWND(hwnd.0 as _), SW_HIDE);
                    }
                }
            }
        }
    });
}

/// Places the overlay bottom-center on the monitor under the mouse cursor
/// and shows it without activating it.
fn show_without_focus(window: &WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else { return };

    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|p| window.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };

    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = (WIDTH * scale) as i32;
    let height = (HEIGHT * scale) as i32;
    let x = area.position.x + (area.size.width as i32 - width) / 2;
    let y = area.position.y + area.size.height as i32 - height - (BOTTOM_MARGIN * scale) as i32;

    unsafe {
        let _ = SetWindowPos(
            HWND(hwnd.0 as _),
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}
