use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, Styled, div, px,
};
use gpui_kit::base::StyledExt as _;

use crate::app::DscanApp;
use crate::theme;
use crate::treemap::CushionTreemapElement;
use dscan_core::format_bytes;

pub fn render_cushion_treemap(app: &DscanApp, cx: &Context<DscanApp>) -> impl IntoElement {
    let t = app.theme();
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
        .bg(t.bg)
        .child(
            CushionTreemapElement::new(app.state.layout_nodes.clone())
                .bg_color(t.bg)
                .select_color(t.accent_blue)
                .hover_color(t.text_primary)
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
            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                let px_x: f32 = event.position.x.into();
                let px_y: f32 = event.position.y.into();
                if let Some(id) = this.state.hit_test(px_x, px_y) {
                    this.open_context_menu(id, px_x, px_y, cx);
                }
            }),
        )
        .when_some(active_node, |pane, node| {
            let ext_color = theme::extension_color(&node.extension);
            let node_id = node.id;
            pane.child(
                div()
                    .absolute()
                    .bottom(px(14.0))
                    .left(px(14.0))
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .bg(t.surface)
                    .border_1()
                    .border_color(t.border_light)
                    .h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().w(px(12.0)).h(px(12.0)).rounded_sm().bg(ext_color))
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(13.0))
                                            .text_color(t.text_primary)
                                            .child(node.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(t.accent_amber)
                                            .child(format_bytes(node.total_bytes)),
                                    ),
                            )
                            .child(div().text_size(px(11.0)).text_color(t.text_dim).child(
                                if node.is_dir {
                                    format!(
                                        "Directory (Depth {}, {} items) — Right-click for options",
                                        node.rel_depth,
                                        node.children_ids.len()
                                    )
                                } else {
                                    format!(
                                        "{} file (Depth {}) — Right-click for options",
                                        node.extension, node.rel_depth
                                    )
                                },
                            )),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .id("treemap-tooltip-reveal")
                                    .px_3()
                                    .py(px(3.0))
                                    .rounded_md()
                                    .bg(t.surface_hover)
                                    .hover(move |h| h.bg(t.border_light))
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text_primary)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _window, cx| {
                                            this.reveal_node(node_id, cx);
                                        }),
                                    )
                                    .child("📂 Reveal"),
                            )
                            .child(
                                div()
                                    .id("treemap-tooltip-trash")
                                    .px_3()
                                    .py(px(3.0))
                                    .rounded_md()
                                    .bg(t.surface_hover)
                                    .hover(move |h| h.bg(t.accent_red))
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.accent_red)
                                    .hover(move |h| h.text_color(t.text_primary))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _window, cx| {
                                            this.request_trash_node(node_id, cx);
                                        }),
                                    )
                                    .child("🗑 Trash"),
                            ),
                    ),
            )
        })
}
