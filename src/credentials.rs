use anyhow::{Context, Result, anyhow, bail};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

fn home_file(parts: &[&str]) -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    for part in parts {
        path.push(part);
    }
    path
}

fn keychain_value(service: &str, account: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    let value = String::from_utf8(
        security_framework::passwords::get_generic_password(service, account).ok()?,
    )
    .ok()?
    .trim()
    .to_string();
    #[cfg(not(target_os = "macos"))]
    let value = String::new();
    (!value.is_empty()).then_some(value)
}

fn keychain_service_value(service: &str) -> Option<String> {
    let account = env::var("USER").ok()?;
    keychain_value(service, &account)
}

fn keychain_json(service: &str, account: &str) -> Option<Value> {
    let raw = keychain_value(service, account)?;
    let decoded = raw
        .strip_prefix("go-keyring-base64:")
        .and_then(|value| STANDARD.decode(value).ok())
        .and_then(|value| String::from_utf8(value).ok())
        .unwrap_or(raw);
    serde_json::from_str(&decoded).ok()
}

fn opencode_auth_key() -> Option<String> {
    let path = home_file(&[".local", "share", "opencode", "auth.json"]);
    let value: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    value
        .get("opencode-go")
        .and_then(|entry| entry.get("key"))
        .and_then(Value::as_str)
        .filter(|key| !key.trim().is_empty())
        .map(str::to_string)
}

pub fn opencode_go_api_key() -> Option<String> {
    env::var("OPENCODE_GO_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .or_else(|| keychain_value("Headroom", "opencode-go"))
        .or_else(opencode_auth_key)
        .or_else(|| fs::read_to_string(home_file(&[".config", "headroom", "opencode-go.key"])).ok())
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

pub fn opencode_go_key_status() -> String {
    if env::var("OPENCODE_GO_API_KEY")
        .ok()
        .is_some_and(|key| !key.trim().is_empty())
    {
        "Environment".into()
    } else if keychain_value("Headroom", "opencode-go").is_some() {
        "Headroom Keychain".into()
    } else if opencode_auth_key().is_some() {
        "OpenCode credentials".into()
    } else if fs::read_to_string(home_file(&[".config", "headroom", "opencode-go.key"])).is_ok() {
        "Headroom config".into()
    } else {
        "Not configured".into()
    }
}

pub fn save_opencode_go_api_key(key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        bail!("API key is empty");
    }

    #[cfg(target_os = "macos")]
    if security_framework::passwords::set_generic_password(
        "Headroom",
        "opencode-go",
        key.as_bytes(),
    )
    .is_ok()
    {
        return Ok(());
    }

    let path = home_file(&[".config", "headroom", "opencode-go.key"]);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, format!("{key}\n"))?;
    #[cfg(unix)]
    fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(())
}

fn refresh_token_from_store() -> Option<String> {
    let path = home_file(&[".local", "share", "opencode", "antigravity-accounts.json"]);
    let value: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let accounts = value.get("accounts")?.as_array()?;
    let active = value
        .get("activeIndex")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    accounts
        .get(active)
        .or_else(|| accounts.first())
        .and_then(|account| account.get("refreshToken"))
        .and_then(Value::as_str)
        .filter(|token| !token.trim().is_empty())
        .map(str::to_string)
}

