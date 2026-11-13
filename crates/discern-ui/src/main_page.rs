use crate::{
    SharedWorkspace, blocks, layout,
    settings::{SettingsChanged, SettingsModel, SharedSettings, open_settings_dialog},
    skin::RoundedSkin,
};
use discern_workspace::{RecentWorkspaces, WorkspaceStore};
use gpui_kit::component::{
    IconName, Sizable, TitleBar, WindowExt as _,
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    button::{Button, ButtonVariants as _},
    dock::{BasePanelView, DockArea, DockEvent, panel_handle, DockPlacement, DockSkin, PanelStyle},
    Theme, ThemeMode,
    status_bar::StatusBar,
};
use gpui_kit::*;

actions!(discern, [OpenSettings]);

/// Registers key bindings. Call once after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    let key = if cfg!(target_os = "macos") { "cmd-," } else { "ctrl-," };
    cx.bind_keys([KeyBinding::new(key, OpenSettings, None)]);
}

/// Applies the `appearance.theme` setting.
fn apply_theme(settings: &SharedSettings, window: &mut Window, cx: &mut App) {
    match settings.read(cx).settings().theme() {
        "light" => Theme::change(ThemeMode::Light, Some(window), cx),
        "dark" => Theme::change(ThemeMode::Dark, Some(window), cx),
        _ => Theme::sync_system_appearance(Some(window), cx),
    }
}

const DOCK_ID: &str = "discern-main-dock";
/// Bump when the default layout changes (invalidates saved layouts).
const DOCK_VERSION: usize = 1;

/// Main window content: title bar, dock area (explorer | editor / results), status bar.
pub struct MainPage {
    workspace: SharedWorkspace,
    settings: SharedSettings,
    editor: Entity<blocks::EditorBlock>,
    layout_generation: u64,
    _layout_changed: Subscription,
    _open_file: Subscription,
    _settings_changed: Subscription,
    dock_area: Entity<DockArea>,
    _skin: std::rc::Rc<DockSkin>,
}

impl MainPage {
    pub fn new(workspace: SharedWorkspace, settings: SharedSettings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut skin = None;
        let dock_area = cx.new(|cx| {
            let inner = DockSkin::new(cx);
            skin = Some(inner.clone());
            DockArea::new(DOCK_ID, Some(DOCK_VERSION), window, cx)
                .with_renderer(RoundedSkin::new(inner))
        });
        let skin = skin.expect("DockSkin::new ran inside the constructor");
        // Always show tabs so every panel has a drag handle.
        skin.set_panel_style(PanelStyle::TabBar, cx);

        // Follow the OS light/dark appearance, now and when it changes.
        apply_theme(&settings, window, cx);
        window
            .observe_window_appearance({
                let settings = settings.clone();
                move |window, cx| {
                    if settings.read(cx).settings().theme() == "system" {
                        Theme::sync_system_appearance(Some(window), cx);
                    }
                }
            })
            .detach();
        let settings_changed = cx.subscribe_in(&settings, window, |this, _, _: &SettingsChanged, window, cx| {
            apply_theme(&this.settings, window, cx);
        });

        let editor = cx.new(|cx| blocks::EditorBlock::new(workspace.clone(), settings.clone(), window, cx));
        let files = cx.new(|cx| blocks::FilesBlock::new(workspace.clone(), cx));
        // Opening a script in the file tree loads it into the editor.
        let open_file = cx.subscribe_in(&files, window, {
            let editor = editor.clone();
            move |_, _, event: &blocks::FilesEvent, window, cx| {
                editor.update(cx, |editor, cx| match event {
                    blocks::FilesEvent::Open { rel, permanent } => editor.open_file(rel, *permanent, window, cx),
                    blocks::FilesEvent::Deleted(rel) => editor.file_deleted(rel, window, cx),
                    blocks::FilesEvent::Renamed { from, to } => editor.file_renamed(from, to, cx),
                });
            }
        });

        let connections = cx.new(|cx| blocks::ConnectionsBlock::new(workspace.clone(), cx));
        let panels: std::collections::HashMap<&'static str, std::sync::Arc<dyn BasePanelView>> = [
            (layout::EDITOR, panel_handle(editor.clone())),
            (layout::EXPLORER, panel_handle(files)),
            (layout::CONNECTIONS, panel_handle(connections)),
            (layout::RESULTS, blocks::results(cx)),
            (layout::HISTORY, blocks::history(cx)),
            (layout::INSPECTOR, blocks::inspector(cx)),
        ]
        .into_iter()
        .collect();
        let saved = settings
            .read(cx)
            .settings()
            .restore_layout()
            .then(|| workspace.borrow().session().layout.as_ref().and_then(layout::SavedLayout::from_json))
            .flatten();
        let initial = layout::normalize(saved.unwrap_or_else(layout::SavedLayout::default_layout));
        dock_area.update(cx, |area, cx| layout::apply(area, &initial, &panels, window, cx));
        // Subscribed after the layout is installed so startup edits are not saved back.
        let layout_changed = cx.subscribe_in(&dock_area, window, |this, _, event: &DockEvent, window, cx| {
            if matches!(event, DockEvent::LayoutChanged) {
                this.schedule_layout_save(window, cx);
            }
        });

