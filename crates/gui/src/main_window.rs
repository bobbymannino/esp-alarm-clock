use std::{
    process::{Command, Stdio},
    thread,
};

use anyhow::{Result, bail};
use futures::{StreamExt as _, channel::mpsc};
use gpui_kit::{
    assets::IconName,
    base::Disableable,
    component::{
        ActiveTheme as _,
        button::{Button, ButtonVariants},
        input::{Textarea, TextareaState},
        label::Label,
        scroll::ScrollableElement,
    },
    *,
};

/// How many bytes are read from the child's pipes at a time.
const CHUNK_SIZE: usize = 1024;

pub struct MainWindow {
    /// Whether a flash read is currently in flight.
    reading_alarms: bool,
    /// The log textarea, kept across renders so its contents survive a repaint.
    logs: Entity<TextareaState>,
    /// Everything the child process has written so far.
    log_text: String,
}

impl MainWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            reading_alarms: false,
            logs: cx.new(|cx| TextareaState::new(window, cx).placeholder("Logs")),
            log_text: String::new(),
        }
    }

    fn read_alarms(&mut self, cx: &mut Context<Self>) {
        if self.reading_alarms {
            return;
        }

        self.reading_alarms = true;
        self.log_text.clear();
        cx.notify();

        // The child runs on a background thread, so its output comes back over a
        // channel that this foreground task drains as it arrives.
        let (sender, mut receiver) = mpsc::unbounded();

        cx.spawn(async move |this, cx| {
            let read = cx.background_executor().spawn(async move { read_flash(&sender) });

            while let Some(chunk) = receiver.next().await {
                this.update_in(cx, |this, window, cx| this.append_logs(&chunk, window, cx)).ok();
            }

            let result = read.await;

            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.append_logs(&format!("\n{error:#}\n"), window, cx);
                }

                this.reading_alarms = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
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

impl Render for MainWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .p_5()
            .gap_5()
            .child(Label::new("ESP Alarm Clock").font_weight(FontWeight::BOLD).text_3xl())
            .child(
                Textarea::new(&self.logs)
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
                    )
                    .child(
                        Button::new("clear_logs")
                            .label("Clear Logs")
                            .disabled(self.reading_alarms)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.logs.update(cx, |state, cx| {
                                    state.set_value(String::new(), window, cx);
                                });
                            })),
                    ),
            )
    }
}

/// Runs `espflash read-flash` for the NVS partition, forwarding everything it
/// writes to `sender` as it is produced.
///
/// Blocking, so this must not be called on the main thread.
fn read_flash(sender: &mpsc::UnboundedSender<String>) -> Result<()> {
    let nvs_path = std::env::temp_dir().join("nvs.bin");

    let mut child = Command::new("espflash")
        .args(["read-flash", "0x9000", "0x6000"])
        .arg(nvs_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        bail!("espflash stdout/stderr were not piped");
    };

    // Both pipes need their own reader, a pipe nobody drains fills its buffer
    // and blocks the child. espflash draws its progress bar on stderr.
    let stderr_sender = sender.clone();
    let stderr_reader = thread::spawn(move || forward(stderr, &stderr_sender));
    forward(stdout, sender);
    stderr_reader.join().ok();

    let status = child.wait()?;
    if !status.success() {
        bail!("espflash exited with {status}");
    }

    Ok(())
}

/// Forwards everything `reader` produces to `sender`, a chunk at a time.
fn forward(mut reader: impl std::io::Read, sender: &mpsc::UnboundedSender<String>) {
    let mut buf = [0u8; CHUNK_SIZE];

    while let Ok(read) = reader.read(&mut buf) {
        if read == 0 {
            break;
        }

        let chunk = String::from_utf8_lossy(buf.get(..read).unwrap_or_default()).into_owned();
        if sender.unbounded_send(chunk).is_err() {
            break;
        }
    }
}
