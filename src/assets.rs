use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

pub struct EmbeddedAssets;

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "providers/antigravity.png" => {
                Some(include_bytes!("../assets/providers/antigravity.png"))
            }
            "providers/claude.svg" => Some(include_bytes!("../assets/providers/claude.svg")),
            "providers/codex.svg" => Some(include_bytes!("../assets/providers/codex.svg")),
            "providers/opencode.svg" => Some(include_bytes!("../assets/providers/opencode.svg")),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(if path == "providers" {
            ["antigravity.png", "claude.svg", "codex.svg", "opencode.svg"]
                .into_iter()
                .map(SharedString::from)
                .collect()
        } else {
            Vec::new()
        })
    }
}
