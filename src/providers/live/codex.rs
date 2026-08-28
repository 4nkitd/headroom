use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow, bail};
use serde_json::Value;

use super::http::{json_number, request};
use crate::credentials::{self, CodexCredentials};
use crate::model::{Cadence, Limit, Provider};
use crate::providers::{SourceDescriptor, UsageSource};

const USAGE_URLS: &[&str] = &[
    "https://chatgpt.com/backend-api/wham/usage",
    "https://chatgpt.com/backend-api/codex/usage",
];
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

pub struct OpenAiCodex;

impl UsageSource for OpenAiCodex {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "openai-codex",
            name: "OpenAI Codex",
            logo: "providers/codex.svg",
            badge: "X",
            badge_bg: 0xf5f5f7,
            badge_fg: 0x111111,
            setup_label: "Open Codex",
            setup_url: Some("https://chatgpt.com/codex"),
        }
    }

    fn fetch(&self) -> Result<Provider> {
        let mut credentials = credentials::codex_credentials()?;
        if credentials.expires_at > 0
            && now_millis() >= credentials.expires_at - 60_000
            && !credentials.refresh_token.is_empty()
        {
            credentials = refresh_credentials(&credentials)?;
        }

        let response = fetch_usage(&credentials)?;
        if response.status == 401 && !credentials.refresh_token.is_empty() {
            credentials = refresh_credentials(&credentials)?;
            return parse_success(fetch_usage(&credentials)?);
        }
        parse_success(response)
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as i64
}

fn refresh_credentials(current: &CodexCredentials) -> Result<CodexCredentials> {
    let body = serde_json::json!({
        "client_id": CLIENT_ID,
        "grant_type": "refresh_token",
        "refresh_token": current.refresh_token,
    });
    let response = request(
        TOKEN_URL,
        &[("Content-Type", "application/json".into())],
        Some(body.to_string()),
    )?;
    if !(200..300).contains(&response.status) {
        bail!("Codex OAuth refresh returned HTTP {}", response.status);
    }
    let value: Value = serde_json::from_str(&response.body)
        .map_err(|_| anyhow!("Codex OAuth response was not JSON"))?;
    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Codex OAuth response is missing an access token"))?
        .to_string();
    let refreshed = CodexCredentials {
        access_token,
        refresh_token: value
            .get("refresh_token")
            .and_then(Value::as_str)
            .unwrap_or(&current.refresh_token)
            .to_string(),
        id_token: value
            .get("id_token")
            .and_then(Value::as_str)
            .unwrap_or(&current.id_token)
            .to_string(),
        account_id: current.account_id.clone(),
        expires_at: now_millis()
            + value
                .get("expires_in")
                .and_then(Value::as_i64)
                .unwrap_or(3600)
                * 1000,
    };
    credentials::save_codex_credentials(&refreshed)?;
    Ok(refreshed)
}

fn fetch_usage(credentials: &CodexCredentials) -> Result<super::http::HttpResponse> {
    let headers = [
        (
            "Authorization",
            format!("Bearer {}", credentials.access_token),
        ),
        ("ChatGPT-Account-Id", credentials.account_id.clone()),
        ("Accept", "application/json".into()),
        ("originator", "codex_cli_rs".into()),
    ];
    let mut last_response = None;
    for url in USAGE_URLS {
        let response = request(url, &headers, None)?;
        if response.status != 404 {
            return Ok(response);
        }
        last_response = Some(response);
    }
    last_response.ok_or_else(|| anyhow!("Codex usage API unavailable"))
}

fn parse_success(response: super::http::HttpResponse) -> Result<Provider> {
    if !(200..300).contains(&response.status) {
        bail!("Codex usage API returned HTTP {}", response.status);
    }
    let value: Value = serde_json::from_str(&response.body)
        .map_err(|_| anyhow!("Codex usage response was not JSON"))?;
    parse_usage(&value)
}

fn parse_usage(value: &Value) -> Result<Provider> {
    let mut limits = Vec::new();
    append_rate_limit(value.get("rate_limit"), None, &mut limits);
    append_rate_limit(
        value.get("code_review_rate_limit"),
        Some("Code review"),
        &mut limits,
    );
    if let Some(additional) = value
        .get("additional_rate_limits")
        .and_then(Value::as_array)
    {
        for entry in additional {
            let label = entry
                .get("limit_name")
                .and_then(Value::as_str)
                .or_else(|| entry.get("metered_feature").and_then(Value::as_str));
            append_rate_limit(entry.get("rate_limit"), label, &mut limits);
        }
    }
    if limits.is_empty() {
        bail!("Codex usage response has no rate-limit windows");
    }
    Ok(Provider {
        id: "openai-codex".into(),
        name: "OpenAI Codex".into(),
        logo: "providers/codex.svg".into(),
        badge: "X".into(),
        badge_bg: 0xf5f5f7,
        badge_fg: 0x111111,
        plan: plan_label(value.get("plan_type").and_then(Value::as_str)).into(),
        console_url: "https://chatgpt.com/codex/settings/usage".into(),
        source_label: "OpenAI Codex HTTP API".into(),
        limits,
    })
}

fn append_rate_limit(value: Option<&Value>, label: Option<&str>, limits: &mut Vec<Limit>) {
    let Some(value) = value else {
        return;
    };
    for window in [value.get("primary_window"), value.get("secondary_window")]
        .into_iter()
        .flatten()
    {
        if let Some(limit) = parse_window(window, label) {
            limits.push(limit);
        }
    }
}

fn parse_window(value: &Value, label: Option<&str>) -> Option<Limit> {
    let used = json_number(value.get("used_percent"))?;
    let window_seconds = json_number(value.get("limit_window_seconds")).unwrap_or(5.0 * 3600.0);
    let cadence = if window_seconds <= 6.0 * 3600.0 {
        Cadence::Session
    } else if window_seconds <= 2.0 * 86400.0 {
        Cadence::Daily
    } else if window_seconds <= 9.0 * 86400.0 {
        Cadence::Weekly
    } else {
        Cadence::Monthly
    };
    let mut limit = Limit::new(cadence, (100.0 - used).clamp(0.0, 100.0) as f32);
    if let Some(label) = label {
        limit = limit.label(format_model_label(label));
    }
    if let Some(reset) = super::parse_reset_time(value.get("reset_at")) {
        limit = limit.resets_at(reset);
    }
    Some(limit)
}

fn plan_label(plan: Option<&str>) -> String {
    match plan.unwrap_or("ChatGPT") {
        "pro" | "prolite" => "ChatGPT Pro".into(),
        "plus" => "ChatGPT Plus".into(),
        "team" | "business" => "ChatGPT Business".into(),
        other => format!("ChatGPT {}", format_model_label(other)),
    }
}

fn format_model_label(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            if matches!(part.to_ascii_lowercase().as_str(), "gpt" | "api") {
                return part.to_ascii_uppercase();
            }
            let mut characters = part.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::parse_usage;

    #[test]
    fn parses_chatgpt_pro_usage_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/codex_usage.json")).unwrap();
        let provider = parse_usage(&fixture).unwrap();
        assert_eq!(provider.plan.as_ref(), "ChatGPT Pro");
        assert_eq!(provider.primary().cadence, crate::model::Cadence::Session);
        assert_eq!(provider.primary().percent_left, 73.0);
        assert_eq!(
            provider.secondary()[0].cadence,
            crate::model::Cadence::Weekly
        );
        assert_eq!(provider.secondary()[0].percent_left, 81.0);
        assert_eq!(
            provider.secondary()[1].display_label(),
            "GPT 5.3 Codex Spark"
        );
    }
}
