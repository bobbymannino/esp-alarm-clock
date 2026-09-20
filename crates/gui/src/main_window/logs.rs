use gpui_kit::{component::input::TextareaState, *};

/// Maximum amount of child output retained by the UI.
const MAX_BYTES: usize = 256 * 1024;

/// Owns the displayed log text and its textarea state.
pub(super) struct Logs {
    textarea: Entity<TextareaState>,
    text: String,
}

impl Logs {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            textarea: cx.new(|cx| TextareaState::new(window, cx).placeholder("Logs")),
            text: String::new(),
        }
    }

    pub(super) const fn textarea(&self) -> &Entity<TextareaState> {
        &self.textarea
    }

    pub(super) fn set(&mut self, text: String, window: &mut Window, cx: &mut App) {
        self.text = text;
        trim(&mut self.text);
        self.sync_textarea(window, cx);
    }

    pub(super) fn clear(&mut self, window: &mut Window, cx: &mut App) {
        self.set(String::new(), window, cx);
    }

    /// Appends a chunk of child output to the log textarea.
    ///
    /// A carriage return is handled the way a terminal would, by rewinding to
    /// the start of the current line, so espflash's progress bar overwrites
    /// itself instead of stacking up.
    pub(super) fn append(&mut self, chunk: &str, window: &mut Window, cx: &mut App) {
        let mut parts = chunk.split('\r');

        if let Some(first) = parts.next() {
            self.text.push_str(first);
        }

        for part in parts {
            let line_start = self.text.rfind('\n').map_or(0, |index| index.saturating_add(1));
            self.text.truncate(line_start);
            self.text.push_str(part);
        }

        trim(&mut self.text);
        self.sync_textarea(window, cx);
    }

    fn sync_textarea(&mut self, window: &mut Window, cx: &mut App) {
        let text = self.text.clone();
        self.textarea.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            // Park the caret at the end so the view follows the newest output.
            let end = state.value().len();
            state.set_selected_range(end..end, cx);
        });
    }
}

fn trim(log: &mut String) {
    if log.len() <= MAX_BYTES {
        return;
    }

    let mut start = log.len().saturating_sub(MAX_BYTES);
    while !log.is_char_boundary(start) {
        start = start.saturating_add(1);
    }
    log.drain(..start);
}

#[cfg(test)]
mod tests {
    use super::{MAX_BYTES, trim};

    #[test]
    fn trim_retains_newest_output() {
        let mut log = format!("old{}", "n".repeat(MAX_BYTES));

        trim(&mut log);

        assert_eq!(log.len(), MAX_BYTES);
        assert!(!log.starts_with("old"));
    }

    #[test]
    fn trim_preserves_utf8_boundaries() {
        let mut log = format!("😀{}", "n".repeat(MAX_BYTES.saturating_sub(1)));

        trim(&mut log);

        assert!(log.len() <= MAX_BYTES);
        assert!(log.chars().all(|character| character == 'n'));
    }
}
