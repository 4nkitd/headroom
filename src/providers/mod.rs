//! Where usage numbers come from.
//!
//! Each backend implements [`UsageSource`] and normalises its HTTP response
//! into a [`Provider`]. The UI never learns which source produced a row, so a
//! provider adapter can change without touching the views.

pub mod live;

use crate::model::Provider;
use anyhow::Result;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub struct SourceDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub logo: &'static str,
    pub badge: &'static str,
    pub badge_bg: u32,
    pub badge_fg: u32,
    pub setup_label: &'static str,
    pub setup_url: Option<&'static str>,
}

pub struct FetchOutcome {
    pub id: String,
    pub elapsed: Duration,
    pub result: Result<Provider>,
}

pub trait UsageSource: Send + Sync {
    fn descriptor(&self) -> SourceDescriptor;

    /// Fetch current usage. Called off the main thread, so it may block.
    fn fetch(&self) -> Result<Provider>;
}

/// Every source the app knows about, in display order.
pub fn all_sources() -> Vec<Box<dyn UsageSource>> {
    vec![
        Box::new(live::ClaudeCode),
        Box::new(live::OpenAiCodex),
        Box::new(live::OpenCodeGo),
        Box::new(live::Antigravity),
    ]
}

/// Fetch every source independently so one slow backend cannot block another.
pub fn fetch_all(
    sources: &[Box<dyn UsageSource>],
    excluded_integrations: &HashSet<String>,
) -> Vec<FetchOutcome> {
    std::thread::scope(|scope| {
        let jobs = sources
            .iter()
            .filter(|source| !excluded_integrations.contains(source.descriptor().id))
            .map(|source| {
                let id = source.descriptor().id.to_string();
                scope.spawn(move || {
                    let started = Instant::now();
                    let result = source.fetch();
                    FetchOutcome {
                        id,
                        elapsed: started.elapsed(),
                        result,
                    }
                })
            })
            .collect::<Vec<_>>();

        jobs.into_iter().filter_map(|job| job.join().ok()).collect()
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{SourceDescriptor, UsageSource, fetch_all};
    use crate::model::{Cadence, Limit, Provider};

    struct FakeSource {
        id: &'static str,
        calls: Arc<AtomicUsize>,
    }

    impl UsageSource for FakeSource {
        fn descriptor(&self) -> SourceDescriptor {
            SourceDescriptor {
                id: self.id,
                name: self.id,
                logo: "",
                badge: "T",
                badge_bg: 0,
                badge_fg: 0,
                setup_label: "Set up",
                setup_url: None,
            }
        }

        fn fetch(&self) -> anyhow::Result<Provider> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Provider {
                id: self.id.into(),
                name: self.id.into(),
                logo: "".into(),
                badge: "T".into(),
                badge_bg: 0,
                badge_fg: 0,
                plan: "Test".into(),
                console_url: "https://example.com".into(),
                source_label: "Test HTTP API".into(),
                limits: vec![Limit::new(Cadence::Daily, 100.0)],
            })
        }
    }

    #[test]
    fn disabled_sources_are_never_fetched() {
        let disabled_calls = Arc::new(AtomicUsize::new(0));
        let enabled_calls = Arc::new(AtomicUsize::new(0));
        let sources: Vec<Box<dyn UsageSource>> = vec![
            Box::new(FakeSource {
                id: "disabled",
                calls: disabled_calls.clone(),
            }),
            Box::new(FakeSource {
                id: "enabled",
                calls: enabled_calls.clone(),
            }),
        ];
        let disabled = HashSet::from(["disabled".to_string()]);

        let outcomes = fetch_all(&sources, &disabled);

        assert_eq!(disabled_calls.load(Ordering::SeqCst), 0);
        assert_eq!(enabled_calls.load(Ordering::SeqCst), 1);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].id, "enabled");
    }
}
