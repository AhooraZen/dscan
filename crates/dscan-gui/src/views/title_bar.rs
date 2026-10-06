use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;

pub fn render_title_bar(app: &DscanApp, cx: &Context<DscanApp>) -> impl IntoElement {
    let t = app.theme();
    let is_scanning = app.state.is_scanning;
    let is_paused = app.state.is_paused;
    let is_dark = app.state.theme_mode.is_dark();

    div()
        .id("title-bar")
        .h_flex()
        .w_full()
        .h(px(56.0))
        .px(px(20.0))
        .bg(t.surface)
        .border_b_1()
        .border_color(t.border)
        .justify_between()
        .items_center()
        .child(
            // Left branding and target path
            div()
                .h_flex()
                .items_center()
                .gap(px(16.0))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(t.accent_green)
                                .text_size(px(18.0))
                                .child("⚡ dscan"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded_md()
                                .bg(t.surface_hover)
                                .text_color(t.text_dim)
                                .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(t.text_dim)
                                .child("Target:"),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .child(app.state.target_path.to_string_lossy().to_string()),
                        ),
                ),
        )
        .child(
            // Center drive chips
            div().h_flex().items_center().gap(px(6.0)).children(
                app.state.drives.iter().enumerate().map(|(idx, drive)| {
                    let is_selected = idx == app.state.selected_drive_idx;
                    let mount_str = drive.mount_point.to_string_lossy().to_string();
                    let used_pct = drive.used_percentage();
                    let label = format!("{mount_str} ({used_pct:.0}%)");

                    div()
                        .id(("drive-chip", idx))
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded_lg()
                        .text_size(px(12.0))
                        .font_weight(if is_selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .cursor_pointer()
                        .when(is_selected, |s| {
                            s.bg(t.accent_blue).text_color(gpui::Rgba {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            })
                        })
                        .when(!is_selected, |s| {
                            s.bg(t.surface_hover).text_color(t.text_muted)
                        })
                        .hover(|s| s.opacity(0.85))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, cx| {
                                this.select_drive(idx, cx);
                            }),
                        )
                        .child(label)
                }),
            ),
        )
        .child(
            // Right scan controls & Theme Toggle
            div()
                .h_flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    // Theme Switcher Button
                    div()
                        .id("btn-theme-toggle")
                        .h_flex()
                        .items_center()
                        .gap_1()
                        .px(px(12.0))
                        .py(px(7.0))
                        .rounded_lg()
                        .bg(t.surface_hover)
                        .hover(move |h| h.bg(t.border_light))
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text_primary)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _window, cx| {
                                this.toggle_theme(cx);
                            }),
                        )
                        .child(if is_dark { "🌙 Dark" } else { "☀️ Light" }),
                )
                .when(!is_scanning, |s| {
                    s.child(
                        div()
                            .id("btn-scan")
                            .px(px(16.0))
                            .py(px(7.0))
                            .rounded_lg()
                            .bg(t.accent_green)
                            .text_color(if is_dark {
                                t.bg
                            } else {
                                gpui::Rgba {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 1.0,
                                }
                            })
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(13.0))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .active(|s| s.opacity(0.70))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.start_scan(cx);
                                }),
                            )
                            .child("▶ Scan"),
                    )
                })
                .when(is_scanning, |s| {
                    s.child(
                        div()
                            .id("btn-pause")
                            .px(px(16.0))
                            .py(px(7.0))
                            .rounded_lg()
                            .bg(t.accent_amber)
                            .text_color(if is_dark {
                                t.bg
                            } else {
                                gpui::Rgba {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 1.0,
                                }
                            })
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(13.0))
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
                            .child(if is_paused { "▶ Resume" } else { "⏸ Pause" }),
                    )
                    .child(
                        div()
                            .id("btn-cancel")
                            .px(px(16.0))
                            .py(px(7.0))
                            .rounded_lg()
                            .bg(t.accent_red)
                            .text_color(gpui::Rgba {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            })
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(13.0))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.cancel_scan(cx);
                                }),
                            )
                            .child("⏹ Cancel"),
                    )
                }),
        )
}
