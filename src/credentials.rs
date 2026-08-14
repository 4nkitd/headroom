use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;

fn home_file(parts: &[&str]) -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    for part in parts {
        path.push(part);
    }
    path
}

fn keychain_value(service: &str, account: &str) -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", service, "-a", account, "-w"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
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

    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            "Headroom",
            "-a",
            "opencode-go",
            "-w",
            key,
        ])
        .status()
        .context("could not run macOS Keychain")?;
    if status.success() {
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
