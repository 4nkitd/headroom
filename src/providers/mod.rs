//! Where usage numbers come from.
//!
//! Each backend implements [`UsageSource`] and normalises whatever it scrapes
//! into a [`Provider`]. The UI never learns which source produced a row, so a
//! real adapter can replace the sample one without touching the views.

pub mod live;

use crate::model::Provider;
use anyhow::Result;

pub trait UsageSource: Send + Sync {
    /// Stable key, matching [`Provider::id`].
    fn id(&self) -> &str;

    /// Fetch current usage. Called off the main thread, so it may block.
    fn fetch(&self) -> Result<Provider>;
}

/// Every source the app knows about, in display order.
pub fn all_sources() -> Vec<Box<dyn UsageSource>> {
    vec![
        Box::new(live::ClaudeCode),
        Box::new(live::OpenCodeGo),
        Box::new(live::Antigravity),
    ]
}

/// Fetch every source, dropping any that fail so one broken backend cannot
/// blank the whole panel.
pub fn fetch_all(sources: &[Box<dyn UsageSource>]) -> Vec<Provider> {
    std::thread::scope(|scope| {
        let jobs = sources
            .iter()
            .map(|source| {
                let id = source.id().to_string();
                scope.spawn(move || (id, source.fetch()))
            })
            .collect::<Vec<_>>();

        jobs.into_iter()
            .filter_map(|job| match job.join() {
                Ok((_id, Ok(provider))) => Some(provider),
                Ok((id, Err(error))) => {
                    eprintln!("headroom: source `{id}` failed: {error:#}");
                    None
                }
                Err(_) => None,
            })
            .collect()
    })
}
