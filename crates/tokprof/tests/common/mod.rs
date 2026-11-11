//! The tokprof binary run as a program on a home directory of its own, reading nothing of the
//! machine it runs on: every variable the program or a log source reads a location from points
//! into that home or is cleared, so a developer's price file, profile directory, cache or agent
//! logs never leak into a test.

// Each test file uses some of these.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository's root, without `..` in it, so that a path the program prints starts with it.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).expect("crates/tokprof is two levels down").to_path_buf()
}

/// A new empty folder in the temporary directory for one test.
pub fn temp_home(name: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!("tokprof-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    home
}

/// tokprof with `home` as the home directory, its logs in it and UTC as its zone on Unix.
pub fn tokprof(home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tokprof"));
    c.env("HOME", home)
        // Where Windows takes the home directory from.
        .env("USERPROFILE", home)
        .env("TOKPROF_DIR", home.join("tokprof"))
        // The user's price file, and OpenCode's stores.
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local").join("share"))
        // The cache folder: its own variable, then Windows, then the rest.
        .env("TOKPROF_CACHE_DIR", home.join("cache"))
        .env("LOCALAPPDATA", home.join("AppData").join("Local"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("CODEX_HOME", home.join(".codex"))
        .env("PI_CODING_AGENT_SESSION_DIR", home.join(".pi").join("agent").join("sessions"))
        .env_remove("PI_CODING_AGENT_DIR")
        .env_remove("OPENCODE_DB")
        .env("NO_COLOR", "1")
        // A Windows binary ignores TZ.
        .env("TZ", "UTC");
    c
}

/// tokprof over the log sources' fixture logs: Claude Code's and Codex's, and with `all` pi's and
/// OpenCode's too. OpenCode's fixtures are SQL text; its stores are built from them in the home's
/// data directory, as its conformance test builds them.
pub fn tokprof_fixtures(home: &Path, all: bool) -> Command {
    let root = root();
    let mut c = tokprof(home);
    c.env("CLAUDE_CONFIG_DIR", root.join("sources").join("claude-code").join("tests").join("fixtures"))
        .env("CODEX_HOME", root.join("sources").join("codex").join("tests").join("fixtures"));
    if all {
        c.env("PI_CODING_AGENT_SESSION_DIR", root.join("sources").join("pi").join("tests").join("fixtures").join("sessions"));
        opencode_stores(&home.join(".local").join("share").join("opencode"));
    }
    c
}

/// A store per SQL fixture of OpenCode in `dir`, named as the fixture without `.sql`; a store
/// built before is built again.
fn opencode_stores(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let fixtures = root().join("sources").join("opencode").join("tests").join("fixtures");
    for entry in std::fs::read_dir(&fixtures).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(store) = name.strip_suffix(".sql") else { continue };
        let path = dir.join(store);
        let _ = std::fs::remove_file(&path);
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
    }
}
