use gpui_kit::{
    component::{button::*, label::Label, *},
    *,
};

pub struct MainWindow;

impl Render for MainWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Label::new("ESP Alarm Clock"))
    }
}
