//! The single source of truth the popover renders from.
//!
//! Held as a `gpui::Entity` so the status-item callback, the refresh timer and
//! the views all mutate the same state and every observer re-renders.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use gpui::{Context, SharedString, Task};

use crate::model::{Prefs, Provider, View};
use crate::providers::{self, UsageSource};
use crate::theme;
use crate::update::UpdateStatus;

/// How often usage is re-fetched in the background.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct IntegrationStatus {
    pub id: SharedString,
    pub name: SharedString,
    pub logo: SharedString,
    pub badge: SharedString,
    pub badge_bg: u32,
    pub badge_fg: u32,
    pub error: Option<SharedString>,
    pub latency_ms: Option<u64>,
    pub last_success: Option<DateTime<Local>>,
    pub consecutive_failures: u32,
    pub retry_at: Option<DateTime<Local>>,
    pub setup_label: SharedString,
    pub setup_url: Option<SharedString>,
}

impl IntegrationStatus {
    pub fn needs_setup(&self) -> bool {
        self.error.as_ref().is_some_and(|error| {
            let error = error.to_lowercase();
            error.contains("credential")
                || error.contains("not configured")
                || error.contains("api key")
                || error.contains("login")
                || error.contains("not found")
                || error.contains("no such file")
                || error.contains("missing")
                || error.contains("signed in")
                || error.contains("token")
                || error.contains("http 401")
                || error.contains("http 403")
        })
    }

    pub fn freshness_label(&self) -> Option<String> {
        let last_success = self.last_success?;
        let seconds = (Local::now() - last_success).num_seconds().max(0);
        Some(if seconds < 60 {
            format!("{seconds}s ago")
        } else if seconds < 3600 {
            format!("{}m ago", seconds / 60)
        } else {
            format!("{}h ago", seconds / 3600)
        })
    }
}

