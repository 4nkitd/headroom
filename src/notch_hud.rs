//! macOS Hardware Notch HUD Overlay System (AppKit + GPUI).
//!
//! Renders a Dynamic Island / Hardware Notch surface directly under the MacBook
//! camera housing (`NSScreen.safeAreaInsets.top`), displaying active account headroom
//! in resting state (split wings) and expanding on hover/click to reveal detailed quota rails.

use std::time::Duration;

use gpui::{
    AnyWindowHandle, App, AppContext, AsyncApp, Bounds, Context, Entity, FontWeight, IntoElement,
    Render, Styled, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, div,
    point, prelude::*, px, rgb, size,
};

use crate::app_state::AppState;
use crate::model::truncate_account_label;
use crate::theme::{self, Health};
use crate::ui::Fonts;
use crate::ui::widgets::{badge, striped_bar};

#[cfg(target_os = "macos")]
use objc2_app_kit::NSScreen;
#[cfg(target_os = "macos")]
use objc2_foundation::MainThreadMarker;

#[allow(dead_code)]
const RESTING_WIDTH: f32 = 220.0;
const RESTING_HEIGHT: f32 = 38.0;
const EXPANDED_WIDTH: f32 = 380.0;
const EXPANDED_HEIGHT: f32 = 180.0;
const TOP_SAFE_PADDING: f32 = 38.0;

const NOTCH_POLL_INTERVAL: Duration = Duration::from_millis(200);

#[cfg(target_os = "macos")]
fn get_main_screen_notch_info() -> (f32, f32, f32, f32) {
    if let Some(mtm) = MainThreadMarker::new()
        && let Some(screen) = NSScreen::mainScreen(mtm)
    {
        let frame = screen.frame();
        let safe_insets = screen.safeAreaInsets();
        let top_safe = safe_insets.top as f32;
        let left = screen.auxiliaryTopLeftArea();
        let right = screen.auxiliaryTopRightArea();
        let notch_w =
            if left.size.width > 0.0 && right.size.width > 0.0 && right.origin.x > left.origin.x {
                (right.origin.x - (left.origin.x + left.size.width)) as f32
            } else {
                210.0
            };
        let safe_padding = if top_safe > 0.0 {
            top_safe + 4.0
        } else {
            TOP_SAFE_PADDING
        };
        (frame.size.width as f32, top_safe, safe_padding, notch_w)
    } else {
        (1800.0, 38.0, TOP_SAFE_PADDING, 210.0)
    }
}

#[cfg(not(target_os = "macos"))]
fn get_main_screen_notch_info() -> (f32, f32, f32, f32) {
    (1800.0, 38.0, TOP_SAFE_PADDING, 210.0)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotchState {
    Resting,
    HoverPreview,
    Expanded,
}

pub struct NotchHudView {
    state: Entity<AppState>,
    hud_state: NotchState,
}

impl NotchHudView {
    pub fn new(state: Entity<AppState>, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
            hud_state: NotchState::Resting,
        }
    }

    fn set_state(&mut self, next: NotchState, cx: &mut Context<Self>) {
        if self.hud_state != next {
            self.hud_state = next;
            cx.notify();
        }
    }
}

impl Render for NotchHudView {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fonts = cx.global::<Fonts>().clone();
        let state = self.state.read(cx);

        let most_constrained = state.most_constrained();
        let lowest_percent = most_constrained
            .map(|p| p.primary().percent_left)
            .unwrap_or(100.0);
        let is_alert = lowest_percent < 15.0;

        let primary_account_label = most_constrained
            .map(|p| truncate_account_label(p.id.as_ref()))
            .unwrap_or_else(|| "Head".into());

        let primary_pct_label = format!("{}%", lowest_percent.round() as i32);
        let health = Health::from_percent_left(lowest_percent, state.prefs.warn_at);

        let current_state = self.hud_state;

        let on_click = cx.listener(|this, _, _, cx| {
            if this.hud_state == NotchState::Expanded {
                this.set_state(NotchState::HoverPreview, cx);
            } else {
                this.set_state(NotchState::Expanded, cx);
            }
        });

        let (_screen_w, top_safe, safe_padding, notch_w) = get_main_screen_notch_info();

        let (target_w, target_h) = match current_state {
            NotchState::Resting => (notch_w + 110.0, RESTING_HEIGHT),
            NotchState::HoverPreview => (notch_w + 150.0, RESTING_HEIGHT + 28.0),
            NotchState::Expanded => (EXPANDED_WIDTH, EXPANDED_HEIGHT),
        };

