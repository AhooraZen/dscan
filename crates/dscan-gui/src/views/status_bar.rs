use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use crate::theme;
use dscan_core::format_bytes;

pub fn render_status_bar(app: &DscanApp, _cx: &Context<DscanApp>) -> impl IntoElement {
    let is_scanning = app.state.is_scanning;
    let is_complete = app.state.is_complete;
    let drive = app.state.drives.get(app.state.selected_drive_idx);

    div()
        .id("status-bar")
        .h_flex()
        .w_full()
        .h(px(26.0))
        .px_3()
        .bg(theme::SURFACE_DARK)
        .border_t_1()
        .border_color(theme::BORDER_DARK)
        .text_size(px(11.0))
        .justify_between()
        .child(
            // Left: Scan progress or status
            div()
                .h_flex()
                .gap_2()
                .items_center()
                .when(is_scanning, |s| {
                    let prog = app.state.progress.as_ref();
                    let rate_str = prog
                        .map(|p| {
                            format!(
                                "{:.0} files/s · {}/s",
                                p.files_per_sec,
                                format_bytes(p.bytes_per_sec as u64)
                            )
                        })
                        .unwrap_or_default();
                    let elapsed = prog
                        .map(|p| p.elapsed_millis as f64 / 1000.0)
                        .unwrap_or(0.0);

                    s.child(
                        div()
                            .w(px(7.0))
                            .h(px(7.0))
                            .rounded_full()
                            .bg(theme::ACCENT_AMBER),
                    )
                    .child(div().text_color(theme::TEXT_PRIMARY).child("Scanning"))
                    .child(
                        div()
                            .text_color(theme::TEXT_MUTED)
                            .child(format!("({rate_str} · {elapsed:.1}s)")),
                    )
                })
                .when(!is_scanning && is_complete, |s| {
                    let prog = app.state.progress.as_ref();
                    let total_files = prog.map(|p| p.total_files).unwrap_or(0);
                    let total_bytes = prog.map(|p| p.total_bytes).unwrap_or(0);
                    let elapsed = prog
                        .map(|p| p.elapsed_millis as f64 / 1000.0)
                        .unwrap_or(0.0);

                    s.child(
                        div()
                            .w(px(7.0))
                            .h(px(7.0))
                            .rounded_full()
                            .bg(theme::ACCENT_GREEN),
                    )
                    .child(div().text_color(theme::TEXT_PRIMARY).child("Scan complete"))
                    .child(div().text_color(theme::TEXT_MUTED).child(format!(
                        "({total_files} files · {} in {elapsed:.1}s)",
                        format_bytes(total_bytes)
                    )))
                })
                .when(!is_scanning && !is_complete, |s| {
                    s.child(
                        div()
                            .w(px(7.0))
                            .h(px(7.0))
                            .rounded_full()
                            .bg(theme::TEXT_DIM),
                    )
                    .child(div().text_color(theme::TEXT_MUTED).child("Ready to scan"))
                }),
        )
        .child(
            // Right: Drive capacity & workers
            div()
                .h_flex()
                .gap_3()
                .items_center()
                .when_some(drive, |s, d| {
                    let used_pct = d.used_percentage();
                    s.child(
                        div()
                            .h_flex()
                            .gap_1()
                            .items_center()
                            .child(div().text_color(theme::TEXT_MUTED).child(format!(
                                "Drive: {} / {} ({used_pct:.0}%)",
                                format_bytes(d.used_space()),
                                format_bytes(d.total_space)
                            )))
                            .child(
                                div()
                                    .w(px(32.0))
                                    .h(px(4.0))
                                    .rounded_sm()
                                    .bg(theme::BORDER_DARK)
                                    .child(
                                        div()
                                            .h_full()
                                            .rounded_sm()
                                            .bg(if used_pct > 90.0 {
                                                theme::ACCENT_RED
                                            } else {
                                                theme::ACCENT_BLUE
                                            })
                                            .w(px(
                                                (32.0 * (used_pct / 100.0) as f32).clamp(1.0, 32.0)
                                            )),
                                    ),
                            ),
                    )
                })
                .child(
                    div()
                        .text_color(theme::TEXT_DIM)
                        .font_weight(FontWeight::NORMAL)
                        .child(format!("Threads: {}", app.threads)),
                ),
        )
}
