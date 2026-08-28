use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Local, Utc};
use serde_json::Value;

use super::http::{request, url_encode};
use crate::credentials;
use crate::model::{Cadence, Limit, Provider};
use crate::providers::{SourceDescriptor, UsageSource};

const OAUTH_TOKEN_URL: &str = "https://claude.ai/v1/oauth/token";
const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

static CREDS: OnceLock<Mutex<Option<credentials::ClaudeCredentials>>> = OnceLock::new();

pub struct ClaudeCode;

impl UsageSource for ClaudeCode {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "claude-code",
            name: "Claude Code",
            logo: "providers/claude.svg",
            badge: "C",
            badge_bg: 0xd97757,
            badge_fg: 0x2b1206,
            setup_label: "Open Claude",
            setup_url: Some("https://claude.ai/login"),
        }
    }

    fn fetch(&self) -> Result<Provider> {
        fetch_usage()
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as i64
}

fn read_credentials() -> Result<credentials::ClaudeCredentials> {
    let cache = CREDS.get_or_init(|| Mutex::new(None));
    if let Ok(cache) = cache.lock()
        && let Some(creds) = cache.clone()
    {
        return Ok(creds);
    }
    let creds = credentials::claude_credentials()?;
    if let Ok(mut cache) = cache.lock() {
        *cache = Some(creds.clone());
    }
    Ok(creds)
}

fn invalidate_credentials() {
    if let Ok(mut cache) = CREDS.get_or_init(|| Mutex::new(None)).lock() {
        *cache = None;
    }
}

fn refresh_token(refresh_token: &str) -> Result<credentials::ClaudeCredentials> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        url_encode(refresh_token),
        url_encode(OAUTH_CLIENT_ID)
    );
    let response = request(
        OAUTH_TOKEN_URL,
        &[
            ("Content-Type", "application/x-www-form-urlencoded".into()),
            (
                "User-Agent",
                "claude-cli/2.1.112 (external, sdk-cli)".into(),
            ),
        ],
        Some(body),
    )?;
    if !(200..300).contains(&response.status) {
        bail!("OAuth refresh failed HTTP {}", response.status);
    }
    let value: Value = serde_json::from_str(&response.body)?;
    let access = value
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("OAuth response has no access token"))?
        .to_string();
    let refresh = value
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or(refresh_token)
        .to_string();
    let expires_in = value
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(28800);
    let credentials = credentials::ClaudeCredentials {
        access_token: access,
        refresh_token: refresh,
        expires_at: now_millis() + expires_in * 1000,
    };
    credentials::save_claude_credentials(&credentials);
    if let Ok(mut cache) = CREDS.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(credentials.clone());
    }
    Ok(credentials)
}

fn fetch_usage() -> Result<Provider> {
    let mut credentials = read_credentials()?;
    if (credentials.access_token.is_empty() || now_millis() >= credentials.expires_at - 60_000)
        && !credentials.refresh_token.is_empty()
        && let Ok(refreshed) = refresh_token(&credentials.refresh_token)
    {
        credentials = refreshed;
    }
    if credentials.access_token.is_empty() {
        bail!("Claude Code credentials missing access token");
    }

    let mut response = query_headers(&credentials.access_token);
    if matches!(&response, Ok(response) if response.status == 401)
        && !credentials.refresh_token.is_empty()
    {
        invalidate_credentials();
        if let Ok(refreshed) = refresh_token(&credentials.refresh_token) {
            credentials = refreshed;
            response = query_headers(&credentials.access_token);
        }
    }
    let response = response?;
    if !(200..300).contains(&response.status) {
        bail!("Claude HTTP probe returned HTTP {}", response.status);
    }
    parse_rate_limit_headers(&response.headers)
}

fn query_headers(access_token: &str) -> Result<super::http::HttpResponse> {
    let headers = [
        ("Authorization", format!("Bearer {access_token}")),
        ("Content-Type", "application/json".into()),
        ("Accept", "application/json".into()),
        ("anthropic-version", "2023-06-01".into()),
        (
            "anthropic-beta",
            "claude-code-20250219,oauth-2025-04-20".into(),
        ),
        (
            "user-agent",
            "claude-cli/2.1.112 (external, sdk-cli)".into(),
        ),
        ("x-app", "cli".into()),
    ];
    let body = serde_json::json!({
        "model": "claude-haiku-4-5",
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "hi"}]
    });
    request(MESSAGES_URL, &headers, Some(body.to_string()))
}

fn format_reset(timestamp: i64) -> Option<String> {
    let date = if timestamp > 1_000_000_000_000 {
        DateTime::<Utc>::from_timestamp(timestamp / 1000, 0)?.with_timezone(&Local)
    } else {
        DateTime::<Utc>::from_timestamp(timestamp, 0)?.with_timezone(&Local)
    };
    if date.date_naive() == Local::now().date_naive() {
        Some(date.format("%-I:%M%P").to_string())
    } else {
        Some(date.format("%b %-d %-I:%M%P").to_string())
    }
}

fn parse_rate_limit_headers(headers: &HashMap<String, String>) -> Result<Provider> {
    let session_usage = headers
        .get("anthropic-ratelimit-unified-5h-utilization")
        .and_then(|value| value.parse::<f32>().ok())
        .ok_or_else(|| anyhow!("missing 5h rate limit utilization header"))?;
    let session_reset = headers
        .get("anthropic-ratelimit-unified-5h-reset")
        .and_then(|value| value.parse::<i64>().ok())
        .and_then(format_reset);
    let mut limits = vec![
        Limit::new(
            Cadence::Session,
            (1.0 - session_usage.clamp(0.0, 1.0)) * 100.0,
        )
        .resets_at(session_reset.unwrap_or_else(|| "unknown".into())),
    ];
    if let Some(weekly_usage) = headers
        .get("anthropic-ratelimit-unified-7d-utilization")
        .and_then(|value| value.parse::<f32>().ok())
    {
        let reset = headers
            .get("anthropic-ratelimit-unified-7d-reset")
            .and_then(|value| value.parse::<i64>().ok())
            .and_then(format_reset);
        limits.push(
            Limit::new(
                Cadence::Weekly,
                (1.0 - weekly_usage.clamp(0.0, 1.0)) * 100.0,
            )
            .resets_at(reset.unwrap_or_else(|| "unknown".into())),
        );
    }
    Ok(Provider {
        id: "claude-code".into(),
        name: "Claude Code".into(),
        logo: "providers/claude.svg".into(),
        badge: "C".into(),
        badge_bg: 0xd97757,
        badge_fg: 0x2b1206,
        plan: "Subscription".into(),
        console_url: "https://claude.ai/settings/usage".into(),
        source_label: "Anthropic HTTP API headers".into(),
        limits,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::parse_rate_limit_headers;

    #[test]
    fn parses_rate_limit_fixture() {
        let fixture: HashMap<String, String> =
            serde_json::from_str(include_str!("../fixtures/claude_headers.json")).unwrap();
        let provider = parse_rate_limit_headers(&fixture).unwrap();
        assert!((provider.primary().percent_left - 85.0).abs() < 0.01);
        assert!((provider.secondary()[0].percent_left - 60.0).abs() < 0.01);
    }
}