        window.resize(size(px(target_w), px(target_h)));

        match current_state {
            NotchState::Resting => {
                let on_hover_resting = cx.listener(|this, hovered: &bool, _, cx| {
                    if *hovered && this.hud_state == NotchState::Resting {
                        this.set_state(NotchState::HoverPreview, cx);
                    }
                });
                div()
                    .id("notch-resting-container")
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .id("notch-hud-resting")
                            .w(px(notch_w + 110.0))
                            .h(px(RESTING_HEIGHT))
                            .bg(rgb(0x000000))
                            .rounded_b(px(12.))
                            .border_1()
                            .border_color(if is_alert {
                                rgb(theme::WARN)
                            } else {
                                theme::c(theme::PANEL_BORDER)
                            })
                            .flex()
                            .items_center()
                            .justify_between()
                            .pt(px((top_safe - 24.0).max(4.0)))
                            .px(px(12.))
                            .pb(px(4.))
                            .cursor_pointer()
                            .on_hover(on_hover_resting)
                            .on_click(on_click)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .size(px(14.))
                                            .rounded(px(3.))
                                            .bg(rgb(theme::LINK))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(9.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child("H"),
                                    )
                                    .child(
                                        div()
                                            .font_family(fonts.mono.clone())
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme::c(theme::TEXT))
                                            .child(primary_account_label),
                                    ),
                            )
                            .child(div().w(px(notch_w - 20.0)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .font_family(fonts.mono.clone())
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(health.color())
                                            .child(primary_pct_label),
                                    )
                                    .child(
                                        div()
                                            .font_family(fonts.mono.clone())
                                            .text_size(px(9.))
                                            .text_color(theme::c(theme::TEXT_MUTED))
                                            .child("5h"),
                                    ),
                            ),
                    )
            }

            NotchState::HoverPreview => {
                let on_hover_preview = cx.listener(|this, hovered: &bool, _, cx| {
                    if !*hovered && this.hud_state == NotchState::HoverPreview {
                        this.set_state(NotchState::Resting, cx);
                    }
                });
                let primary_name = most_constrained
                    .map(|p| p.name.to_string())
                    .unwrap_or_else(|| "Headroom HUD".into());
                let primary_cadence = most_constrained
                    .map(|p| p.primary().display_label().to_string())
                    .unwrap_or_else(|| "Quota".into());
                let primary_reset = most_constrained
                    .and_then(|p| p.primary().resets_at.as_ref())
                    .map(|r| r.to_string());

                div()
                    .id("notch-preview-container")
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .id("notch-hud-preview")
                            .w(px(notch_w + 150.0))
                            .h(px(RESTING_HEIGHT + 28.0))
                            .bg(rgb(0x000000))
                            .rounded_b(px(14.))
                            .border_1()
                            .border_color(if is_alert {
                                rgb(theme::WARN)
                            } else {
                                theme::c(theme::LINK)
                            })
                            .flex()
                            .flex_col()
                            .justify_between()
                            .px(px(12.))
                            .py(px(6.))
                            .cursor_pointer()
                            .on_hover(on_hover_preview)
                            .on_click(on_click)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .size(px(14.))
                                                    .rounded(px(3.))
                                                    .bg(rgb(theme::LINK))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(9.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(0xffffff))
                                                    .child("H"),
                                            )
                                            .child(
                                                div()
                                                    .font_family(fonts.mono.clone())
                                                    .text_size(px(11.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::c(theme::TEXT))
                                                    .child(primary_name),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family(fonts.mono.clone())
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(health.color())
                                            .child(format!("{lowest_percent:.1}%")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_size(px(10.))
                                    .text_color(theme::c(theme::TEXT_MUTED))
                                    .child(primary_cadence)
                                    .child(
                                        div().font_family(fonts.mono.clone()).child(
                                            primary_reset.unwrap_or_else(|| "active".into()),
                                        ),
                                    ),
                            )
                            .child(striped_bar(
                                lowest_percent / 100.0,
                                health.color(),
                                theme::SUB_BAR_HEIGHT,
                            )),
                    )
            }

            NotchState::Expanded => {
                let on_hover_expanded = cx.listener(|this, hovered: &bool, _, cx| {
                    if !*hovered && this.hud_state == NotchState::Expanded {
                        this.set_state(NotchState::Resting, cx);
                    }
                });
                div()
                    .id("notch-expanded-container")
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .id("notch-hud-expanded")
                            .w(px(EXPANDED_WIDTH))
                            .h(px(EXPANDED_HEIGHT))
                            .bg(rgb(0x000000))
                            .rounded_b(px(18.))
                            .border_1()
                            .border_color(if is_alert {
                                rgb(theme::WARN)
                            } else {
                                theme::c(theme::PANEL_BORDER)
                            })
                            .flex()
                            .flex_col()
                            .pt(px(safe_padding))
                            .px(px(16.))
                            .pb(px(12.))
                            .gap(px(10.))
                            .cursor_pointer()
                            .on_hover(on_hover_expanded)
                            .on_click(on_click)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .pb(px(6.))
                                    .border_b_1()
                                    .border_color(theme::c(theme::DIVIDER))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .size(px(18.))
                                                    .rounded(px(4.))
                                                    .bg(rgb(theme::LINK))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(0xffffff))
                                                    .child("H"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::c(theme::TEXT))
                                                    .child(
                                                        most_constrained
                                                            .map(|p| p.name.clone())
                                                            .unwrap_or_else(|| {
                                                                "Headroom HUD".into()
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .px(px(5.))
                                                    .py(px(1.))
                                                    .rounded(px(4.))
                                                    .bg(theme::c(theme::ROW_HOVER))
                                                    .font_family(fonts.mono.clone())
                                                    .text_size(px(10.))
                                                    .text_color(theme::c(theme::TEXT_MUTED))
                                                    .child(primary_account_label),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family(fonts.mono.clone())
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(health.color())
                                            .child(format!("{lowest_percent:.1}% remaining")),
                                    ),
                            )
                            .child(div().flex().flex_col().gap(px(8.)).children(
                                state.providers.iter().take(3).map(|provider| {
                                    let primary = provider.primary();
                                    let h = Health::from_percent_left(
                                        primary.percent_left,
                                        state.prefs.warn_at,
                                    );
                                    let tag = truncate_account_label(provider.id.as_ref());
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(3.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .text_size(px(11.))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(6.))
                                                        .child(badge(
                                                            provider.logo.clone(),
                                                            provider.badge.clone(),
                                                            provider.badge_bg,
                                                            provider.badge_fg,
                                                            14.0,
                                                            3.0,
                                                        ))
                                                        .child(
                                                            div()
                                                                .font_family(fonts.mono.clone())
                                                                .text_color(theme::c(theme::TEXT))
                                                                .child(tag),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .font_family(fonts.mono.clone())
                                                        .text_color(h.color())
                                                        .child(format!(
                                                            "{}%",
                                                            primary.percent_left.round() as i32
                                                        )),
                                                ),
                                        )
                                        .child(striped_bar(
                                            primary.fraction_left(),
                                            h.color(),
                                            theme::SUB_BAR_HEIGHT,
                                        ))
                                }),
                            )),
                    )
            }
        }
    }
}

