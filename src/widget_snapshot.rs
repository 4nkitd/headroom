//! Export sanitized `WidgetSnapshot.json` for macOS WidgetKit targets.
//!
//! Exposes no OAuth tokens or secret keys. Written to the App Group container
//! `group.in.4nkitd.headroom` after every successful state sync.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::model::truncate_account_label;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WidgetAccount {
    pub provider_id: String,
    pub provider_name: String,
    pub badge: String,
    pub badge_bg: u32,
    pub badge_fg: u32,
    pub account_label: String,
    pub is_primary: bool,
    pub percent_left: f32,
    pub cadence: String,
    pub resets_at: Option<String>,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WidgetSnapshot {
    pub synced_at: String,
    pub accounts: Vec<WidgetAccount>,
}

pub fn app_group_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|dir| dir.join("Library/Group Containers/group.in.4nkitd.headroom"))
}

pub fn widget_sandbox_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|dir| dir.join("Library/Containers/com.4nkitd.headroom.widget/Data"))
}

pub fn export(state: &AppState) {
    let accounts = state
        .providers
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            let primary = p.primary();
            let label = truncate_account_label(p.id.as_ref());
            let status = if primary.percent_left < 15.0 {
                "critical"
            } else if primary.percent_left < 35.0 {
                "warn"
            } else {
                "ok"
            };

            let is_primary = state
                .prefs
                .primary_account_id
                .as_ref()
                .map(|p_id| p_id == p.id.as_ref())
                .unwrap_or(idx == 0 && p.id.starts_with("antigravity:"));

            WidgetAccount {
                provider_id: p.id.to_string(),
                provider_name: p.name.to_string(),
                badge: p.badge.to_string(),
                badge_bg: p.badge_bg,
                badge_fg: p.badge_fg,
                account_label: label,
                is_primary,
                percent_left: primary.percent_left,
                cadence: primary.display_label().to_string(),
                resets_at: primary.resets_at.as_ref().map(|s| s.to_string()),
                status: status.to_string(),
            }
        })
        .collect::<Vec<_>>();

    let snapshot = WidgetSnapshot {
        synced_at: state
            .last_sync
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| chrono::Local::now().to_rfc3339()),
        accounts,
    };

    let Ok(bytes) = serde_json::to_vec_pretty(&snapshot) else {
        return;
    };

    let mut dirs_to_write = Vec::new();
    if let Some(dir) = app_group_dir() {
        dirs_to_write.push(dir);
    }
    if let Some(dir) = widget_sandbox_dir() {
        dirs_to_write.push(dir);
    }

    for dir in dirs_to_write {
        if fs::create_dir_all(&dir).is_ok() {
            let target_path = dir.join("WidgetSnapshot.json");
            let temp_path = dir.join("WidgetSnapshot.json.tmp");
            if fs::write(&temp_path, &bytes).is_ok() {
                let _ = fs::rename(temp_path, &target_path);
                eprintln!(
                    "headroom: exported widget snapshot to {}",
                    target_path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_snapshot_serialization() {
        let snapshot = WidgetSnapshot {
            synced_at: "2026-09-05T08:18:00Z".into(),
            accounts: vec![WidgetAccount {
                provider_id: "antigravity:4nkitd@gmail.com".into(),
                provider_name: "Antigravity · 4nkit".into(),
                badge: "G".into(),
                badge_bg: 0x4285f4,
                badge_fg: 0xffffff,
                account_label: "4nkit".into(),
                is_primary: true,
                percent_left: 77.9,
                cadence: "Gemini 5h".into(),
                resets_at: Some("11:55".into()),
                status: "ok".into(),
            }],
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("4nkit"));
        assert!(json.contains("77.9"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("token"));
    }
}
