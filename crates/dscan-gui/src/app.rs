use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Window,
    div,
};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::resizable::*;

use crate::state::AppState;
use crate::theme;
use crate::views::cushion_treemap::render_cushion_treemap;
use crate::views::directory_tree::render_directory_tree;
use crate::views::extension_legend::render_extension_legend;
use crate::views::status_bar::render_status_bar;
use crate::views::title_bar::render_title_bar;

gpui_kit::actions!(dscan, [OpenPath, TogglePause, CancelScan]);

pub struct DscanApp {
    pub state: AppState,
    pub threads: usize,
    pub pacman_phase: usize,
}

impl DscanApp {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            state: AppState::new(),
            threads,
            pacman_phase: 0,
        }
    }

    pub fn select_drive(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.state.select_drive(idx);
        cx.notify();
    }

    pub fn start_scan(&mut self, cx: &mut Context<Self>) {
        let target = self.state.target_path.clone();
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
        if let Some(id) = self.state.hit_test(px, py) {
            self.state.selected_node_id = Some(id);
            cx.notify();
        }
    }

    pub fn poll_progress(&mut self, cx: &mut Context<Self>) {
        self.state.poll_progress();
        self.pacman_phase = (self.pacman_phase + 1) % 4;
        cx.notify();
    }
}

impl Render for DscanApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = window.viewport_size();
        let avail_w: f32 = size.width.into();
        let total_h: f32 = size.height.into();
        let avail_h = (total_h - 44.0 - 26.0).max(100.0);

        // Treemap gets bottom half of available vertical space
        self.state.update_layout_size(avail_w, avail_h * 0.5);

        div()
            .id("dscan-app-root")
            .size_full()
            .v_flex()
            .bg(theme::BG_DARK)
            .text_color(theme::TEXT_PRIMARY)
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
            .child(render_status_bar(self, cx))
    }
}
