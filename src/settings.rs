use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::model::Prefs;

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("headroom/preferences.json"))
}

pub fn load() -> Prefs {
    let Some(path) = path() else {
        return Prefs::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

pub fn save(prefs: &Prefs) -> Result<()> {
    let path = path().context("configuration directory unavailable")?;
    let parent = path.parent().context("invalid preferences path")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(prefs)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}
