use crate::SharedWorkspace;
use discern_workspace::WorkspaceStore;
use gpui_kit::component::{
    IconName, Sizable,
    button::{Button, ButtonVariants as _},
    dock::{BasePanel, Panel, PanelEvent},
    input::{Input, InputState},
    list::ListItem,
    menu::PopupMenuItem,
    tree::{TreeEvent, TreeItem, TreeState, tree},
    WindowExt as _,
};
use gpui_kit::*;
use std::{cell::RefCell, collections::HashSet, rc::Rc};

const SCRIPTS: &str = "scripts";

/// Emitted when the user opens a script from the tree.
#[derive(Debug, Clone)]
pub enum FilesEvent {
    /// Workspace-relative path, e.g. `scripts/reports/a.sql`. `permanent` is
    /// set by a double-click: the file gets a regular tab instead of the preview tab.
    Open { rel: String, permanent: bool },
    Deleted(String),
    Renamed { from: String, to: String },
}

/// File manager for the workspace's `scripts/` folder: tree view plus
/// create / rename / delete for files and folders.
pub struct FilesBlock {
    workspace: SharedWorkspace,
    tree: Entity<TreeState>,
    /// Ids (paths) of directories; an empty directory has no children so the
    /// tree cannot tell it from a file on its own.
    folders: Rc<RefCell<HashSet<String>>>,
    /// Expanded folder ids, persisted in the session so the tree reopens as left.
    expanded: HashSet<String>,
    focus_handle: FocusHandle,
    _tree_events: Subscription,
}

impl FilesBlock {
    pub fn new(workspace: SharedWorkspace, cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        let expanded = workspace.borrow().session().expanded_dirs.iter().cloned().collect();
        let tree_events = cx.subscribe(&tree, |this, _, event: &TreeEvent, cx| {
            match event {
                TreeEvent::Expanded(id) => this.expanded.insert(id.to_string()),
                TreeEvent::Collapsed(id) => this.expanded.remove(id.as_ref()),
            };
            this.save_expanded();
            cx.notify();
        });
        let mut this = Self {
            workspace,
            tree,
            folders: Rc::default(),
            expanded,
            focus_handle: cx.focus_handle(),
            _tree_events: tree_events,
        };
        this.refresh(cx);
        this
    }

    fn save_expanded(&self) {
        let mut ws = self.workspace.borrow_mut();
        let mut dirs: Vec<String> = self.expanded.iter().cloned().collect();
        dirs.sort();
        let session = discern_workspace::Session { expanded_dirs: dirs, ..ws.session().clone() };
        if let Err(e) = ws.save_session(session) {
            eprintln!("discern: failed to save session: {e}");
        }
    }

