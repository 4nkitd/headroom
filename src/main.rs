//! Headroom: macOS Menu Bar AI Subscription Usage Tracker in Rust + GPUI.

mod app_state;
mod credentials;
mod model;
mod providers;
mod theme;
mod ui;

#[cfg(target_os = "macos")]
mod status_item;

use app_state::AppState;
use gpui::{App, AppContext, Application};

fn main() {
    let app = Application::new();

    app.run(move |cx: &mut App| {
        ui::text_input::bind_keys(cx);
        let state = cx.new(|cx| AppState::new(cx));
        AppState::set_global(state.clone(), cx);

        // No window at startup — the popover opens on demand from the menu bar.
        #[cfg(target_os = "macos")]
        status_item::setup_status_bar_item(cx, state);
    });
}
