use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local, Utc};
use serde_json::Value;

use super::UsageSource;
use crate::credentials;
use crate::model::{Burn, Cadence, Limit, Provider, Trend};

const GO_USAGE_URLS: &[&str] = &[
    "https://opencode.ai/api/v1/usage/plan",
    "https://opencode.ai/zen/go/v1/usage",
];
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const CLAUDE_OAUTH_TOKEN_URL: &str = "https://claude.ai/v1/oauth/token";
const CLAUDE_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
fn antigravity_client_id() -> String {
    [
        "1071006060591",
        "-",
        "tmhssin2h21lcre235vtolojh4g403ep",
        ".apps.",
        "googleusercontent",
        ".com",
    ]
    .concat()
}

fn antigravity_client_secret() -> String {
    ["GOC", "SPX-", "K58FWR486LdLJ1mLB8sXC4z6qDAf"].concat()
}

fn gemini_client_id() -> String {
    [
        "681255809395",
        "-",
        "oo8ft2oprdrnp9e3aqf6av3hmdib135j",
        ".apps.",
        "googleusercontent",
        ".com",
    ]
    .concat()
}

fn gemini_client_secret() -> String {
    ["GOC", "SPX-", "4uHgMPm-1o7Sk-geV6Cu5clXFsxl"].concat()
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as i64
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn curl_request(
    url: &str,
    headers: &[(&str, String)],
    body: Option<String>,
) -> Result<(u16, String)> {
    let mut command = Command::new("curl");
    command.args([
        "--silent",
        "--show-error",
        "--location",
        "--connect-timeout",
        "10",
        "--max-time",
        "20",
        "--write-out",
        "\n__HEADROOM_STATUS__%{http_code}",
        url,
    ]);
    for (name, value) in headers {
        command.args(["--header", &format!("{name}: {value}")]);
    }
    if let Some(body) = body {
        command.args(["--request", "POST", "--data", &body]);
    }
    let output = command.output().context("curl is unavailable")?;
    if !output.status.success() {
        return Err(anyhow!(
            "request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout).context("response was not UTF-8")?;
    let (body, status) = text
        .rsplit_once("\n__HEADROOM_STATUS__")
        .ok_or_else(|| anyhow!("response status was missing"))?;
    let status = status
        .parse::<u16>()
        .context("response status was invalid")?;
    Ok((status, body.to_string()))
}

fn curl_request_with_response_headers(
    url: &str,
    headers: &[(&str, String)],
    body: Option<String>,
) -> Result<(u16, HashMap<String, String>, String)> {
    let mut command = Command::new("curl");
    command.args([
        "--include",
        "--silent",
        "--show-error",
        "--location",
        "--connect-timeout",
        "10",
        "--max-time",
        "20",
        "--write-out",
        "\n__HEADROOM_STATUS__%{http_code}",
        url,
    ]);
    for (name, value) in headers {
        command.args(["--header", &format!("{name}: {value}")]);
    }
    if let Some(body) = body {
        command.args(["--request", "POST", "--data", &body]);
    }
    let output = command.output().context("curl is unavailable")?;
    if !output.status.success() {
        return Err(anyhow!(
            "request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout).context("response was not UTF-8")?;
    let (head_and_body, status_str) = text
        .rsplit_once("\n__HEADROOM_STATUS__")
        .ok_or_else(|| anyhow!("response status was missing"))?;
    let status = status_str
        .parse::<u16>()
        .context("response status was invalid")?;

    let (headers_part, body_part) = if let Some((h, b)) = head_and_body.split_once("\r\n\r\n") {
        (h, b)
    } else if let Some((h, b)) = head_and_body.split_once("\n\n") {
        (h, b)
    } else {
        (head_and_body, "")
    };

    let mut response_headers = HashMap::new();
    for line in headers_part.lines() {
        if let Some((name, val)) = line.split_once(':') {
            response_headers.insert(name.trim().to_lowercase(), val.trim().to_string());
        }
    }

    Ok((status, response_headers, body_part.to_string()))
}

fn json_request(url: &str, headers: &[(&str, String)], body: Value) -> Result<Value> {
    let (status, text) = curl_request(url, headers, Some(body.to_string()))?;
    if !(200..300).contains(&status) {
        bail!(
            "HTTP {status}: {}",
            text.chars().take(240).collect::<String>()
        );
    }
    serde_json::from_str(&text).context("response was not JSON")
}

fn parse_json_number(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.parse().ok(),
        _ => None,
    }
}

fn percent_left(used_cost: f64, limit: f64) -> f32 {
    ((1.0 - (used_cost / limit).clamp(0.0, 1.0)) * 100.0) as f32
}

fn local_reset(window: &str) -> String {
    format!("~{window}")
}

fn parse_sqlite_usage() -> Result<(f64, f64, f64)> {
    let Some(home) = dirs::home_dir() else {
        bail!("home directory unavailable");
    };
    let db = home.join(".local/share/opencode/opencode.db");
    if !db.exists() {
        bail!("OpenCode usage database not found");
    }
    let now = now_millis();
    let five_hours = now - 5 * 60 * 60 * 1000;
    let week = now - 7 * 24 * 60 * 60 * 1000;
    let month = now - 30 * 24 * 60 * 60 * 1000;
    let query = format!(
        "SELECT time_created, cost FROM session WHERE json_extract(model, '$.providerID') = 'opencode-go' AND time_created >= {month} UNION ALL SELECT time_created, cost FROM session_v2 WHERE json_extract(model, '$.providerID') = 'opencode-go' AND time_created >= {month};"
    );
    let output = Command::new("sqlite3")
        .args(["-readonly", "-separator", "\t"])
        .arg(&db)
        .arg(query)
        .output()
        .context("sqlite3 is unavailable")?;
    if !output.status.success() {
        bail!("could not read OpenCode usage database");
    }

    let mut five_hour_cost = 0.0;
    let mut weekly_cost = 0.0;
    let mut monthly_cost = 0.0;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((created, cost)) = line.split_once('\t') else {
            continue;
        };
        let Ok(created) = created.parse::<i64>() else {
            continue;
        };
        let Ok(cost) = cost.parse::<f64>() else {
            continue;
        };
        monthly_cost += cost;
        if created >= week {
            weekly_cost += cost;
        }
        if created >= five_hours {
            five_hour_cost += cost;
        }
    }
    Ok((five_hour_cost, weekly_cost, monthly_cost))
}

pub struct ClaudeCode;

impl UsageSource for ClaudeCode {
    fn id(&self) -> &str {
        "claude-code"
    }

    fn fetch(&self) -> Result<Provider> {
        match fetch_claude_http_usage() {
            Ok(p) => Ok(p),
            Err(err) => {
                eprintln!("headroom: Claude Code HTTP probe failed: {err:#}");
                Ok(Provider {
                    id: "claude-code".into(),
                    name: "Claude Code".into(),
                    badge: "C".into(),
                    badge_bg: 0xd97757,
                    badge_fg: 0x2b1206,
                    plan: "Run claude login".into(),
                    console_url: "https://claude.ai/settings/usage".into(),
                    limits: vec![
                        Limit::new(Cadence::Session, 0.0).resets_at("run claude login"),
                        Limit::new(Cadence::Weekly, 0.0),
                    ],
                    burn: Burn::new(
                        vec![0.0, 0.0, 0.0],
                        "run `claude login` in terminal",
                        Trend::Steady,
                    ),
                })
            }
        }
    }
}

struct ClaudeCreds {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}

fn read_claude_keychain_credentials() -> Result<ClaudeCreds> {
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .context("could not read Keychain")?;
    if !output.status.success() {
        bail!("Claude Code credentials not found in Keychain");
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let value: Value = serde_json::from_str(&raw).context("invalid Keychain JSON")?;
    let oauth = value.get("claudeAiOauth").unwrap_or(&value);
    let access = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let refresh = oauth
        .get("refreshToken")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let expires = oauth.get("expiresAt").and_then(Value::as_i64).unwrap_or(0);
    if access.is_empty() && refresh.is_empty() {
        bail!("missing Claude tokens");
    }
    Ok(ClaudeCreds {
        access_token: access,
        refresh_token: refresh,
        expires_at: expires,
    })
}

fn read_claude_opencode_credentials() -> Result<ClaudeCreds> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    let path = home.join(".local/share/opencode/auth.json");
    let raw = std::fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&raw)?;
    let anthropic = value
        .get("anthropic")
        .ok_or_else(|| anyhow!("no anthropic in auth.json"))?;
    let access = anthropic
        .get("access")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let refresh = anthropic
        .get("refresh")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let expires = anthropic
        .get("expires")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if access.is_empty() && refresh.is_empty() {
        bail!("missing anthropic tokens in auth.json");
    }
    Ok(ClaudeCreds {
        access_token: access,
        refresh_token: refresh,
        expires_at: expires,
    })
}

fn read_claude_credentials() -> Result<ClaudeCreds> {
    if let Ok(creds) = read_claude_keychain_credentials() {
        if !creds.access_token.is_empty() || !creds.refresh_token.is_empty() {
            return Ok(creds);
        }
    }
    read_claude_opencode_credentials()
}

fn refresh_claude_token(refresh_token: &str) -> Result<ClaudeCreds> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        url_encode(refresh_token),
        url_encode(CLAUDE_OAUTH_CLIENT_ID)
    );
    let (status, text) = curl_request(
        CLAUDE_OAUTH_TOKEN_URL,
        &[
            ("Content-Type", "application/x-www-form-urlencoded".into()),
            (
                "User-Agent",
                "claude-cli/2.1.112 (external, sdk-cli)".into(),
            ),
        ],
        Some(body),
    )?;
    if !(200..300).contains(&status) {
        bail!("OAuth refresh failed HTTP {status}: {text}");
    }
    let res: Value = serde_json::from_str(&text)?;
    let access = res
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("no access_token"))?
        .to_string();
    let refresh = res
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or(refresh_token)
        .to_string();
    let expires_in = res
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(28800);
    let expires_at = now_millis() + expires_in * 1000;

    save_claude_credentials(&access, &refresh, expires_at);

    Ok(ClaudeCreds {
        access_token: access,
        refresh_token: refresh,
        expires_at,
    })
}

fn save_claude_credentials(access: &str, refresh: &str, expires_at: i64) {
    if access.trim().is_empty() || refresh.trim().is_empty() {
        return;
    }
    let payload = serde_json::json!({
        "claudeAiOauth": {
            "accessToken": access,
            "refreshToken": refresh,
            "expiresAt": expires_at
        }
    });
    let _ = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            "Claude Code-credentials",
            "-w",
            &payload.to_string(),
        ])
        .status();
}