    /// Rescans `scripts/`, keeping the expanded folders expanded.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let expanded: HashSet<SharedString> = self.expanded.iter().map(|s| SharedString::from(s.clone())).collect();
        let mut folders = HashSet::new();
        let items = scan(&self.workspace.borrow(), "", &expanded, &mut folders);
        // Forget folders that no longer exist (deleted or renamed).
        let before = self.expanded.len();
        self.expanded.retain(|id| folders.contains(id));
        if self.expanded.len() != before {
            self.save_expanded();
        }
        *self.folders.borrow_mut() = folders;
        self.tree.update(cx, |tree, cx| tree.set_items(items, cx));
        cx.notify();
    }

    /// Directory new items go into: the target itself if it is a folder, its
    /// parent if it is a file, `scripts/` when nothing is targeted.
    fn target_dir(&self, target: Option<&str>) -> String {
        match target {
            Some(t) if self.folders.borrow().contains(t) => t.to_owned(),
            Some(t) => t.rsplit_once('/').map_or(SCRIPTS, |(dir, _)| dir).to_owned(),
            None => SCRIPTS.to_owned(),
        }
    }

    fn selected_id(&self, cx: &App) -> Option<String> {
        self.tree.read(cx).selected_item().map(|i| i.id.to_string())
    }

    fn create(&mut self, dir: String, is_folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        prompt_name(
            if is_folder { "New folder" } else { "New script" },
            "",
            window,
            cx,
            move |name, _, cx| {
                let name = if is_folder || name.to_lowercase().ends_with(".sql") { name } else { format!("{name}.sql") };
                let rel = format!("{dir}/{name}");
                let result = this
                    .update(cx, |this, cx| {
                        let ws = this.workspace.borrow();
                        let r = if is_folder { ws.create_dir(&rel) } else { ws.create_script(&rel) };
                        drop(ws);
                        this.refresh(cx);
                        if r.is_ok() && !is_folder {
                            cx.emit(FilesEvent::Open { rel: rel.clone(), permanent: true });
                        }
                        r
                    })
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()));
                report(result, cx)
            },
        );
    }

    fn rename(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        let old_name = rel.rsplit('/').next().unwrap_or_default().to_owned();
        prompt_name("Rename", &old_name, window, cx, move |name, _, cx| {
            let parent = rel.rsplit_once('/').map_or(SCRIPTS, |(p, _)| p);
            let to = format!("{parent}/{name}");
            let result = this
                .update(cx, |this, cx| {
                    let r = this.workspace.borrow().rename_script(&rel, &to);
                    if r.is_ok() {
                        cx.emit(FilesEvent::Renamed { from: rel.clone(), to: to.clone() });
                    }
                    this.refresh(cx);
                    r
                })
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
            report(result, cx)
        });
    }

    fn delete(&mut self, rel: String, is_folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        let name = rel.rsplit('/').next().unwrap_or_default().to_owned();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (this, rel) = (this.clone(), rel.clone());
            alert
                .title(format!("Delete {name}?"))
                .description(if is_folder {
                    "The folder and everything in it will be deleted. This cannot be undone."
                } else {
                    "The script will be deleted. This cannot be undone."
                })
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        let ws = this.workspace.borrow();
                        let r = if is_folder { ws.delete_dir(&rel) } else { ws.delete_script(&rel) };
                        drop(ws);
                        match r {
                            Ok(()) => cx.emit(FilesEvent::Deleted(rel.clone())),
                            Err(e) => eprintln!("discern: cannot delete {rel}: {e}"),
                        }
                        this.refresh(cx);
                    });
                    true
                })
        });
    }
}

fn report(result: Result<(), String>, _: &mut App) -> bool {
    match result {
        Ok(()) => true,
        Err(e) => {
            eprintln!("discern: {e}");
            false
        }
    }
}

/// Recursively lists `scripts/<rel>` as tree items (folders first).
fn scan(ws: &WorkspaceStore, rel: &str, expanded: &HashSet<SharedString>, folders: &mut HashSet<String>) -> Vec<TreeItem> {
    let Ok(entries) = ws.list_scripts(rel) else { return Vec::new() };
    entries
        .into_iter()
        .map(|entry| {
            let mut item = TreeItem::new(entry.path.clone(), entry.name.clone());
            if entry.is_dir {
                folders.insert(entry.path.clone());
                let inner = entry.path.strip_prefix("scripts/").unwrap_or_default();
                item = item
                    .children(scan(ws, inner, expanded, folders))
                    .expanded(expanded.contains(entry.path.as_str()));
            }
            item
        })
        .collect()
}

/// Single-field name dialog. `on_submit` returns whether to close it.
fn prompt_name(
    title: &'static str,
    initial: &str,
    window: &mut Window,
    cx: &mut App,
    on_submit: impl Fn(String, &mut Window, &mut App) -> bool + 'static,
) {
    let initial = initial.to_owned();
    let input = cx.new(|cx| InputState::new(window, cx).placeholder("Name").default_value(initial));
    let focus_once = std::cell::Cell::new(true);
    let on_submit = Rc::new(on_submit);
    window.open_dialog(cx, move |dialog, window, cx| {
        if focus_once.replace(false) {
            input.focus_handle(cx).focus(window, cx);
        }
        let (input_ok, on_submit) = (input.clone(), on_submit.clone());
        dialog.title(title).child(Input::new(&input)).on_ok(move |_, window, cx| {
            let name = input_ok.read(cx).value().trim().to_owned();
            if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
                return false;
            }
            on_submit(name, window, cx)
        })
    });
}

