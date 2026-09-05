//! Domain types: what a tracked subscription looks like and how much of it is
//! left. Deliberately provider-agnostic — every backend in `providers/`
//! normalises into these.

use std::collections::HashSet;

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
    /// Provider-specific bucket name when cadence alone is ambiguous.
    pub label: Option<SharedString>,
    /// Headroom remaining, 0.0–100.0.
    pub percent_left: f32,
    /// Human-readable reset time, e.g. `"11:55"`. `None` when unknown.
    pub resets_at: Option<SharedString>,
}

impl Limit {
    pub fn new(cadence: Cadence, percent_left: f32) -> Self {
        Self {
            cadence,
            label: None,
            percent_left: percent_left.clamp(0.0, 100.0),
            resets_at: None,
        }
    }

    pub fn resets_at(mut self, at: impl Into<SharedString>) -> Self {
        self.resets_at = Some(at.into());
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn display_label(&self) -> &str {
        self.label
            .as_ref()
            .map(|label| label.as_ref())
            .unwrap_or_else(|| self.cadence.label())
    }

    /// Fraction of the bucket remaining — what the progress bar fills to match preview.html.
    pub fn fraction_left(&self) -> f32 {
        (self.percent_left / 100.0).clamp(0.0, 1.0)
    }
}

/// A tracked subscription and its current state.
#[derive(Clone, Debug)]
pub struct Provider {
    /// Stable key used for element ids and preference lookups.
    pub id: SharedString,
    pub name: SharedString,
    /// Embedded brand asset shown in provider rows.
    pub logo: SharedString,
    /// Single-letter fallback if the brand asset cannot be decoded.
    pub badge: SharedString,
    /// Badge fill / badge glyph colours, as `0xRRGGBB`.
    pub badge_bg: u32,
    pub badge_fg: u32,
    /// Plan name shown under Connected accounts, e.g. `"Max 20×"`.
    pub plan: SharedString,
    /// Where "Console ↗" points.
    pub console_url: SharedString,
    /// Human-readable provenance for the usage value.
    pub source_label: SharedString,
    /// At least one entry; `limits[0]` is the headline limit.
    pub limits: Vec<Limit>,
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
    #[allow(dead_code)]
    pub fn subtitle(&self) -> String {
        let primary = self.primary();
        let window = primary
            .label
            .as_ref()
            .map(|label| label.as_ref())
            .unwrap_or(match primary.cadence {
                Cadence::Session => "5h session",
                Cadence::Daily => "Daily",
                Cadence::Weekly => "Weekly",
                Cadence::Monthly => "Monthly",
            });
        match &primary.resets_at {
            Some(at) => format!("{window} \u{00b7} {at}"),
            None => window.to_string(),
        }
    }
}

pub fn truncate_account_label(label: &str) -> String {
    let raw = label.rsplit(':').next().unwrap_or(label);
    if raw.contains('@') {
        let name = raw.split('@').next().unwrap_or(raw);
        name.chars().take(5).collect()
    } else if raw == "openai-codex" {
        "Codex".into()
    } else if raw == "claude-code" {
        "Claude".into()
    } else if raw == "OpenCode Go" || raw == "opencode-go" {
        "OpenCode".into()
    } else {
        raw.chars().take(8).collect()
    }
}

fn default_true() -> bool {
    true
}

/// User-adjustable settings, mirroring the Preferences pane.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefs {
    pub show_percentage_in_menu_bar: bool,
    /// Collapse weekly/monthly and surface only the headline limit.
    pub only_show_active_limit: bool,
    #[serde(default = "default_true")]
    pub enable_notch_hud: bool,
    pub launch_at_login: bool,
    /// Consumed-percentage threshold at which a limit turns amber.
    pub warn_at: f32,
    #[serde(default)]
    pub primary_account_id: Option<String>,
    #[serde(default)]
    pub disabled_integrations: HashSet<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            show_percentage_in_menu_bar: true,
            only_show_active_limit: false,
            enable_notch_hud: true,
            launch_at_login: crate::autostart::is_enabled(),
            warn_at: 80.0,
            primary_account_id: None,
            disabled_integrations: HashSet::new(),
        }
    }
}

/// Which pane the popover is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Usage,
    Prefs,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_account_label() {
        assert_eq!(truncate_account_label("4nkitd@gmail.com"), "4nkit");
        assert_eq!(
            truncate_account_label("antigravity:3anjuy@gmail.com"),
            "3anju"
        );
        assert_eq!(truncate_account_label("opencode-go:Work"), "Work");
        assert_eq!(truncate_account_label("opencode-go"), "OpenCode");
    }

    #[test]
    fn old_preferences_default_to_all_integrations_enabled() {
        let prefs: Prefs = serde_json::from_str(
            r#"{"show_percentage_in_menu_bar":true,"only_show_active_limit":false,"launch_at_login":false,"warn_at":80}"#,
        )
        .unwrap();
        assert!(prefs.disabled_integrations.is_empty());
    }
}
