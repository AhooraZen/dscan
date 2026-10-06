use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use dscan_core::format_bytes;

pub fn render_status_bar(app: &DscanApp, _cx: &Context<DscanApp>) -> impl IntoElement {
    let t = app.theme();
    let is_scanning = app.state.is_scanning;
    let is_complete = app.state.is_complete;
    let drive = app.state.drives.get(app.state.selected_drive_idx);

    div()
        .id("status-bar")
        .h_flex()
        .w_full()
        .h(px(30.0))
        .px_4()
        .bg(t.surface)
        .border_t_1()
        .border_color(t.border)
        .text_size(px(12.0))
        .justify_between()
        .items_center()
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
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(t.accent_amber),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .child("Scanning..."),
                    )
                    .child(
                        div()
                            .text_color(t.text_muted)
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
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(t.accent_green),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .child("Scan complete"),
                    )
                    .child(div().text_color(t.text_muted).child(format!(
                        "({total_files} files · {} in {elapsed:.1}s)",
                        format_bytes(total_bytes)
                    )))
                })
                .when(!is_scanning && !is_complete, |s| {
                    s.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(t.text_dim))
                        .child(div().text_color(t.text_muted).child("Ready to scan"))
                }),
        )
        .child(
            // Right: Drive capacity & workers
            div()
                .h_flex()
                .gap_4()
                .items_center()
                .when_some(drive, |s, d| {
                    let used_pct = d.used_percentage();
                    s.child(
                        div()
                            .h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_color(t.text_muted).child(format!(
                                "Drive: {} / {} ({used_pct:.0}%)",
                                format_bytes(d.used_space()),
                                format_bytes(d.total_space)
                            )))
                            .child(
                                div()
                                    .w(px(40.0))
                                    .h(px(5.0))
                                    .rounded_sm()
                                    .bg(t.border_light)
                                    .child(
                                        div()
                                            .h_full()
                                            .rounded_sm()
                                            .bg(if used_pct > 90.0 {
                                                t.accent_red
                                            } else {
                                                t.accent_blue
                                            })
                                            .w(px(
                                                (40.0 * (used_pct / 100.0) as f32).clamp(1.0, 40.0)
                                            )),
                                    ),
                            ),
                    )
                })
                .child(
                    div()
                        .px_2()
                        .py(px(1.0))
                        .rounded_sm()
                        .bg(t.surface_hover)
                        .text_color(t.text_muted)
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(px(11.0))
                        .child(format!("Workers: {}", app.threads)),
                ),
        )
}
