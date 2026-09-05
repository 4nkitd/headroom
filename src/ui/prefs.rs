//! The Preferences pane: global controls, integrations, and warning threshold.

use gpui::{
    Context, Entity, Focusable, IntoElement, SharedString, Styled, div, prelude::*, px, rgb,
};

use crate::app_state::{AppState, IntegrationStatus};
use crate::credentials;
use crate::model::{Prefs, Provider, View};
use crate::theme;
use crate::ui::Activate;
use crate::ui::Fonts;
use crate::ui::panel::Panel;
use crate::ui::text_input::SecretInput;
use crate::ui::widgets::{badge, divider, section_label, toggle};

struct PrefsViewData {
    prefs: Prefs,
    providers: Vec<Provider>,
    integrations: Vec<IntegrationStatus>,
    update_status: crate::update::UpdateStatus,
    support_notice: Option<SharedString>,
}

pub fn render(
    entity: &Entity<AppState>,
    _fonts: &Fonts,
    api_key_input: Entity<SecretInput>,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let state = entity.read(cx);
    let data = PrefsViewData {
        prefs: state.prefs.clone(),
        providers: state.providers.clone(),
        integrations: state.integrations.clone(),
        update_status: state.update_status.clone(),
        support_notice: state.support_notice.clone(),
    };

    div()
        .flex()
        .flex_col()
        .child(header(entity, cx))
        .child(divider().mx(px(14.)))
        .child(body(data, api_key_input, entity, cx))
}

fn header(entity: &Entity<AppState>, cx: &mut Context<Panel>) -> impl IntoElement {
    let back = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.set_view(View::Usage, cx));
        })
    };
    let keyboard_back = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
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
                .tab_index(1)
                .text_size(px(12.))
                .text_color(theme::c(theme::TEXT_ACTION))
                .hover(|s| s.text_color(theme::c(0xffffffff)))
                .focus(|s| s.text_color(theme::c(0xffffffff)))
                .cursor_pointer()
                .on_click(back)
                .on_action(keyboard_back)
                .child("\u{2039} Back"),
        )
        .child(section_label("Preferences"))
}

