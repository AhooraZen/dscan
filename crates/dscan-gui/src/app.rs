use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Render, Styled as _, Window, div, px,
};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::resizable::*;

use crate::state::AppState;
use crate::views::cushion_treemap::render_cushion_treemap;
use crate::views::directory_tree::render_directory_tree;
use crate::views::extension_legend::render_extension_legend;
use crate::views::status_bar::render_status_bar;
use crate::views::title_bar::render_title_bar;

gpui_kit::actions!(dscan, [OpenPath, TogglePause, CancelScan]);

#[derive(Debug, Clone, PartialEq)]
pub struct PendingTrash {
    pub node_id: u32,
    pub name: String,
    pub total_bytes: u64,
    pub path: PathBuf,
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuState {
    pub node_id: u32,
    pub pos_x: f32,
    pub pos_y: f32,
}

pub struct DscanApp {
    pub state: AppState,
    pub threads: usize,
    pub pacman_phase: usize,
    pub context_menu: Option<ContextMenuState>,
    pub pending_trash: Option<PendingTrash>,
}

impl DscanApp {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let app_state = AppState::new();
        let threads = dscan_core::ScanOptions::auto_threads_for_path(std::path::Path::new(
            &app_state.target_path,
        ));

        Self {
            state: app_state,
            threads,
            pacman_phase: 0,
            context_menu: None,
            pending_trash: None,
        }
    }

    pub fn select_drive(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.state.select_drive(idx);
        self.threads = dscan_core::ScanOptions::auto_threads_for_path(std::path::Path::new(
            &self.state.target_path,
        ));
        cx.notify();
    }

    pub fn start_scan(&mut self, cx: &mut Context<Self>) {
        let target = self.state.target_path.clone();
        self.threads =
            dscan_core::ScanOptions::auto_threads_for_path(std::path::Path::new(&target));
        if self.state.start_scan(&target, self.threads).is_ok() {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(50))
                        .await;

                    if let Some(this) = this.upgrade() {
                        let keep_running = this.update(cx, |this, cx| {
                            if this.state.is_scanning {
                                this.poll_progress(cx);
                                true
                            } else if this.state.is_complete {
                                this.poll_progress(cx);
                                false
                            } else {
                                false
                            }
                        });
                        if !keep_running {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.notify();
    }

    pub fn pause_scan(&mut self, cx: &mut Context<Self>) {
        self.state.pause_scan();
        cx.notify();
    }

    pub fn resume_scan(&mut self, cx: &mut Context<Self>) {
        self.state.resume_scan();
        cx.notify();
    }

    pub fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        self.state.cancel_scan();
        cx.notify();
    }

    pub fn select_node(&mut self, id: u32, cx: &mut Context<Self>) {
        self.state.selected_node_id = Some(id);
        cx.notify();
    }

    pub fn toggle_dir_expanded(&mut self, id: u32, cx: &mut Context<Self>) {
        self.state.toggle_dir_expanded(id);
        cx.notify();
    }

    pub fn toggle_extension_filter(&mut self, ext: &str, cx: &mut Context<Self>) {
        self.state.toggle_extension_filter(ext);
        cx.notify();
    }

    pub fn handle_treemap_mouse_move(&mut self, px: f32, py: f32, cx: &mut Context<Self>) {
        let hit = self.state.hit_test(px, py);
        if self.state.hovered_node_id != hit {
            self.state.hovered_node_id = hit;
            cx.notify();
        }
    }

    pub fn handle_treemap_click(&mut self, px: f32, py: f32, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        if let Some(id) = self.state.hit_test(px, py) {
            self.state.selected_node_id = Some(id);
            cx.notify();
        }
    }

    pub fn open_context_menu(
        &mut self,
        node_id: u32,
        pos_x: f32,
        pos_y: f32,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = Some(ContextMenuState {
            node_id,
            pos_x,
            pos_y,
        });
        cx.notify();
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.is_some() {
            self.context_menu = None;
            cx.notify();
        }
    }

    pub fn reveal_node(&mut self, node_id: u32, cx: &mut Context<Self>) {
        let path = self.state.node_full_path(node_id);
        let _ = crate::system::reveal_in_file_manager(&path);
        self.close_context_menu(cx);
    }

    pub fn request_trash_node(&mut self, node_id: u32, cx: &mut Context<Self>) {
        self.context_menu = None;
        if let Some(node) = self.state.find_node(node_id) {
            let path = self.state.node_full_path(node_id);
            self.pending_trash = Some(PendingTrash {
                node_id,
                name: node.name.clone(),
                total_bytes: node.total_bytes,
                path,
                error_msg: None,
            });
            cx.notify();
        }
    }

    pub fn confirm_trash(&mut self, cx: &mut Context<Self>) {
        if let Some(ref pending) = self.pending_trash {
            match crate::system::move_to_trash(&pending.path, &self.state.target_path) {
                Ok(()) => {
                    let id = pending.node_id;
                    self.pending_trash = None;
                    self.state.prune_node_and_bubble_size(id as usize);
                }
                Err(err) => {
                    if let Some(ref mut p) = self.pending_trash {
                        p.error_msg = Some(err);
                    }
                }
            }
            cx.notify();
        }
    }

    pub fn cancel_trash(&mut self, cx: &mut Context<Self>) {
        self.pending_trash = None;
        cx.notify();
    }

    pub fn poll_progress(&mut self, cx: &mut Context<Self>) {
        self.state.poll_progress();
        self.pacman_phase = (self.pacman_phase + 1) % 4;
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_theme();
        cx.notify();
    }

    pub fn theme(&self) -> crate::theme::ThemeColors {
        self.state.theme_mode.colors()
    }
}

impl Render for DscanApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let size = window.viewport_size();
        let avail_w: f32 = size.width.into();
        let total_h: f32 = size.height.into();
        let avail_h = (total_h - 48.0 - 30.0).max(100.0);