        Self {
            layout_generation: 0,
            _layout_changed: layout_changed,
            settings,
            editor,
            _settings_changed: settings_changed,
            _open_file: open_file,
            workspace,
            dock_area,
            _skin: skin,
        }
    }

    /// Writes the current arrangement to the session, unless the user turned
    /// `workbench.restoreLayout` off.
    fn schedule_layout_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.layout_generation += 1;
        let generation = self.layout_generation;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            let _ = this.update(cx, |this, cx| {
                if this.layout_generation == generation && this.settings.read(cx).settings().restore_layout() {
                    let layout = crate::layout::capture(this.dock_area.read(cx), cx);
                    let mut ws = this.workspace.borrow_mut();
                    let session = discern_workspace::Session { layout: Some(layout.to_json()), ..ws.session().clone() };
                    if let Err(e) = ws.save_session(session) {
                        eprintln!("discern: failed to save layout: {e}");
                    }
                }
            });
        })
        .detach();
    }

    /// Title-bar dropdown: current workspace, every recent one, and "New workspace…".
    fn workspace_menu(&self) -> impl IntoElement {
        let (name, current) = {
            let ws = self.workspace.borrow();
            (ws.name().to_owned(), ws.root().to_owned())
        };
        let current_for_menu = current.clone();
        Button::new("workspace-menu")
            .ghost()
            .xsmall()
            .label(name)
            .icon(IconName::ChevronDown)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.min_w(px(220.));
                let mut entries = RecentWorkspaces::default_location()
                    .and_then(|r| r.list().ok())
                    .unwrap_or_default();
                if !entries.iter().any(|e| e.path == current_for_menu) {
                    entries.insert(0, discern_workspace::RecentEntry {
                        path: current_for_menu.clone(),
                        name: current_for_menu.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                        last_opened: 0,
                    });
                }
                for entry in entries {
                    let is_current = entry.path == current_for_menu;
                    let path = entry.path.clone();
                    let name = entry.name.clone();
                    menu = menu.item(
                        PopupMenuItem::new(entry.name.clone())
                            .checked(is_current)
                            .on_click(move |_, window, cx| {
                                if is_current {
                                    return;
                                }
                                if path.as_os_str().is_empty() {
                                    // Known workspace without a path: force the location prompt.
                                    open_create_dialog(Some(name.clone()), window, cx);
                                } else {
                                    switch_workspace(&path, window, cx);
                                }
                            }),
                    );
                }
                menu.separator().item(
                    PopupMenuItem::new("New workspace…")
                        .icon(IconName::Plus)
                        .on_click(|_, window, cx| open_create_dialog(None, window, cx)),
                )
            })
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let open_json: crate::settings::OpenJson = std::rc::Rc::new(move |window, cx| {
            editor.update(cx, |editor, cx| editor.open_settings_file(window, cx));
        });
        open_settings_dialog(self.settings.clone(), open_json, window, cx);
    }

    fn toggle_button(
        &self,
        id: &'static str,
        icon: IconName,
        tooltip: &'static str,
        placement: DockPlacement,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .ghost()
            .xsmall()
            .icon(icon)
            .tooltip(tooltip)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.dock_area
                    .update(cx, |area, cx| area.toggle_dock(placement, window, cx));
            }))
    }
}

