//! The Preferences pane: global controls, connected accounts, and warning
//! threshold slider.

use gpui::{Context, Entity, IntoElement, SharedString, Styled, div, prelude::*, px, rgb};

use crate::app_state::AppState;
use crate::credentials;
use crate::model::{Prefs, Provider, View};
use crate::theme;
use crate::ui::Fonts;
use crate::ui::panel::Panel;
use crate::ui::text_input::SecretInput;
use crate::ui::widgets::{badge, divider, section_label, slider, toggle};

pub fn render(
    entity: &Entity<AppState>,
    _fonts: &Fonts,
    api_key_input: Entity<SecretInput>,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let state = entity.read(cx);
    let prefs = state.prefs.clone();
    let providers = state.providers.clone();
    let opencode_go_status = credentials::opencode_go_key_status();

    div()
        .flex()
        .flex_col()
        .child(header(entity, cx))
        .child(divider().mx(px(14.)))
        .child(body(
            prefs,
            providers,
            opencode_go_status,
            api_key_input,
            entity,
            cx,
        ))
}

fn header(entity: &Entity<AppState>, cx: &mut Context<Panel>) -> impl IntoElement {
    let back = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.set_view(View::Usage, cx));
        })
    };

    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .pt(px(11.))
        .px(px(14.))
        .pb(px(9.))
        .child(
            div()
                .id("back-button")
                .text_size(px(12.))
                .text_color(theme::c(theme::TEXT_ACTION))
                .hover(|s| s.text_color(theme::c(0xffffffff)))
                .cursor_pointer()
                .on_click(back)
                .child("\u{2039} Back"),
        )
        .child(section_label("Preferences"))
}

fn body(
    prefs: Prefs,
    providers: Vec<Provider>,
    opencode_go_status: String,
    api_key_input: Entity<SecretInput>,
    entity: &Entity<AppState>,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let toggle_show_pct = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| {
                state.prefs.show_percentage_in_menu_bar = !state.prefs.show_percentage_in_menu_bar;
                cx.notify();
            });
        })
    };

    let toggle_only_active = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| {
                state.prefs.only_show_active_limit = !state.prefs.only_show_active_limit;
                cx.notify();
            });
        })
    };

    let toggle_launch = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| {
                state.prefs.launch_at_login = !state.prefs.launch_at_login;
                cx.notify();
            });
        })
    };

    let save_opencode_go_key = {
        let entity = entity.clone();
        let api_key_input = api_key_input.clone();
        cx.listener(move |_, _, _, cx| {
            let key = api_key_input.read(cx).value();
            if key.trim().is_empty() {
                return;
            }
            if let Err(error) = credentials::save_opencode_go_api_key(&key) {
                eprintln!("headroom: could not save OpenCode Go key: {error:#}");
                return;
            }
            let _ = api_key_input.update(cx, |input, cx| input.clear(cx));
            let _ = entity.update(cx, |_, cx| cx.notify());
        })
    };

    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(14.))
        .child(
            div()
                .id("pref-show-percentage")
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .cursor_pointer()
                .on_click(toggle_show_pct)
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme::c(theme::TEXT))
                        .child("Show percentage in menu bar"),
                )
                .child(toggle(prefs.show_percentage_in_menu_bar)),
        )
        .child(
            div()
                .id("pref-only-active")
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .cursor_pointer()
                .on_click(toggle_only_active)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme::c(theme::TEXT))
                                .child("Only show active limit"),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .mt(px(1.))
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child("Weekly and monthly stay collapsed"),
                        ),
                )
                .child(toggle(prefs.only_show_active_limit)),
        )
        .child(
            div()
                .id("pref-launch-login")
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .cursor_pointer()
                .on_click(toggle_launch)
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme::c(theme::TEXT))
                        .child("Launch at login"),
                )
                .child(toggle(prefs.launch_at_login)),
        )
        .child(
            div()
                .id("pref-opencode-go-key")
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme::c(theme::TEXT))
                                .child("OpenCode Go API key"),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .mt(px(1.))
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child(opencode_go_status),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().w(px(142.)).child(api_key_input.clone()))
                        .child(
                            div()
                                .id("save-opencode-go-key")
                                .cursor_pointer()
                                .text_size(px(12.))
                                .text_color(rgb(theme::LINK))
                                .hover(|s| s.text_color(rgb(theme::LINK_HOVER)))
                                .on_click(save_opencode_go_key)
                                .child("Save"),
                        ),
                ),
        )
        .child(divider().my(px(2.)))
        .child(section_label("Connected accounts"))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(9.))
                .children(providers.into_iter().map(|p| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(badge(p.badge.clone(), p.badge_bg, p.badge_fg, 18.0, 5.0))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(13.))
                                .text_color(theme::c(theme::TEXT))
                                .child(p.name.clone()),
                        )
                        .child(
                            div()
                                .font_family(SharedString::from("IBM Plex Mono"))
                                .text_size(px(11.))
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child(p.plan.clone()),
                        )
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .opacity(0.5)
                        .child(
                            div()
                                .size(px(18.))
                                .rounded(px(5.))
                                .bg(theme::c(0xffffff24))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11.))
                                .child("+"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(13.))
                                .text_color(theme::c(theme::TEXT))
                                .child("Add a subscription\u{2026}"),
                        ),
                ),
        )
        .child(divider().my(px(2.)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme::c(theme::TEXT))
                        .child("Warn me at"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(slider(prefs.warn_at / 100.0, 120.0))
                        .child(
                            div()
                                .font_family(SharedString::from("IBM Plex Mono"))
                                .text_size(px(11.))
                                .text_color(theme::c(theme::TEXT_DETAIL))
                                .child(format!("{}%", prefs.warn_at as i32)),
                        ),
                ),
        )
}
