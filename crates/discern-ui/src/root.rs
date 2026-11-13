use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, rgb};

/// Top-level view; will host sidebar, editor tabs and result grid.
#[derive(Debug)]
pub struct Root;

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x1e1e2e)).text_color(rgb(0xcdd6f4)).child("discern")
    }
}