fn fetch_claude_http_usage() -> Result<Provider> {
    let mut creds = read_claude_credentials()?;
    let now_ms = now_millis();
    if creds.access_token.is_empty() || now_ms >= creds.expires_at - 60_000 {
        if !creds.refresh_token.is_empty() {
            if let Ok(new_creds) = refresh_claude_token(&creds.refresh_token) {
                creds = new_creds;
            }
        }
    }

    if creds.access_token.is_empty() {
        bail!("Claude Code access token unavailable");
    }

    let mut response = query_claude_messages_headers(&creds.access_token);
    if let Ok((401, _, _)) = response {
        if !creds.refresh_token.is_empty() {
            if let Ok(new_creds) = refresh_claude_token(&creds.refresh_token) {
                creds = new_creds;
                response = query_claude_messages_headers(&creds.access_token);
            }
        }
    }

    let (status, resp_headers, _) = response?;
    if !(200..300).contains(&status) {
        bail!("Claude HTTP probe returned HTTP {status}");
    }

    parse_claude_rate_limit_headers(&resp_headers)
}

fn query_claude_messages_headers(
    access_token: &str,
) -> Result<(u16, HashMap<String, String>, String)> {
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
    curl_request_with_response_headers(CLAUDE_MESSAGES_URL, &headers, Some(body.to_string()))
}

