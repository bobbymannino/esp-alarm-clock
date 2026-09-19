use gpui_kit::{
    assets::IconName,
    base::Disableable,
    component::{
        ActiveTheme as _,
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
        label::Label,
        scroll::ScrollableElement,
        tooltip::Tooltip,
    },
    *,
};

use super::MainWindow;

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
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .id("flash-address-input")
                            .tooltip(|window, cx| Tooltip::new("Start address to read, in hexadecimal").build(window, cx))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .w_48()
                            .child(Label::new("Flash address"))
                            .child(Input::new(&self.flash_address).disabled(self.is_reading_flash)),
                    )
                    .child(
                        div()
                            .id("flash-size-input")
                            .tooltip(|window, cx| Tooltip::new("Number of bytes to read, in hexadecimal").build(window, cx))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .w_48()
                            .child(Label::new("Flash size"))
                            .child(Input::new(&self.flash_size).disabled(self.is_reading_flash)),
                    ),
            )
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
                            .loading(self.is_reading_flash)
                            .on_click(cx.listener(|this, _, window, cx| this.read_alarms(cx, window))),
                    )
                    .child(
                        Button::new("clear_logs")
                            .label("Clear Logs")
                            .disabled(self.is_reading_flash)
                            .on_click(cx.listener(|this, _, window, cx| this.clear_logs(window, cx))),
                    ),
            )
    }
}
