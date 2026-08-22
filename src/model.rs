//! Domain types: what a tracked subscription looks like and how much of it is
//! left. Deliberately provider-agnostic — every backend in `providers/`
//! normalises into these.

use gpui::SharedString;
use serde::{Deserialize, Serialize};

/// Which reset cadence a limit runs on. Ordered so the primary (fastest) limit
/// sorts first when a provider reports several.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Cadence {
    /// Claude Code's rolling 5-hour session window.
    Session,
    Daily,
    Weekly,
    Monthly,
}

impl Cadence {
    pub fn label(self) -> &'static str {
        match self {
            Cadence::Session => "Session",
            Cadence::Daily => "Daily",
            Cadence::Weekly => "Weekly",
            Cadence::Monthly => "Monthly",
        }
    }
}

/// One quota bucket for a provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Limit {
    pub cadence: Cadence,
    /// Headroom remaining, 0.0–100.0.
    pub percent_left: f32,
    /// Human-readable reset time, e.g. `"11:55"`. `None` when unknown.
    pub resets_at: Option<SharedString>,
}

impl Limit {
    pub fn new(cadence: Cadence, percent_left: f32) -> Self {
        Self {
            cadence,
            percent_left: percent_left.clamp(0.0, 100.0),
            resets_at: None,
        }
    }

    pub fn resets_at(mut self, at: impl Into<SharedString>) -> Self {
        self.resets_at = Some(at.into());
        self
    }

    /// Fraction of the bucket already consumed — what the bar actually fills.
    pub fn consumed(&self) -> f32 {
        (100.0 - self.percent_left) / 100.0
    }
}

/// Direction of travel for a provider's recent consumption.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trend {
    Rising,
    Steady,
    Falling,
}

/// Recent consumption history plus a one-line read on it, rendered as the
/// sparkline in the expanded row.
#[derive(Clone, Debug)]
pub struct Burn {
    /// Normalised 0.0–1.0 samples, oldest first.
    pub samples: Vec<f32>,
    pub note: SharedString,
    pub trend: Trend,
}

impl Burn {
    pub fn new(samples: Vec<f32>, note: impl Into<SharedString>, trend: Trend) -> Self {
        Self {
            samples,
            note: note.into(),
            trend,
        }
    }
}

/// A tracked subscription and its current state.
#[derive(Clone, Debug)]
pub struct Provider {
    /// Stable key used for element ids and preference lookups.
    pub id: SharedString,
    pub name: SharedString,
    /// Single-letter mark shown in the row's rounded badge.
    pub badge: SharedString,
    /// Badge fill / badge glyph colours, as `0xRRGGBB`.
    pub badge_bg: u32,
    pub badge_fg: u32,
    /// Plan name shown under Connected accounts, e.g. `"Max 20×"`.
    pub plan: SharedString,
    /// Where "Console ↗" points.
    pub console_url: SharedString,
    /// At least one entry; `limits[0]` is the headline limit.
    pub limits: Vec<Limit>,
    pub burn: Burn,
}

impl Provider {
    pub fn primary(&self) -> &Limit {
        &self.limits[0]
    }

    /// The limits shown only once the row is expanded.
    pub fn secondary(&self) -> &[Limit] {
        &self.limits[1..]
    }

    /// Compact subtitle line with the active window and reset time.
    pub fn subtitle(&self) -> String {
        let primary = self.primary();
        let window = match primary.cadence {
            Cadence::Session => "5h session",
            Cadence::Daily => "Daily",
            Cadence::Weekly => "Weekly",
            Cadence::Monthly => "Monthly",
        };
        match &primary.resets_at {
            Some(at) => format!("{window} \u{00b7} reset {at}"),
            None => window.to_string(),
        }
    }
}

/// User-adjustable settings, mirroring the Preferences pane.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefs {
    pub show_percentage_in_menu_bar: bool,
    /// Collapse weekly/monthly and surface only the headline limit.
    pub only_show_active_limit: bool,
    pub launch_at_login: bool,
    /// Consumed-percentage threshold at which a limit turns amber.
    pub warn_at: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            show_percentage_in_menu_bar: true,
            only_show_active_limit: false,
            launch_at_login: crate::autostart::is_enabled(),
            warn_at: 80.0,
        }
    }
}

/// Which pane the popover is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Usage,
    Prefs,
}
