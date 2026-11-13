//! Dock appearance: gpui-kit's `DockSkin` with rounded, floating panels.
//!
//! Everything is delegated to `DockSkin`; only the dock and center frames are
//! wrapped (margin + rounded border + clip).

use gpui_kit::component::{
    ActiveTheme as _,
    dock::{
        BasePanelView as PanelView, DockAreaRenderer, DockContext, DockSkin, NodeId, PanelState,
        TabGroupRenderer,
    },
};
use gpui_kit::base::ResizeHandleContext;
use gpui_kit::*;
use std::{rc::Rc, sync::Arc};

/// Gap between floating panels.
const GAP: Pixels = px(4.);

pub struct RoundedSkin {
    inner: Rc<DockSkin>,
}

impl RoundedSkin {
    pub fn new(inner: Rc<DockSkin>) -> Rc<Self> {
        Rc::new(Self { inner })
    }
}

impl DockAreaRenderer for RoundedSkin {
    fn frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        // Panels float on the window background.
        self.inner.frame(window, cx).bg(cx.theme().background).p(GAP / 2.)
    }

    fn center_frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner
            .center_frame(window, cx)
            .m(GAP / 2.)
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
    }

    fn split_frame(&self, node: NodeId, axis: Axis, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner.split_frame(node, axis, window, cx)
    }

    fn render_split_handle(
        &self,
        handle: &ResizeHandleContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.inner.render_split_handle(handle, window, cx)
    }

    fn render_dock(
        &self,
        dock: &DockContext,
        content: AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let radius = cx.theme().radius_lg;
        let border = cx.theme().border;
        let inner = self.inner.render_dock(dock, content, window, cx);
        div()
            .size_full()
            .p(GAP / 2.)
            .child(
                div()
                    .size_full()
                    .rounded(radius)
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    .child(inner),
            )
            .into_any_element()
    }

    fn build_placeholder(
        &self,
        state: &PanelState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<dyn PanelView>> {
        self.inner.build_placeholder(state, window, cx)
    }

    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        self.inner.tab_group_renderer()
    }
}
