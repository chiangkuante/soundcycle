//! UI language: Traditional Chinese when the Windows display language is
//! Chinese, English otherwise. The frontend asks for the same choice via the
//! `ui_language` command so both halves always agree.

use std::sync::OnceLock;

use windows::Win32::Globalization::GetUserDefaultUILanguage;

static ZH: OnceLock<bool> = OnceLock::new();

pub fn is_zh() -> bool {
    *ZH.get_or_init(detect)
}

pub fn code() -> &'static str {
    if is_zh() { "zh" } else { "en" }
}

fn detect() -> bool {
    // Tests assert on the Chinese messages whatever the machine's language.
    if cfg!(test) {
        return true;
    }
    // Override for checking the other language, e.g. SOUNDCYCLE_LANG=en.
    if let Ok(lang) = std::env::var("SOUNDCYCLE_LANG") {
        return lang.eq_ignore_ascii_case("zh");
    }
    const LANG_CHINESE: u16 = 0x04;
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    lang_id & 0x3ff == LANG_CHINESE
}

/// `tr!("中文", "English")` gives a `&'static str`; with arguments,
/// `tr!("「{}」未連接", "\"{}\" is not connected", name)` gives a `String`.
#[macro_export]
macro_rules! tr {
    ($zh:literal, $en:literal $(,)?) => {
        if $crate::i18n::is_zh() { $zh } else { $en }
    };
    ($zh:literal, $en:literal, $($arg:expr),+ $(,)?) => {
        if $crate::i18n::is_zh() { format!($zh, $($arg),+) } else { format!($en, $($arg),+) }
    };
}