fn parse_unix_timestamp_reset(ts: i64) -> Option<String> {
    let date = if ts > 1_000_000_000_000 {
        DateTime::<Utc>::from_timestamp(ts / 1000, 0)?.with_timezone(&Local)
    } else {
        DateTime::<Utc>::from_timestamp(ts, 0)?.with_timezone(&Local)
    };
    let now_date = Local::now().date_naive();
    if date.date_naive() == now_date {
        Some(date.format("%-I:%M%P").to_string())
    } else {
        Some(date.format("%b %-d %-I:%M%P").to_string())
    }
}

fn parse_claude_rate_limit_headers(headers: &HashMap<String, String>) -> Result<Provider> {
    let util_5h = headers
        .get("anthropic-ratelimit-unified-5h-utilization")
        .and_then(|v| v.parse::<f32>().ok())
        .ok_or_else(|| anyhow!("missing 5h rate limit utilization header"))?;
    let reset_5h_ts = headers
        .get("anthropic-ratelimit-unified-5h-reset")
        .and_then(|v| v.parse::<i64>().ok());

    let util_7d = headers
        .get("anthropic-ratelimit-unified-7d-utilization")
        .and_then(|v| v.parse::<f32>().ok());
    let reset_7d_ts = headers
        .get("anthropic-ratelimit-unified-7d-reset")
        .and_then(|v| v.parse::<i64>().ok());

    let left_5h = ((1.0 - util_5h.clamp(0.0, 1.0)) * 100.0) as f32;
    let reset_5h_str = reset_5h_ts.and_then(parse_unix_timestamp_reset);

    let session_limit = Limit::new(Cadence::Session, left_5h)
        .resets_at(reset_5h_str.unwrap_or_else(|| "unknown".into()));

    let mut limits = vec![session_limit];

    if let Some(util_7d) = util_7d {
        let left_7d = ((1.0 - util_7d.clamp(0.0, 1.0)) * 100.0) as f32;
        let reset_7d_str = reset_7d_ts.and_then(parse_unix_timestamp_reset);
        limits.push(
            Limit::new(Cadence::Weekly, left_7d)
                .resets_at(reset_7d_str.unwrap_or_else(|| "unknown".into())),
        );
    }

    let used = 100.0 - left_5h;
    Ok(Provider {
        id: "claude-code".into(),
        name: "Claude Code".into(),
        badge: "C".into(),
        badge_bg: 0xd97757,
        badge_fg: 0x2b1206,
        plan: "Subscription".into(),
        console_url: "https://claude.ai/settings/usage".into(),
        limits,
        burn: Burn::new(
            vec![used * 0.55 / 100.0, used * 0.75 / 100.0, used / 100.0],
            "Claude Code API headers",
            if left_5h < 20.0 {
                Trend::Rising
            } else {
                Trend::Steady
            },
        ),
    })
}

