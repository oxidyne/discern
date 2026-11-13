use discern_ui::MainPage;
use gpui_kit::assets::Assets;
use gpui_kit::component::TitleBar;
use gpui_kit::*;

fn main() {
    gpui_kit::application().with_assets(Assets).run(|cx| {
        gpui_kit::init(cx);

        let mut options = TitleBar::window_options();
        options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1440.), px(900.)),
            cx,
        )));

        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| MainPage::new(window, cx)))
            .expect("failed to open window");
        cx.activate(true);
    });
}
