use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use crate::system::reveal_in_file_manager;
use crate::theme;
use crate::treemap::CushionTreemapElement;
use dscan_core::format_bytes;

pub fn render_cushion_treemap(app: &DscanApp, cx: &Context<DscanApp>) -> impl IntoElement {
    let hovered_node = app
        .state
        .hovered_node_id
        .and_then(|id| app.state.find_node(id));
    let selected_node = app
        .state
        .selected_node_id
        .and_then(|id| app.state.find_node(id));
    let active_node = hovered_node.or(selected_node);

    div()
        .id("treemap-pane")
        .relative()
        .size_full()
        .bg(theme::BG_DARK)
        .child(
            CushionTreemapElement::new(app.state.layout_nodes.clone())
                .selected_id(app.state.selected_node_id)
                .hovered_id(app.state.hovered_node_id)
                .filter_ext(app.state.selected_extension.clone()),
        )
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
            let px_x: f32 = event.position.x.into();
            let px_y: f32 = event.position.y.into();
            this.handle_treemap_mouse_move(px_x, px_y, cx);
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                let px_x: f32 = event.position.x.into();
                let px_y: f32 = event.position.y.into();
                this.handle_treemap_click(px_x, px_y, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, event: &MouseDownEvent, _window, _cx| {
                let px_x: f32 = event.position.x.into();
                let px_y: f32 = event.position.y.into();
                if let Some(node) = this
                    .state
                    .hit_test(px_x, px_y)
                    .and_then(|id| this.state.find_node(id))
                {
                    let path = this.state.target_path.join(&node.name);
                    let _ = reveal_in_file_manager(&path);
                }
            }),
        )
        .when_some(active_node, |pane, node| {
            let ext_color = theme::extension_color(&node.extension);
            pane.child(
                div()
                    .absolute()
                    .bottom(px(12.0))
                    .left(px(12.0))
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme::SURFACE_DARK)
                    .border_1()
                    .border_color(theme::BORDER_LIGHT)
                    .h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().w(px(10.0)).h(px(10.0)).rounded_sm().bg(ext_color))
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(12.0))
                                            .text_color(theme::TEXT_PRIMARY)
                                            .child(node.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(theme::TEXT_MUTED)
                                            .child(format_bytes(node.total_bytes)),
                                    ),
                            )
                            .child(div().text_size(px(10.0)).text_color(theme::TEXT_DIM).child(
                                if node.is_dir {
                                    format!(
                                        "Directory (Depth {}, {} items) — Right-click to open",
                                        node.rel_depth,
                                        node.children_ids.len()
                                    )
                                } else {
                                    format!(
                                        "{} file (Depth {}) — Right-click to open in file manager",
                                        node.extension, node.rel_depth
                                    )
                                },
                            )),
                    ),
            )
        })
}
