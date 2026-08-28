use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{Local, SecondsFormat};
use serde::Serialize;

use crate::app_state::{AppState, IntegrationStatus};

pub const RELEASE_CHANNEL: &str = "stable";

#[derive(Serialize)]
struct SupportReport {
    generated_at: String,
    app: AppInfo,
    preferences: PreferenceInfo,
    credentials: BTreeMap<&'static str, String>,
    integrations: Vec<IntegrationInfo>,
    runtime: RuntimeInfo,
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    release_channel: &'static str,
    os: &'static str,
    architecture: &'static str,
}

#[derive(Serialize)]
struct PreferenceInfo {
    show_percentage_in_menu_bar: bool,
    only_show_active_limit: bool,
    launch_at_login: bool,
    warn_at: f32,
    disabled_integrations: Vec<String>,
}

#[derive(Serialize)]
struct IntegrationInfo {
    id: String,
    enabled: bool,
    state: &'static str,
    latency_ms: Option<u64>,
    last_success: Option<String>,
    consecutive_failures: u32,
    retry_at: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct RuntimeInfo {
    refreshing: bool,
    last_refresh_duration_ms: Option<u64>,
    last_sync: Option<String>,
}

pub fn report_json(state: &AppState) -> Result<String> {
    let mut disabled = state
        .prefs
        .disabled_integrations
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    disabled.sort();
    let report = SupportReport {
        generated_at: Local::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        app: AppInfo {
            version: env!("CARGO_PKG_VERSION"),
            release_channel: RELEASE_CHANNEL,
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        },
        preferences: PreferenceInfo {
            show_percentage_in_menu_bar: state.prefs.show_percentage_in_menu_bar,
            only_show_active_limit: state.prefs.only_show_active_limit,
            launch_at_login: state.prefs.launch_at_login,
            warn_at: state.prefs.warn_at,
            disabled_integrations: disabled,
        },
        credentials: BTreeMap::from([
            (
                "antigravity",
                crate::credentials::antigravity_credentials_status().into(),
            ),
            (
                "claude-code",
                crate::credentials::claude_credentials_status().into(),
            ),
            (
                "openai-codex",
                crate::credentials::codex_credentials_status().into(),
            ),
            ("opencode-go", crate::credentials::opencode_go_key_status()),
        ]),
        integrations: state
            .integrations
            .iter()
            .map(|status| integration_info(state, status))
            .collect(),
        runtime: RuntimeInfo {
            refreshing: state.is_refreshing,
            last_refresh_duration_ms: state.last_refresh_duration_ms,
            last_sync: state
                .last_sync
                .map(|time| time.to_rfc3339_opts(SecondsFormat::Secs, true)),
        },
    };
    serde_json::to_string_pretty(&report).context("could not serialize support report")
}

fn integration_info(state: &AppState, status: &IntegrationStatus) -> IntegrationInfo {
    let enabled = state.integration_enabled(status.id.as_ref());
    let has_provider = state
        .providers
        .iter()
        .any(|provider| provider.id == status.id);
    let state_label = if !enabled {
        "disabled"
    } else if status.error.is_some() && has_provider {
        "cached"
    } else if status.needs_setup() {
        "setup-required"
    } else if status.error.is_some() {
        "error"
    } else if has_provider {
        "live"
    } else {
        "loading"
    };
    IntegrationInfo {
        id: status.id.to_string(),
        enabled,
        state: state_label,
        latency_ms: status.latency_ms,
        last_success: status
            .last_success
            .map(|time| time.to_rfc3339_opts(SecondsFormat::Secs, true)),
        consecutive_failures: status.consecutive_failures,
        retry_at: status
            .retry_at
            .map(|time| time.to_rfc3339_opts(SecondsFormat::Secs, true)),
        error: status.error.as_ref().map(|error| redact(error)),
    }
}

fn redact(value: &str) -> String {
    let mut value = value.to_string();
    if let Some(home) = dirs::home_dir().and_then(|path| path.to_str().map(str::to_string)) {
        value = value.replace(&home, "~");
    }
    let mut redact_next = false;
    let sanitized = value
        .split_whitespace()
        .map(|word| {
            if redact_next {
                redact_next = false;
                return "<redacted>".to_string();
            }
            if word.eq_ignore_ascii_case("bearer")
                || word.eq_ignore_ascii_case("token")
                || word.eq_ignore_ascii_case("key")
            {
                redact_next = true;
                return word.to_string();
            }
            let opaque = word.len() >= 32
                && word.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "-_.".contains(character)
                });
            if opaque {
                "<redacted>".to_string()
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    sanitized.chars().take(300).collect()
}

pub fn export(state: &AppState) -> Result<PathBuf> {
    let directory = dirs::download_dir()
        .or_else(dirs::desktop_dir)
        .or_else(dirs::home_dir)
        .context("no export directory is available")?;
    let path = directory.join(format!(
        "headroom-support-{}.json",
        Local::now().format("%Y%m%d-%H%M%S")
    ));
    fs::write(&path, report_json(state)?)
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(path)
}

pub fn static_report_json() -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "app": {
            "version": env!("CARGO_PKG_VERSION"),
            "release_channel": RELEASE_CHANNEL,
            "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH
        },
        "credentials": {
            "claude-code": crate::credentials::claude_credentials_status(),
            "openai-codex": crate::credentials::codex_credentials_status(),
            "opencode-go": crate::credentials::opencode_go_key_status(),
            "antigravity": crate::credentials::antigravity_credentials_status()
        },
        "note": "No credential values are included. Open the app to export runtime integration state."
    }))
    .context("could not serialize diagnostics")
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redacts_bearer_tokens_and_opaque_values() {
        let report =
            redact("request failed Bearer sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789");
        assert_eq!(report, "request failed Bearer <redacted>");
        assert!(!report.contains("abcdefghijklmnopqrstuvwxyz"));
    }
}