pub struct AppState {
    pub providers: Vec<Provider>,
    pub integrations: Vec<IntegrationStatus>,
    pub prefs: Prefs,
    pub view: View,
    /// Provider ids whose detail section is open.
    expanded: HashSet<SharedString>,
    pub last_sync: Option<DateTime<Local>>,
    pub is_refreshing: bool,
    pub last_refresh_duration_ms: Option<u64>,
    pub update_status: UpdateStatus,
    pub support_notice: Option<SharedString>,
    refresh_pending: bool,
    refresh_started: Option<Instant>,
    sources: Arc<Vec<Box<dyn UsageSource>>>,
    _refresh: Option<Task<()>>,
    _update: Option<Task<()>>,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let sources = Arc::new(providers::all_sources());
        let integrations = sources
            .iter()
            .map(|source| {
                let descriptor = source.descriptor();
                IntegrationStatus {
                    id: descriptor.id.into(),
                    name: descriptor.name.into(),
                    logo: descriptor.logo.into(),
                    badge: descriptor.badge.into(),
                    badge_bg: descriptor.badge_bg,
                    badge_fg: descriptor.badge_fg,
                    error: None,
                    latency_ms: None,
                    last_success: None,
                    consecutive_failures: 0,
                    retry_at: None,
                    setup_label: descriptor.setup_label.into(),
                    setup_url: descriptor.setup_url.map(Into::into),
                }
            })
            .collect();
        let mut prefs = crate::settings::load();
        prefs.launch_at_login = crate::autostart::is_enabled();
        let mut this = Self {
            providers: Vec::new(),
            integrations,
            prefs,
            view: View::Usage,
            expanded: HashSet::from(["antigravity".into()]),
            last_sync: None,
            is_refreshing: true,
            last_refresh_duration_ms: None,
            update_status: UpdateStatus {
                checking: true,
                ..Default::default()
            },
            support_notice: None,
            refresh_pending: false,
            refresh_started: Some(Instant::now()),
            sources,
            _refresh: None,
            _update: None,
        };
        this._refresh = Some(this.spawn_refresh_loop(cx));
        this._update = Some(this.spawn_update_check(cx));
        this
    }

    fn spawn_update_check(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async { crate::update::check() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.update_status = match result {
                    Ok(status) => status,
                    Err(error) => UpdateStatus {
                        checking: false,
                        error: Some(error.to_string()),
                        ..Default::default()
                    },
                };
                cx.notify();
            });
        })
    }

    fn spawn_refresh_loop(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            let mut first_refresh = true;
            loop {
                if first_refresh {
                    first_refresh = false;
                } else {
                    cx.background_executor().timer(REFRESH_INTERVAL).await;
                    let Ok(acquired) = this.update(cx, |this, cx| {
                        if this.is_refreshing {
                            false
                        } else {
                            this.is_refreshing = true;
                            this.refresh_started = Some(Instant::now());
                            cx.notify();
                            true
                        }
                    }) else {
                        return;
                    };
                    if !acquired {
                        continue;
                    }
                }
                let Ok((sources, excluded)) = this.read_with(cx, |this, _| {
                    (this.sources.clone(), this.scheduled_exclusions())
                }) else {
                    return;
                };
                let fetched = cx
                    .background_executor()
                    .spawn(async move { providers::fetch_all(&sources, &excluded) })
                    .await;
                if this
                    .update(cx, |this, cx| this.apply_fetch(fetched, cx))
                    .is_err()
                {
                    return;
                }
            }
        })
    }

    /// Refresh now, off the main thread. Bound to ⌘R and "Refresh now".
    pub fn refresh_now(&mut self, cx: &mut Context<Self>) {
        if self.is_refreshing {
            self.refresh_pending = true;
            return;
        }
        self.is_refreshing = true;
        self.refresh_started = Some(Instant::now());
        cx.notify();
        let sources = self.sources.clone();
        let disabled = self.prefs.disabled_integrations.clone();
        cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move { providers::fetch_all(&sources, &disabled) })
                .await;
            let _ = this.update(cx, |this, cx| this.apply_fetch(fetched, cx));
        })
        .detach();
    }

    fn apply_fetch(&mut self, fetched: Vec<providers::FetchOutcome>, cx: &mut Context<Self>) {
        let mut any_success = false;
        for outcome in fetched {
            if self.prefs.disabled_integrations.contains(&outcome.id) {
                continue;
            }
            let Some(status) = self
                .integrations
                .iter_mut()
                .find(|status| status.id.as_ref() == outcome.id)
            else {
                continue;
            };
            status.latency_ms = Some(outcome.elapsed.as_millis().min(u64::MAX as u128) as u64);
            match outcome.result {
                Ok(provider) => {
                    any_success = true;
                    status.error = None;
                    status.last_success = Some(Local::now());
                    status.consecutive_failures = 0;
                    status.retry_at = None;
                    if outcome.id == "opencode-go"
                        && self
                            .support_notice
                            .as_ref()
                            .is_some_and(|notice| notice.contains("validating"))
                    {
                        self.support_notice = Some("OpenCode Go key verified".into());
                    }
                    for provider in provider {
                        if let Some(existing) = self
                            .providers
                            .iter_mut()
                            .find(|existing| existing.id == provider.id)
                        {
                            *existing = provider;
                        } else {
                            if provider.id.starts_with("antigravity:")
                                && self.expanded.contains(&SharedString::from("antigravity"))
                            {
                                self.expanded.insert(provider.id.clone());
                            }
                            self.providers.push(provider);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("headroom: source `{}` failed: {error:#}", outcome.id);
                    status.error = Some(format!("{error:#}").into());
                    status.consecutive_failures = status.consecutive_failures.saturating_add(1);
                    status.retry_at = Some(
                        Local::now()
                            + chrono::Duration::from_std(failure_backoff(
                                status.consecutive_failures,
                            ))
                            .unwrap_or_default(),
                    );
                }
            }
        }
        self.providers.sort_by_key(|provider| {
            self.integrations
                .iter()
                .position(|status| status.id == provider.id)
                .unwrap_or(usize::MAX)
        });
        if any_success {
            self.last_sync = Some(Local::now());
        }
        self.is_refreshing = false;
        if let Some(started) = self.refresh_started.take() {
            let duration = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            self.last_refresh_duration_ms = Some(duration);
            eprintln!("headroom: metric refresh_duration_ms={duration}");
        }
        cx.notify();
        if std::mem::take(&mut self.refresh_pending) {
            self.refresh_now(cx);
        }
    }

    pub fn is_expanded(&self, id: &SharedString) -> bool {
        self.expanded.contains(id)
    }

    pub fn toggle_expanded(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if !self.expanded.remove(id) {
            self.expanded.insert(id.clone());
        }
        cx.notify();
    }

    pub fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.view = view;
        cx.notify();
    }

    pub fn save_prefs(&self) {
        if let Err(error) = crate::settings::save(&self.prefs) {
            eprintln!("headroom: could not save preferences: {error:#}");
        }
    }

    pub fn integration_enabled(&self, id: &str) -> bool {
        !self.prefs.disabled_integrations.contains(id)
    }

    fn scheduled_exclusions(&self) -> HashSet<String> {
        let now = Local::now();
        let mut excluded = self.prefs.disabled_integrations.clone();
        excluded.extend(
            self.integrations
                .iter()
                .filter(|status| status.retry_at.is_some_and(|retry_at| retry_at > now))
                .map(|status| status.id.to_string()),
        );
        excluded
    }

    pub fn set_integration_enabled(&mut self, id: &str, enabled: bool, cx: &mut Context<Self>) {
        if enabled {
            if !self.prefs.disabled_integrations.remove(id) {
                return;
            }
            if let Some(status) = self
                .integrations
                .iter_mut()
                .find(|status| status.id.as_ref() == id)
            {
                status.error = None;
                status.latency_ms = None;
                status.consecutive_failures = 0;
                status.retry_at = None;
            }
            self.save_prefs();
            self.refresh_now(cx);
        } else {
            if !self.prefs.disabled_integrations.insert(id.to_string()) {
                return;
            }
            self.providers.retain(|provider| provider.id.as_ref() != id);
            if let Some(status) = self
                .integrations
                .iter_mut()
                .find(|status| status.id.as_ref() == id)
            {
                status.error = None;
                status.latency_ms = None;
                status.consecutive_failures = 0;
                status.retry_at = None;
            }
            self.save_prefs();
            cx.notify();
        }
    }

    pub fn adjust_warn_at(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.prefs.warn_at = (self.prefs.warn_at + delta).clamp(50.0, 95.0);
        self.save_prefs();
        cx.notify();
    }

    pub fn export_support_report(&mut self, cx: &mut Context<Self>) {
        self.support_notice = Some(match crate::diagnostics::export(self) {
            Ok(path) => format!("Saved {}", path.display()).into(),
            Err(error) => format!("Export failed: {error}").into(),
        });
        cx.notify();
    }

    /// The limit that most needs attention — what the menu bar shows.
    pub fn most_constrained(&self) -> Option<&Provider> {
        self.providers.iter().min_by(|a, b| {
            a.primary()
                .percent_left
                .total_cmp(&b.primary().percent_left)
        })
    }

    /// "synced 40s ago" / "synced 3m ago".
    pub fn synced_label(&self) -> String {
        if self.is_refreshing {
            return "updating…".into();
        }
        let Some(last_sync) = self.last_sync else {
            return "not synced".into();
        };
        let secs = (Local::now() - last_sync).num_seconds().max(0);
        if secs < 60 {
            format!("synced {secs}s ago")
        } else if secs < 3600 {
            format!("synced {}m ago", secs / 60)
        } else {
            format!("synced {}h ago", secs / 3600)
        }
    }

    pub fn panel_height(&self) -> f32 {
        match self.view {
            View::Usage => {
                let provider_count = self
                    .integrations
                    .iter()
                    .filter(|integration| self.integration_enabled(integration.id.as_ref()))
                    .map(|integration| {
                        let count = self
                            .providers
                            .iter()
                            .filter(|provider| {
                                provider.id.as_ref() == integration.id.as_ref()
                                    || provider.id.starts_with(&format!("{}:", integration.id))
                            })
                            .count();
                        count.max(1)
                    })
                    .sum::<usize>();
                let separators = provider_count.saturating_sub(1) as f32 * 0.5;
                let details = self
                    .providers
                    .iter()
                    .filter(|provider| self.is_expanded(&provider.id))
                    .map(|provider| {
                        let secondary = if self.prefs.only_show_active_limit {
                            0
                        } else {
                            provider.secondary().len()
                        };
                        theme::PANEL_DETAIL_HEIGHT
                            + secondary as f32 * theme::PANEL_SECONDARY_ROW_HEIGHT
                    })
                    .sum::<f32>();
                theme::PANEL_USAGE_CHROME_HEIGHT
                    + provider_count as f32 * theme::PANEL_PROVIDER_ROW_HEIGHT
                    + separators
                    + details
            }
            View::Prefs => theme::PANEL_PREFS_HEIGHT,
        }
    }
}

fn failure_backoff(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(4);
    Duration::from_secs(5 * 60 * (1_u64 << exponent))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::failure_backoff;

    #[test]
    fn provider_backoff_caps_at_eighty_minutes() {
        assert_eq!(failure_backoff(1), Duration::from_secs(5 * 60));
        assert_eq!(failure_backoff(2), Duration::from_secs(10 * 60));
        assert_eq!(failure_backoff(8), Duration::from_secs(80 * 60));
    }
}
