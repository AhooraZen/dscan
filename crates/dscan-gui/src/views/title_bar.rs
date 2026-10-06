use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use crate::theme;

pub fn render_title_bar(app: &DscanApp, cx: &Context<DscanApp>) -> impl IntoElement {
    let is_scanning = app.state.is_scanning;
    let is_paused = app.state.is_paused;

    div()
        .id("title-bar")
        .h_flex()
        .w_full()
        .h(px(44.0))
        .px_4()
        .bg(theme::SURFACE_DARK)
        .border_b_1()
        .border_color(theme::BORDER_DARK)
        .justify_between()
        .child(
            // Left branding and target path
            div()
                .h_flex()
                .gap_3()
                .child(
                    div()
                        .h_flex()
                        .gap_1()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::ACCENT_GREEN)
                                .text_size(px(14.0))
                                .child("dscan"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .px_1()
                                .rounded_sm()
                                .bg(theme::BORDER_DARK)
                                .text_color(theme::TEXT_MUTED)
                                .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::TEXT_MUTED)
                                .child("Target:"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::TEXT_PRIMARY)
                                .child(app.state.target_path.to_string_lossy().to_string()),
                        ),
                ),
        )
        .child(
            // Center drive chips
            div()
                .h_flex()
                .gap_1()
                .children(app.state.drives.iter().enumerate().map(|(idx, drive)| {
                    let is_selected = idx == app.state.selected_drive_idx;
                    let mount_str = drive.mount_point.to_string_lossy().to_string();
                    let used_pct = drive.used_percentage();
                    let label = format!("{mount_str} ({used_pct:.0}%)");

                    div()
                        .id(("drive-chip", idx))
                        .px_2()
                        .py(px(2.0))
                        .rounded_md()
                        .text_size(px(11.0))
                        .cursor_pointer()
                        .when(is_selected, |s| {
                            s.bg(theme::BORDER_LIGHT).text_color(theme::TEXT_PRIMARY)
                        })
                        .when(!is_selected, |s| {
                            s.bg(theme::SURFACE_HOVER).text_color(theme::TEXT_MUTED)
                        })
                        .hover(|s| s.opacity(0.85))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, cx| {
                                this.select_drive(idx, cx);
                            }),
                        )
                        .child(label)
                })),
        )
        .child(
            // Right scan controls
            div()
                .h_flex()
                .gap_2()
                .when(!is_scanning, |s| {
                    s.child(
                        div()
                            .id("btn-scan")
                            .px_3()
                            .py(px(3.0))
                            .rounded_md()
                            .bg(theme::ACCENT_GREEN)
                            .text_color(theme::BG_DARK)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.0))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .active(|s| s.opacity(0.70))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.start_scan(cx);
                                }),
                            )
                            .child("Scan"),
                    )
                })
                .when(is_scanning, |s| {
                    s.child(
                        div()
                            .id("btn-pause")
                            .px_3()
                            .py(px(3.0))
                            .rounded_md()
                            .bg(theme::ACCENT_AMBER)
                            .text_color(theme::BG_DARK)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.0))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    if this.state.is_paused {
                                        this.resume_scan(cx);
                                    } else {
                                        this.pause_scan(cx);
                                    }
                                }),
                            )
                            .child(if is_paused { "Resume" } else { "Pause" }),
                    )
                    .child(
                        div()
                            .id("btn-cancel")
                            .px_3()
                            .py(px(3.0))
                            .rounded_md()
                            .bg(theme::ACCENT_RED)
                            .text_color(theme::TEXT_PRIMARY)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.0))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.cancel_scan(cx);
                                }),
                            )
                            .child("Cancel"),
                    )
                }),
        )
}