pub fn setup_notch_hud(cx: &mut App, state: Entity<AppState>) {
    let cx = cx.to_async();
    cx.spawn(async move |cx| notch_hud_loop(cx, state).await)
        .detach();
}

async fn notch_hud_loop(cx: &mut AsyncApp, state: Entity<AppState>) {
    let mut window: Option<AnyWindowHandle> = None;

    loop {
        cx.background_executor().timer(NOTCH_POLL_INTERVAL).await;

        let enabled = match cx.read_entity(&state, |s, _| s.prefs.enable_notch_hud) {
            Ok(res) => res,
            Err(_) => return,
        };

        if enabled && window.is_none() {
            let screen_w: f32 = cx
                .update(|app| {
                    app.primary_display()
                        .map(|d| f32::from(d.bounds().size.width))
                })
                .ok()
                .flatten()
                .unwrap_or(1800.0);

            let width = EXPANDED_WIDTH;
            let height = EXPANDED_HEIGHT;
            let x = px((screen_w - width) / 2.0);
            let y = px(-14.0);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(x, y),
                    size: size(px(width), px(height)),
                })),
                titlebar: None,
                kind: WindowKind::PopUp,
                window_background: WindowBackgroundAppearance::Transparent,
                is_movable: false,
                focus: false,
                display_id: None,
                ..Default::default()
            };

            let state = state.clone();
            if let Ok(handle) = cx.open_window(options, move |_, cx| {
                cx.new(|cx| NotchHudView::new(state.clone(), cx))
            }) {
                window = Some(handle.into());
            }
        } else if !enabled && let Some(handle) = window.take() {
            let _ = handle.update(cx, |_, w, _| w.remove_window());
        }
    }
}
