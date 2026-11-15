use std::borrow::Cow;

use gpui::{
    App, AppContext as _, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowDecorations, WindowOptions, px, size,
};

use bluet::app::BluetApp;
use bluet::icons::BluetAssets;
use bluet::input;

const INTER: &[u8] = include_bytes!("../assets/fonts/Inter.ttf");
const MONO: &[u8] = include_bytes!("../assets/fonts/GeistMono.ttf");

fn main() {
    Application::new()
        .with_assets(BluetAssets)
        .run(|cx: &mut App| {
            cx.text_system()
                .add_fonts(vec![Cow::Borrowed(INTER), Cow::Borrowed(MONO)])
                .expect("bundled fonts");
            cx.bind_keys(input::key_bindings());

            let bounds = Bounds::centered(None, size(px(1440.0), px(1000.0)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(1200.0), px(760.0))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Bluet".into()),
                            appears_transparent: true,
                            traffic_light_position: None,
                        }),
                        window_decorations: Some(WindowDecorations::Client),
                        window_background: WindowBackgroundAppearance::Opaque,
                        app_id: Some("dev.bluet.manager".into()),
                        ..Default::default()
                    },
                    |_window, cx| cx.new(BluetApp::new),
                )
                .expect("open Bluet window");

            window
                .update(cx, |app, window, cx| {
                    app.install_shutdown(window, cx);
                })
                .ok();

            cx.activate(true);
        });
}
