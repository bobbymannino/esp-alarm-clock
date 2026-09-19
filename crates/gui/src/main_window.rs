mod flash;
mod view;

use futures::{StreamExt as _, channel::mpsc};
use gpui_kit::{
    component::input::{InputState, TextareaState},
    *,
};

pub struct MainWindow {
    /// Whether a flash read is currently in flight.
    is_reading_alarms: bool,
    /// The flash address passed to `espflash read-flash`.
    flash_address: Entity<InputState>,
    /// The number of bytes passed to `espflash read-flash`.
    flash_size: Entity<InputState>,
    /// The log textarea, kept across renders so its contents survive a repaint.
    logs: Entity<TextareaState>,
    /// Everything the child process has written so far.
    log_text: String,
}

impl MainWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            is_reading_alarms: false,
            flash_address: cx.new(|cx| InputState::new(window, cx).default_value(flash::DEFAULT_ADDRESS)),
            flash_size: cx.new(|cx| InputState::new(window, cx).default_value(flash::DEFAULT_SIZE)),
            logs: cx.new(|cx| TextareaState::new(window, cx).placeholder("Logs")),
            log_text: String::new(),
        }
    }

    fn read_alarms(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        if self.is_reading_alarms {
            return;
        }

        let flash_address = self.flash_address.read(cx).value().to_string();
        if !flash::is_valid_hex(&flash_address) {
            self.show_validation_error(
                "Flash address must start with \"0x\" and be valid hex",
                &self.flash_address.clone(),
                window,
                cx,
            );
            return;
        }

        let flash_size = self.flash_size.read(cx).value().to_string();
        if !flash::is_valid_hex(&flash_size) {
            self.show_validation_error(
                "Flash size must start with \"0x\" and be valid hex",
                &self.flash_size.clone(),
                window,
                cx,
            );
            return;
        }

        self.is_reading_alarms = true;
        self.log_text.clear();
        cx.notify();

        // The child runs on a background thread, so its output comes back over a
        // channel that this foreground task drains as it arrives.
        let (sender, mut receiver) = mpsc::unbounded();

        cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn(async move { flash::read(&flash_address, &flash_size, &sender) });

            while let Some(chunk) = receiver.next().await {
                this.update_in(cx, |this, window, cx| this.append_logs(&chunk, window, cx)).ok();
            }

            let result = read.await;

            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.append_logs(&format!("\n{error:#}\n"), window, cx);
                }

                this.is_reading_alarms = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn show_validation_error(&mut self, message: &str, input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        self.logs.update(cx, |logs, cx| {
            logs.set_value(message, window, cx);
        });
        input.focus_handle(cx).focus(window, cx);
    }

    /// Appends a chunk of child output to the log textarea.
    ///
    /// A carriage return is handled the way a terminal would, by rewinding to
    /// the start of the current line, so espflash's progress bar overwrites
    /// itself instead of stacking up.
    fn append_logs(&mut self, chunk: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut parts = chunk.split('\r');

        if let Some(first) = parts.next() {
            self.log_text.push_str(first);
        }

        for part in parts {
            let line_start = self.log_text.rfind('\n').map_or(0, |index| index.saturating_add(1));
            self.log_text.truncate(line_start);
            self.log_text.push_str(part);
        }

        let text = self.log_text.clone();
        self.logs.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            // Park the caret at the end so the view follows the newest output.
            let end = state.value().len();
            state.set_selected_range(end..end, cx);
        });
    }
}
