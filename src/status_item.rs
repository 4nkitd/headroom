//! macOS status bar item + lazy popover window management.
//!
//! The status item's click handler runs as an Objective-C action, outside
//! GPUI's context, so it only flips an atomic flag. A GPUI task polls the
//! flag and owns the popover lifecycle: open on click, close on outside click
//! (window loses key status) or on a second click.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::{
    AnyWindowHandle, App, AppContext, AsyncApp, Bounds, Entity, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, point, px, size,
};

use crate::app_state::AppState;
use crate::theme;
use crate::ui::panel::Panel;

#[cfg(target_os = "macos")]
use objc2::define_class;
#[cfg(target_os = "macos")]
use objc2::extern_methods;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{MainThreadMarker, NSObject, NSString};

/// Set by the status item action, consumed by the popover loop.
static TOGGLE_REQUESTED: AtomicBool = AtomicBool::new(false);

/// The live status item as a raw pointer (NSStatusItem is !Sync; the status
/// bar owns it for the app's lifetime anyway).
struct StatusItemPtr(*mut std::ffi::c_void);
unsafe impl Send for StatusItemPtr {}
unsafe impl Sync for StatusItemPtr {}

static STATUS_ITEM: OnceLock<StatusItemPtr> = OnceLock::new();

const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// A freshly opened popover must have this long to become key before an
/// inactive status is treated as an outside click.
const ACTIVE_GRACE: Duration = Duration::from_millis(400);
/// A second click right after an outside-click close is the same click
/// closing the panel — swallow the reopen.
const TOGGLE_DEBOUNCE: Duration = Duration::from_millis(300);
const TITLE_REFRESH: Duration = Duration::from_secs(5);

/// What the menu bar shows: the most constrained provider's badge + headroom.
pub fn status_label(state: &AppState) -> String {
    match state.most_constrained() {
        Some(p) if state.prefs.show_percentage_in_menu_bar => {
            format!("{} {}%", p.badge, p.primary().percent_left.round() as i32)
        }
        Some(p) => p.badge.to_string(),
        None => "?".into(),
    }
}

#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "HeadroomStatusTarget"]
    struct StatusTarget;

    impl StatusTarget {
        #[unsafe(method(action_clicked:))]
        fn action_clicked(&self, _sender: &AnyObject) {
            TOGGLE_REQUESTED.store(true, Ordering::Relaxed);
        }
    }
);

#[cfg(target_os = "macos")]
impl StatusTarget {
    extern_methods!(
        #[unsafe(method(new))]
        fn new() -> Retained<Self>;
    );
}

#[cfg(target_os = "macos")]
pub fn setup_status_bar_item(cx: &mut App, state: Entity<AppState>) {
    if let Some(mtm) = MainThreadMarker::new() {
        let application = NSApplication::sharedApplication(mtm);
        application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        let status_bar = NSStatusBar::systemStatusBar();
        let item = status_bar.statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            button.setTitle(&NSString::from_str(&status_label(state.read(cx))));
            let target = StatusTarget::new();
            unsafe {
                button.setTarget(Some(&target));
                button.setAction(Some(objc2::sel!(action_clicked:)));
            }
            // Both must live for the app's lifetime. Leak the item (the
            // status bar does not retain it) and keep the target retained by
            // the button.
            let _ = STATUS_ITEM.set(StatusItemPtr(Retained::as_ptr(&item) as *mut _));
            std::mem::forget(item);
            std::mem::forget(target);
        }
    }

    let cx = cx.to_async();
    cx.spawn(async move |cx| popover_loop(cx, state).await)
        .detach();
}

#[cfg(target_os = "macos")]
fn set_status_title(label: &str) {
    if let Some(mtm) = MainThreadMarker::new() {
        if let Some(ptr) = STATUS_ITEM.get() {
            let item: &NSStatusItem = unsafe { &*(ptr.0 as *const NSStatusItem) };
            if let Some(button) = item.button(mtm) {
                button.setTitle(&NSString::from_str(label));
            }
        }
    }
}

#[cfg(target_os = "macos")]
async fn popover_loop(cx: &mut AsyncApp, state: Entity<AppState>) {
    let mut window: Option<AnyWindowHandle> = None;
    let mut opened_at: Option<Instant> = None;
    let mut closed_at: Option<Instant> = None;
    let mut last_title = Instant::now() - TITLE_REFRESH;

    loop {
        cx.background_executor().timer(POLL_INTERVAL).await;
        let now = Instant::now();

        if now.duration_since(last_title) >= TITLE_REFRESH {
            last_title = now;
            if let Ok(label) = cx.read_entity(&state, |s, _| status_label(s)) {
                set_status_title(&label);
            }
        }

        if TOGGLE_REQUESTED.swap(false, Ordering::Relaxed) {
            if let Some(handle) = window.take() {
                let _ = handle.update(cx, |_, w, _| w.remove_window());
                opened_at = None;
                closed_at = Some(now);
            } else if closed_at.is_none_or(|t| now.duration_since(t) >= TOGGLE_DEBOUNCE) {
                match open_popover(cx, state.clone()) {
                    Ok(handle) => {
                        window = Some(handle.into());
                        opened_at = Some(now);
                    }
                    Err(err) => eprintln!("headroom: failed to open popover: {err:#}"),
                }
            }
        }

        // Outside click: the popover resigns key — close it.
        if let Some(handle) = &window {
            let active = handle
                .update(cx, |_, w, _| w.is_window_active())
                .unwrap_or(false);
            let past_grace = opened_at
                .map(|t| now.duration_since(t) >= ACTIVE_GRACE)
                .unwrap_or(true);
            if !active && past_grace {
                let _ = handle.update(cx, |_, w, _| w.remove_window());
                window = None;
                opened_at = None;
                closed_at = Some(now);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn open_popover(cx: &mut AsyncApp, state: Entity<AppState>) -> anyhow::Result<WindowHandle<Panel>> {
    let width = theme::PANEL_WIDTH;
    let height = cx
        .read_entity(&state, |state, _| state.panel_height())
        .unwrap_or(theme::PANEL_PREFS_HEIGHT);
    let screen: Bounds<gpui::Pixels> = cx
        .update(|app| app.primary_display().map(|d| d.bounds()))
        .ok()
        .flatten()
        .unwrap_or_else(|| Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1440.), px(900.)),
        });
    let x = (screen.size.width - px(width) - px(theme::PANEL_EDGE_MARGIN)).max(px(0.));

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(x, px(theme::PANEL_TOP_INSET)),
            size: size(px(width), px(height)),
        })),
        titlebar: None,
        kind: WindowKind::PopUp,
        window_background: WindowBackgroundAppearance::Opaque,
        is_movable: false,
        focus: true,
        display_id: None,
        ..Default::default()
    };

    cx.open_window(options, move |_, cx| {
        cx.new(|cx| Panel::new(state.clone(), cx))
    })
}