impl Render for MainPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left = self.toggle_button(
            "toggle-left",
            IconName::PanelLeft,
            "Toggle Explorer",
            DockPlacement::Left,
            cx,
        );
        let bottom = self.toggle_button(
            "toggle-bottom",
            IconName::PanelBottom,
            "Toggle Results",
            DockPlacement::Bottom,
            cx,
        );

        let right = self.toggle_button(
            "toggle-right",
            IconName::PanelRight,
            "Toggle Inspector",
            DockPlacement::Right,
            cx,
        );
        let settings_button = Button::new("open-settings")
            .ghost()
            .xsmall()
            .icon(IconName::Settings)
            .tooltip("Settings")
            .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx)));

        div()
            .size_full()
            .flex()
            .flex_col()
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)))
            .child(TitleBar::new().child(self.workspace_menu()))
            .child(div().flex_1().min_h_0().child(self.dock_area.clone()))
            .child(StatusBar::new().left(left).left(bottom).left(right).right(settings_button))
    }
}

/// Opens the main window for `workspace`.
pub fn open_main_window(workspace: SharedWorkspace, cx: &mut App) {
    let settings = SettingsModel::load(cx);
    let mut options = TitleBar::window_options();
    options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx)));
    if let Err(e) = gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| MainPage::new(workspace, settings, window, cx))
    }) {
        eprintln!("discern: failed to open window: {e}");
    }
}

/// Opens `root` in a new window and closes the current one.
fn switch_workspace(root: &std::path::Path, window: &mut Window, cx: &mut App) {
    match WorkspaceStore::open(root) {
        Ok(store) => {
            if let Some(recent) = RecentWorkspaces::default_location() {
                let _ = recent.record(root, store.name());
            }
            open_main_window(std::rc::Rc::new(std::cell::RefCell::new(store)), cx);
            window.remove_window();
        }
        Err(e) => {
            let message = e.to_string();
            window.open_alert_dialog(cx, move |alert, _, _| {
                alert.title("Cannot open workspace").description(message.clone())
            });
        }
    }
}

/// Name + location prompt. A per-platform default location is proposed and can
/// be changed with Browse; the chosen location is always shown before creating.
/// `name` pre-fills the field, e.g. for a known workspace that has no path yet.
fn open_create_dialog(name: Option<String>, window: &mut Window, cx: &mut App) {
    use std::{cell::RefCell, path::PathBuf, rc::Rc};

    let input = cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Workspace name");
        if let Some(name) = &name {
            state = state.default_value(name.clone());
        }
        state
    });
    let location: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(discern_workspace::default_workspaces_dir()));
    let focus_once = std::cell::Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if focus_once.replace(false) {
            input.focus_handle(cx).focus(window, cx);
        }
        let chosen = location.borrow().clone();
        let label = chosen.as_ref().map_or_else(
            || "No location selected".to_owned(),
            |p| p.display().to_string(),
        );
        let browse = Button::new("workspace-browse").small().label("Browse…").on_click({
            let location = location.clone();
            move |_, window, cx| {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Choose workspace location".into()),
                });
                let location = location.clone();
                window
                    .spawn(cx, async move |cx| {
                        if let Ok(Ok(Some(paths))) = rx.await {
                            *location.borrow_mut() = paths.into_iter().next();
                            let _ = cx.update(|window, _| window.refresh());
                        }
                    })
                    .detach();
            }
        });

        let (input_ok, location_ok) = (input.clone(), location.clone());
        dialog
            .title("New workspace")
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Input::new(&input))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().min_w_0().truncate().child(label))
                            .child(browse),
                    ),
            )
            .on_ok(move |_, window, cx| {
                let name = input_ok.read(cx).value().trim().to_owned();
                let Some(parent) = location_ok.borrow().clone() else {
                    return false; // location is mandatory
                };
                if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
                    return false;
                }
                let root = parent.join(&name);
                match WorkspaceStore::open_or_create(&root, name) {
                    Ok(_) => {
                        switch_workspace(&root, window, cx);
                        true
                    }
                    Err(e) => {
                        eprintln!("discern: cannot create workspace: {e}");
                        false
                    }
                }
            })
    });
}
