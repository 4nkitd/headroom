//! The main popover container view.

use gpui::{Context, Entity, IntoElement, Render, Styled, Window, div, prelude::*, px, size};

use crate::app_state::AppState;
use crate::model::View;
use crate::theme;
use crate::ui::{Fonts, prefs, text_input::SecretInput, usage};

pub struct Panel {
    state: Entity<AppState>,
    api_key_input: Entity<SecretInput>,
    resize_requested_height: Option<f32>,
}

impl Panel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let api_key_input = cx.new(SecretInput::new);
        Self {
            state,
            api_key_input,
            resize_requested_height: None,
        }
    }
}

impl Render for Panel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fonts = Fonts::resolve(cx);
        let (view, height) = {
            let state = self.state.read(cx);
            (state.view, state.panel_height())
        };
        if self.resize_requested_height != Some(height) {
            self.resize_requested_height = Some(height);
            _window.resize(size(px(theme::PANEL_WIDTH), px(height)));
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
            .child(content)
    }
}