pub fn antigravity_refresh_token() -> Option<String> {
    keychain_json("gemini", "antigravity")
        .and_then(|value| value.get("token").cloned().or(Some(value)))
        .and_then(|value| {
            value
                .get("refresh_token")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(refresh_token_from_store)
        .or_else(|| {
            let path = env::var_os("GEMINI_OAUTH_CREDS")
                .map(PathBuf::from)
                .unwrap_or_else(|| home_file(&[".gemini", "oauth_creds.json"]));
            let value: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
            value
                .get("refresh_token")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

pub fn antigravity_credentials_status() -> &'static str {
    if keychain_json("gemini", "antigravity").is_some() {
        "Gemini Keychain"
    } else if refresh_token_from_store().is_some() {
        "Antigravity credentials"
    } else {
        let path = env::var_os("GEMINI_OAUTH_CREDS")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_file(&[".gemini", "oauth_creds.json"]));
        if fs::read_to_string(path).is_ok() {
            "Gemini credentials"
        } else {
            "Not configured"
        }
    }
}

#[derive(Clone)]
pub struct CodexCredentials {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub account_id: String,
    pub expires_at: i64,
}

fn codex_auth_path() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_file(&[".codex"]))
        .join("auth.json")
}

pub fn codex_credentials() -> Result<CodexCredentials> {
    let raw = fs::read_to_string(codex_auth_path()).context("Codex credentials not found")?;
    let value: Value = serde_json::from_str(&raw).context("Codex credentials are invalid")?;
    let tokens = value
        .get("tokens")
        .ok_or_else(|| anyhow!("Codex is not signed in with ChatGPT"))?;
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let refresh_token = tokens
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let id_token = tokens
        .get("id_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if access_token.is_empty() || account_id.is_empty() {
        bail!("Codex ChatGPT credentials are missing tokens");
    }
    Ok(CodexCredentials {
        expires_at: jwt_expiry_millis(&access_token).unwrap_or(0),
        access_token,
        refresh_token,
        id_token,
        account_id,
    })
}

fn jwt_expiry_millis(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: Value = serde_json::from_slice(&decoded).ok()?;
    value.get("exp")?.as_i64()?.checked_mul(1000)
}

pub fn save_codex_credentials(credentials: &CodexCredentials) -> Result<()> {
    let path = codex_auth_path();
    let raw = fs::read_to_string(&path).context("could not read Codex credentials")?;
    let mut value: Value = serde_json::from_str(&raw).context("Codex credentials are invalid")?;
    let tokens = value
        .get_mut("tokens")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("Codex credentials have no token store"))?;
    tokens.insert(
        "access_token".into(),
        Value::String(credentials.access_token.clone()),
    );
    tokens.insert(
        "refresh_token".into(),
        Value::String(credentials.refresh_token.clone()),
    );
    if !credentials.id_token.is_empty() {
        tokens.insert(
            "id_token".into(),
            Value::String(credentials.id_token.clone()),
        );
    }
    tokens.insert(
        "account_id".into(),
        Value::String(credentials.account_id.clone()),
    );
    value["last_refresh"] = Value::String(chrono::Utc::now().to_rfc3339());

    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&value)?)?;
    #[cfg(unix)]
    fs::set_permissions(
        &temporary,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )?;
    fs::rename(temporary, path)?;
    Ok(())
}

pub fn codex_credentials_status() -> &'static str {
    if codex_credentials().is_ok() {
        "Codex credentials"
    } else {
        "Not configured"
    }
}

#[derive(Clone)]
pub struct ClaudeCredentials {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

pub fn claude_credentials() -> Result<ClaudeCredentials> {
    if let Some(raw) = keychain_service_value("Claude Code-credentials")
        && let Ok(credentials) =
            parse_claude_credentials(&raw, "accessToken", "refreshToken", "expiresAt")
    {
        return Ok(credentials);
    }

    let path = home_file(&[".local", "share", "opencode", "auth.json"]);
    let raw = fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&raw)?;
    let anthropic = value
        .get("anthropic")
        .ok_or_else(|| anyhow!("no anthropic credentials in OpenCode auth"))?;
    parse_claude_value(anthropic, "access", "refresh", "expires")
}

pub fn claude_credentials_status() -> &'static str {
    if keychain_service_value("Claude Code-credentials").is_some() {
        "Claude Keychain"
    } else {
        let path = home_file(&[".local", "share", "opencode", "auth.json"]);
        let configured = fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|value| value.get("anthropic").cloned())
            .is_some();
        if configured {
            "OpenCode credentials"
        } else {
            "Not configured"
        }
    }
}

fn parse_claude_credentials(
    raw: &str,
    access_key: &str,
    refresh_key: &str,
    expires_key: &str,
) -> Result<ClaudeCredentials> {
    let value: Value = serde_json::from_str(raw)?;
    let oauth = value.get("claudeAiOauth").unwrap_or(&value);
    parse_claude_value(oauth, access_key, refresh_key, expires_key)
}

fn parse_claude_value(
    value: &Value,
    access_key: &str,
    refresh_key: &str,
    expires_key: &str,
) -> Result<ClaudeCredentials> {
    let access_token = value
        .get(access_key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let refresh_token = value
        .get(refresh_key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if access_token.is_empty() && refresh_token.is_empty() {
        bail!("Claude credentials are missing OAuth tokens");
    }
    Ok(ClaudeCredentials {
        access_token,
        refresh_token,
        expires_at: value.get(expires_key).and_then(Value::as_i64).unwrap_or(0),
    })
}

pub fn save_claude_credentials(credentials: &ClaudeCredentials) {
    if credentials.access_token.trim().is_empty() || credentials.refresh_token.trim().is_empty() {
        return;
    }
    let payload = serde_json::json!({
        "claudeAiOauth": {
            "accessToken": credentials.access_token,
            "refreshToken": credentials.refresh_token,
            "expiresAt": credentials.expires_at
        }
    });
    #[cfg(target_os = "macos")]
    if let Ok(account) = env::var("USER") {
        let _ = security_framework::passwords::set_generic_password(
            "Claude Code-credentials",
            &account,
            payload.to_string().as_bytes(),
        );
    }
}
