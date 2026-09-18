use gpui_kit::{
    component::{
        ActiveTheme,
        button::Button,
        input::{Textarea, TextareaState},
        label::Label,
        scroll::ScrollableElement,
    },
    *,
};

pub struct MainWindow;

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
                div().flex().child(
                    Button::new("close")
                        .outline()
                        .label("Close")
                        .on_click(|_, window, _| window.remove_window()),
                ),
            )
    }
}
