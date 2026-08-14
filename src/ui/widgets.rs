//! Small reusable pieces of the popover: bars, toggles, sliders, sparklines.
//!
//! The striped bars and the sparkline paint through `canvas` because they need
//! their laid-out width, which a `1fr`-style flex child only knows at paint
//! time.

use gpui::{
    Bounds, Div, FontWeight, IntoElement, PathBuilder, Pixels, Rgba, SharedString, Styled, canvas,
    div, point, prelude::*, px, quad, rgb, size, transparent_black,
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

/// The rounded single-letter provider mark.
pub fn badge(
    letter: SharedString,
    bg: u32,
    fg: u32,
    size_px: f32,
    radius: f32,
) -> impl IntoElement {
    div()
        .size(px(size_px))
        .flex_shrink_0()
        .rounded(px(radius))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(bg))
        .text_color(rgb(fg))
        .text_size(px(size_px * 0.55))
        .font_weight(FontWeight(700.0))
        .child(letter)
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

/// The recent-consumption sparkline. `samples` are 0.0–1.0, oldest first, and
/// are drawn with 0 at the bottom.
pub fn sparkline(samples: Vec<f32>, color: Rgba, width: f32, height: f32) -> impl IntoElement {
    div().w(px(width)).h(px(height)).flex_shrink_0().child(
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                if samples.len() < 2 {
                    return;
                }
                // Inset by half the stroke so the extremes are not clipped.
                let inset = px(1.0);
                let w = bounds.size.width;
                let h = bounds.size.height - inset * 2.;
                let step = w / (samples.len() - 1) as f32;

                let mut builder = PathBuilder::stroke(px(1.4));
                for (i, sample) in samples.iter().enumerate() {
                    let x = bounds.origin.x + step * i as f32;
                    let y = bounds.origin.y + inset + h * (1.0 - sample.clamp(0.0, 1.0));
                    let p = point(x, y);
                    if i == 0 {
                        builder.move_to(p);
                    } else {
                        builder.line_to(p);
                    }
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
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

/// Continuous slider for the warn threshold. `fraction` is 0.0–1.0.
pub fn slider(fraction: f32, width: f32) -> impl IntoElement {
    let fraction = fraction.clamp(0.0, 1.0);
    div()
        .w(px(width))
        .h(px(14.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds: Bounds<Pixels>, _, window, _| {
                    let track_h = px(4.);
                    let track_y = bounds.origin.y + (bounds.size.height - track_h) / 2.;
                    let track = Bounds {
                        origin: point(bounds.origin.x, track_y),
                        size: size(bounds.size.width, track_h),
                    };
                    window.paint_quad(quad(
                        track,
                        px(2.),
                        theme::c(theme::SLIDER_TRACK),
                        px(0.),
                        transparent_black(),
                        Default::default(),
                    ));

                    let filled = bounds.size.width * fraction;
                    window.paint_quad(quad(
                        Bounds {
                            origin: track.origin,
                            size: size(filled, track_h),
                        },
                        px(2.),
                        theme::c(theme::SLIDER_FILL),
                        px(0.),
                        transparent_black(),
                        Default::default(),
                    ));

                    let knob = px(14.);
                    window.paint_quad(quad(
                        Bounds {
                            origin: point(
                                bounds.origin.x + filled - knob / 2.,
                                bounds.origin.y + (bounds.size.height - knob) / 2.,
                            ),
                            size: size(knob, knob),
                        },
                        knob / 2.,
                        theme::c(theme::CONTROL_KNOB),
                        px(0.),
                        transparent_black(),
                        Default::default(),
                    ));
                },
            )
            .size_full(),
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
