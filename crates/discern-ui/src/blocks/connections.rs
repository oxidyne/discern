use crate::SharedWorkspace;
use discern_workspace::SavedConnection;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dock::{BasePanel, Panel, PanelEvent},
    input::{Input, InputState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{cell::RefCell, rc::Rc};

const DRIVERS: &[&str] = &["postgres", "mysql", "sqlite"];

/// Saved data sources of the workspace: list, add, edit, delete.
/// Passwords are never stored in the workspace.
pub struct ConnectionsBlock {
    workspace: SharedWorkspace,
    focus_handle: FocusHandle,
}

impl ConnectionsBlock {
    pub fn new(workspace: SharedWorkspace, cx: &mut Context<Self>) -> Self {
        Self { workspace, focus_handle: cx.focus_handle() }
    }

    fn edit(&mut self, existing: Option<SavedConnection>, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        let field = |window: &mut Window, cx: &mut App, placeholder: &'static str, value: String| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
        };
        let name = field(window, cx, "Name", existing.as_ref().map(|c| c.name.clone()).unwrap_or_default());
        let url = field(window, cx, "URL, e.g. postgres://localhost:5432/app", existing.as_ref().map(|c| c.url.clone()).unwrap_or_default());
        let user = field(window, cx, "User (optional)", existing.as_ref().and_then(|c| c.user.clone()).unwrap_or_default());
        let driver = Rc::new(RefCell::new(
            existing.as_ref().map_or_else(|| DRIVERS[0].to_owned(), |c| c.driver.clone()),
        ));
        let error: Rc<RefCell<Option<String>>> = Rc::default();
        let focus_once = std::cell::Cell::new(true);
        let title = if existing.is_some() { "Edit connection" } else { "New connection" };

        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                name.focus_handle(cx).focus(window, cx);
            }
            let current_driver = driver.borrow().clone();
            let drivers = div().flex().gap_1().children(DRIVERS.iter().enumerate().map(|(i, d)| {
                let selected = *d == current_driver;
                let driver = driver.clone();
                Button::new(("driver", i))
                    .small()
                    .label(*d)
                    .when(selected, |b| b.primary())
                    .when(!selected, |b| b.ghost())
                    .on_click(move |_, window, _| {
                        *driver.borrow_mut() = (*d).to_owned();
                        window.refresh();
                    })
            }));
            let err = error.borrow().clone();
            let (name_ok, url_ok, user_ok, driver_ok, error_ok) =
                (name.clone(), url.clone(), user.clone(), driver.clone(), error.clone());
            let (this, existing) = (this.clone(), existing.clone());
            dialog
                .title(title)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(Input::new(&name))
                        .child(drivers)
                        .child(Input::new(&url))
                        .child(Input::new(&user))
                        .when_some(err, |d, e| d.child(div().text_sm().text_color(cx.theme().danger).child(e))),
                )
                .on_ok(move |_, _, cx| {
                    let (n, u, usr) = (
                        name_ok.read(cx).value().trim().to_owned(),
                        url_ok.read(cx).value().trim().to_owned(),
                        user_ok.read(cx).value().trim().to_owned(),
                    );
                    if n.is_empty() || u.is_empty() {
                        *error_ok.borrow_mut() = Some("Name and URL are required.".into());
                        return false;
                    }
                    let mut conn = existing.clone().unwrap_or_else(|| SavedConnection::new("", "", ""));
                    conn.name = n;
                    conn.url = u;
                    conn.driver = driver_ok.borrow().clone();
                    conn.user = (!usr.is_empty()).then_some(usr);
                    let saved = this.update(cx, |this, cx| {
                        let r = this.workspace.borrow_mut().upsert_connection(conn);
                        cx.notify();
                        r
                    });
                    match saved {
                        Ok(Ok(())) => true,
                        Ok(Err(e)) => { *error_ok.borrow_mut() = Some(e.to_string()); false }
                        Err(_) => true, // block closed meanwhile
                    }
                })
        });
    }

    fn delete(&mut self, conn: SavedConnection, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (this, id) = (this.clone(), conn.id.clone());
            alert
                .title(format!("Delete connection {}?", conn.name))
                .description("The saved connection will be removed from this workspace.")
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if let Err(e) = this.workspace.borrow_mut().remove_connection(&id) {
                            eprintln!("discern: cannot delete connection: {e}");
                        }
                        cx.notify();
                    });
                    true
                })
        });
    }
}

impl BasePanel for ConnectionsBlock {
    fn panel_name(&self) -> &'static str {
        "discern.connections"
    }
}

impl Panel for ConnectionsBlock {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("Connections".into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Connections"
    }
}

impl EventEmitter<PanelEvent> for ConnectionsBlock {}

impl Focusable for ConnectionsBlock {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConnectionsBlock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connections = self.workspace.borrow().connections().to_vec();
        let (muted, border) = (cx.theme().muted_foreground, cx.theme().border);

        let toolbar = div().flex().items_center().p_1().child(
            Button::new("connection-new").ghost().xsmall().icon(IconName::Plus).label("New connection").on_click(
                cx.listener(|this, _, window, cx| this.edit(None, window, cx)),
            ),
        );

        let rows = connections.into_iter().enumerate().map(|(ix, conn)| {
            let (edit_conn, delete_conn) = (conn.clone(), conn.clone());
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_2()
                .py_1()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().child(conn.name.clone()))
                        .child(div().truncate().text_xs().text_color(muted).child(format!("{} · {}", conn.driver, conn.url))),
                )
                .child(
                    div()
                        .flex()
                        .child(Button::new(("connection-edit", ix)).ghost().xsmall().label("Edit").on_click(
                            cx.listener(move |this, _, window, cx| this.edit(Some(edit_conn.clone()), window, cx)),
                        ))
                        .child(Button::new(("connection-delete", ix)).ghost().xsmall().label("Delete").on_click(
                            cx.listener(move |this, _, window, cx| this.delete(delete_conn.clone(), window, cx)),
                        )),
                )
        });

        let empty = self.workspace.borrow().connections().is_empty();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(toolbar)
            .when(empty, |d| d.child(div().p_3().text_sm().text_color(muted).child("No connections yet.")))
            .child(div().id("connections-list").flex_1().min_h_0().overflow_y_scroll().children(rows))
    }
}
