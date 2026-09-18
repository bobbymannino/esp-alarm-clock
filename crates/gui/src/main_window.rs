use anyhow::{Result, bail};
use gpui_kit::{
    assets::IconName,
    base::Disableable,
    component::{
        ActiveTheme as _, ActiveTheme, Icon,
        button::{Button, ButtonVariants},
        input::{Textarea, TextareaState},
        label::Label,
        scroll::ScrollableElement,
        spinner::Spinner,
    },
    *,
};

#[derive(Default)]
pub struct MainWindow {
    /// Whether a flash read is currently in flight.
    reading_alarms: bool,
}

impl MainWindow {
    fn read_alarms(&mut self, cx: &mut Context<Self>) {
        if self.reading_alarms {
            return;
        }

        self.reading_alarms = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async { read_flash() }).await;

            if let Err(error) = result {
                println!("Failed to read flash: {error:?}");
            }

            this.update(cx, |this, cx| {
                this.reading_alarms = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let logs = cx.new(|cx| TextareaState::new(window, cx).placeholder("Logs"));
        let theme = cx.theme();

        div()
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .p_5()
            .gap_5()
            .child(Label::new("ESP Alarm Clock").font_weight(FontWeight::BOLD).text_3xl())
            .child(
                Textarea::new(&logs)
                    .h_96()
                    .border_2()
                    .border_color(theme.border)
                    .rounded_lg()
                    .readonly(true),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(Button::new("close").label("Close").on_click(|_, window, _| window.remove_window()))
                    .child(
                        Button::new("read_alarms")
                            .primary()
                            .label("Read Alarms")
                            .icon(IconName::Eye)
                            .loading(self.reading_alarms)
                            .on_click(cx.listener(|this, _, _, cx| this.read_alarms(cx))),
                    ),
            )
    }
}

/// Blocking `espflash read-flash` of the NVS partition into a temp file.
fn read_flash() -> Result<()> {
    let nvs_path = std::env::temp_dir().join("nvs.bin");

    let status = std::process::Command::new("espflash")
        .args(["read-flash", "0x9000", "0x6000"])
        .arg(nvs_path)
        .spawn()?
        .wait()?;

    if !status.success() {
        bail!("espflash exited with {status}");
    }

    Ok(())
}