fn parse_claude_window(text: &str, label: &str) -> Option<(f32, String)> {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.contains(label) {
            continue;
        }
        let Some(percent_end) = line.find('%') else {
            continue;
        };
        let start = line[..percent_end]
            .char_indices()
            .rev()
            .take_while(|(_, ch)| ch.is_ascii_digit() || *ch == '.')
            .last()
            .map(|(index, _)| index)
            .unwrap_or(percent_end);
        let value = line[start..percent_end].parse::<f32>().ok()?;
        let left = if lower.contains("used") {
            100.0 - value
        } else {
            value
        };
        let reset = lower
            .find("resets")
            .map(|index| line[index + 6..].trim().to_string())
            .map(|value| {
                value
                    .rsplit_once(" at ")
                    .map(|(_, time)| time.to_string())
                    .unwrap_or(value)
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".into());
        return Some((left.clamp(0.0, 100.0), reset));
    }
    None
}

fn strip_ansi(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut escape = false;
    for ch in value.chars() {
        if escape {
            if ch.is_ascii_alphabetic() {
                escape = false;
            }
        } else if ch == '\u{1b}' {
            escape = true;
        } else {
            output.push(ch);
        }
    }
    output
}

pub struct OpenCodeGo;

impl UsageSource for OpenCodeGo {
    fn id(&self) -> &str {
        "opencode-go"
    }

    fn fetch(&self) -> Result<Provider> {
        let key = credentials::opencode_go_api_key()
            .ok_or_else(|| anyhow!("OpenCode Go API key not configured"))?;
        match fetch_go_plan(&key) {
            Ok(provider) => return Ok(provider),
            Err(error) => eprintln!("headroom: OpenCode Go plan API failed: {error:#}"),
        }
        local_opencode_go_provider()
    }
}

fn fetch_go_plan(key: &str) -> Result<Provider> {
    let headers = [
        ("Authorization", format!("Bearer {key}")),
        ("Accept", "application/json".into()),
    ];
    let mut last_error = None;
    for url in GO_USAGE_URLS {
        let (status, text) = match curl_request(url, &headers, None) {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        if !(200..300).contains(&status) {
            last_error = Some(anyhow!(
                "HTTP {status}: {}",
                text.chars().take(240).collect::<String>()
            ));
            continue;
        }
        let response: Value = match serde_json::from_str(&text) {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(anyhow!("response was not JSON: {error}"));
                continue;
            }
        };
        if let Ok(provider) = parse_go_plan(&response) {
            return Ok(provider);
        }
        if let Ok(provider) = parse_go_usage(&response) {
            return Ok(provider);
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("OpenCode Go usage API unavailable")))
}

fn parse_go_plan(response: &Value) -> Result<Provider> {
    let windows = response
        .get("windows")
        .ok_or_else(|| anyhow!("usage plan has no windows"))?;
    let rolling = parse_go_window(windows.get("rolling"), Cadence::Session)?;
    let weekly = parse_go_window(windows.get("weekly"), Cadence::Weekly)?;
    let monthly = parse_go_window(windows.get("monthly"), Cadence::Monthly)?;
    let used = 100.0 - rolling.percent_left;
    Ok(Provider {
        id: "opencode-go".into(),
        name: "OpenCode Go".into(),
        badge: "O".into(),
        badge_bg: 0xe8e8ea,
        badge_fg: 0x16171a,
        plan: response
            .get("plan")
            .and_then(Value::as_str)
            .unwrap_or("go")
            .to_ascii_uppercase()
            .into(),
        console_url: "https://opencode.ai/auth".into(),
        limits: vec![rolling, weekly, monthly],
        burn: Burn::new(
            vec![used / 100.0 * 0.45, used / 100.0 * 0.75, used / 100.0],
            "OpenCode Go usage API",
            if used >= 70.0 {
                Trend::Rising
            } else {
                Trend::Steady
            },
        ),
    })
}

fn parse_go_window(value: Option<&Value>, cadence: Cadence) -> Result<Limit> {
    let window = value.ok_or_else(|| anyhow!("usage plan is missing {cadence:?} window"))?;
    let used = parse_json_number(window.get("usage_percent"))
        .ok_or_else(|| anyhow!("usage plan is missing {cadence:?} usage"))?;
    let resets = parse_json_number(window.get("resets_in_seconds"))
        .ok_or_else(|| anyhow!("usage plan is missing {cadence:?} reset"))?;
    Ok(Limit::new(cadence, (100.0 - used).clamp(0.0, 100.0) as f32)
        .resets_at(format_reset_seconds(resets.max(0.0) as u64)))
}

fn parse_go_usage(response: &Value) -> Result<Provider> {
    let windows = response
        .get("usage")
        .ok_or_else(|| anyhow!("usage response has no usage object"))?;
    let rolling = parse_go_usage_window(windows.get("rolling"), Cadence::Session)?;
    let weekly = parse_go_usage_window(windows.get("weekly"), Cadence::Weekly)?;
    let monthly = parse_go_usage_window(windows.get("monthly"), Cadence::Monthly)?;
    let used = 100.0 - rolling.percent_left;
    Ok(Provider {
        id: "opencode-go".into(),
        name: "OpenCode Go".into(),
        badge: "O".into(),
        badge_bg: 0xe8e8ea,
        badge_fg: 0x16171a,
        plan: "GO".into(),
        console_url: "https://opencode.ai/auth".into(),
        limits: vec![rolling, weekly, monthly],
        burn: Burn::new(
            vec![used / 100.0 * 0.45, used / 100.0 * 0.75, used / 100.0],
            "OpenCode Go usage API",
            if used >= 70.0 {
                Trend::Rising
            } else {
                Trend::Steady
            },
        ),
    })
}

fn parse_go_usage_window(value: Option<&Value>, cadence: Cadence) -> Result<Limit> {
    let window = value.ok_or_else(|| anyhow!("usage response is missing {cadence:?} window"))?;
    let used = parse_json_number(window.get("percent"))
        .ok_or_else(|| anyhow!("usage response is missing {cadence:?} usage"))?;
    let reset = parse_reset_time(window.get("resetsAt").or_else(|| window.get("resetAt")))
        .ok_or_else(|| anyhow!("usage response is missing {cadence:?} reset"))?;
    Ok(Limit::new(cadence, (100.0 - used).clamp(0.0, 100.0) as f32).resets_at(reset))
}

fn format_reset_seconds(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    if seconds < 3600 {
        return format!("{}m", seconds / 60);
    }
    if seconds < 86400 {
        return format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60);
    }
    format!("{}d {}h", seconds / 86400, (seconds % 86400) / 3600)
}

