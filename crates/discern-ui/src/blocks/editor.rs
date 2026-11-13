use crate::{
    SharedWorkspace,
    settings::{SettingsChanged, SharedSettings, problems_banner},
};
use discern_workspace::{Session, TabSource, TabState};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable,
    button::{Button, ButtonVariants as _},
    dock::{BasePanel, Panel, PanelEvent},
    input::{Editor, EditorState, InputEvent, TabSize},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

const INITIAL_SQL: &str = "SELECT 1;\n";

struct Tab {
    source: TabSource,
    state: Entity<EditorState>,
    /// VS Code-style preview: shown in italics and replaced by the next file
    /// opened from the tree, until the user edits it or pins it.
    preview: bool,
    /// Edited since the last write to disk.
    needs_write: bool,
    _subscription: Subscription,
}

/// SQL editor block with tabs, built on gpui-kit's `Editor` (tree-sitter SQL
/// highlighting). Content lives in the workspace: script files, or auto-saved
/// drafts for consoles. The previous session's tabs are restored on startup.
pub struct EditorBlock {
    workspace: SharedWorkspace,
    settings: SharedSettings,
    _settings_subscription: Subscription,
    tabs: Vec<Tab>,
    active: usize,
    save_generation: u64,
}

impl EditorBlock {
    pub fn new(workspace: SharedWorkspace, settings: SharedSettings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe_in(&settings, window, |this, _, _: &SettingsChanged, window, cx| {
            this.apply_settings(window, cx);
        });
        let mut this = Self {
            workspace,
            settings,
            _settings_subscription: subscription,
            tabs: Vec::new(),
            active: 0,
            save_generation: 0,
        };

        let (saved, active) = {
            let ws = this.workspace.borrow();
            (ws.session().tabs.clone(), ws.session().active_tab.unwrap_or(0))
        };
        for tab in saved {
            let text = {
                let ws = this.workspace.borrow();
                match &tab.source {
                    TabSource::File(rel) => ws.read_script(rel).ok(),
                    TabSource::Draft(id) => ws.read_draft(id).ok().flatten(),
                    TabSource::Settings => this.settings.read(cx).read_text().ok(),
                }
            };
            if let Some(text) = text {
                this.push_tab(tab.source, &text, false, window, cx);
            }
        }
        if this.tabs.is_empty() {
            this.push_new_draft(window, cx);
        }
        this.active = active.min(this.tabs.len() - 1);
        this.persist(cx);
        this
    }

    fn push_tab(&mut self, source: TabSource, text: &str, preview: bool, window: &mut Window, cx: &mut Context<Self>) -> usize {
        let tab = self.make_tab(source, text, preview, window, cx);
        self.tabs.push(tab);
        self.tabs.len() - 1
    }

