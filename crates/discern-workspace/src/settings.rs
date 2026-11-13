//! User settings, stored as one `settings.json` that can be edited either
//! through the settings UI or by hand.
//!
//! The file is *untrusted input*: it may contain comments, trailing commas,
//! unknown keys, wrong types, out-of-range numbers, or not be JSON at all.
//! Loading never fails: every invalid entry falls back to its default and is
//! reported as a [`Problem`], so a typo can never stop the app from starting.

use crate::{
    error::{IoContext, Result, WorkspaceError},
    fsutil::write_atomic,
};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

// ---- registry --------------------------------------------------------------

/// A setting value.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Bool(bool),
    Int(i64),
    Str(String),
}

impl Val {
    fn to_json(&self) -> Value {
        match self {
            Val::Bool(b) => Value::Bool(*b),
            Val::Int(i) => Value::from(*i),
            Val::Str(s) => Value::String(s.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Bool,
    Int { min: i64, max: i64 },
    Enum(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy)]
enum DefaultVal {
    Bool(bool),
    Int(i64),
    Str(&'static str),
}

/// Static description of one setting; drives validation *and* the settings UI.
#[derive(Debug, Clone, Copy)]
pub struct Def {
    pub key: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    default: DefaultVal,
}

impl Def {
    pub fn default_value(&self) -> Val {
        match self.default {
            DefaultVal::Bool(b) => Val::Bool(b),
            DefaultVal::Int(i) => Val::Int(i),
            DefaultVal::Str(s) => Val::Str(s.to_owned()),
        }
    }

    /// Checks a JSON value against this setting's type and range.
    pub fn coerce(&self, value: &Value) -> std::result::Result<Val, String> {
        match self.kind {
            Kind::Bool => value.as_bool().map(Val::Bool).ok_or_else(|| format!("expected true or false, got {}", describe(value))),
            Kind::Int { min, max } => {
                let n = value
                    .as_i64()
                    .or_else(|| value.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 1e15).map(|f| f as i64))
                    .ok_or_else(|| format!("expected a whole number, got {}", describe(value)))?;
                if (min..=max).contains(&n) { Ok(Val::Int(n)) } else { Err(format!("must be between {min} and {max}, got {n}")) }
            }
            Kind::Enum(options) => {
                let s = value.as_str().ok_or_else(|| format!("expected a string, got {}", describe(value)))?;
                if options.contains(&s) {
                    Ok(Val::Str(s.to_owned()))
                } else {
                    Err(format!("must be one of {}, got \"{s}\"", options.iter().map(|o| format!("\"{o}\"")).collect::<Vec<_>>().join(", ")))
                }
            }
        }
    }
}

fn describe(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => format!("{b}"),
        Value::Number(n) => format!("number {n}"),
        Value::String(s) => format!("string \"{s}\""),
        Value::Array(_) => "an array".into(),
        Value::Object(_) => "an object".into(),
    }
}

pub const DEFINITIONS: &[Def] = &[
    Def { key: "workbench.restoreLayout", category: "Workbench", title: "Restore panel layout", description: "Reopen the workspace with the panels where you left them (after dragging a panel to another dock, for example). When off, the default layout is used every time.", kind: Kind::Bool, default: DefaultVal::Bool(true) },
    Def { key: "appearance.theme", category: "Appearance", title: "Theme", description: "Color theme: follow the system, or force light or dark.", kind: Kind::Enum(&["system", "light", "dark"]), default: DefaultVal::Str("system") },
    Def { key: "editor.tabSize", category: "Editor", title: "Tab size", description: "Number of spaces a tab is worth.", kind: Kind::Int { min: 1, max: 16 }, default: DefaultVal::Int(2) },
    Def { key: "editor.softWrap", category: "Editor", title: "Word wrap", description: "Wrap long lines instead of scrolling horizontally.", kind: Kind::Bool, default: DefaultVal::Bool(false) },
    Def { key: "editor.lineNumbers", category: "Editor", title: "Line numbers", description: "Show line numbers in the gutter.", kind: Kind::Bool, default: DefaultVal::Bool(true) },
    Def { key: "editor.indentGuides", category: "Editor", title: "Indent guides", description: "Draw vertical guides at each indentation level.", kind: Kind::Bool, default: DefaultVal::Bool(true) },
    Def { key: "editor.autoSaveDelay", category: "Editor", title: "Auto-save delay (ms)", description: "How long to wait after the last keystroke before saving.", kind: Kind::Int { min: 100, max: 10_000 }, default: DefaultVal::Int(400) },
];

pub fn definition(key: &str) -> Option<&'static Def> {
    DEFINITIONS.iter().find(|d| d.key == key)
}

// ---- values ------------------------------------------------------------------

/// Effective settings: every key always has a valid value.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    values: BTreeMap<&'static str, Val>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { values: DEFINITIONS.iter().map(|d| (d.key, d.default_value())).collect() }
    }
}