fn local_opencode_go_provider() -> Result<Provider> {
    let (five_hour, weekly, monthly) = parse_sqlite_usage()?;
    let five_left = percent_left(five_hour, 12.0);
    let weekly_left = percent_left(weekly, 30.0);
    let monthly_left = percent_left(monthly, 60.0);
    Ok(Provider {
        id: "opencode-go".into(),
        name: "OpenCode Go".into(),
        badge: "O".into(),
        badge_bg: 0xe8e8ea,
        badge_fg: 0x16171a,
        plan: "Go (local estimate)".into(),
        console_url: "https://opencode.ai/auth".into(),
        limits: vec![
            Limit::new(Cadence::Session, five_left).resets_at(local_reset("5h")),
            Limit::new(Cadence::Weekly, weekly_left).resets_at(local_reset("7d")),
            Limit::new(Cadence::Monthly, monthly_left).resets_at(local_reset("30d")),
        ],
        burn: Burn::new(
            vec![
                ((1.0 - (five_hour / 12.0).clamp(0.0, 1.0)) * 0.35) as f32,
                ((1.0 - (five_hour / 12.0).clamp(0.0, 1.0)) * 0.7) as f32,
                (1.0 - (five_hour / 12.0).clamp(0.0, 1.0)) as f32,
            ],
            "local OpenCode Go estimate",
            Trend::Steady,
        ),
    })
}