impl BasePanel for FilesBlock {
    fn panel_name(&self) -> &'static str {
        "discern.explorer"
    }
}

impl Panel for FilesBlock {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("Explorer".into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Explorer"
    }
}

impl EventEmitter<PanelEvent> for FilesBlock {}
impl EventEmitter<FilesEvent> for FilesBlock {}

impl Focusable for FilesBlock {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FilesBlock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let folders = self.folders.clone();

        let toolbar = div()
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .child(Button::new("files-new-file").ghost().xsmall().icon(IconName::Plus).label("File").on_click(
                cx.listener(|this, _, window, cx| {
                    let dir = this.target_dir(this.selected_id(cx).as_deref());
                    this.create(dir, false, window, cx);
                }),
            ))
            .child(Button::new("files-new-folder").ghost().xsmall().icon(IconName::Plus).label("Folder").on_click(
                cx.listener(|this, _, window, cx| {
                    let dir = this.target_dir(this.selected_id(cx).as_deref());
                    this.create(dir, true, window, cx);
                }),
            ))
            .child(Button::new("files-refresh").ghost().xsmall().label("Refresh").on_click(
                cx.listener(|this, _, _, cx| this.refresh(cx)),
            ));

        let tree = tree(&self.tree, {
            let (weak, folders) = (weak.clone(), folders.clone());
            move |ix, entry, _selected, _window, _cx| {
                let id = entry.item().id.to_string();
                let is_folder = folders.borrow().contains(&id);
                let icon = if is_folder { IconName::Folder } else { IconName::File };
                let item = ListItem::new(ix)
                    .pl(px(8. + 14. * entry.depth() as f32))
                    .child(div().flex().items_center().gap_2().child(icon).child(entry.item().label.clone()));
                if is_folder {
                    item
                } else {
                    let weak = weak.clone();
                    item.on_click(move |event, _, cx| {
                        let permanent = event.click_count() >= 2;
                        let _ = weak.update(cx, |_, cx| cx.emit(FilesEvent::Open { rel: id.clone(), permanent }));
                    })
                }
            }
        })
        .context_menu({
            let (weak, folders) = (weak, folders);
            move |_, entry, menu, _, _| {
                let id = entry.item().id.to_string();
                let is_folder = folders.borrow().contains(&id);
                let action = |f: fn(&mut FilesBlock, String, bool, &mut Window, &mut Context<FilesBlock>)| {
                    let (weak, id) = (weak.clone(), id.clone());
                    move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                        let (weak, id) = (weak.clone(), id.clone());
                        window.defer(cx, move |window, cx| {
                            let _ = weak.update(cx, |this, cx| f(this, id, is_folder, window, cx));
                        });
                    }
                };
                menu.item(PopupMenuItem::new("New script").on_click(action(|this, id, _, window, cx| {
                    let dir = this.target_dir(Some(&id));
                    this.create(dir, false, window, cx);
                })))
                .item(PopupMenuItem::new("New folder").on_click(action(|this, id, _, window, cx| {
                    let dir = this.target_dir(Some(&id));
                    this.create(dir, true, window, cx);
                })))
                .separator()
                .item(PopupMenuItem::new("Rename").on_click(action(|this, id, _, window, cx| this.rename(id, window, cx))))
                .item(PopupMenuItem::new("Delete").on_click(action(|this, id, is_folder, window, cx| {
                    this.delete(id, is_folder, window, cx);
                })))
            }
        });

        div().size_full().flex().flex_col().child(toolbar).child(div().flex_1().min_h_0().child(tree))
    }
}
