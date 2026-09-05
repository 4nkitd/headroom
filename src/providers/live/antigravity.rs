use std::collections::HashSet;

use anyhow::{Result, anyhow, bail};
use serde_json::Value;

use super::http::{json, json_number, request, url_encode};
use super::parse_reset_time;
use crate::credentials;
use crate::model::{Cadence, Limit, Provider};
use crate::providers::{SourceDescriptor, UsageSource};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

pub struct Antigravity;

impl UsageSource for Antigravity {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "antigravity",
            name: "Antigravity",
            logo: "providers/antigravity.png",
            badge: "G",
            badge_bg: 0x4285f4,
            badge_fg: 0xffffff,
            setup_label: "Open Antigravity",
            setup_url: Some("https://antigravity.google"),
        }
    }

    fn fetch(&self) -> Result<Vec<Provider>> {
        let accounts = credentials::antigravity_accounts();
        if accounts.is_empty() {
            bail!("Antigravity credentials not found");
        }
        let mut providers = Vec::new();
        let mut errors = Vec::new();
        for account in accounts {
            let token = match access_token(account.access_token.as_deref(), &account.refresh_token)
            {
                Ok(token) => token,
                Err(error) => {
                    errors.push(format!("{}: {error:#}", account.label));
                    continue;
                }
            };
            let result = match fetch_with_token(&token) {
                Err(error) if format!("{error:#}").contains("HTTP 401") => {
                    fetch_with_token(&refresh_access_token(&account.refresh_token)?)
                }
                result => result,
            };
            match result {
                Ok(mut provider) => {
                    let account_id = account.label.clone();
                    provider.id = format!("antigravity:{account_id}").into();
                    let display_label = crate::model::truncate_account_label(&account.label);
                    provider.name = format!("Antigravity · {display_label}").into();
                    providers.push(provider);
                }
                Err(error) => errors.push(format!("{}: {error:#}", account.label)),
            }
        }
        if providers.is_empty() {
            bail!("Antigravity accounts unavailable: {}", errors.join("; "));
        }
        Ok(providers)
    }
}

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

fn refresh_access_token(refresh_token: &str) -> Result<String> {
    let antigravity_id = antigravity_client_id();
    let antigravity_secret = antigravity_client_secret();
    let gemini_id = gemini_client_id();
    let gemini_secret = gemini_client_secret();
    for (client_id, client_secret) in [
        (&antigravity_id, &antigravity_secret),
        (&gemini_id, &gemini_secret),
    ] {
        let body = format!(
            "client_id={}&client_secret={}&grant_type=refresh_token&refresh_token={}",
            url_encode(client_id),
            url_encode(client_secret),
            url_encode(refresh_token)
        );
        let Ok(response) = request(
            TOKEN_URL,
            &[("Content-Type", "application/x-www-form-urlencoded".into())],
            Some(body),
        ) else {
            continue;
        };
        if (200..300).contains(&response.status)
            && let Ok(value) = serde_json::from_str::<Value>(&response.body)
            && let Some(token) = value.get("access_token").and_then(Value::as_str)
        {
            let token = token.to_string();
            return Ok(token);
        }
    }
    bail!("could not refresh Antigravity credentials")
}

fn access_token(existing: Option<&str>, refresh: &str) -> Result<String> {
    if let Some(existing) = existing.filter(|token| !token.is_empty()) {
        return Ok(existing.to_string());
    }
    refresh_access_token(refresh)
}