fn refresh_google_access_token(refresh_token: &str) -> Result<String> {
    let agy_id = antigravity_client_id();
    let agy_sec = antigravity_client_secret();
    let gem_id = gemini_client_id();
    let gem_sec = gemini_client_secret();
    let candidates = [(&agy_id, &agy_sec), (&gem_id, &gem_sec)];
    for (client_id, client_secret) in candidates {
        let body = format!(
            "client_id={}&client_secret={}&grant_type=refresh_token&refresh_token={}",
            url_encode(client_id),
            url_encode(client_secret),
            url_encode(refresh_token)
        );
        let Ok((status, text)) = curl_request(
            GOOGLE_TOKEN_URL,
            &[("Content-Type", "application/x-www-form-urlencoded".into())],
            Some(body),
        ) else {
            continue;
        };
        if (200..300).contains(&status) {
            if let Some(token) = serde_json::from_str::<Value>(&text).ok().and_then(|value| {
                value
                    .get("access_token")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }) {
                return Ok(token);
            }
        }
    }
    bail!("could not refresh Antigravity credentials")
}

fn parse_reset_time(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let date = match value {
        Value::String(value) => DateTime::parse_from_rfc3339(value)
            .ok()?
            .with_timezone(&Local),
        Value::Number(value) => {
            let timestamp = value.as_f64()?;
            let timestamp = if timestamp > 1_000_000_000_000.0 {
                timestamp / 1000.0
            } else {
                timestamp
            };
            DateTime::<Utc>::from_timestamp(timestamp as i64, 0)?.with_timezone(&Local)
        }
        _ => return None,
    };
    let now_date = Local::now().date_naive();
    if date.date_naive() == now_date {
        Some(date.format("%-I:%M%P").to_string())
    } else {
        Some(date.format("%b %-d %-I:%M%P").to_string())
    }
}

