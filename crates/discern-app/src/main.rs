use discern_ui::{SharedWorkspace, open_main_window};
use discern_workspace::{RecentWorkspaces, WorkspaceStore};
use gpui_kit::assets::Assets;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

/// `discern [workspace-dir]`; without an argument the last workspace is
/// reopened, or a default one is created.
fn open_workspace() -> WorkspaceStore {
    let recent = RecentWorkspaces::default_location();
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| recent.as_ref()?.list().ok()?.into_iter().next().map(|e| e.path))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Discern")))
        .expect("no workspace path: pass one as an argument");

    let name = root.file_name().map_or_else(|| "Workspace".into(), |n| n.to_string_lossy().into_owned());
    let store = WorkspaceStore::open_or_create(&root, name)
        .unwrap_or_else(|e| panic!("cannot open workspace {}: {e}", root.display()));
    if let Some(recent) = recent {
        if let Err(e) = recent.record(&root, store.name()) {
            eprintln!("discern: could not update recent workspaces: {e}");
        }
    }
    store
}

fn main() {
    let workspace: SharedWorkspace = Rc::new(RefCell::new(open_workspace()));

    gpui_kit::application().with_assets(Assets).run(move |cx| {
        gpui_kit::init(cx);
        discern_ui::init(cx);
        open_main_window(workspace, cx);
        cx.activate(true);
    });
}
