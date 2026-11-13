use crate::{
    cache::SchemaSnapshot,
    connections::{ConnectionsFile, SavedConnection},
    error::{IoContext, Result, WorkspaceError},
    fsutil::{read_json, read_json_opt, write_atomic, write_json},
    layout::{FORMAT_VERSION, Paths},
    manifest::Manifest,
    scripts::{self, Entry},
    session::Session,
};
use std::{fs, path::{Path, PathBuf}};

/// An open workspace folder. Construction reads three small files; everything
/// else (scripts, drafts, schema caches) is loaded on demand.
#[derive(Debug)]
pub struct WorkspaceStore {
    paths: Paths,
    manifest: Manifest,
    connections: Vec<SavedConnection>,
    session: Session,
}

impl WorkspaceStore {
    /// Creates a new workspace in `root` (created if missing).
    pub fn create(root: impl Into<PathBuf>, name: impl Into<String>) -> Result<Self> {
        let paths = Paths::new(root);
        if paths.manifest().exists() {
            return Err(WorkspaceError::AlreadyExists(paths.root().to_owned()));
        }
        fs::create_dir_all(paths.scripts()).at(&paths.scripts())?;
        fs::create_dir_all(paths.state()).at(&paths.state())?;
        // Machine-local state must not be committed with the scripts.
        write_atomic(&paths.state().join(".gitignore"), b"*\n")?;

        let store = Self { manifest: Manifest::new(name.into()), connections: Vec::new(), session: Session::default(), paths };
        write_json(&store.paths.manifest(), &store.manifest)?;
        store.save_connections()?;
        Ok(store)
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let paths = Paths::new(root);
        let manifest: Manifest = match read_json_opt(&paths.manifest())? {
            Some(m) => m,
            None => return Err(WorkspaceError::NotAWorkspace(paths.root().to_owned())),
        };
        if manifest.format > FORMAT_VERSION {
            return Err(WorkspaceError::TooNew { path: paths.root().to_owned(), found: manifest.format, supported: FORMAT_VERSION });
        }
        let connections = read_json_opt::<ConnectionsFile>(&paths.connections())?.unwrap_or_default().connections;
        // The session is only a convenience: a corrupt one must never block opening.
        let session = read_json::<Session>(&paths.session()).unwrap_or_default();
        Ok(Self { paths, manifest, connections, session })
    }

    pub fn open_or_create(root: impl Into<PathBuf>, name: impl Into<String>) -> Result<Self> {
        let root = root.into();
        if Paths::new(&root).manifest().exists() { Self::open(root) } else { Self::create(root, name) }
    }

    pub fn root(&self) -> &Path { self.paths.root() }
    pub fn name(&self) -> &str { &self.manifest.name }
    pub fn id(&self) -> &str { &self.manifest.id }

    // ---- connections -------------------------------------------------

    pub fn connections(&self) -> &[SavedConnection] { &self.connections }

    pub fn connection(&self, id: &str) -> Option<&SavedConnection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// Inserts or replaces (matched by id) and persists.
    pub fn upsert_connection(&mut self, conn: SavedConnection) -> Result<()> {
        match self.connections.iter_mut().find(|c| c.id == conn.id) {
            Some(slot) => *slot = conn,
            None => self.connections.push(conn),
        }
        self.save_connections()
    }

    pub fn remove_connection(&mut self, id: &str) -> Result<()> {
        let before = self.connections.len();
        self.connections.retain(|c| c.id != id);
        if self.connections.len() == before {
            return Err(WorkspaceError::UnknownConnection(id.to_owned()));
        }
        self.save_connections()?;
        self.remove_file_if_exists(&self.schema_cache_path(id))
    }

    fn save_connections(&self) -> Result<()> {
        write_json(&self.paths.connections(), &ConnectionsFile { connections: self.connections.clone() })
    }

    // ---- session -----------------------------------------------------

    pub fn session(&self) -> &Session { &self.session }

    /// Replaces the in-memory session and writes it. Cheap enough to call on
    /// every tab change; callers may debounce for cursor movement.
    pub fn save_session(&mut self, session: Session) -> Result<()> {
        self.session = session;
        write_json(&self.paths.session(), &self.session)
    }

    // ---- scripts -----------------------------------------------------

    /// Lists one directory of `scripts/` lazily. `rel` is relative to the
    /// scripts directory ("" for the top level); returned paths are relative
    /// to the workspace root (`scripts/...`).
    pub fn list_scripts(&self, rel: &str) -> Result<Vec<Entry>> {
        let full = if rel.is_empty() { crate::layout::SCRIPTS_DIR.to_owned() } else { format!("{}/{rel}", crate::layout::SCRIPTS_DIR) };
        scripts::list_dir(self.root(), &full)
    }

