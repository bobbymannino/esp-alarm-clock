use gpui_kit::{Size, component::Root, *};

use crate::main_window::MainWindow;

fn window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
        window_min_size: Some(Size::new(Pixels::from(600.0), Pixels::from(600.0))),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from("ESP Alarm Clock")),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        window_bounds: Some(WindowBounds::centered(Size::new(Pixels::from(600.0), Pixels::from(600.0)), cx)),
        app_id: Some(String::from("esp-alarm-clock")),
        ..Default::default()
    }
}

pub fn run() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_kit::init(cx);

        crate::actions::init(cx);

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let window_options = window_options(cx);

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|cx| MainWindow::new(window, cx));
                // This first level on the window, should be a Root.
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
