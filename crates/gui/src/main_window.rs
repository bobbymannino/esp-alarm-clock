mod flash;
mod view;

use futures::{StreamExt as _, channel::mpsc};
use gpui_kit::{
    component::input::{InputState, TextareaState},
    *,
};

/// Maximum amount of child output retained by the UI.
const MAX_LOG_BYTES: usize = 256 * 1024;

pub struct MainWindow {
    /// Whether a flash read is currently in flight.
    is_reading_flash: bool,
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
            is_reading_flash: false,
            flash_address: cx.new(|cx| InputState::new(window, cx).default_value(flash::DEFAULT_ADDRESS)),
            flash_size: cx.new(|cx| InputState::new(window, cx).default_value(flash::DEFAULT_SIZE)),
            logs: cx.new(|cx| TextareaState::new(window, cx).placeholder("Logs")),
            log_text: String::new(),
        }
    }

    fn read_alarms(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        if self.is_reading_flash {
            return;
        }

        let flash_address = self.flash_address.read(cx).value().to_string();
        let Some(flash_address) = flash::parse_hex(&flash_address) else {
            self.show_validation_error(
                "Flash address must start with \"0x\" and fit in 32-bit hex",
                &self.flash_address.clone(),
                window,
                cx,
            );
            return;
        };

        let flash_size = self.flash_size.read(cx).value().to_string();
        let Some(flash_size) = flash::parse_hex(&flash_size) else {
            self.show_validation_error(
                "Flash size must start with \"0x\" and fit in 32-bit hex",
                &self.flash_size.clone(),
                window,
                cx,
            );
            return;
        };

        let flash_read = flash::FlashRead::new(flash_address, flash_size);

        self.is_reading_flash = true;
        self.clear_logs(window, cx);
        cx.notify();

        // The child runs on a background thread, so its output comes back over a
        // channel that this foreground task drains as it arrives.
        let (sender, mut receiver) = mpsc::unbounded();

        cx.spawn(async move |this, cx| {
            let read = cx.background_executor().spawn(async move { flash::read(flash_read, &sender) });

            while let Some(chunk) = receiver.next().await {
                this.update_in(cx, |this, window, cx| this.append_logs(&chunk, window, cx)).ok();
            }

            let result = read.await;

            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.append_logs(&format!("\n{error:#}\n"), window, cx);
                }

                this.is_reading_flash = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn show_validation_error(&mut self, message: &str, input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        self.set_logs(message.to_owned(), window, cx);
        input.focus_handle(cx).focus(window, cx);
    }

    fn set_logs(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.log_text = text;
        trim_log(&mut self.log_text);
        self.sync_log_textarea(window, cx);
    }

    fn clear_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_logs(String::new(), window, cx);
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

        trim_log(&mut self.log_text);
        self.sync_log_textarea(window, cx);
    }

    fn sync_log_textarea(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.log_text.clone();
        self.logs.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            // Park the caret at the end so the view follows the newest output.
            let end = state.value().len();
            state.set_selected_range(end..end, cx);
        });
    }
}

fn trim_log(log: &mut String) {
    if log.len() <= MAX_LOG_BYTES {
        return;
    }

    let mut start = log.len().saturating_sub(MAX_LOG_BYTES);
    while !log.is_char_boundary(start) {
        start = start.saturating_add(1);
    }
    log.drain(..start);
}

#[cfg(test)]
mod tests {
    use super::{MAX_LOG_BYTES, trim_log};

    #[test]
    fn trim_log_retains_newest_output() {
        let mut log = format!("old{}", "n".repeat(MAX_LOG_BYTES));

        trim_log(&mut log);

        assert_eq!(log.len(), MAX_LOG_BYTES);
        assert!(!log.starts_with("old"));
    }

    #[test]
    fn trim_log_preserves_utf8_boundaries() {
        let mut log = format!("😀{}", "n".repeat(MAX_LOG_BYTES.saturating_sub(1)));

        trim_log(&mut log);

        assert!(log.len() <= MAX_LOG_BYTES);
        assert!(log.chars().all(|character| character == 'n'));
    }
}
