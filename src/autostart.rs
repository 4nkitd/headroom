//! macOS LaunchAgent auto-start integration.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

const LAUNCH_AGENT_LABEL: &str = "com.4nkitd.headroom";

fn plist_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join("Library/LaunchAgents")
            .join(format!("{LAUNCH_AGENT_LABEL}.plist"))
    })
}

pub fn is_enabled() -> bool {
    plist_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn enable() -> Result<()> {
    let plist = plist_path().context("could not determine home directory")?;
    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent)?;
    }

    let exe = env::current_exe().context("could not resolve executable path")?;
    let exe_str = exe.to_string_lossy();

    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_str}</string>
        <string>-d</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#
    );

    fs::write(&plist, content)?;

    let _ = Command::new("launchctl")
        .args(["load", &plist.to_string_lossy()])
        .status();

    Ok(())
}

pub fn disable() -> Result<()> {
    let plist = plist_path().context("could not determine home directory")?;
    if plist.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", &plist.to_string_lossy()])
            .status();
        let _ = fs::remove_file(&plist);
    }
    Ok(())
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    if enabled { enable() } else { disable() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plist_path_format() {
        if let Some(path) = plist_path() {
            assert!(path.to_string_lossy().contains("com.4nkitd.headroom.plist"));
        }
    }
}
