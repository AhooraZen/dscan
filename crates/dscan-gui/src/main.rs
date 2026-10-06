extern crate gpui_kit as gpui;

use gpui::AppContext as _;
use gpui::{App, KeyBinding, WindowBounds, WindowOptions, px, size};
use gpui_kit::open_window;

pub mod app;
pub mod state;
pub mod system;
pub mod theme;
pub mod treemap;
pub mod views;

use app::{CancelScan, DscanApp, OpenPath, TogglePause};

fn main() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);

        cx.bind_keys([
            KeyBinding::new("ctrl-o", OpenPath, None),
            KeyBinding::new("space", TogglePause, None),
            KeyBinding::new("escape", CancelScan, None),
        ]);

        let bounds = WindowBounds::centered(size(px(1280.0), px(800.0)), cx);

        let window_options = WindowOptions {
            window_bounds: Some(bounds),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("dscan".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        };

        if let Err(e) = open_window(window_options, cx, |_, cx| cx.new(DscanApp::new)) {
            eprintln!("dscan-gui error: failed to initialize window: {e:?}");
            std::process::exit(1);
        }
    });
}
