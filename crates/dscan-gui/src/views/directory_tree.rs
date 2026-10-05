use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use crate::system::reveal_in_file_manager;
use crate::theme;
use dscan_core::format_bytes;

pub fn render_directory_tree(app: &DscanApp, cx: &Context<DscanApp>) -> impl IntoElement {
    let is_scanning = app.state.is_scanning;
    let is_complete = app.state.is_complete;

    div()
        .id("directory-tree-pane")
        .v_flex()
        .size_full()
        .bg(theme::BG_DARK)
        .border_r_1()
        .border_color(theme::BORDER_DARK)
        .child(
            // Tree Table Header
            div()
                .h_flex()
                .w_full()
                .h(px(28.0))
                .px_3()
                .bg(theme::SURFACE_DARK)
                .border_b_1()
                .border_color(theme::BORDER_DARK)
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::TEXT_MUTED)
                .justify_between()
                .child(div().w(px(240.0)).child("Name"))
                .child(div().w(px(80.0)).text_right().child("Size"))
                .child(div().w(px(60.0)).text_right().child("%"))
                .child(div().w(px(60.0)).text_right().child("Items")),
        )
        .child(
            // Content region
            div()
                .id("directory-tree-content")
                .v_flex()
                .flex_1()
                .overflow_y_scroll()
                .px_2()
                .py_1()
                .when(is_scanning, |s| {
                    // WinDirStat Pacman scanning indicator
                    let pacman_art = match app.pacman_phase % 4 {
                        0 => "ᗧ · · · · ·",
                        1 => "O · · · · ·",
                        2 => "ᗧ · · · · ·",
                        _ => "o · · · · ·",
                    };
                    let prog = app.state.progress.as_ref();
                    let scanned_bytes = prog.map(|p| p.total_bytes).unwrap_or(0);
                    let scanned_files = prog.map(|p| p.total_files).unwrap_or(0);
                    let current_path = prog.map(|p| p.current_path.clone()).unwrap_or_default();

                    s.child(
                        div()
                            .v_flex()
                            .p_4()
                            .gap_2()
                            .rounded_md()
                            .bg(theme::SURFACE_DARK)
                            .border_1()
                            .border_color(theme::BORDER_DARK)
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(18.0))
                                            .text_color(theme::ACCENT_AMBER)
                                            .font_weight(FontWeight::BOLD)
                                            .child(pacman_art),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::TEXT_PRIMARY)
                                            .child("Scanning directories..."),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::TEXT_MUTED)
                                    .child(format!(
                                        "Scanned: {scanned_files} files ({})",
                                        format_bytes(scanned_bytes)
                                    )),
                            )
                            .child(div().text_size(px(10.0)).text_color(theme::TEXT_DIM).child(
                                if current_path.is_empty() {
                                    "Traversing...".to_string()
                                } else {
                                    current_path
                                },
                            )),
                    )
                })
                .when(
                    !is_scanning && !is_complete && app.state.raw_nodes.is_empty(),
                    |s| {
                        s.child(
                            div()
                                .v_flex()
                                .items_center()
                                .justify_center()
                                .p_8()
                                .text_color(theme::TEXT_MUTED)
                                .text_size(px(13.0))
                                .child("Select a drive and click 'Scan' to start"),
                        )
                    },
                )
                .when(!app.state.raw_nodes.is_empty(), |s| {
                    let total_bytes = app.state.raw_nodes[0].total_bytes.max(1);
                    let mut visible_rows = Vec::new();
                    collect_visible_tree_rows(
                        &app.state.raw_nodes,
                        0,
                        &app.state.expanded_dirs,
                        &mut visible_rows,
                    );

                    s.children(visible_rows.into_iter().map(|node_id| {
                        let node = &app.state.raw_nodes[node_id as usize];
                        let is_selected = app.state.selected_node_id == Some(node_id);
                        let is_expanded = app.state.expanded_dirs.contains(&node_id);
                        let indent = (node.rel_depth as f32) * 12.0;
                        let pct = (node.total_bytes as f64 / total_bytes as f64) * 100.0;

                        div()
                            .id(("tree-row", node_id))
                            .h_flex()
                            .w_full()
                            .h(px(22.0))
                            .px_2()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_size(px(11.0))
                            .when(is_selected, |s| s.bg(theme::SURFACE_HOVER))
                            .when(!is_selected, |s| s.hover(|h| h.bg(theme::SURFACE_DARK)))
                            .justify_between()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    this.select_node(node_id, cx);
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, _, _window, _cx| {
                                    let path = this
                                        .state
                                        .target_path
                                        .join(&this.state.raw_nodes[node_id as usize].name);
                                    let _ = reveal_in_file_manager(&path);
                                }),
                            )
                            .child(
                                // Name column with indent and expand toggle
                                div()
                                    .h_flex()
                                    .w(px(240.0))
                                    .items_center()
                                    .pl(px(indent))
                                    .gap_1()
                                    .when(node.is_dir, |row| {
                                        let caret = if is_expanded { "▾" } else { "▸" };
                                        row.child(
                                            div()
                                                .cursor_pointer()
                                                .text_color(theme::TEXT_MUTED)
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _window, cx| {
                                                        this.toggle_dir_expanded(node_id, cx);
                                                    }),
                                                )
                                                .child(caret),
                                        )
                                        .child(div().text_color(theme::ACCENT_AMBER).child("📁"))
                                    })
                                    .when(!node.is_dir, |row| {
                                        row.child(div().w(px(10.0))).child(
                                            div()
                                                .text_color(theme::extension_color(&node.extension))
                                                .child("🗎"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_color(if is_selected {
                                                theme::ACCENT_BLUE
                                            } else {
                                                theme::TEXT_PRIMARY
                                            })
                                            .font_weight(if node.is_dir {
                                                FontWeight::MEDIUM
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .child(node.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(80.0))
                                    .text_right()
                                    .text_color(theme::TEXT_MUTED)
                                    .child(format_bytes(node.total_bytes)),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .w(px(60.0))
                                    .justify_end()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(4.0))
                                            .rounded_sm()
                                            .bg(theme::BORDER_DARK)
                                            .child(
                                                div()
                                                    .h_full()
                                                    .rounded_sm()
                                                    .bg(theme::ACCENT_BLUE)
                                                    .w(px((24.0 * (pct / 100.0) as f32)
                                                        .clamp(1.0, 24.0))),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(theme::TEXT_DIM)
                                            .child(format!("{pct:.1}%")),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(60.0))
                                    .text_right()
                                    .text_color(theme::TEXT_DIM)
                                    .child(if node.is_dir {
                                        format!("{}", node.children_ids.len())
                                    } else {
                                        "-".to_string()
                                    }),
                            )
                    }))
                }),
        )
}

fn collect_visible_tree_rows(
    nodes: &[dscan_core::TreemapNodeDto],
    curr_id: u32,
    expanded: &std::collections::HashSet<u32>,
    out: &mut Vec<u32>,
) {
    if curr_id as usize >= nodes.len() {
        return;
    }
    out.push(curr_id);

    if expanded.contains(&curr_id) {
        let node = &nodes[curr_id as usize];
        for &cid in &node.children_ids {
            collect_visible_tree_rows(nodes, cid, expanded, out);
        }
    }
}