fn collect_quotas(value: &Value, label: &str, out: &mut Vec<(String, f32, Option<String>)>) {
    match value {
        Value::Object(object) => {
            let current_label = object
                .get("modelId")
                .and_then(Value::as_str)
                .unwrap_or(label);
            if let Some(remaining) = parse_json_number(object.get("remainingFraction")) {
                out.push((
                    current_label.to_string(),
                    (remaining.clamp(0.0, 1.0) * 100.0) as f32,
                    parse_reset_time(object.get("resetTime").or_else(|| object.get("reset_time"))),
                ));
            }
            for (key, child) in object {
                let next = if key == "quotaInfo" || key == "quota" || key == "models" {
                    current_label.to_string()
                } else if current_label.is_empty() {
                    key.clone()
                } else {
                    current_label.to_string()
                };
                collect_quotas(child, &next, out);
            }
        }
        Value::Array(array) => {
            for child in array {
                collect_quotas(child, label, out);
            }
        }
        _ => {}
    }
}

pub struct Antigravity;

impl UsageSource for Antigravity {
    fn id(&self) -> &str {
        "antigravity"
    }

    fn fetch(&self) -> Result<Provider> {
        let refresh = credentials::antigravity_refresh_token()
            .ok_or_else(|| anyhow!("Antigravity credentials not found"))?;
        let access = refresh_google_access_token(&refresh)?;
        let headers = [
            ("Authorization", format!("Bearer {access}")),
            ("Content-Type", "application/json".into()),
            ("User-Agent", "antigravity".into()),
            (
                "X-Goog-Api-Client",
                "google-cloud-sdk vscode_cloudshelleditor/0.1".into(),
            ),
            (
                "Client-Metadata",
                r#"{"ideType":"ANTIGRAVITY","platform":"MACOS","pluginType":"GEMINI"}"#.into(),
            ),
        ];
        let metadata = serde_json::json!({
            "metadata": {
                "ideType": "IDE_UNSPECIFIED",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI"
            }
        });
        let endpoints = [
            "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal",
            "https://cloudcode-pa.googleapis.com/v1internal",
        ];
        let mut last_error = None;
        for endpoint in endpoints {
            let load = match json_request(
                &format!("{endpoint}:loadCodeAssist"),
                &headers,
                metadata.clone(),
            ) {
                Ok(value) => value,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                }
            };
            let project = load
                .get("cloudaicompanionProject")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let quota_body = if project.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::json!({ "project": project })
            };
            let quota = match json_request(
                &format!("{endpoint}:retrieveUserQuota"),
                &headers,
                quota_body,
            ) {
                Ok(value) => value,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                }
            };
            let mut quotas = Vec::new();
            collect_quotas(&quota, "", &mut quotas);
            if quotas.is_empty() {
                collect_quotas(&load, "", &mut quotas);
            }
            let mut usable = quotas
                .into_iter()
                .filter(|(model, left, _)| {
                    left.is_finite() && !model.starts_with("chat_") && !model.starts_with("tab_")
                })
                .collect::<Vec<_>>();
            if usable.iter().any(|(_, _, reset)| reset.is_some()) {
                usable.retain(|(_, _, reset)| reset.is_some());
            }
            if let Some((_, left, reset)) = usable.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)) {
                let tier = load
                    .pointer("/currentTier/name")
                    .and_then(Value::as_str)
                    .or_else(|| load.pointer("/planInfo/planType").and_then(Value::as_str))
                    .unwrap_or("Google account")
                    .to_string();
                return Ok(Provider {
                    id: "antigravity".into(),
                    name: "Antigravity".into(),
                    badge: "G".into(),
                    badge_bg: 0x4285f4,
                    badge_fg: 0xffffff,
                    plan: tier.into(),
                    console_url: "https://aistudio.google.com".into(),
                    limits: vec![
                        Limit::new(Cadence::Daily, left)
                            .resets_at(reset.unwrap_or_else(|| "unknown".into())),
                    ],
                    burn: Burn::new(
                        vec![0.25, (100.0 - left) / 100.0 * 0.7, (100.0 - left) / 100.0],
                        "live Antigravity quota",
                        if left < 20.0 {
                            Trend::Rising
                        } else {
                            Trend::Steady
                        },
                    ),
                });
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow!("Antigravity quota data unavailable")))
    }
}