        // Treemap gets bottom half of available vertical space
        self.state.update_layout_size(avail_w, avail_h * 0.5);

        let mut root = div()
            .id("dscan-app-root")
            .size_full()
            .v_flex()
            .bg(t.bg)
            .text_color(t.text_primary)
            .on_action(cx.listener(|this, _: &OpenPath, _window, cx| {
                let next_idx = (this.state.selected_drive_idx + 1) % this.state.drives.len().max(1);
                this.select_drive(next_idx, cx);
            }))
            .on_action(cx.listener(|this, _: &TogglePause, _window, cx| {
                if this.state.is_scanning {
                    if this.state.is_paused {
                        this.resume_scan(cx);
                    } else {
                        this.pause_scan(cx);
                    }
                } else {
                    this.start_scan(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CancelScan, _window, cx| {
                if this.state.is_scanning {
                    this.cancel_scan(cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    if this.context_menu.is_some() {
                        this.close_context_menu(cx);
                    }
                }),
            )
            .child(render_title_bar(self, cx))
            .child(
                div().flex_1().w_full().child(
                    v_resizable("root-v-split")
                        .child(
                            h_resizable("top-h-split")
                                .child(render_directory_tree(self, cx).into_any_element())
                                .child(render_extension_legend(self, cx).into_any_element()),
                        )
                        .child(render_cushion_treemap(self, cx).into_any_element()),
                ),
            )
            .child(render_status_bar(self, cx));

        // Render context menu popup if active
        if let Some(ref menu) = self.context_menu {
            let node_id = menu.node_id;
            let menu_w = 210.0;
            let menu_h = 75.0;
            let x = menu.pos_x.min(avail_w - menu_w).max(8.0);
            let y = menu.pos_y.min(total_h - menu_h).max(8.0);

            root = root.child(
                div()
                    .id("app-context-menu")
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(menu_w))
                    .py(px(4.0))
                    .rounded_md()
                    .bg(t.surface)
                    .border_1()
                    .border_color(t.border_light)
                    .child(
                        div()
                            .id("ctx-reveal-btn")
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py(px(6.0))
                            .cursor_pointer()
                            .hover(move |h| h.bg(t.surface_hover))
                            .text_size(px(12.0))
                            .text_color(t.text_primary)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    this.reveal_node(node_id, cx);
                                }),
                            )
                            .child("📂 Reveal in File Manager"),
                    )
                    .child(
                        div()
                            .id("ctx-trash-btn")
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py(px(6.0))
                            .cursor_pointer()
                            .hover(move |h| h.bg(t.surface_hover))
                            .text_size(px(12.0))
                            .text_color(t.accent_red)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    this.request_trash_node(node_id, cx);
                                }),
                            )
                            .child("🗑 Move to Trash"),
                    ),
            );
        }

        // Render confirmation dialog modal if active
        if let Some(ref pending) = self.pending_trash {
            let name = pending.name.clone();
            let path_str = pending.path.to_string_lossy().to_string();
            let size_str = dscan_core::format_bytes(pending.total_bytes);
            let err_opt = pending.error_msg.clone();

            root = root.child(
                div()
                    .id("trash-confirm-modal-backdrop")
                    .absolute()
                    .inset_0()
                    .bg(gpui::Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.7,
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .id("trash-confirm-modal")
                            .v_flex()
                            .w(px(460.0))
                            .p_6()
                            .gap_4()
                            .rounded_lg()
                            .bg(t.surface)
                            .border_1()
                            .border_color(t.border_light)
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text_primary)
                                    .child(div().text_color(t.accent_red).child("🗑"))
                                    .child("Move to Trash?"),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap_2()
                                    .text_size(px(12.0))
                                    .text_color(t.text_muted)
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(45.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("Name:"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(t.text_primary)
                                                    .child(name),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(45.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("Path:"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(t.text_dim)
                                                    .text_size(px(11.0))
                                                    .child(path_str),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(45.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("Size:"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(t.accent_amber)
                                                    .child(size_str),
                                            ),
                                    ),
                            )
                            .when_some(err_opt, move |d, err| {
                                d.child(
                                    div()
                                        .p_2()
                                        .rounded_sm()
                                        .bg(gpui::Rgba {
                                            r: 0.9,
                                            g: 0.2,
                                            b: 0.2,
                                            a: 0.2,
                                        })
                                        .border_1()
                                        .border_color(t.accent_red)
                                        .text_color(t.accent_red)
                                        .text_size(px(11.0))
                                        .child(format!("Error: {err}")),
                                )
                            })
                            .child(
                                div()
                                    .h_flex()
                                    .justify_end()
                                    .gap_3()
                                    .mt_2()
                                    .child(
                                        div()
                                            .id("cancel-trash-btn")
                                            .px_4()
                                            .py(px(6.0))
                                            .rounded_md()
                                            .bg(t.surface_hover)
                                            .hover(move |h| h.bg(t.border_light))
                                            .cursor_pointer()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(t.text_primary)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _window, cx| {
                                                    this.cancel_trash(cx);
                                                }),
                                            )
                                            .child("Cancel"),
                                    )
                                    .child(
                                        div()
                                            .id("confirm-trash-btn")
                                            .px_4()
                                            .py(px(6.0))
                                            .rounded_md()
                                            .bg(t.accent_red)
                                            .hover(|h| h.opacity(0.85))
                                            .cursor_pointer()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui::Rgba {
                                                r: 1.0,
                                                g: 1.0,
                                                b: 1.0,
                                                a: 1.0,
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _window, cx| {
                                                    this.confirm_trash(cx);
                                                }),
                                            )
                                            .child("Move to Trash"),
                                    ),
                            ),
                    ),
            );
        }

        root
    }
}