impl Settings {
    pub fn get(&self, key: &str) -> Option<&Val> { self.values.get(key) }

    fn bool(&self, key: &str) -> bool {
        matches!(self.get(key), Some(Val::Bool(true)))
    }
    fn int(&self, key: &str) -> i64 {
        match self.get(key) { Some(Val::Int(i)) => *i, _ => 0 }
    }
    fn str(&self, key: &str) -> &str {
        match self.get(key) { Some(Val::Str(s)) => s, _ => "" }
    }

    pub fn restore_layout(&self) -> bool { self.bool("workbench.restoreLayout") }
    pub fn theme(&self) -> &str { self.str("appearance.theme") }
    pub fn tab_size(&self) -> usize { self.int("editor.tabSize") as usize }
    pub fn soft_wrap(&self) -> bool { self.bool("editor.softWrap") }
    pub fn line_numbers(&self) -> bool { self.bool("editor.lineNumbers") }
    pub fn indent_guides(&self) -> bool { self.bool("editor.indentGuides") }
    pub fn auto_save_delay_ms(&self) -> u64 { self.int("editor.autoSaveDelay") as u64 }
}

// ---- problems ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The value was ignored and the default used.
    Error,
    /// Harmless (e.g. an unknown key, kept in the file untouched).
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub severity: Severity,
    pub key: Option<String>,
    /// 1-based line in the file, when it could be located.
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line { write!(f, "line {line}: ")?; }
        if let Some(key) = &self.key { write!(f, "\"{key}\": ")?; }
        f.write_str(&self.message)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub settings: Settings,
    pub problems: Vec<Problem>,
}

// ---- parsing -----------------------------------------------------------------

/// Removes `//` and `/* */` comments and trailing commas (VS Code allows both)
/// while keeping every other byte in place, so line/column positions stay valid.
fn strip_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let (mut i, mut in_str) = (0, false);
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    out.extend([' ', ' ']);
                    i += 2;
                    closed = true;
                    break;
                }
                out.push(if chars[i] == '\n' { '\n' } else { ' ' });
                i += 1;
            }
            if !closed { break; }
            continue;
        } else {
            out.push(c);
        }
        i += 1;
    }

    // Trailing commas: a `,` whose next significant char closes a container.
    let mut result = out.clone();
    let mut in_str = false;
    let mut k = 0;
    while k < out.len() {
        let c = out[k];
        if in_str {
            if c == '\\' { k += 1; } else if c == '"' { in_str = false; }
        } else if c == '"' {
            in_str = true;
        } else if c == ',' {
            let next = out[k + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}' | ']')) {
                result[k] = ' ';
            }
        }
        k += 1;
    }
    result.into_iter().collect()
}

fn key_line(text: &str, key: &str) -> Option<usize> {
    let needle = format!("\"{key}\"");
    text.lines().position(|l| l.contains(&needle)).map(|i| i + 1)
}

/// Parses the file as a JSON object. Syntax errors are returned as a problem.
fn parse_object(text: &str) -> std::result::Result<Map<String, Value>, Problem> {
    let stripped = strip_jsonc(text);
    if stripped.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&stripped) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(other) => Err(Problem {
            severity: Severity::Error,
            key: None,
            line: Some(1),
            message: format!("the file must contain a JSON object ({{ ... }}), found {}", describe(&other)),
        }),
        Err(e) => Err(Problem {
            severity: Severity::Error,
            key: None,
            line: Some(e.line().max(1)),
            message: format!("invalid JSON (column {}): {e}", e.column()),
        }),
    }
}

/// Never fails: invalid input yields defaults plus a list of problems.
pub fn parse_settings(text: &str) -> Parsed {
    let mut settings = Settings::default();
    let mut problems = Vec::new();
    let map = match parse_object(text) {
        Ok(map) => map,
        Err(problem) => {
            problems.push(problem);
            return Parsed { settings, problems };
        }
    };
    for (key, value) in &map {
        let line = key_line(text, key);
        match definition(key) {
            None => problems.push(Problem {
                severity: Severity::Warning,
                key: Some(key.clone()),
                line,
                message: "unknown setting (ignored)".into(),
            }),
            Some(def) => match def.coerce(value) {
                Ok(v) => { settings.values.insert(def.key, v); }
                Err(message) => problems.push(Problem {
                    severity: Severity::Error,
                    key: Some(key.clone()),
                    line,
                    message: format!("{message}; using default {}", display_val(&def.default_value())),
                }),
            },
        }
    }
    Parsed { settings, problems }
}

