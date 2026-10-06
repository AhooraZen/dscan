use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use crate::theme;
use dscan_core::format_bytes;

pub fn render_directory_tree(app: &DscanApp, cx: &Context<DscanApp>) -> impl IntoElement {
    let t = app.theme();
    let is_scanning = app.state.is_scanning;
    let is_complete = app.state.is_complete;

    div()
        .id("directory-tree-pane")
        .v_flex()
        .size_full()
        .bg(t.bg)
        .border_r_1()
        .border_color(t.border)
        .child(
            // Tree Table Header
            div()
                .h_flex()
                .w_full()
                .h(px(36.0))
                .px(px(12.0))
                .bg(t.surface)
                .border_b_1()
                .border_color(t.border)
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text_dim)
                .justify_between()
                .items_center()
                .child(div().w(px(240.0)).child("Name"))
                .child(div().w(px(80.0)).text_right().child("Size"))
                .child(div().w(px(55.0)).text_right().child("%"))
                .child(div().w(px(55.0)).text_right().child("Items"))
                .child(div().w(px(60.0)).text_center().child("Actions")),
        )
        .child(
            // Content region
            div()
                .id("directory-tree-content")
                .v_flex()
                .flex_1()
                .overflow_y_scroll()
                .px(px(8.0))
                .py(px(4.0))
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
                            .p(px(16.0))
                            .gap(px(8.0))
                            .rounded_lg()
                            .bg(t.surface)
                            .border_1()
                            .border_color(t.border)
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(22.0))
                                            .text_color(t.accent_amber)
                                            .font_weight(FontWeight::BOLD)
                                            .child(pacman_art),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(15.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(t.text_primary)
                                            .child("Scanning directories..."),
                                    ),
                            )
                            .child(div().text_size(px(13.0)).text_color(t.text_muted).child(
                                format!(
                                    "Scanned: {scanned_files} files ({})",
                                    format_bytes(scanned_bytes)
                                ),
                            ))
                            .child(div().text_size(px(12.0)).text_color(t.text_dim).child(
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
                                .p(px(32.0))
                                .text_color(t.text_dim)
                                .text_size(px(14.0))
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
                        let indent = (node.rel_depth as f32) * 18.0;
                        let pct = (node.total_bytes as f64 / total_bytes as f64) * 100.0;

                        div()
                            .id(("tree-row", node_id))
                            .h_flex()
                            .w_full()
                            .h(px(32.0))
                            .px(px(8.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_size(px(12.0))
                            .when(is_selected, |s| s.bg(t.surface_hover))
                            .when(!is_selected, |s| s.hover(|h| h.bg(t.surface)))
                            .justify_between()
                            .items_center()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    this.select_node(node_id, cx);
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, _window, cx| {
                                        let px_x: f32 = event.position.x.into();
                                        let px_y: f32 = event.position.y.into();
                                        this.open_context_menu(node_id, px_x, px_y, cx);
                                    },
                                ),
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
                                                .text_color(t.text_muted)
                                                .px(px(2.0))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _window, cx| {
                                                        this.toggle_dir_expanded(node_id, cx);
                                                    }),
                                                )
                                                .child(caret),
                                        )
                                        .child(div().text_color(t.accent_amber).child("📁"))
                                    })
                                    .when(!node.is_dir, |row| {
                                        row.child(div().w(px(14.0))).child(
                                            div()
                                                .text_color(theme::extension_color(&node.extension))
                                                .child("🗎"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_color(if is_selected {
                                                t.accent_blue
                                            } else {
                                                t.text_primary
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
                                    .text_color(t.text_muted)
                                    .child(format_bytes(node.total_bytes)),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .w(px(55.0))
                                    .justify_end()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .w(px(32.0))
                                            .h(px(5.0))
                                            .rounded_md()
                                            .bg(t.border_light)
                                            .child(
                                                div()
                                                    .h_full()
                                                    .rounded_md()
                                                    .bg(t.accent_blue)
                                                    .w(px((32.0 * (pct / 100.0) as f32)
                                                        .clamp(1.0, 32.0))),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(t.text_dim)
                                            .child(format!("{pct:.1}%")),
                                    ),
                            )
                            .child(div().w(px(55.0)).text_right().text_color(t.text_dim).child(
                                if node.is_dir {
                                    format!("{}", node.children_ids.len())
                                } else {
                                    "-".to_string()
                                },
                            ))
                            .child(
                                div()
                                    .h_flex()
                                    .w(px(60.0))
                                    .justify_center()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id(("reveal-btn", node_id))
                                            .cursor_pointer()
                                            .px(px(4.0))
                                            .py(px(2.0))
                                            .rounded_sm()
                                            .hover(move |h| h.bg(t.surface_hover))
                                            .text_size(px(12.0))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _window, cx| {
                                                    this.reveal_node(node_id, cx);
                                                }),
                                            )
                                            .child("📂"),
                                    )
                                    .child(
                                        div()
                                            .id(("trash-btn", node_id))
                                            .cursor_pointer()
                                            .px(px(4.0))
                                            .py(px(2.0))
                                            .rounded_sm()
                                            .hover(move |h| h.bg(t.surface_hover))
                                            .text_size(px(12.0))
                                            .text_color(t.accent_red)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _window, cx| {
                                                    this.request_trash_node(node_id, cx);
                                                }),
                                            )
                                            .child("🗑"),
                                    ),
                            )
                    }))
                }),
        )
}

fn collect_visible_tree_rows(
    nodes: &[dscan_core::snapshot::TreemapNodeDto],
    node_id: u32,
    expanded: &std::collections::HashSet<u32>,
    out: &mut Vec<u32>,
) {
    if node_id as usize >= nodes.len() {
        return;
    }
    out.push(node_id);
    let node = &nodes[node_id as usize];
    if node.is_dir && expanded.contains(&node_id) {
        for &child_id in &node.children_ids {
            collect_visible_tree_rows(nodes, child_id, expanded, out);
        }
    }
}