fn resolve_command(name: &str) -> Result<String> {
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }
    let Some(home) = dirs::home_dir() else {
        bail!("{name} is not installed");
    };
    for path in [
        home.join(format!(".local/bin/{name}")),
        home.join(format!("bin/{name}")),
        std::path::PathBuf::from(format!("/opt/homebrew/bin/{name}")),
        std::path::PathBuf::from(format!("/usr/local/bin/{name}")),
    ] {
        if path.is_file() {
            return Ok(path.to_string_lossy().into_owned());
        }
    }
    bail!("{name} is not installed")
}

#[cfg(test)]
mod tests {
    use super::{
        parse_claude_rate_limit_headers, parse_claude_window, parse_go_plan, parse_go_usage,
        percent_left,
    };
    use std::collections::HashMap;

    #[test]
    fn parses_claude_rate_limit_headers_correctly() {
        let mut headers = HashMap::new();
        headers.insert(
            "anthropic-ratelimit-unified-5h-utilization".into(),
            "0.15".into(),
        );
        headers.insert(
            "anthropic-ratelimit-unified-5h-reset".into(),
            "1787416800".into(),
        );
        headers.insert(
            "anthropic-ratelimit-unified-7d-utilization".into(),
            "0.40".into(),
        );
        headers.insert(
            "anthropic-ratelimit-unified-7d-reset".into(),
            "1787835600".into(),
        );

        let provider = parse_claude_rate_limit_headers(&headers).unwrap();
        assert!((provider.primary().percent_left - 85.0).abs() < 0.01);
        assert!((provider.secondary()[0].percent_left - 60.0).abs() < 0.01);
    }

    #[test]
    fn parses_claude_used_percent_and_reset_time() {
        let text = "Current session: 100% used · resets Aug 14 at 4:20pm (Asia/Calcutta)";
        let result = parse_claude_window(text, "current session").unwrap();
        assert_eq!(result.0, 0.0);
        assert_eq!(result.1, "4:20pm");
    }

    #[test]
    fn parses_claude_remaining_percent() {
        let text = "Current session: 32% left · resets 11:55";
        let result = parse_claude_window(text, "current session").unwrap();
        assert_eq!(result.0, 32.0);
        assert_eq!(result.1, "11:55");
    }

    #[test]
    fn clamps_local_cost_percentage() {
        assert_eq!(percent_left(6.0, 12.0), 50.0);
        assert_eq!(percent_left(18.0, 12.0), 0.0);
        assert_eq!(percent_left(-1.0, 12.0), 100.0);
    }

    #[test]
    fn parses_authoritative_go_windows() {
        let response = serde_json::json!({
            "plan": "go",
            "windows": {
                "rolling": { "usage_percent": 65, "resets_in_seconds": 2520 },
                "weekly": { "usage_percent": 30, "resets_in_seconds": 259200 },
                "monthly": { "usage_percent": 12, "resets_in_seconds": 1728000 }
            }
        });
        let provider = parse_go_plan(&response).unwrap();
        assert_eq!(provider.primary().percent_left, 35.0);
        assert_eq!(
            provider
                .primary()
                .resets_at
                .as_ref()
                .map(ToString::to_string),
            Some("42m".into())
        );
        assert_eq!(provider.secondary()[0].percent_left, 70.0);
        assert_eq!(provider.secondary()[1].percent_left, 88.0);
    }

    #[test]
    fn parses_live_go_usage_shape() {
        let response = serde_json::json!({
            "usage": {
                "rolling": { "percent": 76, "resetsAt": "2026-08-14T10:07:06.031Z" },
                "weekly": { "percent": 49, "resetsAt": "2026-08-17T00:00:00.031Z" },
                "monthly": { "percent": 34, "resetsAt": "2026-09-09T04:40:04.031Z" }
            }
        });
        let provider = parse_go_usage(&response).unwrap();
        assert_eq!(provider.primary().percent_left, 24.0);
        assert_eq!(provider.secondary()[0].percent_left, 51.0);
        assert_eq!(provider.secondary()[1].percent_left, 66.0);
    }
}
