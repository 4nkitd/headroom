//! The "Current limits" pane: one row per provider, expandable into its
//! secondary limits and API provenance.

use gpui::{
    Context, Entity, FontWeight, IntoElement, SharedString, Styled, div, prelude::*, px, rgb,
};

use crate::app_state::{AppState, IntegrationStatus};
use crate::model::{Limit, Provider, View};
use crate::theme::{self, Health};
use crate::ui::Activate;
use crate::ui::Fonts;
use crate::ui::panel::Panel;
use crate::ui::widgets::{badge, divider, section_label, striped_bar};

#[derive(Clone, Copy)]
struct RowOptions {
    warn_at: f32,
    only_active: bool,
    tab_index: isize,
}

pub fn render(
    entity: &Entity<AppState>,
    fonts: &Fonts,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let state = entity.read(cx);
    let synced_label = state.synced_label();
    let options = RowOptions {
        warn_at: state.prefs.warn_at,
        only_active: state.prefs.only_show_active_limit,
        tab_index: 0,
    };
    let refreshing = state.is_refreshing;

    let rows = state
        .integrations
        .iter()
        .filter(|status| state.integration_enabled(status.id.as_ref()))
        .flat_map(|status| {
            let providers = state
                .providers
                .iter()
                .filter(|provider| {
                    provider.id.as_ref() == status.id.as_ref()
                        || provider.id.starts_with(&format!("{}:", status.id))
                })
                .cloned()
                .collect::<Vec<_>>();
            if providers.is_empty() {
                vec![(status.clone(), None, false)]
            } else {
                providers
                    .into_iter()
                    .map(|provider| {
                        let expanded = state.is_expanded(&provider.id);
                        (status.clone(), Some(provider), expanded)
                    })
                    .collect()
            }
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .child(header(synced_label, fonts))
        .child(
            div()
                .flex()
                .flex_col()
                .children(
                    rows.into_iter()
                        .enumerate()
                        .map(|(i, (status, provider, expanded))| {
                            let tab_index = 10 + i as isize;
                            let row_options = RowOptions {
                                tab_index,
                                ..options
                            };
                            let row = div()
                                .flex()
                                .flex_col()
                                .when(i > 0, |d| d.child(divider().mx(px(14.))));
                            if let Some(provider) = provider {
                                row.child(provider_row(
                                    provider,
                                    status,
                                    expanded,
                                    row_options,
                                    entity,
                                    fonts,
                                    cx,
                                ))
                            } else {
                                row.child(unavailable_row(
                                    status, refreshing, tab_index, entity, cx,
                                ))
                            }
                        }),
                ),
        )
        .child(divider().mt(px(4.)))
        .child(footer(entity, cx))
}

fn header(synced_label: String, fonts: &Fonts) -> impl IntoElement {
    div()
        .flex()
        .items_baseline()
        .justify_between()
        .pt(px(12.))
        .px(px(14.))
        .pb(px(8.))
        .child(section_label("Current limits"))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .font_family(fonts.mono.clone())
                .text_size(px(11.))
                .text_color(theme::c(theme::TEXT_FAINT))
                .child(
                    div()
                        .px(px(5.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .bg(theme::c(theme::API_BADGE_BG))
                        .text_color(theme::c(theme::API_BADGE_TEXT))
                        .child("HTTP API"),
                )
                .child(synced_label),
        )
}

fn provider_row(
    provider: Provider,
    status: IntegrationStatus,
    expanded: bool,
    options: RowOptions,
    entity: &Entity<AppState>,
    fonts: &Fonts,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let primary = provider.primary();
    let show_secondary = !options.only_active;
    let health = Health::from_percent_left(primary.percent_left, options.warn_at);

    let id = provider.id.clone();
    let toggle = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.toggle_expanded(&id, cx));
        })
    };
    let keyboard_toggle = {
        let entity = entity.clone();
        let id = provider.id.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| state.toggle_expanded(&id, cx));
        })
    };

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id(SharedString::from(format!("row-{}", provider.id)))
                .tab_index(options.tab_index)
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(14.))
                .py(px(9.))
                .hover(|s| s.bg(theme::c(theme::ROW_HOVER)))
                .focus(|s| s.bg(theme::c(theme::ROW_HOVER)))
                .cursor_pointer()
                .on_click(toggle)
                .on_action(keyboard_toggle)
                .child(badge(
                    provider.logo.clone(),
                    provider.badge.clone(),
                    provider.badge_bg,
                    provider.badge_fg,
                    22.0,
                    6.0,
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight(590.0))
                                .text_color(theme::c(theme::TEXT))
                                .whitespace_nowrap()
                                .child(provider.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .mt(px(1.))
                                .whitespace_nowrap()
                                .text_color(match health {
                                    Health::Ok => theme::c(theme::TEXT_MUTED),
                                    _ => health.color(),
                                })
                                .child(provider.subtitle()),
                        ),
                )
                .child(
                    div()
                        .w(px(theme::BAR_WIDTH))
                        .flex_shrink_0()
                        .flex()
                        .child(striped_bar(
                            primary.consumed(),
                            health.color(),
                            theme::BAR_HEIGHT,
                        )),
                )
                .child(
                    div()
                        .w(px(42.))
                        .flex_shrink_0()
                        .font_family(fonts.mono.clone())
                        .text_size(px(12.))
                        .font_weight(FontWeight(500.0))
                        .text_color(health.color())
                        .text_right()
                        .child(format!("{}%", primary.percent_left.round() as i32)),
                )
                .child(
                    div()
                        .w(px(12.))
                        .flex_shrink_0()
                        .text_size(px(9.))
                        .text_color(if status.error.is_some() {
                            theme::c(theme::WARN_TEXT)
                        } else {
                            theme::c(theme::TEXT_DIM)
                        })
                        .text_center()
                        .child(if status.error.is_some() {
                            "!"
                        } else if expanded {
                            "\u{25be}"
                        } else {
                            "\u{25b8}"
                        }),
                ),
        )
        .when(expanded, |d| {
            d.child(detail(
                provider,
                status,
                options.warn_at,
                show_secondary,
                options.tab_index,
                cx,
            ))
        })
}

