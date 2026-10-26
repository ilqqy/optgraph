use gpui::{App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size};

struct Hello;

impl Render for Hello {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x0a0c10))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_5()
                    .w(px(420.))
                    .rounded(px(13.))
                    .border_1()
                    .border_color(rgb(0x3ddc97))
                    .bg(rgb(0x151922))
                    .text_color(rgb(0xe8ebf1))
                    .child(div().text_xs().text_color(rgb(0x8992a5)).child("OPTGRAPH"))
                    .child(div().text_xl().child("Who set what, and why it won."))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .px_2()
                                    .rounded(px(6.))
                                    .bg(rgb(0x3ddc97))
                                    .text_color(rgb(0x03140b))
                                    .child("50 mkForce"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(rgb(0xffa45e))
                                    .text_color(rgb(0xffa45e))
                                    .child("1000 mkDefault"),
                            ),
                    ),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.), px(500.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Hello),
        )
        .unwrap();
        cx.activate(true);
    });
}