    fn make_tab(&mut self, source: TabSource, text: &str, preview: bool, window: &mut Window, cx: &mut Context<Self>) -> Tab {
        let text = text.to_owned();
        let settings = self.settings.read(cx).settings().clone();
        let language = if source == TabSource::Settings { "json" } else { "sql" };
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(language)
                .line_number(settings.line_numbers())
                .indent_guides(settings.indent_guides())
                .tab_size(TabSize { tab_size: settings.tab_size(), hard_tabs: false })
                .soft_wrap(settings.soft_wrap())
                .default_value(text)
                .placeholder("Write SQL here...")
        });
        let subscription = cx.subscribe_in(&state, window, |this, emitter, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                this.on_change(emitter.entity_id(), window, cx);
            }
        });
        Tab { source, state, preview, needs_write: false, _subscription: subscription }
    }

    fn push_new_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.workspace.borrow().new_draft_id();
        self.active = self.push_tab(TabSource::Draft(id), INITIAL_SQL, false, window, cx);
        // Write the draft file so the session never points at a missing one.
        self.tabs[self.active].needs_write = true;
    }

    /// Opens a script from the file tree.
    ///
    /// Single click (`permanent == false`) uses the preview tab: an untouched
    /// preview is replaced by the new file. Editing or double-clicking pins a
    /// tab, after which it is kept and a new preview tab is used.
    pub fn open_file(&mut self, rel: &str, permanent: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.tabs.iter().position(|t| t.source == TabSource::File(rel.to_owned())) {
            if permanent {
                self.tabs[ix].preview = false;
            }
            self.activate(ix, window, cx);
            return;
        }
        let text = match self.workspace.borrow().read_script(rel) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("discern: cannot open {rel}: {e}");
                return;
            }
        };
        let source = TabSource::File(rel.to_owned());
        match self.tabs.iter().position(|t| t.preview) {
            Some(ix) => {
                let tab = self.make_tab(source, &text, !permanent, window, cx);
                self.tabs[ix] = tab; // the old preview was never edited: nothing to save
                self.active = ix;
            }
            None => {
                self.active = self.push_tab(source, &text, !permanent, window, cx);
            }
        }
        self.focus_active(window, cx);
        self.persist(cx);
        cx.notify();
    }

    /// Opens `settings.json` in a regular tab.
    pub fn open_settings_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.tabs.iter().position(|t| t.source == TabSource::Settings) {
            self.activate(ix, window, cx);
            return;
        }
        match self.settings.read(cx).read_text() {
            Ok(text) => {
                self.active = self.push_tab(TabSource::Settings, &text, false, window, cx);
                self.focus_active(window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(e) => eprintln!("discern: cannot open settings.json: {e}"),
        }
    }

    /// Applies the settings that can change on an open editor. Tab size and
    /// indent guides are fixed at creation and apply to tabs opened afterwards.
    fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.settings.read(cx).settings().clone();
        for tab in &self.tabs {
            tab.state.update(cx, |state, cx| {
                state.set_soft_wrap(settings.soft_wrap(), window, cx);
                state.set_line_number(settings.line_numbers(), window, cx);
            });
        }
        cx.notify();
    }

    fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active = ix;
        self.focus_active(window, cx);
        self.persist(cx);
        cx.notify();
    }

    fn focus_active(&self, window: &mut Window, cx: &mut App) {
        self.tabs[self.active].state.focus_handle(cx).focus(window, cx);
    }

    fn close_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.persist(cx);
        let tab = self.tabs.remove(ix);
        if let TabSource::Draft(id) = &tab.source {
            let _ = self.workspace.borrow().delete_draft(id);
        }
        if self.tabs.is_empty() {
            self.push_new_draft(window, cx);
        } else if self.active > ix || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        self.persist(cx);
        cx.notify();
    }

    /// A script was deleted in the file tree: close the tabs showing it.
    pub fn file_deleted(&mut self, rel: &str, window: &mut Window, cx: &mut Context<Self>) {
        let prefix = format!("{rel}/");
        while let Some(ix) = self.tabs.iter().position(
            |t| matches!(&t.source, TabSource::File(p) if p == rel || p.starts_with(&prefix)),
        ) {
            // Skip the write-back: the file no longer exists.
            self.tabs[ix].needs_write = false;
            self.close_tab(ix, window, cx);
        }
    }

    /// A script or folder was renamed in the file tree: repoint its tabs.
    pub fn file_renamed(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        let prefix = format!("{from}/");
        for tab in &mut self.tabs {
            if let TabSource::File(p) = &mut tab.source {
                if p == from {
                    *p = to.to_owned();
                } else if let Some(rest) = p.strip_prefix(&prefix) {
                    *p = format!("{to}/{rest}");
                }
            }
        }
        self.persist(cx);
        cx.notify();
    }

    fn on_change(&mut self, state_id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.state.entity_id() == state_id) {
            tab.preview = false; // editing pins the tab
            tab.needs_write = true;
            cx.notify();
        }
        self.schedule_save(window, cx);
    }

    fn schedule_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_generation += 1;
        let generation = self.save_generation;
        let delay = Duration::from_millis(self.settings.read(cx).settings().auto_save_delay_ms());
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(delay).await;
            // Only the last edit of a burst writes.
            let _ = this.update_in(cx, |this, _, cx| {
                if this.save_generation == generation {
                    this.persist(cx);
                }
            });
        })
        .detach();
    }

    /// Writes edited buffers and the session. Failures are reported but never
    /// interrupt editing.
    fn persist(&mut self, cx: &mut Context<Self>) {
        let settings = self.settings.clone();
        let ws = self.workspace.clone();
        let mut ws = ws.borrow_mut();
        for tab in self.tabs.iter_mut().filter(|t| t.needs_write) {
            let text = tab.state.read(cx).value();
            let written = match &tab.source {
                TabSource::File(rel) => ws.write_script(rel, &text).map_err(|e| e.to_string()),
                TabSource::Draft(id) => ws.write_draft(id, &text).map_err(|e| e.to_string()),
                // Saved verbatim whatever it contains; problems are reported, never fatal.
                TabSource::Settings => settings.update(cx, |m, cx| m.write_text(&text, cx)),
            };
            match written {
                Ok(()) => tab.needs_write = false,
                Err(e) => eprintln!("discern: failed to save workspace: {e}"),
            }
        }
        let session = Session {
            tabs: self
                .tabs
                .iter()
                .map(|t| TabState {
                    source: t.source.clone(),
                    connection: None,
                    cursor: t.state.read(cx).cursor(),
                    scroll_line: 0,
                })
                .collect(),
            active_tab: Some(self.active),
            ..ws.session().clone()
        };
        if let Err(e) = ws.save_session(session) {
            eprintln!("discern: failed to save session: {e}");
        }
    }

    fn tab_label(source: &TabSource) -> String {
        match source {
            TabSource::File(rel) => rel.rsplit('/').next().unwrap_or(rel).to_owned(),
            TabSource::Draft(_) => "Console".to_owned(),
            TabSource::Settings => "settings.json".to_owned(),
        }
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tabs = self.tabs.iter().enumerate().map(|(ix, tab)| {
            let active = ix == self.active;
            let label = div()
                .px_3()
                .when(tab.preview, |d| d.italic())
                .child(Self::tab_label(&tab.source));
            div()
                .id(("editor-tab", ix))
                .flex()
                .items_center()
                .h_full()
                .border_r_1()
                .border_color(theme.border)
                .cursor_pointer()
                .when(active, |d| d.bg(theme.background))
                .when(!active, |d| d.text_color(theme.muted_foreground))
                .child(label)
                .child(
                    Button::new(("editor-tab-close", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_tab(ix, window, cx);
                        })),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    if event.click_count() >= 2 {
                        this.tabs[ix].preview = false; // double-click pins
                    }
                    this.activate(ix, window, cx);
                }))
        });
        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(32.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.muted)
            .overflow_x_hidden()
            .children(tabs)
    }
}

impl BasePanel for EditorBlock {
    fn panel_name(&self) -> &'static str {
        "discern.editor"
    }
}

impl Panel for EditorBlock {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("Editor".into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Editor"
    }

    // The editor draws its own surface; no extra padding around it.
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}

impl EventEmitter<PanelEvent> for EditorBlock {}

impl Focusable for EditorBlock {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.tabs[self.active].state.focus_handle(cx)
    }
}

impl Render for EditorBlock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = Editor::new(&self.tabs[self.active].state)
            .bordered(false)
            .p_0()
            .size_full()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(cx.theme().mono_font_size);
        let banner = (self.tabs[self.active].source == TabSource::Settings)
            .then(|| problems_banner(self.settings.read(cx).problems(), cx))
            .flatten();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_tab_bar(cx))
            .children(banner.map(|b| div().p_2().child(b)))
            .child(div().flex_1().min_h_0().child(editor))
    }
}