    pub fn read_script(&self, rel: &str) -> Result<String> {
        let p = scripts::resolve(self.root(), rel)?;
        fs::read_to_string(&p).at(&p)
    }

    pub fn write_script(&self, rel: &str, text: &str) -> Result<()> {
        write_atomic(&scripts::resolve(self.root(), rel)?, text.as_bytes())
    }

    pub fn rename_script(&self, from: &str, to: &str) -> Result<()> {
        let (a, b) = (scripts::resolve(self.root(), from)?, scripts::resolve(self.root(), to)?);
        if let Some(dir) = b.parent() { fs::create_dir_all(dir).at(dir)?; }
        fs::rename(&a, &b).at(&a)
    }

    /// Creates a directory (and its parents) inside the workspace.
    pub fn create_dir(&self, rel: &str) -> Result<()> {
        let p = scripts::resolve(self.root(), rel)?;
        fs::create_dir_all(&p).at(&p)
    }

    /// Creates a new empty script, failing if the path is already taken.
    pub fn create_script(&self, rel: &str) -> Result<()> {
        let p = scripts::resolve(self.root(), rel)?;
        if let Some(dir) = p.parent() { fs::create_dir_all(dir).at(dir)?; }
        fs::OpenOptions::new().write(true).create_new(true).open(&p).at(&p).map(drop)
    }

    /// Deletes a directory and everything in it.
    pub fn delete_dir(&self, rel: &str) -> Result<()> {
        let p = scripts::resolve(self.root(), rel)?;
        fs::remove_dir_all(&p).at(&p)
    }

    pub fn delete_script(&self, rel: &str) -> Result<()> {
        let p = scripts::resolve(self.root(), rel)?;
        fs::remove_file(&p).at(&p)
    }

    // ---- drafts (unsaved consoles) ------------------------------------

