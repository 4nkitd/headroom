//! Design tokens lifted from the "Menu Bar Usage" mock.
//!
//! Everything the UI paints resolves through here so the whole surface can be
//! retuned in one place.

use gpui::{Rgba, rgb, rgba};

/// Popover fill. Fully opaque black per user preference.
pub const PANEL_BG: u32 = 0x000000ff;
pub const PANEL_BORDER: u32 = 0xffffff24;

pub const TEXT: u32 = 0xf5f5f7ff;
/// Section headers ("CURRENT LIMITS", "PREFERENCES").
pub const TEXT_LABEL: u32 = 0xffffff73;
/// Row subtitles ("5-hour session · resets 11:55").
pub const TEXT_MUTED: u32 = 0xffffff6b;
/// Monospace metadata ("synced 40s ago").
pub const TEXT_FAINT: u32 = 0xffffff59;
/// Footer actions ("Preferences…", "Refresh now").
pub const TEXT_ACTION: u32 = 0xffffff9e;
/// Expanded-detail numerics.
pub const TEXT_DETAIL: u32 = 0xffffffa6;
pub const TEXT_DIM: u32 = 0xffffff4d;

pub const DIVIDER: u32 = 0xffffff17;
pub const TRACK: u32 = 0xffffff1f;
pub const ROW_HOVER: u32 = 0xffffff0f;
pub const API_BADGE_BG: u32 = 0x30d1581f;
pub const API_BADGE_TEXT: u32 = 0x63df7fff;
pub const OK_TEXT: u32 = 0x30d158ff;
pub const WARN_TEXT: u32 = 0xffd60aff;

pub const LINK: u32 = 0x6fa8ff;
pub const LINK_HOVER: u32 = 0x9cc4ff;

pub const OK: u32 = 0x30d158;
pub const WARN: u32 = 0xffd60a;
pub const CRITICAL: u32 = 0xff9f0a;

/// Toggle knob + slider handle.
pub const CONTROL_ON: u32 = 0x30d158ff;
pub const CONTROL_OFF: u32 = 0xffffff2e;
pub const CONTROL_KNOB: u32 = 0xffffffff;

/// Preferred monospace families, best first. Resolved against the installed
/// font set at startup — see [`crate::ui::Fonts`].
pub const MONO_CANDIDATES: &[&str] = &["IBM Plex Mono", "SF Mono", "Menlo", "Monaco"];
pub const UI_CANDIDATES: &[&str] = &[".SystemUIFont", "SF Pro Text", "Helvetica Neue"];

/// Bar geometry.
pub const BAR_HEIGHT: f32 = 7.0;
pub const BAR_WIDTH: f32 = 88.0;
pub const SUB_BAR_HEIGHT: f32 = 5.0;
/// Width of one stripe and the gap that follows it, matching the mock's
/// `repeating-linear-gradient(90deg, C 0 5px, transparent 5px 7px)`.
pub const STRIPE_ON: f32 = 5.0;
pub const STRIPE_OFF: f32 = 2.0;

pub const PANEL_WIDTH: f32 = 372.0;
/// Distance from the top of the screen to the top of the popover.
pub const PANEL_TOP_INSET: f32 = 34.0;
/// Minimum gap between the popover and the right screen edge.
pub const PANEL_EDGE_MARGIN: f32 = 12.0;
pub const PANEL_USAGE_CHROME_HEIGHT: f32 = 134.0;
pub const PANEL_PROVIDER_ROW_HEIGHT: f32 = 40.0;
pub const PANEL_DETAIL_HEIGHT: f32 = 66.0;
pub const PANEL_SECONDARY_ROW_HEIGHT: f32 = 28.0;
pub const PANEL_PREFS_HEIGHT: f32 = 620.0;

/// How a limit is doing, derived from headroom remaining and the user's warn
/// threshold rather than hardcoded cutoffs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Health {
    Ok,
    Warn,
    Critical,
}

impl Health {
    /// `percent_left` is headroom remaining; `warn_at` is the *consumed*
    /// percentage the user wants to be warned at (the Preferences slider).
    pub fn from_percent_left(percent_left: f32, warn_at: f32) -> Self {
        let warn_below = 100.0 - warn_at;
        if percent_left <= warn_below / 2.0 {
            Health::Critical
        } else if percent_left <= warn_below {
            Health::Warn
        } else {
            Health::Ok
        }
    }

    pub fn color(self) -> Rgba {
        match self {
            Health::Ok => rgb(OK),
            Health::Warn => rgb(WARN),
            Health::Critical => rgb(CRITICAL),
        }
    }
}

pub fn c(token: u32) -> Rgba {
    rgba(token)
}
