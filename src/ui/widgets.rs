//! Small reusable pieces of the popover: bars, toggles, sliders, sparklines.
//!
//! The striped bars and the sparkline paint through `canvas` because they need
//! their laid-out width, which a `1fr`-style flex child only knows at paint
//! time.

use gpui::{
    Bounds, Div, FontWeight, IntoElement, ObjectFit, Pixels, Rgba, SharedString, Styled,
    StyledImage, canvas, div, img, point, prelude::*, px, quad, rgb, size, transparent_black,
};

use crate::theme;

/// A hairline separator. The mock uses 0.5px; on a Retina display that lands on
/// exactly one physical pixel.
pub fn divider() -> Div {
    div()
        .h(px(0.5))
        .bg(theme::c(theme::DIVIDER))
        .flex_shrink_0()
}

/// A provider's brand mark with the legacy initial as a decode fallback.
pub fn badge(
    logo: SharedString,
    letter: SharedString,
    bg: u32,
    fg: u32,
    size_px: f32,
    radius: f32,
) -> impl IntoElement {
    let fallback = move || {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(bg))
            .text_color(rgb(fg))
            .text_size(px(size_px * 0.55))
            .font_weight(FontWeight(700.0))
            .child(letter.clone())
            .into_any_element()
    };

    div()
        .size(px(size_px))
        .flex_shrink_0()
        .rounded(px(radius))
        .overflow_hidden()
        .child(
            img(logo)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .with_fallback(fallback),
        )
}

/// A quota bar: rounded track with a striped fill covering the consumed
/// fraction. `fraction` is 0.0–1.0 of the track that is *used up*.
pub fn striped_bar(fraction: f32, color: Rgba, height: f32) -> impl IntoElement {
    let fraction = fraction.clamp(0.0, 1.0);
    let radius = height / 2.0;

    div()
        .h(px(height))
        .flex_1()
        .min_w(px(0.))
        .rounded(px(radius))
        .bg(theme::c(theme::TRACK))
        .overflow_hidden()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds: Bounds<Pixels>, _, window, _| {
                    let filled = bounds.size.width * fraction;
                    if filled <= px(0.) {
                        return;
                    }
                    let end = bounds.origin.x + filled;
                    let stripe_radius = px((height / 3.0).min(2.0));
                    let mut x = bounds.origin.x;
                    while x < end {
                        // Clip the last stripe so the fill stops exactly at the
                        // consumed boundary instead of overshooting it.
                        let width = px(theme::STRIPE_ON).min(end - x);
                        window.paint_quad(quad(
                            Bounds {
                                origin: point(x, bounds.origin.y),
                                size: size(width, bounds.size.height),
                            },
                            stripe_radius,
                            color,
                            px(0.),
                            transparent_black(),
                            Default::default(),
                        ));
                        x += px(theme::STRIPE_ON + theme::STRIPE_OFF);
                    }
                },
            )
            .size_full(),
        )
}

/// macOS-style switch. Purely presentational — the caller wires the click.
pub fn toggle(on: bool) -> impl IntoElement {
    let track = if on {
        theme::c(theme::CONTROL_ON)
    } else {
        theme::c(theme::CONTROL_OFF)
    };
    div()
        .w(px(34.))
        .h(px(20.))
        .flex_shrink_0()
        .rounded(px(10.))
        .bg(track)
        .p(px(2.))
        .flex()
        .when(on, |d| d.justify_end())
        .when(!on, |d| d.justify_start())
        .child(
            div()
                .size(px(16.))
                .rounded(px(8.))
                .bg(theme::c(theme::CONTROL_KNOB)),
        )
}

/// Section header: "CURRENT LIMITS", "PREFERENCES", "CONNECTED ACCOUNTS".
pub fn section_label(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight(600.0))
        .text_color(theme::c(theme::TEXT_LABEL))
        .child(text.into().to_uppercase())
}