    fn draft_path(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(WorkspaceError::InvalidPath(id.to_owned()));
        }
        Ok(self.paths.drafts().join(format!("{id}.sql")))
    }

    pub fn new_draft_id(&self) -> String { crate::fsutil::new_id() }

    pub fn write_draft(&self, id: &str, text: &str) -> Result<()> {
        write_atomic(&self.draft_path(id)?, text.as_bytes())
    }

    pub fn read_draft(&self, id: &str) -> Result<Option<String>> {
        let p = self.draft_path(id)?;
        match fs::read_to_string(&p) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(WorkspaceError::Io { path: p, source }),
        }
    }

    pub fn delete_draft(&self, id: &str) -> Result<()> {
        self.remove_file_if_exists(&self.draft_path(id)?)
    }

    // ---- schema cache --------------------------------------------------

    fn schema_cache_path(&self, conn_id: &str) -> PathBuf {
        let safe: String = conn_id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        self.paths.schema_cache().join(format!("{safe}.json"))
    }

    /// The last persisted introspection, if any. A corrupt cache is treated
    /// as absent: it is only an accelerator and is rebuilt by the next refresh.
    pub fn load_schema_cache(&self, conn_id: &str) -> Option<SchemaSnapshot> {
        read_json(&self.schema_cache_path(conn_id)).ok()
    }

    pub fn save_schema_cache(&self, conn_id: &str, snapshot: &SchemaSnapshot) -> Result<()> {
        write_json(&self.schema_cache_path(conn_id), snapshot)
    }

    fn remove_file_if_exists(&self, p: &Path) -> Result<()> {
        match fs::remove_file(p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(WorkspaceError::Io { path: p.to_owned(), source }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache::*, session::*};

    #[test]
    fn create_then_reopen_roundtrips_everything() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        let mut ws = WorkspaceStore::create(&root, "Demo").unwrap();

        let conn = SavedConnection::new("local", "postgres", "postgres://localhost/app");
        let cid = conn.id.clone();
        ws.upsert_connection(conn).unwrap();
        ws.write_script("scripts/a/report.sql", "select 1;").unwrap();
        let draft = ws.new_draft_id();
        ws.write_draft(&draft, "select 2").unwrap();
        ws.save_session(Session {
            tabs: vec![
                TabState { source: TabSource::File("scripts/a/report.sql".into()), connection: Some(cid.clone()), cursor: 4, scroll_line: 0 },
                TabState { source: TabSource::Draft(draft.clone()), connection: None, cursor: 0, scroll_line: 0 },
            ],
            active_tab: Some(1),
            ..Session::default()
        }).unwrap();
        ws.save_schema_cache(&cid, &SchemaSnapshot {
            fetched_at: 1,
            schemas: vec![SchemaInfo { name: "public".into(), tables: vec![TableInfo { name: "t".into(), kind: TableKind::Table, columns: vec![] }] }],
        }).unwrap();
        drop(ws);

        let ws = WorkspaceStore::open(&root).unwrap();
        assert_eq!(ws.name(), "Demo");
        assert_eq!(ws.connections().len(), 1);
        assert_eq!(ws.session().tabs.len(), 2);
        assert_eq!(ws.session().active_tab, Some(1));
        assert_eq!(ws.read_script("scripts/a/report.sql").unwrap(), "select 1;");
        assert_eq!(ws.read_draft(&draft).unwrap().as_deref(), Some("select 2"));
        assert_eq!(ws.load_schema_cache(&cid).unwrap().schemas[0].name, "public");
    }

    #[test]
    fn open_rejects_non_workspace_and_double_create() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(WorkspaceStore::open(dir.path()), Err(WorkspaceError::NotAWorkspace(_))));
        WorkspaceStore::create(dir.path(), "x").unwrap();
        assert!(matches!(WorkspaceStore::create(dir.path(), "x"), Err(WorkspaceError::AlreadyExists(_))));
    }

    #[test]
    fn corrupt_session_and_cache_do_not_block_open() {
        let dir = tempfile::tempdir().unwrap();
        let ws = WorkspaceStore::create(dir.path(), "x").unwrap();
        fs::write(dir.path().join(".discern/session.json"), "{garbage").unwrap();
        fs::create_dir_all(dir.path().join(".discern/cache/schema")).unwrap();
        fs::write(dir.path().join(".discern/cache/schema/c.json"), "nope").unwrap();
        drop(ws);
        let ws = WorkspaceStore::open(dir.path()).unwrap();
        assert!(ws.session().tabs.is_empty());
        assert!(ws.load_schema_cache("c").is_none());
    }

    #[test]
    fn paths_cannot_escape_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let ws = WorkspaceStore::create(dir.path(), "x").unwrap();
        assert!(ws.read_script("../secret.sql").is_err());
        assert!(ws.write_script("/etc/x.sql", "").is_err());
        assert!(ws.write_draft("../x", "").is_err());
    }

    #[test]
    fn lists_directories_first_and_only_sql() {
        let dir = tempfile::tempdir().unwrap();
        let ws = WorkspaceStore::create(dir.path(), "x").unwrap();
        ws.write_script("scripts/b.sql", "").unwrap();
        ws.write_script("scripts/A.sql", "").unwrap();
        ws.write_script("scripts/notes.txt", "").unwrap();
        ws.write_script("scripts/sub/c.sql", "").unwrap();
        let names: Vec<_> = ws.list_scripts("").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["sub", "A.sql", "b.sql"]);
        assert_eq!(ws.list_scripts("sub").unwrap()[0].path, "scripts/sub/c.sql");
    }

    #[test]
    fn creates_dirs_and_scripts_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let ws = WorkspaceStore::create(dir.path(), "x").unwrap();
        ws.create_dir("scripts/reports/2026").unwrap();
        ws.create_script("scripts/reports/a.sql").unwrap();
        assert!(ws.create_script("scripts/reports/a.sql").is_err());
        assert_eq!(ws.list_scripts("reports").unwrap().len(), 2);
        ws.delete_dir("scripts/reports").unwrap();
        assert!(ws.list_scripts("").unwrap().is_empty());
    }

    #[test]
    fn newer_format_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        WorkspaceStore::create(dir.path(), "x").unwrap();
        let p = dir.path().join(crate::layout::MANIFEST_FILE);
        let text = fs::read_to_string(&p).unwrap().replace("\"format\": 1", "\"format\": 99");
        fs::write(&p, text).unwrap();
        assert!(matches!(WorkspaceStore::open(dir.path()), Err(WorkspaceError::TooNew { .. })));
    }

    #[test]
    fn recent_list_is_deduped_and_ordered() {
        let dir = tempfile::tempdir().unwrap();
        let r = crate::recent::RecentWorkspaces::at(dir.path().join("cfg/recent.json"));
        r.record(Path::new("/a"), "A").unwrap();
        r.record(Path::new("/b"), "B").unwrap();
        r.record(Path::new("/a"), "A").unwrap();
        let l = r.list().unwrap();
        assert_eq!(l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        r.forget(Path::new("/a")).unwrap();
        assert_eq!(r.list().unwrap().len(), 1);
    }
}
