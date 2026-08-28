use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde::Deserialize;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/4nkitd/headroom/releases/latest";

static CLIENT: OnceLock<Client> = OnceLock::new();

#[derive(Clone, Default)]
pub struct UpdateStatus {
    pub checking: bool,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(8))
            .user_agent(concat!("headroom/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("valid update client")
    })
}

pub fn check() -> Result<UpdateStatus> {
    let response = client()
        .get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("update request failed")?;
    if !response.status().is_success() {
        bail!("update API returned HTTP {}", response.status().as_u16());
    }
    let body = response.text().context("could not read update response")?;
    let release: GitHubRelease = serde_json::from_str(&body).context("invalid update response")?;
    let latest = release.tag_name.trim_start_matches('v');
    if latest.is_empty() {
        return Err(anyhow!("latest release has no version"));
    }
    let available = is_newer(latest, env!("CARGO_PKG_VERSION"));
    Ok(UpdateStatus {
        checking: false,
        latest_version: available.then(|| latest.to_string()),
        release_url: available.then_some(release.html_url),
        error: None,
    })
}

fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(version: &str) -> Vec<u64> {
        version
            .split('.')
            .map(|part| {
                part.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }
    parts(candidate) > parts(current)
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_release_versions() {
        assert!(is_newer("0.4.0", "0.3.4"));
        assert!(is_newer("1.0.0", "0.99.9"));
        assert!(!is_newer("0.3.4", "0.3.4"));
        assert!(!is_newer("0.3.3", "0.3.4"));
    }
}
