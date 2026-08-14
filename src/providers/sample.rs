//! Stand-in sources that reproduce the numbers from the design mock.
//!
//! These exist so the app is runnable and the layout is exercised end to end.
//! Swap any one of them for a real adapter by implementing [`UsageSource`]
//! against the same provider id.

use super::UsageSource;
use crate::model::{Burn, Cadence, Limit, Provider, Trend};
use anyhow::Result;

pub fn sources() -> Vec<Box<dyn UsageSource>> {
    vec![
        Box::new(ClaudeCode),
        Box::new(OpenCode),
        Box::new(GeminiCli),
    ]
}

pub struct ClaudeCode;

impl UsageSource for ClaudeCode {
    fn id(&self) -> &str {
        "claude-code"
    }

    fn fetch(&self) -> Result<Provider> {
        Ok(Provider {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            badge: "C".into(),
            badge_bg: 0xd97757,
            badge_fg: 0x2b1206,
            plan: "Max 20\u{00d7}".into(),
            console_url: "https://claude.ai/settings/usage".into(),
            limits: vec![
                Limit::new(Cadence::Session, 32.0).resets_at("11:55"),
                Limit::new(Cadence::Weekly, 41.0),
                Limit::new(Cadence::Monthly, 66.0),
            ],
            burn: Burn::new(
                vec![0.20, 0.30, 0.25, 0.55, 0.45, 0.70, 0.65, 0.85, 0.80],
                "burn rate rising",
                Trend::Rising,
            ),
        })
    }
}

pub struct OpenCode;

impl UsageSource for OpenCode {
    fn id(&self) -> &str {
        "opencode"
    }

    fn fetch(&self) -> Result<Provider> {
        Ok(Provider {
            id: "opencode".into(),
            name: "OpenCode".into(),
            badge: "O".into(),
            badge_bg: 0xe8e8ea,
            badge_fg: 0x16171a,
            plan: "Pro".into(),
            console_url: "https://opencode.ai".into(),
            limits: vec![
                Limit::new(Cadence::Daily, 78.0).resets_at("00:00"),
                Limit::new(Cadence::Weekly, 70.0),
            ],
            burn: Burn::new(
                vec![0.40, 0.35, 0.45, 0.30, 0.40, 0.25, 0.35, 0.30, 0.35],
                "steady",
                Trend::Steady,
            ),
        })
    }
}

pub struct GeminiCli;

impl UsageSource for GeminiCli {
    fn id(&self) -> &str {
        "gemini-cli"
    }

    fn fetch(&self) -> Result<Provider> {
        Ok(Provider {
            id: "gemini-cli".into(),
            name: "Gemini CLI".into(),
            badge: "G".into(),
            badge_bg: 0x4285f4,
            badge_fg: 0xffffff,
            plan: "Free tier".into(),
            console_url: "https://aistudio.google.com".into(),
            limits: vec![
                Limit::new(Cadence::Daily, 9.0).resets_at("15:59"),
                Limit::new(Cadence::Monthly, 40.0),
            ],
            burn: Burn::new(
                vec![0.15, 0.25, 0.20, 0.45, 0.65, 0.60, 0.80, 0.90, 0.95],
                "exhausted by ~15:20",
                Trend::Rising,
            ),
        })
    }
}