fn body(
    data: PrefsViewData,
    api_key_input: Entity<SecretInput>,
    entity: &Entity<AppState>,
    cx: &mut Context<Panel>,
) -> impl IntoElement {
    let PrefsViewData {
        prefs,
        providers,
        integrations,
        update_status,
        support_notice,
    } = data;
    let disabled_integrations = prefs.disabled_integrations.clone();
    let toggle_show_pct = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| {
                state.prefs.show_percentage_in_menu_bar = !state.prefs.show_percentage_in_menu_bar;
                state.save_prefs();
                cx.notify();
            });
        })
    };
    let keyboard_toggle_show_pct = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| {
                state.prefs.show_percentage_in_menu_bar = !state.prefs.show_percentage_in_menu_bar;
                state.save_prefs();
                cx.notify();
            });
        })
    };

    let toggle_only_active = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| {
                state.prefs.only_show_active_limit = !state.prefs.only_show_active_limit;
                state.save_prefs();
                cx.notify();
            });
        })
    };
    let keyboard_toggle_only_active = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| {
                state.prefs.only_show_active_limit = !state.prefs.only_show_active_limit;
                state.save_prefs();
                cx.notify();
            });
        })
    };

    let toggle_launch = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| {
                let target_state = !state.prefs.launch_at_login;
                let _ = crate::autostart::set_enabled(target_state);
                state.prefs.launch_at_login = crate::autostart::is_enabled();
                state.save_prefs();
                cx.notify();
            });
        })
    };
    let keyboard_toggle_launch = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| {
                let target_state = !state.prefs.launch_at_login;
                let _ = crate::autostart::set_enabled(target_state);
                state.prefs.launch_at_login = crate::autostart::is_enabled();
                state.save_prefs();
                cx.notify();
            });
        })
    };

    let save_opencode_go_key = {
        let entity = entity.clone();
        let api_key_input = api_key_input.clone();
        cx.listener(move |_, _, _, cx| {
            let value = api_key_input.read(cx).value();
            let (label, key) = value
                .split_once('|')
                .map(|(label, key)| (label.trim().to_string(), key.trim().to_string()))
                .unwrap_or_else(|| ("OpenCode Go".into(), value.trim().to_string()));
            if key.is_empty() {
                return;
            }
            if let Err(error) = credentials::save_opencode_go_account(&label, &key) {
                eprintln!("headroom: could not save OpenCode Go key: {error:#}");
                return;
            }
            api_key_input.update(cx, |input, cx| input.clear(cx));
            entity.update(cx, |state, cx| {
                state.support_notice = Some("OpenCode Go key saved · validating…".into());
                state.refresh_now(cx);
            });
        })
    };
    let save_opencode_go_key_keyboard = {
        let entity = entity.clone();
        let api_key_input = api_key_input.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            let value = api_key_input.read(cx).value();
            let (label, key) = value
                .split_once('|')
                .map(|(label, key)| (label.trim().to_string(), key.trim().to_string()))
                .unwrap_or_else(|| ("OpenCode Go".into(), value.trim().to_string()));
            if key.is_empty() {
                return;
            }
            if credentials::save_opencode_go_account(&label, &key).is_err() {
                return;
            }
            api_key_input.update(cx, |input, cx| input.clear(cx));
            entity.update(cx, |state, cx| {
                state.support_notice = Some("OpenCode Go key saved · validating…".into());
                state.refresh_now(cx);
            });
        })
    };

    let lower_warning = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.adjust_warn_at(-5.0, cx));
        })
    };
    let raise_warning = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.adjust_warn_at(5.0, cx));
        })
    };
    let lower_warning_keyboard = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| state.adjust_warn_at(-5.0, cx));
        })
    };
    let raise_warning_keyboard = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| state.adjust_warn_at(5.0, cx));
        })
    };
    let export_report = {
        let entity = entity.clone();
        cx.listener(move |_, _, _, cx| {
            entity.update(cx, |state, cx| state.export_support_report(cx));
        })
    };
    let export_report_keyboard = {
        let entity = entity.clone();
        cx.listener(move |_, _: &Activate, _, cx| {
            entity.update(cx, |state, cx| state.export_support_report(cx));
        })
    };
    let update_url = update_status.release_url.clone();
    let open_update = cx.listener(move |_, _, _, cx| {
        if let Some(url) = update_url.as_ref() {
            cx.open_url(url);
        }
    });
    let update_url = update_status.release_url.clone();
    let open_update_keyboard = cx.listener(move |_, _: &Activate, _, cx| {
        if let Some(url) = update_url.as_ref() {
            cx.open_url(url);
        }
    });
    let update_text = if update_status.checking {
        "Checking for updates…".to_string()
    } else if let Some(version) = update_status.latest_version.as_ref() {
        format!("v{version} available ↗")
    } else if update_status.error.is_some() {
        "Update check unavailable".to_string()
    } else {
        "Up to date · stable".to_string()
    };

    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(14.))
        .child(
            div()
                .id("pref-show-percentage")
                .tab_index(2)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .cursor_pointer()
                .focus(|style| style.bg(theme::c(theme::ROW_HOVER)))
                .on_click(toggle_show_pct)
                .on_action(keyboard_toggle_show_pct)
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
                .tab_index(3)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .cursor_pointer()
                .focus(|style| style.bg(theme::c(theme::ROW_HOVER)))
                .on_click(toggle_only_active)
                .on_action(keyboard_toggle_only_active)
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
                .tab_index(5)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .cursor_pointer()
                .focus(|style| style.bg(theme::c(theme::ROW_HOVER)))
                .on_click(toggle_launch)
                .on_action(keyboard_toggle_launch)
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
                                .child("OpenCode Go account"),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .mt(px(1.))
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child("Use label|API key to add or update"),
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
                                .tab_index(6)
                                .cursor_pointer()
                                .text_size(px(12.))
                                .text_color(rgb(theme::LINK))
                                .hover(|s| s.text_color(rgb(theme::LINK_HOVER)))
                                .focus(|s| s.text_color(rgb(theme::LINK_HOVER)))
                                .on_click(save_opencode_go_key)
                                .on_action(save_opencode_go_key_keyboard)
                                .child("Save"),
                        ),
                ),
        )
        .child(divider().my(px(2.)))
        .child(section_label("Connected accounts"))
        .child(
            div()
                .mt(px(-7.))
                .text_size(px(10.))
                .text_color(theme::c(theme::TEXT_MUTED))
                .child("Usage is read directly from provider HTTP APIs."),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(9.))
                .children(integrations.into_iter().map(|status| {
                    let enabled = !disabled_integrations.contains(status.id.as_ref());
                    let provider = providers.iter().find(|provider| {
                        provider.id == status.id
                            || provider.id.starts_with(&format!("{}:", status.id))
                    });
                    let status_text: SharedString = if !enabled {
                        "Disabled".into()
                    } else if provider.is_some() {
                        if status.error.is_some() {
                            status
                                .freshness_label()
                                .map(|freshness| format!("Cached · {freshness}"))
                                .unwrap_or_else(|| "Cached".into())
                                .into()
                        } else {
                            status
                                .latency_ms
                                .map(|latency| format!("Verified · {latency}ms"))
                                .unwrap_or_else(|| "Verified".into())
                                .into()
                        }
                    } else if status.needs_setup() {
                        "Setup required".into()
                    } else if status.error.is_some() {
                        "API error".into()
                    } else {
                        "Connecting…".into()
                    };
                    let toggle_integration = {
                        let entity = entity.clone();
                        let id = status.id.clone();
                        cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            entity.update(cx, |state, cx| {
                                state.set_integration_enabled(id.as_ref(), !enabled, cx);
                            });
                        })
                    };
                    let keyboard_toggle_integration = {
                        let entity = entity.clone();
                        let id = status.id.clone();
                        cx.listener(move |_, _: &Activate, _, cx| {
                            cx.stop_propagation();
                            entity.update(cx, |state, cx| {
                                state.set_integration_enabled(id.as_ref(), !enabled, cx);
                            });
                        })
                    };
                    let setup_url = status.setup_url.clone();
                    let setup_input = api_key_input.clone();
                    let setup = cx.listener(move |_, _, window, cx| {
                        cx.stop_propagation();
                        if let Some(url) = setup_url.as_ref() {
                            cx.open_url(url);
                        } else {
                            setup_input.focus_handle(cx).focus(window);
                        }
                    });
                    let setup_url = status.setup_url.clone();
                    let setup_input = api_key_input.clone();
                    let setup_keyboard = cx.listener(move |_, _: &Activate, window, cx| {
                        cx.stop_propagation();
                        if let Some(url) = setup_url.as_ref() {
                            cx.open_url(url);
                        } else {
                            setup_input.focus_handle(cx).focus(window);
                        }
                    });
                    let credential_source = match status.id.as_ref() {
                        "claude-code" => credentials::claude_credentials_status().to_string(),
                        "openai-codex" => credentials::codex_credentials_status().to_string(),
                        "opencode-go" => {
                            format!("{} account(s)", credentials::opencode_go_accounts().len())
                        }
                        "antigravity" => credentials::antigravity_credentials_status().to_string(),
                        _ => "Unknown source".into(),
                    };
                    let credential_source: SharedString = provider
                        .map(|provider| format!("{credential_source} · {}", provider.plan))
                        .unwrap_or(credential_source)
                        .into();
                    let toggle_index = match status.id.as_ref() {
                        "claude-code" => 10,
                        "openai-codex" => 12,
                        "opencode-go" => 14,
                        _ => 16,
                    };
                    div()
                        .id(SharedString::from(format!("integration-{}", status.id)))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(badge(
                            status.logo.clone(),
                            status.badge.clone(),
                            status.badge_bg,
                            status.badge_fg,
                            18.0,
                            5.0,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(theme::c(theme::TEXT))
                                        .child(status.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(theme::c(theme::TEXT_FAINT))
                                        .child(credential_source),
                                ),
                        )
                        .child(
                            div()
                                .font_family(SharedString::from("IBM Plex Mono"))
                                .text_size(px(11.))
                                .text_color(if enabled && status.error.is_some() {
                                    theme::c(theme::WARN_TEXT)
                                } else {
                                    theme::c(theme::TEXT_MUTED)
                                })
                                .child(status_text),
                        )
                        .when(enabled && status.needs_setup(), |row| {
                            row.child(
                                div()
                                    .id(SharedString::from(format!("setup-{}", status.id)))
                                    .tab_index(toggle_index)
                                    .text_size(px(10.))
                                    .text_color(rgb(theme::LINK))
                                    .cursor_pointer()
                                    .hover(|style| style.text_color(rgb(theme::LINK_HOVER)))
                                    .focus(|style| style.text_color(rgb(theme::LINK_HOVER)))
                                    .on_click(setup)
                                    .on_action(setup_keyboard)
                                    .child(status.setup_label.clone()),
                            )
                        })
                        .child(
                            div()
                                .id(SharedString::from(format!("toggle-{}", status.id)))
                                .tab_index(toggle_index + 1)
                                .cursor_pointer()
                                .focus(|style| style.bg(theme::c(theme::ROW_HOVER)))
                                .on_click(toggle_integration)
                                .on_action(keyboard_toggle_integration)
                                .child(toggle(enabled)),
                        )
                })),
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
                        .gap(px(6.))
                        .child(
                            div()
                                .id("warning-lower")
                                .tab_index(20)
                                .size(px(18.))
                                .rounded(px(5.))
                                .bg(theme::c(theme::CONTROL_OFF))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(lower_warning)
                                .focus(|style| style.bg(theme::c(theme::ROW_HOVER)))
                                .on_action(lower_warning_keyboard)
                                .child("−"),
                        )
                        .child(
                            div()
                                .w(px(42.))
                                .text_center()
                                .font_family(SharedString::from("IBM Plex Mono"))
                                .text_size(px(11.))
                                .text_color(theme::c(theme::TEXT_DETAIL))
                                .child(format!("{}%", prefs.warn_at as i32)),
                        )
                        .child(
                            div()
                                .id("warning-raise")
                                .tab_index(21)
                                .size(px(18.))
                                .rounded(px(5.))
                                .bg(theme::c(theme::CONTROL_OFF))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(raise_warning)
                                .focus(|style| style.bg(theme::c(theme::ROW_HOVER)))
                                .on_action(raise_warning_keyboard)
                                .child("+"),
                        ),
                ),
        )
        .child(divider().my(px(2.)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme::c(theme::TEXT_MUTED))
                        .child(format!("Headroom v{}", env!("CARGO_PKG_VERSION"))),
                )
                .child(
                    div()
                        .id("open-update")
                        .tab_index(22)
                        .text_size(px(11.))
                        .text_color(if update_status.latest_version.is_some() {
                            rgb(theme::LINK)
                        } else {
                            theme::c(theme::TEXT_FAINT)
                        })
                        .when(update_status.release_url.is_some(), |item| {
                            item.cursor_pointer()
                                .hover(|style| style.text_color(rgb(theme::LINK_HOVER)))
                                .focus(|style| style.text_color(rgb(theme::LINK_HOVER)))
                                .on_click(open_update)
                                .on_action(open_update_keyboard)
                        })
                        .child(update_text),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(10.))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_size(px(10.))
                        .text_color(theme::c(theme::TEXT_FAINT))
                        .child(support_notice.unwrap_or_else(|| {
                            "Support reports never include credential values".into()
                        })),
                )
                .child(
                    div()
                        .id("export-support-report")
                        .tab_index(23)
                        .cursor_pointer()
                        .text_size(px(11.))
                        .text_color(rgb(theme::LINK))
                        .hover(|style| style.text_color(rgb(theme::LINK_HOVER)))
                        .focus(|style| style.text_color(rgb(theme::LINK_HOVER)))
                        .on_click(export_report)
                        .on_action(export_report_keyboard)
                        .child("Export report"),
                ),
        )
}
