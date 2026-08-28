mod antigravity;
mod claude;
mod codex;
mod http;
mod opencode;

use chrono::{DateTime, Local, Utc};
use serde_json::Value;

pub use antigravity::Antigravity;
pub use claude::ClaudeCode;
pub use codex::OpenAiCodex;
pub use opencode::OpenCodeGo;

fn parse_reset_time(value: Option<&Value>) -> Option<String> {
    let date = match value? {
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
    if date.date_naive() == Local::now().date_naive() {
        Some(date.format("%-I:%M%P").to_string())
    } else {
        Some(date.format("%b %-d %-I:%M%P").to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn usage_adapters_are_http_only() {
        let source = [
            include_str!("live/http.rs"),
            include_str!("live/claude.rs"),
            include_str!("live/codex.rs"),
            include_str!("live/opencode.rs"),
            include_str!("live/antigravity.rs"),
        ]
        .join("\n");
        for forbidden in [
            ["Command", "::new"].concat(),
            ["std::", "process"].concat(),
            ["sqlite", "3"].concat(),
            ["curl", "_request"].concat(),
        ] {
            assert!(
                !source.contains(&forbidden),
                "found forbidden usage path: {forbidden}"
            );
        }
    }
}