fn detail(
    provider: Provider,
    status: IntegrationStatus,
    warn_at: f32,
    show_secondary: bool,
    tab_index: isize,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let console_url = provider.console_url.clone();
    let open_console = cx.listener(move |_, _, _, cx| {
        cx.stop_propagation();
        cx.open_url(&console_url);
    });
    let console_url = provider.console_url.clone();
    let open_console_keyboard = cx.listener(move |_, _: &Activate, _, cx| {
        cx.stop_propagation();
        cx.open_url(&console_url);
    });

    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .pt(px(4.))
        .pr(px(14.))
        .pb(px(14.))
        .pl(px(46.))
        .when(show_secondary, |d| {
            d.children(
                provider
                    .secondary()
                    .iter()
                    .map(|limit| secondary_row(limit, warn_at)),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .pt(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .size(px(6.))
                                .rounded(px(3.))
                                .bg(if status.error.is_some() {
                                    theme::c(theme::WARN_TEXT)
                                } else {
                                    theme::c(theme::OK_TEXT)
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(11.))
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child(if status.error.is_some() {
                                    "Cached · last API update failed".into()
                                } else {
                                    provider.source_label.clone()
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pl(px(14.))
                        .child(
                            div()
                                .font_family(SharedString::from("IBM Plex Mono"))
                                .text_size(px(10.))
                                .text_color(theme::c(theme::TEXT_FAINT))
                                .child(
                                    status
                                        .latency_ms
                                        .map(format_latency)
                                        .unwrap_or_else(|| "Awaiting response".into()),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("console-{}", provider.id)))
                                .tab_index(100 + tab_index)
                                .flex_shrink_0()
                                .text_size(px(11.))
                                .text_color(rgb(theme::LINK))
                                .hover(|s| s.text_color(rgb(theme::LINK_HOVER)))
                                .focus(|s| s.text_color(rgb(theme::LINK_HOVER)))
                                .cursor_pointer()
                                .on_click(open_console)
                                .on_action(open_console_keyboard)
                                .child("Console \u{2197}"),
                        ),
                ),
        )
}

fn format_latency(milliseconds: u64) -> String {
    if milliseconds < 1000 {
        format!("{milliseconds}ms response")
    } else {
        format!("{:.1}s response", milliseconds as f64 / 1000.0)
    }
}

fn secondary_row(limit: &Limit, warn_at: f32) -> impl IntoElement {
    let health = Health::from_percent_left(limit.percent_left, warn_at);
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .w(px(108.))
                .flex_shrink_0()
                .text_size(px(11.))
                .text_color(theme::c(theme::TEXT_LABEL))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(limit.display_label().to_string()),
        )
        .child(div().flex_1().min_w(px(0.)).flex().child(striped_bar(
            limit.consumed(),
            health.color(),
            theme::SUB_BAR_HEIGHT,
        )))
        .child(
            div()
                .w(px(100.))
                .flex_shrink_0()
                .font_family(SharedString::from("IBM Plex Mono"))
                .text_size(px(11.))
                .text_color(theme::c(theme::TEXT_DETAIL))
                .text_right()
                .child(match &limit.resets_at {
                    Some(reset) => format!("{}% · {reset}", limit.percent_left.round() as i32),
                    None => format!("{}%", limit.percent_left.round() as i32),
                }),
        )
}

fn unavailable_row(
    status: IntegrationStatus,
    refreshing: bool,
    tab_index: isize,
    entity: &Entity<AppState>,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let needs_setup = status.needs_setup();
    let subtitle = if refreshing && status.error.is_none() {
        "Connecting to HTTP API…"
    } else if needs_setup {
        "Setup required"
    } else {
        "HTTP API unavailable"
    };
    let open_prefs = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| {
                if needs_setup {
                    state.set_view(View::Prefs, cx);
                } else {
                    state.refresh_now(cx);
                }
            });
        })
    };
    let keyboard_action = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| {
                if needs_setup {
                    state.set_view(View::Prefs, cx);
                } else {
                    state.refresh_now(cx);
                }
            });
        })
    };

    div()
        .id(SharedString::from(format!("row-{}", status.id)))
        .tab_index(tab_index)
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .py(px(9.))
        .cursor_pointer()
        .hover(|style| style.bg(theme::c(theme::ROW_HOVER)))
        .focus(|style| style.bg(theme::c(theme::ROW_HOVER)))
        .on_click(open_prefs)
        .on_action(keyboard_action)
        .child(badge(
            status.logo,
            status.badge,
            status.badge_bg,
            status.badge_fg,
            22.0,
            6.0,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight(590.0))
                        .child(status.name),
                )
                .child(
                    div()
                        .mt(px(1.))
                        .text_size(px(11.))
                        .text_color(if needs_setup {
                            theme::c(theme::WARN_TEXT)
                        } else {
                            theme::c(theme::TEXT_MUTED)
                        })
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .font_family(SharedString::from("IBM Plex Mono"))
                .text_size(px(10.))
                .text_color(theme::c(theme::TEXT_LABEL))
                .child(if needs_setup { "SET UP" } else { "RETRY" }),
        )
}

