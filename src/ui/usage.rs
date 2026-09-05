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

    let groups = state
        .integrations
        .iter()
        .filter(|status| state.integration_enabled(status.id.as_ref()))
        .map(|status| {
            let providers = state
                .providers
                .iter()
                .filter(|provider| {
                    provider.id.as_ref() == status.id.as_ref()
                        || provider.id.starts_with(&format!("{}:", status.id))
                })
                .cloned()
                .collect::<Vec<_>>();
            (status.clone(), providers)
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .child(header(synced_label, fonts, entity, cx))
        .child(divider().mx(px(14.)))
        .child(
            div()
                .px(px(14.))
                .pt(px(10.))
                .pb(px(6.))
                .child(section_label("Current Limits")),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .children(
                    groups
                        .into_iter()
                        .enumerate()
                        .map(|(i, (status, providers))| {
                            let tab_index = 10 + i as isize * 5;
                            let row_options = RowOptions {
                                tab_index,
                                ..options
                            };
                            let row = div()
                                .flex()
                                .flex_col()
                                .when(i > 0, |d| d.child(divider().mx(px(14.))));
                            if providers.is_empty() {
                                row.child(unavailable_row(
                                    status, refreshing, tab_index, entity, cx,
                                ))
                            } else if status.id.as_ref() == "antigravity" {
                                row.child(antigravity_grouped_card(
                                    status,
                                    providers,
                                    row_options,
                                    entity,
                                    fonts,
                                    cx,
                                ))
                            } else {
                                let mut provider_col = div().flex().flex_col();
                                for (p_idx, provider) in providers.into_iter().enumerate() {
                                    let expanded = entity.read(cx).is_expanded(&provider.id);
                                    provider_col = provider_col.child(provider_row(
                                        provider,
                                        status.clone(),
                                        expanded,
                                        RowOptions {
                                            tab_index: tab_index + p_idx as isize,
                                            ..row_options
                                        },
                                        entity,
                                        fonts,
                                        cx,
                                    ));
                                }
                                row.child(provider_col)
                            }
                        }),
                ),
        )
}

fn header(
    synced_label: String,
    fonts: &Fonts,
    entity: &Entity<AppState>,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
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

    div()
        .flex()
        .items_center()
        .justify_between()
        .pt(px(11.))
        .px(px(14.))
        .pb(px(9.))
        .child(
            div()
                .font_family(fonts.mono.clone())
                .text_size(px(11.))
                .text_color(theme::c(theme::TEXT_FAINT))
                .child(synced_label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(12.))
                .child(
                    div()
                        .id("header-open-prefs")
                        .tab_index(1)
                        .cursor_pointer()
                        .text_color(theme::c(theme::TEXT_ACTION))
                        .hover(|s| s.text_color(theme::c(0xffffffff)))
                        .on_click(open_prefs)
                        .child("Preferences…"),
                )
                .child(
                    div()
                        .id("header-refresh-now")
                        .tab_index(2)
                        .cursor_pointer()
                        .text_color(theme::c(theme::TEXT_ACTION))
                        .hover(|s| s.text_color(theme::c(0xffffffff)))
                        .on_click(refresh)
                        .child("Refresh now"),
                ),
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

    let account_pill_label = if provider.id.starts_with("opencode-go:") {
        let raw = provider.id.as_ref();
        raw.rsplit(':')
            .next()
            .filter(|l| *l != "OpenCode Go" && *l != "Default")
    } else {
        None
    };

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id(SharedString::from(format!("row-{}", provider.id)))
                .tab_index(options.tab_index)
                .flex()
                .flex_col()
                .gap(px(6.))
                .px(px(14.))
                .py(px(9.))
                .hover(|s| s.bg(theme::c(theme::ROW_HOVER)))
                .focus(|s| s.bg(theme::c(theme::ROW_HOVER)))
                .cursor_pointer()
                .on_click(toggle)
                .on_action(keyboard_toggle)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
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
                                        .text_size(px(13.))
                                        .font_weight(FontWeight(590.0))
                                        .text_color(theme::c(theme::TEXT))
                                        .child(provider.name.clone()),
                                )
                                .when_some(account_pill_label, |d, label| {
                                    d.child(
                                        div()
                                            .font_family(fonts.mono.clone())
                                            .text_size(px(10.))
                                            .px(px(5.))
                                            .py(px(1.))
                                            .rounded(px(4.))
                                            .bg(theme::c(theme::ROW_HOVER))
                                            .text_color(theme::c(theme::TEXT_MUTED))
                                            .child(label.to_string()),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .font_family(fonts.mono.clone())
                                .text_size(px(11.))
                                .font_weight(FontWeight(600.0))
                                .text_color(health.color())
                                .child(format!("{:.1}%", primary.percent_left)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(11.))
                        .text_color(theme::c(theme::TEXT_MUTED))
                        .child(primary.display_label().to_string())
                        .child(
                            div()
                                .font_family(fonts.mono.clone())
                                .text_color(theme::c(theme::TEXT_DETAIL))
                                .child(match &primary.resets_at {
                                    Some(at) => {
                                        format!(
                                            "{}% \u{00b7} {at}",
                                            primary.percent_left.round() as i32
                                        )
                                    }
                                    None => format!("{}%", primary.percent_left.round() as i32),
                                }),
                        ),
                )
                .child(striped_bar(
                    primary.fraction_left(),
                    health.color(),
                    theme::BAR_HEIGHT,
                )),
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
            limit.fraction_left(),
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

fn antigravity_grouped_card(
    status: IntegrationStatus,
    providers: Vec<Provider>,
    options: RowOptions,
    entity: &Entity<AppState>,
    fonts: &Fonts,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let primary_account_id = entity.read(cx).prefs.primary_account_id.clone();
    let mut card = div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .px(px(14.))
        .py(px(9.))
        .hover(|s| s.bg(theme::c(theme::ROW_HOVER)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(badge(
                            status.logo.clone(),
                            status.badge.clone(),
                            status.badge_bg,
                            status.badge_fg,
                            22.0,
                            6.0,
                        ))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight(590.0))
                                .text_color(theme::c(theme::TEXT))
                                .child("Antigravity"),
                        ),
                )
                .child(
                    div()
                        .font_family(fonts.mono.clone())
                        .text_size(px(11.))
                        .text_color(theme::c(theme::TEXT_FAINT))
                        .child("Google Code Assist API"),
                ),
        );

    for (idx, provider) in providers.into_iter().enumerate() {
        let primary = provider.primary();
        let health = Health::from_percent_left(primary.percent_left, options.warn_at);
        let account_tag = crate::model::truncate_account_label(provider.id.as_ref());
        let is_primary = primary_account_id
            .as_ref()
            .map(|p_id| p_id == provider.id.as_ref())
            .unwrap_or(idx == 0);

        let subrow = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(
                                div()
                                    .font_family(fonts.mono.clone())
                                    .text_size(px(10.))
                                    .px(px(5.))
                                    .py(px(1.))
                                    .rounded(px(4.))
                                    .bg(theme::c(theme::ROW_HOVER))
                                    .text_color(theme::c(theme::TEXT_MUTED))
                                    .child(account_tag),
                            )
                            .when(is_primary, |d| {
                                d.child(
                                    div()
                                        .font_family(fonts.mono.clone())
                                        .text_size(px(9.))
                                        .font_weight(FontWeight::BOLD)
                                        .px(px(4.))
                                        .py(px(1.))
                                        .rounded(px(3.))
                                        .bg(theme::c(theme::API_BADGE_BG))
                                        .text_color(theme::c(theme::OK_TEXT))
                                        .child("PRIMARY"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .font_family(fonts.mono.clone())
                            .text_size(px(11.))
                            .font_weight(FontWeight(600.0))
                            .text_color(health.color())
                            .child(format!("{:.1}%", primary.percent_left)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.))
                    .text_color(theme::c(theme::TEXT_MUTED))
                    .child("Gemini 5h session")
                    .child(
                        div()
                            .font_family(fonts.mono.clone())
                            .text_color(theme::c(theme::TEXT_DETAIL))
                            .child(match &primary.resets_at {
                                Some(at) => format!("{:.1}% \u{00b7} {at}", primary.percent_left),
                                None => format!("{:.1}%", primary.percent_left),
                            }),
                    ),
            )
            .child(striped_bar(
                primary.fraction_left(),
                health.color(),
                theme::BAR_HEIGHT,
            ));

        card = card.child(subrow);
    }

    card
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