fn display_val(v: &Val) -> String {
    match v {
        Val::Bool(b) => b.to_string(),
        Val::Int(i) => i.to_string(),
        Val::Str(s) => format!("\"{s}\""),
    }
}

// ---- store -------------------------------------------------------------------

/// The settings file on disk plus its last parsed state.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
    problems: Vec<Problem>,
}

impl SettingsStore {
    /// `<config dir>/settings.json`.
    pub fn default_path() -> Option<PathBuf> {
        crate::recent::config_dir().map(|d| d.join("settings.json"))
    }

    /// Loads the file; a missing or unreadable file yields defaults.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let mut store = Self { path: path.into(), settings: Settings::default(), problems: Vec::new() };
        store.reload();
        store
    }

    pub fn path(&self) -> &Path { &self.path }
    pub fn settings(&self) -> &Settings { &self.settings }
    pub fn problems(&self) -> &[Problem] { &self.problems }

    /// Re-reads the file (after the user edited it).
    pub fn reload(&mut self) {
        let parsed = match std::fs::read_to_string(&self.path) {
            Ok(text) => parse_settings(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Parsed { settings: Settings::default(), problems: Vec::new() }
            }
            Err(e) => Parsed {
                settings: Settings::default(),
                problems: vec![Problem {
                    severity: Severity::Error,
                    key: None,
                    line: None,
                    message: format!("cannot read {}: {e}", self.path.display()),
                }],
            },
        };
        self.settings = parsed.settings;
        self.problems = parsed.problems;
    }

    /// Raw file text for the JSON editor; creates an empty file if missing.
    pub fn read_text(&self) -> Result<String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let text = "{\n}\n".to_owned();
                write_atomic(&self.path, text.as_bytes())?;
                Ok(text)
            }
            Err(source) => Err(WorkspaceError::Io { path: self.path.clone(), source }),
        }
    }

    /// Writes hand-edited text verbatim (whatever it contains) and reloads.
    pub fn write_text(&mut self, text: &str) -> Result<()> {
        write_atomic(&self.path, text.as_bytes())?;
        self.reload();
        Ok(())
    }

    /// Sets one value from the UI. Unknown keys and invalid values are
    /// rejected; if the file currently is not valid JSON the write is
    /// refused (rather than clobbering what the user typed).
    /// Unknown keys already in the file are preserved; comments are not.
    pub fn set(&mut self, key: &str, value: Val) -> Result<()> {
        let def = definition(key).ok_or_else(|| WorkspaceError::Settings(format!("unknown setting \"{key}\"")))?;
        def.coerce(&value.to_json()).map_err(WorkspaceError::Settings)?;
        self.edit(|map| { map.insert(def.key.to_owned(), value.to_json()); })
    }

    /// Removes the key from the file, restoring its default.
    pub fn reset(&mut self, key: &str) -> Result<()> {
        self.edit(|map| { map.remove(key); })
    }

    fn edit(&mut self, f: impl FnOnce(&mut Map<String, Value>)) -> Result<()> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).at(&self.path),
        };
        let mut map = parse_object(&text).map_err(|p| {
            WorkspaceError::Settings(format!("settings.json has errors ({p}); fix it before changing settings here"))
        })?;
        f(&mut map);
        let mut bytes = serde_json::to_vec_pretty(&map).map_err(|source| WorkspaceError::Json { path: self.path.clone(), source })?;
        bytes.push(b'\n');
        write_atomic(&self.path, &bytes)?;
        self.reload();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_missing_give_defaults() {
        for text in ["", "  \n", "{}"] {
            let p = parse_settings(text);
            assert_eq!(p.settings, Settings::default());
            assert!(p.problems.is_empty(), "{text:?}");
        }
    }

    #[test]
    fn valid_values_are_applied() {
        let p = parse_settings(r#"{ "editor.tabSize": 4, "editor.softWrap": true, "appearance.theme": "dark" }"#);
        assert!(p.problems.is_empty());
        assert_eq!(p.settings.tab_size(), 4);
        assert!(p.settings.soft_wrap());
        assert_eq!(p.settings.theme(), "dark");
    }

    #[test]
    fn comments_and_trailing_commas_are_accepted() {
        let p = parse_settings("{\n  // indent\n  \"editor.tabSize\": 8, /* x */\n  \"editor.softWrap\": true,\n}\n");
        assert!(p.problems.is_empty(), "{:?}", p.problems);
        assert_eq!(p.settings.tab_size(), 8);
        // a // inside a string is not a comment
        let p = parse_settings(r#"{ "appearance.theme": "http://x" }"#);
        assert_eq!(p.problems.len(), 1);
    }

    #[test]
    fn syntax_error_falls_back_to_defaults_with_line() {
        let p = parse_settings("{\n  \"editor.tabSize\": 4,\n  oops\n}");
        assert_eq!(p.settings, Settings::default());
        assert_eq!(p.problems.len(), 1);
        assert_eq!(p.problems[0].severity, Severity::Error);
        assert_eq!(p.problems[0].line, Some(3));
    }

    #[test]
    fn non_object_roots_are_rejected() {
        for text in ["[1,2]", "42", "\"hi\"", "null", "{", "\u{0}\u{1}garbage"] {
            let p = parse_settings(text);
            assert_eq!(p.settings, Settings::default(), "{text:?}");
            assert_eq!(p.problems.len(), 1, "{text:?}");
        }
    }

    #[test]
    fn bad_values_only_reset_their_own_key() {
        let p = parse_settings(
            "{\n\"editor.tabSize\": \"four\",\n\"editor.softWrap\": true,\n\"editor.autoSaveDelay\": 5,\n\"appearance.theme\": \"purple\",\n\"editor.lineNumbers\": null\n}",
        );
        assert!(p.settings.soft_wrap()); // the good one survives
        assert_eq!(p.settings.tab_size(), 2);
        assert_eq!(p.settings.auto_save_delay_ms(), 400);
        assert_eq!(p.settings.theme(), "system");
        assert!(p.settings.line_numbers());
        assert_eq!(p.problems.len(), 4);
        assert!(p.problems.iter().all(|p| p.severity == Severity::Error && p.line.is_some()));
    }

    #[test]
    fn unknown_keys_warn_but_do_not_break() {
        let p = parse_settings(r#"{ "editor.fontSize": 14, "editor.tabSize": 3 }"#);
        assert_eq!(p.settings.tab_size(), 3);
        assert_eq!(p.problems.len(), 1);
        assert_eq!(p.problems[0].severity, Severity::Warning);
    }

    #[test]
    fn float_that_is_whole_is_accepted_fraction_is_not() {
        assert_eq!(parse_settings(r#"{"editor.tabSize": 4.0}"#).settings.tab_size(), 4);
        assert_eq!(parse_settings(r#"{"editor.tabSize": 4.5}"#).problems.len(), 1);
        assert_eq!(parse_settings(r#"{"editor.tabSize": 99999999999999999999}"#).problems.len(), 1);
    }

    #[test]
    fn store_set_reset_roundtrip_and_preserves_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/settings.json");
        let mut store = SettingsStore::load(&path);
        assert!(store.problems().is_empty());

        store.set("editor.tabSize", Val::Int(6)).unwrap();
        assert_eq!(store.settings().tab_size(), 6);
        std::fs::write(&path, r#"{"editor.tabSize": 6, "custom.thing": [1]}"#).unwrap();
        store.set("editor.softWrap", Val::Bool(true)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("custom.thing"));
        assert!(SettingsStore::load(&path).settings().soft_wrap());

        store.reset("editor.tabSize").unwrap();
        assert_eq!(store.settings().tab_size(), 2);
    }

    #[test]
    fn store_rejects_invalid_ui_values_and_refuses_to_clobber_broken_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut store = SettingsStore::load(&path);
        assert!(store.set("editor.tabSize", Val::Int(0)).is_err());
        assert!(store.set("editor.tabSize", Val::Bool(true)).is_err());
        assert!(store.set("nope", Val::Bool(true)).is_err());

        std::fs::write(&path, "{ this is not json").unwrap();
        store.reload();
        assert_eq!(store.problems().len(), 1);
        assert!(store.set("editor.softWrap", Val::Bool(true)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ this is not json");
    }

    #[test]
    fn unreadable_file_reports_problem() {
        let dir = tempfile::tempdir().unwrap();
        // a directory where the file should be
        let store = SettingsStore::load(dir.path());
        assert_eq!(store.problems().len(), 1);
        assert_eq!(*store.settings(), Settings::default());
    }

    #[test]
    fn binary_garbage_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x80]).unwrap();
        let store = SettingsStore::load(&path); // not valid UTF-8
        assert_eq!(store.problems().len(), 1);
    }
}
