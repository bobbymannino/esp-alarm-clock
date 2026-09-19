mod flash;
mod logs;
mod view;

use futures::{StreamExt as _, channel::mpsc};
use gpui_kit::{component::input::InputState, *};
use logs::Logs;

pub struct MainWindow {
    /// Whether a flash read is currently in flight.
    is_reading_flash: bool,
    /// The flash address passed to `espflash read-flash`.
    flash_address: Entity<InputState>,
    /// The number of bytes passed to `espflash read-flash`.
    flash_size: Entity<InputState>,
    /// Child-process output displayed in the log textarea.
    logs: Logs,
}

impl MainWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            is_reading_flash: false,
            flash_address: cx.new(|cx| InputState::new(window, cx).default_value(flash::DEFAULT_ADDRESS)),
            flash_size: cx.new(|cx| InputState::new(window, cx).default_value(flash::DEFAULT_SIZE)),
            logs: Logs::new(window, cx),
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
                this.update_in(cx, |this, window, cx| this.logs.append(&chunk, window, cx)).ok();
            }

            let result = read.await;

            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.logs.append(&format!("\n{error:#}\n"), window, cx);
                }

                this.is_reading_flash = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn show_validation_error(&mut self, message: &str, input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        self.logs.set(message.to_owned(), window, cx);
        input.focus_handle(cx).focus(window, cx);
    }

    fn clear_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.logs.clear(window, cx);
    }
}