fn footer(entity: &Entity<AppState>, cx: &mut Context<Panel>) -> impl IntoElement {
    let refreshing = entity.read(cx).is_refreshing;
    let open_prefs = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.set_view(View::Prefs, cx));
        })
    };
    let refresh = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.refresh_now(cx));
        })
    };
    let open_prefs_keyboard = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| state.set_view(View::Prefs, cx));
        })
    };
    let refresh_keyboard = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| state.refresh_now(cx));
        })
    };

    div()
        .flex()
        .items_center()
        .justify_between()
        .pt(px(8.))
        .px(px(14.))
        .pb(px(10.))
        .text_size(px(12.))
        .text_color(theme::c(theme::TEXT_ACTION))
        .child(
            div()
                .id("open-prefs")
                .tab_index(200)
                .cursor_pointer()
                .hover(|s| s.text_color(theme::c(0xffffffff)))
                .focus(|s| s.text_color(theme::c(0xffffffff)))
                .on_click(open_prefs)
                .on_action(open_prefs_keyboard)
                .child("Preferences\u{2026}"),
        )
        .child(
            div()
                .id("refresh-now")
                .tab_index(201)
                .cursor_pointer()
                .hover(|s| s.text_color(theme::c(0xffffffff)))
                .focus(|s| s.text_color(theme::c(0xffffffff)))
                .on_click(refresh)
                .on_action(refresh_keyboard)
                .child(if refreshing {
                    "Updating…".to_string()
                } else {
                    "Refresh now \u{2318}R".to_string()
                }),
        )
}
