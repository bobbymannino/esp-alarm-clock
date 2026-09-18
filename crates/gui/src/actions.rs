use gpui_kit::*;

gpui_kit::actions!(App, [CloseWindow, Quit]);

/// Close and quit use the platform's primary modifier: Cmd on macOS, Ctrl elsewhere.
const CLOSE_WINDOW_KEYSTROKE: &str = if cfg!(target_os = "macos") { "cmd-w" } else { "ctrl-w" };
const QUIT_KEYSTROKE: &str = if cfg!(target_os = "macos") { "cmd-q" } else { "ctrl-q" };

/// Registers the app-wide keybindings and their handlers.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(CLOSE_WINDOW_KEYSTROKE, CloseWindow, None),
        KeyBinding::new(QUIT_KEYSTROKE, Quit, None),
    ]);

    cx.on_action(|_: &CloseWindow, cx| {
        if let Some(handle) = cx.active_window() {
            cx.defer(move |cx| {
                handle
                    .update(cx, |_, window, _| {
                        window.remove_window();
                    })
                    .ok();
            });
        }
    });

    cx.on_action(|_: &Quit, cx| cx.quit());
}
