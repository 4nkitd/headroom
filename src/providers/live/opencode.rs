use anyhow::{Result, anyhow};
use serde_json::Value;

use super::http::{json_number, request};
use crate::credentials;
use crate::model::{Cadence, Limit, Provider};
use crate::providers::{SourceDescriptor, UsageSource};

const USAGE_URLS: &[&str] = &[
    "https://opencode.ai/zen/go/v1/usage",
    "https://opencode.ai/api/v1/usage/plan",
];

pub struct OpenCodeGo;

impl UsageSource for OpenCodeGo {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "opencode-go",
            name: "OpenCode Go",
            logo: "providers/opencode.svg",
            badge: "O",
            badge_bg: 0xe8e8ea,
            badge_fg: 0x16171a,
            setup_label: "Enter API key",
            setup_url: None,
        }
    }

    fn fetch(&self) -> Result<Vec<Provider>> {
        let accounts = credentials::opencode_go_accounts();
        if accounts.is_empty() {
            return Err(anyhow!("OpenCode Go API key not configured"));
        }
        let mut providers = Vec::new();
        for account in accounts {
            let mut provider = fetch_plan(&account.key)?;
            provider.id = format!("opencode-go:{}", account.label).into();
            provider.name = format!("OpenCode Go · {}", account.label).into();
            providers.push(provider);
        }
        Ok(providers)
    }
}

fn fetch_plan(key: &str) -> Result<Provider> {
    let headers = [
        ("Authorization", format!("Bearer {key}")),
        ("Accept", "application/json".into()),
    ];
    let mut last_error = None;
    for url in USAGE_URLS {
        let response = match request(url, &headers, None) {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        if !(200..300).contains(&response.status) {
            last_error = Some(anyhow!("OpenCode Go API returned HTTP {}", response.status));
            continue;
        }
        let value: Value = match serde_json::from_str(&response.body) {
            Ok(value) => value,
            Err(error) => {
                last_error = Some(anyhow!("OpenCode Go response was not JSON: {error}"));
                continue;
            }
        };
        if let Ok(provider) = parse_plan(&value) {
            return Ok(provider);
        }
        if let Ok(provider) = parse_usage(&value) {
            return Ok(provider);
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("OpenCode Go usage API unavailable")))
}

fn parse_plan(response: &Value) -> Result<Provider> {
    let windows = response
        .get("windows")
        .ok_or_else(|| anyhow!("usage plan has no windows"))?;
    Ok(provider(
        response
            .get("plan")
            .and_then(Value::as_str)
            .unwrap_or("go")
            .to_ascii_uppercase(),
        vec![
            parse_plan_window(windows.get("rolling"), Cadence::Session)?,
            parse_plan_window(windows.get("weekly"), Cadence::Weekly)?,
            parse_plan_window(windows.get("monthly"), Cadence::Monthly)?,
        ],
    ))
}

fn parse_plan_window(value: Option<&Value>, cadence: Cadence) -> Result<Limit> {
    let window = value.ok_or_else(|| anyhow!("usage plan is missing {cadence:?} window"))?;
    let used = json_number(window.get("usage_percent"))
        .ok_or_else(|| anyhow!("usage plan is missing {cadence:?} usage"))?;
    let resets = json_number(window.get("resets_in_seconds"))
        .ok_or_else(|| anyhow!("usage plan is missing {cadence:?} reset"))?;
    Ok(Limit::new(cadence, (100.0 - used).clamp(0.0, 100.0) as f32)
        .resets_at(format_reset_seconds(resets.max(0.0) as u64)))
}

fn parse_usage(response: &Value) -> Result<Provider> {
    let windows = response
        .get("usage")
        .ok_or_else(|| anyhow!("usage response has no usage object"))?;
    Ok(provider(
        "GO".into(),
        vec![
            parse_usage_window(windows.get("rolling"), Cadence::Session)?,
            parse_usage_window(windows.get("weekly"), Cadence::Weekly)?,
            parse_usage_window(windows.get("monthly"), Cadence::Monthly)?,
        ],
    ))
}

fn parse_usage_window(value: Option<&Value>, cadence: Cadence) -> Result<Limit> {
    let window = value.ok_or_else(|| anyhow!("usage response is missing {cadence:?} window"))?;
    let used = json_number(window.get("percent"))
        .ok_or_else(|| anyhow!("usage response is missing {cadence:?} usage"))?;
    let reset = super::parse_reset_time(window.get("resetsAt").or_else(|| window.get("resetAt")))
        .ok_or_else(|| anyhow!("usage response is missing {cadence:?} reset"))?;
    Ok(Limit::new(cadence, (100.0 - used).clamp(0.0, 100.0) as f32).resets_at(reset))
}

fn provider(plan: String, limits: Vec<Limit>) -> Provider {
    Provider {
        id: "opencode-go".into(),
        name: "OpenCode Go".into(),
        logo: "providers/opencode.svg".into(),
        badge: "O".into(),
        badge_bg: 0xe8e8ea,
        badge_fg: 0x16171a,
        plan: plan.into(),
        console_url: "https://opencode.ai/auth".into(),
        source_label: "OpenCode Go HTTP API".into(),
        limits,
    }
}

fn format_reset_seconds(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{}d {}h", seconds / 86400, (seconds % 86400) / 3600)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{parse_plan, parse_usage};

    #[test]
    fn parses_plan_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/opencode_plan.json")).unwrap();
        let provider = parse_plan(&fixture).unwrap();
        assert_eq!(provider.primary().percent_left, 35.0);
        assert_eq!(provider.secondary()[0].percent_left, 70.0);
        assert_eq!(provider.secondary()[1].percent_left, 88.0);
    }

    #[test]
    fn parses_usage_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/opencode_usage.json")).unwrap();
        let provider = parse_usage(&fixture).unwrap();
        assert_eq!(provider.primary().percent_left, 24.0);
        assert_eq!(provider.secondary()[0].percent_left, 51.0);
        assert_eq!(provider.secondary()[1].percent_left, 66.0);
    }
}
