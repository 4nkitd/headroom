//! The single source of truth the popover renders from.
//!
//! Held as a `gpui::Entity` so the status-item callback, the refresh timer and
//! the views all mutate the same state and every observer re-renders.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local};
use gpui::{App, Context, Entity, Global, SharedString, Task};

use crate::model::{Prefs, Provider, View};
use crate::providers::{self, UsageSource};
use crate::theme::{self, Health};

/// How often usage is re-fetched in the background.
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

pub struct AppState {
    pub providers: Vec<Provider>,
    pub prefs: Prefs,
    pub view: View,
    /// Provider ids whose detail section is open.
    expanded: HashSet<SharedString>,
    pub last_sync: DateTime<Local>,
    sources: Arc<Vec<Box<dyn UsageSource>>>,
    _refresh: Option<Task<()>>,
}

/// Lets code holding only an `&mut App` — the status-item click handler, menu
/// actions — reach the state entity.
struct GlobalAppState(Entity<AppState>);

impl Global for GlobalAppState {}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let sources = Arc::new(providers::all_sources());
        let mut this = Self {
            providers: Vec::new(),
            prefs: Prefs::default(),
            view: View::Usage,
            expanded: HashSet::from(["antigravity".into()]),
            last_sync: Local::now(),
            sources,
            _refresh: None,
        };
        this._refresh = Some(this.spawn_refresh_loop(cx));
        this
    }

    pub fn set_global(entity: Entity<AppState>, cx: &mut App) {
        cx.set_global(GlobalAppState(entity));
    }

    pub fn global(cx: &App) -> Entity<AppState> {
        cx.global::<GlobalAppState>().0.clone()
    }

    pub fn try_global(cx: &App) -> Option<Entity<AppState>> {
        cx.try_global::<GlobalAppState>().map(|g| g.0.clone())
    }

    fn spawn_refresh_loop(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(sources) = this.read_with(cx, |this, _| this.sources.clone()) else {
                    // Entity dropped — the app is shutting down.
                    return;
                };
                let fetched = cx
                    .background_executor()
                    .spawn(async move { providers::fetch_all(&sources) })
                    .await;
                if this
                    .update(cx, |this, cx| this.apply_fetch(fetched, cx))
                    .is_err()
                {
                    return;
                }
                cx.background_executor().timer(REFRESH_INTERVAL).await;
            }
        })
    }

    /// Refresh now, off the main thread. Bound to ⌘R and "Refresh now".
    pub fn refresh_now(&mut self, cx: &mut Context<Self>) {
        let sources = self.sources.clone();
        cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move { providers::fetch_all(&sources) })
                .await;
            let _ = this.update(cx, |this, cx| this.apply_fetch(fetched, cx));
        })
        .detach();
    }

    fn apply_fetch(&mut self, fetched: Vec<Provider>, cx: &mut Context<Self>) {
        if fetched.is_empty() {
            // Every source failed; keep the last good numbers on screen rather
            // than flashing an empty panel.
            return;
        }
        self.providers = fetched;
        self.last_sync = Local::now();
        cx.notify();
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

    /// Health of a limit under the user's current warn threshold.
    pub fn health(&self, percent_left: f32) -> Health {
        Health::from_percent_left(percent_left, self.prefs.warn_at)
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
        let secs = (Local::now() - self.last_sync).num_seconds().max(0);
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
                let provider_count = self.providers.len();
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
