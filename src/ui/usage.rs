//! The "Current limits" pane: one row per provider, expandable into its
//! secondary limits and burn-rate readout.

use gpui::{
    Context, Entity, FontWeight, IntoElement, SharedString, Styled, div, prelude::*, px, rgb,
};

use crate::app_state::AppState;
use crate::model::{Limit, Provider, View};
use crate::theme::{self, Health};
use crate::ui::Fonts;
use crate::ui::panel::Panel;
use crate::ui::widgets::{badge, divider, section_label, sparkline, striped_bar};

pub fn render(
    entity: &Entity<AppState>,
    fonts: &Fonts,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let state = entity.read(cx);
    let synced_label = state.synced_label();
    let warn_at = state.prefs.warn_at;
    let only_active = state.prefs.only_show_active_limit;

    let rows: Vec<(Provider, Health, bool)> = state
        .providers
        .iter()
        .map(|p| {
            let health = Health::from_percent_left(p.primary().percent_left, warn_at);
            let expanded = state.is_expanded(&p.id);
            (p.clone(), health, expanded)
        })
        .collect();

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
                        .map(|(i, (provider, health, expanded))| {
                            div()
                                .flex()
                                .flex_col()
                                .when(i > 0, |d| d.child(divider().mx(px(14.))))
                                .child(provider_row(
                                    provider,
                                    health,
                                    expanded,
                                    warn_at,
                                    only_active,
                                    entity,
                                    fonts,
                                    cx,
                                ))
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
                .gap(px(10.))
                .font_family(fonts.mono.clone())
                .text_size(px(11.))
                .text_color(theme::c(theme::TEXT_FAINT))
                .child(synced_label)
                .child(
                    div()
                        .text_color(theme::c(theme::TEXT_LABEL))
                        .child("% left"),
                ),
        )
}

fn provider_row(
    provider: Provider,
    health: Health,
    expanded: bool,
    warn_at: f32,
    only_active: bool,
    entity: &Entity<AppState>,
    fonts: &Fonts,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let primary = provider.primary();
    let show_secondary = !only_active;
    let is_unauth = primary.resets_at.as_deref().map(|s| s as &str) == Some("login needed")
        || provider.plan == "Run claude login";
    let effective_health = if is_unauth { Health::Ok } else { health };

    let id = provider.id.clone();
    let toggle = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.toggle_expanded(&id, cx));
        })
    };

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id(SharedString::from(format!("row-{}", provider.id)))
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(14.))
                .py(px(9.))
                .hover(|s| s.bg(theme::c(theme::ROW_HOVER)))
                .on_click(toggle)
                .child(badge(
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
                                .text_color(match effective_health {
                                    Health::Ok => theme::c(theme::TEXT_MUTED).into(),
                                    _ => effective_health.color(),
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
                            if is_unauth { 0.0 } else { primary.consumed() },
                            effective_health.color(),
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
                        .text_color(if is_unauth {
                            theme::c(theme::TEXT_MUTED)
                        } else {
                            effective_health.color()
                        })
                        .text_right()
                        .child(if is_unauth {
                            "--".to_string()
                        } else {
                            format!("{}%", primary.percent_left.round() as i32)
                        }),
                )
                .child(
                    div()
                        .w(px(12.))
                        .flex_shrink_0()
                        .text_size(px(9.))
                        .text_color(theme::c(theme::TEXT_DIM))
                        .text_center()
                        .child(if expanded { "\u{25be}" } else { "\u{25b8}" }),
                ),
        )
        .when(expanded, |d| {
            d.child(detail(
                provider,
                effective_health,
                warn_at,
                show_secondary,
                cx,
            ))
        })
}

fn detail(
    provider: Provider,
    headline_health: Health,
    warn_at: f32,
    show_secondary: bool,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let console_url = provider.console_url.clone();
    let open_console = cx.listener(move |_, _, _, cx| {
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
                .items_center()
                .justify_between()
                .gap(px(12.))
                .pt(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(sparkline(
                            provider.burn.samples.clone(),
                            match provider.burn.trend {
                                crate::model::Trend::Rising if headline_health != Health::Ok => {
                                    headline_health.color()
                                }
                                _ => theme::alpha(0xffffff, 0.5).into(),
                            },
                            72.0,
                            20.0,
                        ))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child(provider.burn.note.clone()),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("console-{}", provider.id)))
                        .text_size(px(11.))
                        .text_color(rgb(theme::LINK))
                        .hover(|s| s.text_color(rgb(theme::LINK_HOVER)))
                        .cursor_pointer()
                        .on_click(open_console)
                        .child("Console \u{2197}"),
                ),
        )
}

fn secondary_row(limit: &Limit, warn_at: f32) -> impl IntoElement {
    let health = Health::from_percent_left(limit.percent_left, warn_at);
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .w(px(64.))
                .flex_shrink_0()
                .text_size(px(11.))
                .text_color(theme::c(theme::TEXT_LABEL))
                .child(limit.cadence.label()),
        )
        .child(div().flex_1().min_w(px(0.)).flex().child(striped_bar(
            limit.consumed(),
            health.color(),
            theme::SUB_BAR_HEIGHT,
        )))
        .child(
            div()
                .w(px(46.))
                .flex_shrink_0()
                .font_family(SharedString::from("IBM Plex Mono"))
                .text_size(px(11.))
                .text_color(theme::c(theme::TEXT_DETAIL))
                .text_right()
                .child(format!("{}%", limit.percent_left.round() as i32)),
        )
}

fn footer(entity: &Entity<AppState>, cx: &mut Context<Panel>) -> impl IntoElement {
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
        .pt(px(8.))
        .px(px(14.))
        .pb(px(10.))
        .text_size(px(12.))
        .text_color(theme::c(theme::TEXT_ACTION))
        .child(
            div()
                .id("open-prefs")
                .cursor_pointer()
                .hover(|s| s.text_color(theme::c(0xffffffff)))
                .on_click(open_prefs)
                .child("Preferences\u{2026}"),
        )
        .child(
            div()
                .id("refresh-now")
                .cursor_pointer()
                .hover(|s| s.text_color(theme::c(0xffffffff)))
                .on_click(refresh)
                .child("Refresh now \u{2318}R"),
        )
}