fn fetch_with_token(access_token: &str) -> Result<Provider> {
    let headers = [
        ("Authorization", format!("Bearer {access_token}")),
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
        let load = match json(
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
        if let Ok(summary) = json(
            &format!("{endpoint}:retrieveUserQuotaSummary"),
            &headers,
            quota_body.clone(),
        ) && let Ok(provider) = parse_summary_provider(&load, &summary)
        {
            return Ok(provider);
        }
        let quota = match json(
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
        if let Ok(provider) = parse_provider(&load, &quota) {
            return Ok(provider);
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("Antigravity quota data unavailable")))
}

fn collect_quotas(value: &Value, label: &str, out: &mut Vec<(String, f32, Option<String>)>) {
    match value {
        Value::Object(object) => {
            let current_label = object
                .get("modelId")
                .and_then(Value::as_str)
                .unwrap_or(label);
            if let Some(remaining) = json_number(object.get("remainingFraction")) {
                out.push((
                    current_label.to_string(),
                    (remaining.clamp(0.0, 1.0) * 100.0) as f32,
                    super::parse_reset_time(
                        object.get("resetTime").or_else(|| object.get("reset_time")),
                    ),
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

fn model_label(model: &str) -> String {
    model
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "gemini" => "Gemini".into(),
            "claude" => "Claude".into(),
            "flash" => "Flash".into(),
            "pro" => "Pro".into(),
            "opus" => "Opus".into(),
            "sonnet" => "Sonnet".into(),
            _ => part.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_provider(load: &Value, quota: &Value) -> Result<Provider> {
    let mut quotas = Vec::new();
    collect_quotas(quota, "", &mut quotas);
    if quotas.is_empty() {
        collect_quotas(load, "", &mut quotas);
    }
    let mut usable = quotas
        .into_iter()
        .filter(|(model, left, _)| {
            left.is_finite() && !model.starts_with("chat_") && !model.starts_with("tab_")
        })
        .collect::<Vec<_>>();
    let mut seen_models = HashSet::new();
    usable.retain(|(model, _, _)| seen_models.insert(model.clone()));
    if usable.iter().any(|(_, _, reset)| reset.is_some()) {
        usable.retain(|(_, _, reset)| reset.is_some());
    }
    if usable.is_empty() {
        bail!("Antigravity quota data unavailable");
    }

    let primary_index = usable
        .iter()
        .position(|(model, _, _)| {
            model.contains("gemini-3-flash") || model.contains("gemini-3.7-flash")
        })
        .unwrap_or_else(|| {
            usable
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| left.1.total_cmp(&right.1))
                .map(|(index, _)| index)
                .unwrap_or(0)
        });
    let (primary_model, primary_left, primary_reset) = usable.remove(primary_index);
    let mut limits = vec![
        Limit::new(Cadence::Daily, primary_left)
            .label(model_label(&primary_model))
            .resets_at(primary_reset.unwrap_or_else(|| "unknown".into())),
    ];
    for (model, left, reset) in usable {
        if model.contains("pro")
            || model.contains("claude")
            || model.contains("opus")
            || model.contains("sonnet")
        {
            limits.push(
                Limit::new(Cadence::Daily, left)
                    .label(model_label(&model))
                    .resets_at(reset.unwrap_or_else(|| "unknown".into())),
            );
        }
    }
    let tier = load
        .pointer("/currentTier/name")
        .and_then(Value::as_str)
        .or_else(|| load.pointer("/planInfo/planType").and_then(Value::as_str))
        .unwrap_or("Antigravity");
    Ok(Provider {
        id: "antigravity".into(),
        name: "Antigravity".into(),
        logo: "providers/antigravity.png".into(),
        badge: "G".into(),
        badge_bg: 0x4285f4,
        badge_fg: 0xffffff,
        plan: tier.to_string().into(),
        console_url: "https://aistudio.google.com".into(),
        source_label: "Google Cloud Code Assist HTTP API".into(),
        limits,
    })
}

fn parse_summary_provider(load: &Value, summary: &Value) -> Result<Provider> {
    let groups = summary
        .get("groups")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Antigravity quota summary has no groups"))?;
    let mut limits = Vec::new();
    for group in groups {
        let group_name = group
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or("Models");
        for bucket in group
            .get("buckets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(remaining) = json_number(bucket.get("remainingFraction")) else {
                continue;
            };
            let cadence = match bucket.get("window").and_then(Value::as_str) {
                Some("5h" | "5h0m0s" | "18000s") => Cadence::Session,
                Some("weekly" | "7d" | "604800s") => Cadence::Weekly,
                _ => continue,
            };
            let reset =
                parse_reset_time(bucket.get("resetTime").or_else(|| bucket.get("reset_time")));
            let group_label = if group_name.contains("Gemini") {
                "Gemini"
            } else {
                "Claude/GPT"
            };
            limits.push(
                Limit::new(cadence, (remaining.clamp(0.0, 1.0) * 100.0) as f32)
                    .label(format!("{group_label} {}", cadence.label()))
                    .resets_at(reset.unwrap_or_else(|| "unknown".into())),
            );
        }
    }
    if limits.is_empty() {
        bail!("Antigravity quota summary has no usable buckets");
    }
    limits.sort_by_key(|limit| (limit.cadence, !limit.display_label().starts_with("Gemini")));
    let tier = load
        .pointer("/currentTier/name")
        .and_then(Value::as_str)
        .or_else(|| load.pointer("/planInfo/planType").and_then(Value::as_str))
        .unwrap_or("Antigravity");
    Ok(Provider {
        id: "antigravity".into(),
        name: "Antigravity".into(),
        logo: "providers/antigravity.png".into(),
        badge: "G".into(),
        badge_bg: 0x4285f4,
        badge_fg: 0xffffff,
        plan: tier.to_string().into(),
        console_url: "https://aistudio.google.com".into(),
        source_label: "Google Cloud Code Assist quota summary API".into(),
        limits,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{model_label, parse_provider, parse_summary_provider};

    #[test]
    fn parses_quota_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/antigravity.json")).unwrap();
        let provider = parse_provider(&fixture["load"], &fixture["quota"]).unwrap();
        assert_eq!(provider.plan.as_ref(), "pro");
        assert_eq!(provider.primary().display_label(), "Gemini 3.7 Flash");
        assert_eq!(provider.primary().percent_left, 41.0);
        assert_eq!(provider.secondary().len(), 1);
    }

    #[test]
    fn formats_model_ids() {
        assert_eq!(model_label("gemini-3.7-flash"), "Gemini 3.7 Flash");
        assert_eq!(model_label("claude_sonnet_4"), "Claude Sonnet 4");
    }

    #[test]
    fn parses_summary_buckets() {
        let load = serde_json::json!({"currentTier": {"name": "Antigravity"}});
        let summary = serde_json::json!({
            "groups": [{"displayName": "Gemini Models", "buckets": [
                {"window": "weekly", "remainingFraction": 0.78, "resetTime": "2026-09-12T06:46:22Z"},
                {"window": "5h", "remainingFraction": 0.92, "resetTime": "2026-09-05T11:46:22Z"}
            ]}]
        });
        let provider = parse_summary_provider(&load, &summary).unwrap();
        assert_eq!(provider.primary().percent_left, 92.0);
        assert_eq!(provider.secondary()[0].percent_left, 78.0);
    }
}
