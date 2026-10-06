use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use crate::theme;
use dscan_core::format_bytes;

pub fn render_extension_legend(app: &DscanApp, cx: &Context<DscanApp>) -> impl IntoElement {
    let t = app.theme();

    div()
        .id("extension-legend-pane")
        .v_flex()
        .size_full()
        .bg(t.bg)
        .child(
            // Legend Table Header
            div()
                .h_flex()
                .w_full()
                .h(px(32.0))
                .px_3()
                .bg(t.surface)
                .border_b_1()
                .border_color(t.border)
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text_muted)
                .justify_between()
                .items_center()
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().w(px(16.0)).child(""))
                        .child(div().w(px(90.0)).child("Extension")),
                )
                .child(div().w(px(60.0)).text_right().child("Files"))
                .child(div().w(px(80.0)).text_right().child("Bytes"))
                .child(div().w(px(55.0)).text_right().child("%")),
        )
        .child(
            // Extensions list
            div()
                .id("extension-legend-content")
                .v_flex()
                .flex_1()
                .overflow_y_scroll()
                .px_2()
                .py_1()
                .when(app.state.extensions.is_empty(), |s| {
                    s.child(
                        div()
                            .v_flex()
                            .items_center()
                            .justify_center()
                            .p_8()
                            .text_color(t.text_dim)
                            .text_size(px(12.0))
                            .child("No extension data yet"),
                    )
                })
                .when(!app.state.extensions.is_empty(), |s| {
                    s.children(app.state.extensions.iter().enumerate().map(|(idx, ext)| {
                        let color = theme::extension_color(&ext.extension);
                        let is_active =
                            app.state.selected_extension.as_deref() == Some(&ext.extension);
                        let ext_name = ext.extension.clone();

                        div()
                            .id(("ext-row", idx))
                            .h_flex()
                            .w_full()
                            .h(px(26.0))
                            .px_2()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_size(px(12.0))
                            .when(is_active, |s| {
                                s.bg(t.surface_hover).border_1().border_color(t.accent_blue)
                            })
                            .when(!is_active, |s| s.hover(|h| h.bg(t.surface)))
                            .justify_between()
                            .items_center()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    this.toggle_extension_filter(&ext_name, cx);
                                }),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().w(px(14.0)).h(px(14.0)).rounded_sm().bg(color))
                                    .child(
                                        div()
                                            .w(px(90.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_active {
                                                t.accent_blue
                                            } else {
                                                t.text_primary
                                            })
                                            .child(ext.extension.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(60.0))
                                    .text_right()
                                    .text_color(t.text_muted)
                                    .child(format!("{}", ext.file_count)),
                            )
                            .child(
                                div()
                                    .w(px(80.0))
                                    .text_right()
                                    .text_color(t.text_muted)
                                    .child(format_bytes(ext.total_bytes)),
                            )
                            .child(
                                div()
                                    .w(px(55.0))
                                    .text_right()
                                    .text_color(t.text_dim)
                                    .child(format!("{:.1}%", ext.percentage_of_total)),
                            )
                    }))
                }),
        )
}
