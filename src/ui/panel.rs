//! The main popover container view.

use gpui::{
    Context, Entity, FocusHandle, IntoElement, Render, Styled, Window, div, prelude::*, px, size,
};

use crate::app_state::AppState;
use crate::model::View;
use crate::theme;
use crate::ui::{Back, Fonts, Refresh, Tab, TabPrev, prefs, text_input::SecretInput, usage};

pub struct Panel {
    state: Entity<AppState>,
    api_key_input: Entity<SecretInput>,
    resize_requested_height: Option<f32>,
    focus_handle: FocusHandle,
    focus_initialized: bool,
}

impl Panel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let api_key_input = cx.new(SecretInput::new);
        Self {
            state,
            api_key_input,
            resize_requested_height: None,
            focus_handle: cx.focus_handle(),
            focus_initialized: false,
        }
    }

    fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.refresh_now(cx));
    }

    fn focus_next(&mut self, _: &Tab, window: &mut Window, _: &mut Context<Self>) {
        window.focus_next();
    }

    fn focus_previous(&mut self, _: &TabPrev, window: &mut Window, _: &mut Context<Self>) {
        window.focus_prev();
    }

    fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if state.view == View::Prefs {
                state.set_view(View::Usage, cx);
            }
        });
    }
}

impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus_initialized {
            self.focus_handle.focus(window);
            self.focus_initialized = true;
        }
        let fonts = Fonts::get(cx);
        let (view, height) = {
            let state = self.state.read(cx);
            (state.view, state.panel_height())
        };
        if self.resize_requested_height != Some(height) {
            self.resize_requested_height = Some(height);
            window.resize(size(px(theme::PANEL_WIDTH), px(height)));
        }

        let content = match view {
            View::Usage => usage::render(&self.state, &fonts, cx).into_any_element(),
            View::Prefs => prefs::render(&self.state, &fonts, self.api_key_input.clone(), cx)
                .into_any_element(),
        };

        div()
            .w(px(theme::PANEL_WIDTH))
            .h_full()
            .rounded(px(14.))
            .overflow_hidden()
            .bg(theme::c(theme::PANEL_BG))
            .border_1()
            .border_color(theme::c(theme::PANEL_BORDER))
            .text_color(theme::c(theme::TEXT))
            .font_family(fonts.ui.clone())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_previous))
            .on_action(cx.listener(Self::back))
            .child(content)
    }
}
