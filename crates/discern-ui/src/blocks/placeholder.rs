use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent};
use gpui_kit::*;

/// Empty block showing its title. Stand-in until the real block exists.
pub struct Placeholder {
    name: &'static str,
    title: SharedString,
    focus_handle: FocusHandle,
}

impl Placeholder {
    pub fn new(name: &'static str, title: impl Into<SharedString>, cx: &mut App) -> Self {
        Self { name, title: title.into(), focus_handle: cx.focus_handle() }
    }
}

impl BasePanel for Placeholder {
    fn panel_name(&self) -> &'static str {
        self.name
    }
}

impl Panel for Placeholder {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.title.clone())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.title.clone()
    }
}

impl EventEmitter<PanelEvent> for Placeholder {}

impl Focusable for Placeholder {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Placeholder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().items_center().justify_center().child(self.title.clone())
    }
}
