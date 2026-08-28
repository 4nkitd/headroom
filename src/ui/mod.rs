pub mod panel;
pub mod prefs;
pub mod text_input;
pub mod usage;
pub mod widgets;

use gpui::{App, Global, KeyBinding, SharedString, actions};

use crate::theme;

actions!(headroom, [Refresh, Activate, Tab, TabPrev, Back]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("enter", Activate, None),
        KeyBinding::new("space", Activate, None),
        KeyBinding::new("tab", Tab, None),
        KeyBinding::new("shift-tab", TabPrev, None),
        KeyBinding::new("escape", Back, None),
    ]);
}

/// Font families resolved against what is actually installed, so a missing
/// IBM Plex Mono degrades to SF Mono rather than to the UI font.
#[derive(Clone)]
pub struct Fonts {
    pub mono: SharedString,
    pub ui: SharedString,
}

impl Global for Fonts {}

impl Fonts {
    pub fn resolve(cx: &App) -> Self {
        let installed = cx.text_system().all_font_names();
        let pick = |candidates: &[&str], fallback: &str| -> SharedString {
            candidates
                .iter()
                .find(|c| installed.iter().any(|f| f == *c))
                .map(|c| SharedString::from(c.to_string()))
                .unwrap_or_else(|| SharedString::from(fallback.to_string()))
        };
        Self {
            mono: pick(theme::MONO_CANDIDATES, "monospace"),
            ui: pick(theme::UI_CANDIDATES, "Helvetica"),
        }
    }

    pub fn get(cx: &App) -> Self {
        cx.global::<Fonts>().clone()
    }
}
