//! The layout rules of the workspace: no agent knowledge outside `sources/`, and the dependency
//! edges between the packages.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// What only a source may know: agent names, their instruction files, the names of agents' tools,
/// and log fields.
const AGENT_WORDS: &[&str] = &[
    "Claude",
    "claude",
    "CLAUDE.md",
    "AGENTS.md",
    "Codex",
    "codex",
    "BashOutput",
    "exec_command",
    "MultiEdit",
    "WebFetch",
    "TodoWrite",
    "update_plan",
    "apply_patch",
    "sessionId",
    "session_meta",
    "cache_creation",
    "parent_thread_id",
    "token_count",
    "response_item",
    "PI_CODING_AGENT",
    "pi-coding",
    "parentSession",
    "OpenCode",
    "opencode",
    "session_v2",
    "step-finish",
    "Junie",
    "junie",
];

/// The one file of the framework that may name vendors: the cache rules of their APIs.
const ALLOWED: &[&str] = &["crates/tokprof-core/src/api.rs"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Lines of code and string literals outside test items and comments.
fn code_lines(text: &str) -> Vec<(usize, String)> {
    let test = test_lines(text);
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if test[i] {
            continue;
        }
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let code = code.split(" // ").next().unwrap_or(code);
        out.push((i + 1, code.to_string()));
    }
    out
}

/// For each line of `text`, whether it belongs to an item under `#[cfg(test)]`: the attribute,
/// the attributes after it, and the item up to its closing brace, or up to its `;` when it has no
/// body. Braces in strings, character literals and comments do not count.
fn test_lines(text: &str) -> Vec<bool> {
    let mut out = Vec::new();
    let mut item: Option<Item> = None;
    for line in text.lines() {
        match item.as_mut() {
            Some(it) => {
                out.push(true);
                if it.feed(line) {
                    item = None;
                }
            }
            None => {
                let code = line.trim_start();
                match code.strip_prefix("#[cfg(test)]") {
                    Some(rest) => {
                        out.push(true);
                        let mut it = Item::default();
                        if !it.feed(rest) {
                            item = Some(it);
                        }
                    }
                    None => out.push(false),
                }
            }
        }
    }
    out
}

/// The lexer state of an item being skipped, carried from line to line.
#[derive(Default)]
struct Item {
    /// The depth of `{`, `[` and `(` inside the item.
    depth: usize,
    /// Inside a string: `None` for a plain one, `Some(n)` for a raw one closed by `"` and n `#`.
    string: Option<Option<usize>>,
    /// The depth of block comments.
    comment: usize,
}

impl Item {
    /// Reads one more line of the item; true when the item ends on it.
    fn feed(&mut self, line: &str) -> bool {
        let c: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < c.len() {
            if self.comment > 0 {
                if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    self.comment -= 1;
                    i += 2;
                } else if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    self.comment += 1;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            match self.string {
                Some(None) => {
                    match c[i] {
                        '\\' => i += 1,
                        '"' => self.string = None,
                        _ => {}
                    }
                    i += 1;
                    continue;
                }
                Some(Some(hashes)) => {
                    if c[i] == '"' && (1..=hashes).all(|k| c.get(i + k) == Some(&'#')) {
                        self.string = None;
                        i += hashes;
                    }
                    i += 1;
                    continue;
                }
                None => {}
            }
            let ident_before = |at: usize| at > 0 && (c[at - 1].is_alphanumeric() || c[at - 1] == '_');
            match c[i] {
                '/' if c.get(i + 1) == Some(&'/') => return false,
                '/' if c.get(i + 1) == Some(&'*') => {
                    self.comment += 1;
                    i += 1;
                }
                '"' => self.string = Some(None),
                'r' if !ident_before(i) || (c[i - 1] == 'b' && !ident_before(i - 1)) => {
                    let hashes = c[i + 1..].iter().take_while(|&&h| h == '#').count();
                    if c.get(i + 1 + hashes) == Some(&'"') {
                        self.string = Some(Some(hashes));
                        i += 1 + hashes;
                    }
                }
                // A character literal; a lifetime has no closing quote right after one character.
                '\'' if c.get(i + 1) == Some(&'\\') => {
                    i += 3;
                    while i < c.len() && c[i] != '\'' {
                        i += 1;
                    }
                }
                '\'' if c.get(i + 2) == Some(&'\'') => i += 2,
                '{' | '[' | '(' => self.depth += 1,
                '}' | ']' | ')' => {
                    if self.depth == 0 {
                        return true;
                    }
                    self.depth -= 1;
                    if self.depth == 0 && c[i] == '}' {
                        return true;
                    }
                }
                ';' if self.depth == 0 => return true,
                _ => {}
            }
            i += 1;
        }
        false
    }
}

/// A `mod <name>;` that a file declares, with its `#[path]` and whether it is under `#[cfg(test)]`.
#[derive(Debug, PartialEq)]
struct ModDecl {
    name: String,
    path: Option<String>,
    test: bool,
}

/// The modules `text` declares in files of their own.
fn mod_decls(text: &str) -> Vec<ModDecl> {
    let mut out = Vec::new();
    let mut attrs: Vec<String> = Vec::new();
    for line in text.lines() {
        let mut code = line.trim();
        if code.starts_with("//") {
            continue;
        }
        // The attributes before the item, on lines of their own or on its line.
        while code.starts_with("#[") {
            let Some(mut end) = code.find(']') else { break };
            // The `]` that closes the attribute, past those of any `[...]` inside it.
            while code[..=end].matches('[').count() > code[..=end].matches(']').count() {
                match code[end + 1..].find(']') {
                    Some(next) => end += 1 + next,
                    None => break,
                }
            }
            attrs.push(code[..=end].to_string());
            code = code[end + 1..].trim_start();
        }
        if code.is_empty() {
            continue;
        }
        let item = code.strip_prefix("pub ").or_else(|| code.strip_prefix("pub(crate) ")).unwrap_or(code);
        if let Some(name) = item.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
            let name = name.trim();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                let path = attrs.iter().find_map(|a| {
                    let v = a.strip_prefix("#[path")?.trim_start().strip_prefix('=')?.trim();
                    Some(v.strip_suffix(']')?.trim().trim_matches('"').to_string())
                });
                let test = attrs.iter().any(|a| a == "#[cfg(test)]");
                out.push(ModDecl { name: name.to_string(), path, test });
            }
        }
        attrs.clear();
    }
    out
}

/// The files that may hold the module `decl` declared in `file`.
fn mod_files(file: &Path, decl: &ModDecl) -> Vec<PathBuf> {
    let dir = file.parent().unwrap();
    if let Some(path) = &decl.path {
        return vec![dir.join(path)];
    }
    let stem = file.file_stem().unwrap().to_str().unwrap();
    let base = if ["main", "lib", "mod"].contains(&stem) { dir.to_path_buf() } else { dir.join(stem) };
    vec![base.join(format!("{}.rs", decl.name)), base.join(&decl.name).join("mod.rs")]
}

/// The files of `files` that are test modules: declared under `#[cfg(test)]`, or declared by a
/// file that is one.
fn test_files(files: &[PathBuf]) -> Vec<PathBuf> {
    let decls: Vec<(&PathBuf, Vec<ModDecl>)> = files.iter().map(|f| (f, mod_decls(&std::fs::read_to_string(f).unwrap()))).collect();
    let mut test: Vec<PathBuf> = Vec::new();
    loop {
        let before = test.len();
        for (file, decls) in &decls {
            let in_test = test.contains(file);
            for decl in decls.iter().filter(|d| d.test || in_test) {
                let found = mod_files(file, decl).into_iter().find(|p| p.is_file());
                let found = found.unwrap_or_else(|| panic!("{}: no file for the test module {}", file.display(), decl.name));
                let found = files.iter().find(|f| same_file(f, &found)).cloned().unwrap_or(found);
                if !test.contains(&found) {
                    test.push(found);
                }
            }
        }
        if test.len() == before {
            return test;
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    a.canonicalize().ok().is_some_and(|a| b.canonicalize().ok().is_some_and(|b| a == b))
}

/// The numbers of the lines of `text` that `code_lines` keeps.
fn kept(text: &str) -> Vec<usize> {
    code_lines(text).into_iter().map(|(n, _)| n).collect()
}

#[test]
fn a_test_helper_mid_file_hides_only_itself() {
    let text = "fn a() {}\n#[cfg(test)]\nfn helper() -> u8 {\n    1\n}\nfn b() {}\n";
    assert_eq!(kept(text), [1, 6]);
}

#[test]
fn a_trailing_test_module_is_skipped_to_its_end() {
    let text = "fn a() {}\n#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n    fn t() {\n        if true {\n            a();\n        }\n    }\n}\n";
    assert_eq!(kept(text), [1]);
}

#[test]
fn an_item_without_a_body_ends_at_its_semicolon() {
    let text = "#[cfg(test)]\nconst N: [u8; 2] = [\n    1, 2,\n];\nfn a() {}\n#[cfg(test)]\nmod tests;\nfn b() {}\n#[cfg(test)] use std::fmt;\nfn c() {}\n";
    assert_eq!(kept(text), [5, 8, 10]);
}

#[test]
fn nested_braces_and_braces_in_strings_and_comments_do_not_end_the_item() {
    let text = concat!(
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    const S: &str = \"}}\";\n",
        "    const R: &str = r#\"{\"a\": \"}\"}\"#;\n",
        "    const M: &str = \"\n",
        "    }\n",
        "    \";\n",
        "    const C: char = '}';\n",
        "    const E: char = '\\'';\n",
        "    fn f<'a>(x: &'a str) -> &'a str { x } // }\n",
        "    /* } { */\n",
        "    fn g() { let _ = || { [()] }; }\n",
        "}\n",
        "fn a() {}\n",
    );
    assert_eq!(kept(text), [14]);
}

#[test]
fn attributes_after_cfg_test_belong_to_the_item() {
    let text = "#[cfg(test)]\n#[allow(dead_code)]\n/// A doc comment.\nfn helper() {\n}\nfn a() {}\n";
    assert_eq!(kept(text), [6]);
}

#[test]
fn test_modules_in_files_of_their_own_are_found() {
    let text = "mod a;\n#[cfg(test)]\nmod tests;\n#[cfg(test)] pub(crate) mod more;\n#[path = \"x_tests.rs\"]\nmod x;\n";
    let decl = |name: &str, path: Option<&str>, test| ModDecl { name: name.into(), path: path.map(Into::into), test };
    assert_eq!(mod_decls(text), [decl("a", None, false), decl("tests", None, true), decl("more", None, true), decl("x", Some("x_tests.rs"), false)]);
    let main = Path::new("src").join("main.rs");
    assert_eq!(mod_files(&main, &decl("tests", None, true)), [Path::new("src").join("tests.rs"), Path::new("src").join("tests").join("mod.rs")]);
    let tests = Path::new("src").join("tests.rs");
    assert_eq!(mod_files(&tests, &decl("x", Some("x_tests.rs"), false)), [Path::new("src").join("x_tests.rs")]);
    assert_eq!(mod_files(&tests, &decl("y", None, false))[0], Path::new("src").join("tests").join("y.rs"));
}

#[test]
fn the_framework_names_no_agent() {
    let root = root();
    let mut files = Vec::new();
    for dir in ["crates/tokprof-core/src", "crates/tokprof-logio/src", "crates/tokprof-browse/src", "crates/tokprof/src", "crates/tokprof-desk/src"] {
        rust_files(&root.join(dir), &mut files);
    }
    let tests = test_files(&files);
    let mut found = Vec::new();
    for file in &files {
        let shown = file.strip_prefix(&root).unwrap().display().to_string();
        if ALLOWED.contains(&shown.replace('\\', "/").as_str()) || tests.contains(file) {
            continue;
        }
        for (n, line) in code_lines(&std::fs::read_to_string(file).unwrap()) {
            for word in AGENT_WORDS.iter().filter(|w| line.contains(**w)) {
                found.push(format!("{shown}:{n}: {word}: {line}"));
            }
        }
    }
    assert!(found.is_empty(), "agent knowledge outside sources/:\n{}", found.join("\n"));
}

/// The packages of the workspace at `dir`, without resolving their dependencies, so that nothing
/// is fetched.
fn packages(dir: &Path) -> Vec<serde_json::Value> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo).args(["metadata", "--format-version", "1", "--no-deps"]).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    meta["packages"].as_array().unwrap().clone()
}

#[test]
fn the_packages_depend_as_the_layout_says() {
    let root = root();
    // The window is a workspace of its own, so that this one never fetches Rune.
    let mut all = packages(&root);
    all.extend(packages(&root.join("crates/tokprof-desk")));
    let mut broken = Vec::new();
    for package in &all {
        let name = package["name"].as_str().unwrap();
        // Normal dependencies on other packages of the workspace; dev-dependencies are free.
        let deps: Vec<&str> = package["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["kind"].is_null() && d["path"].is_string())
            .map(|d| d["name"].as_str().unwrap())
            .collect();
        let allowed: &[&str] = match name {
            "tokprof-core" | "tokprof-logio" => &[],
            n if n.starts_with("tokprof-source-") => &["tokprof-core", "tokprof-logio"],
            "tokprof-sources" => &["tokprof-core", "tokprof-source-*"],
            "tokprof-browse" => &["tokprof-core"],
            "tokprof" | "tokprof-desk" => &["tokprof-core", "tokprof-sources", "tokprof-browse"],
            other => panic!("a package the layout does not know: {other}"),
        };
        for dep in deps {
            let ok = allowed.iter().any(|a| *a == dep || (a.ends_with('*') && dep.starts_with(a.trim_end_matches('*'))));
            if !ok {
                broken.push(format!("{name} depends on {dep}"));
            }
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn the_window_is_the_version_of_the_cli() {
    let root = root();
    let version = |dir: &Path, name: &str| {
        let all = packages(dir);
        let package = all.iter().find(|p| p["name"] == name).unwrap_or_else(|| panic!("no package {name}"));
        package["version"].as_str().unwrap().to_string()
    };
    assert_eq!(version(&root, "tokprof"), version(&root.join("crates/tokprof-desk"), "tokprof-desk"));
}

/// A dependency as a manifest writes it: its version requirement, features and default features.
#[derive(Debug, PartialEq)]
struct Dep {
    version: Option<String>,
    features: BTreeSet<String>,
    default_features: bool,
}

/// The dependencies of the manifest `text` in the sections that `section` accepts, by name. An
/// inline table may span lines.
fn manifest_deps(text: &str, section: impl Fn(&str) -> bool) -> BTreeMap<String, Dep> {
    let mut out = BTreeMap::new();
    let mut inside = false;
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            inside = section(line.trim_matches(['[', ']']));
            continue;
        }
        if !inside {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else { continue };
        let mut value = value.trim().to_string();
        while value.matches(['{', '[']).count() > value.matches(['}', ']']).count() {
            let Some(next) = lines.next() else { break };
            value.push(' ');
            value.push_str(next.trim());
        }
        out.insert(name.trim().to_string(), parse_dep(&value));
    }
    out
}

fn parse_dep(value: &str) -> Dep {
    let quoted = |s: &str| s.trim().strip_prefix('"').and_then(|s| s.split_once('"')).map(|(v, _)| v.to_string());
    if !value.starts_with('{') {
        return Dep { version: quoted(value), features: BTreeSet::new(), default_features: true };
    }
    // The value of `key`, and not of a key that ends in it, as `default-features` ends in `features`.
    let field = |key: &str| {
        let (at, _) = value.match_indices(&format!("{key} =")).find(|(at, _)| value[..*at].ends_with(['{', ',', ' ']))?;
        Some(value[at + key.len()..].trim_start().strip_prefix('=').unwrap().trim_start())
    };
    let features = field("features").map(|f| {
        let list = &f[1..f.find(']').unwrap()];
        list.split(',').filter_map(quoted).collect()
    });
    Dep {
        version: field("version").and_then(quoted),
        features: features.unwrap_or_default(),
        default_features: !field("default-features").is_some_and(|v| v.starts_with("false")),
    }
}

#[test]
fn manifests_are_read_with_features_and_default_features() {
    let text = "[workspace.dependencies]\n# A comment.\na = \"1\"\nb = { version = \"0.3\", default-features = false, features = [\"x\", \"y\"] }\nc = { version = \"2\", features = [\n    \"z\",\n] }\n[profile.release]\nd = \"1\"\n";
    let deps = manifest_deps(text, |s| s == "workspace.dependencies");
    let dep = |v: &str, f: &[&str], default_features| Dep { version: Some(v.into()), features: f.iter().map(|s| s.to_string()).collect(), default_features };
    assert_eq!(deps.len(), 3);
    assert_eq!(deps["a"], dep("1", &[], true));
    assert_eq!(deps["b"], dep("0.3", &["x", "y"], false));
    assert_eq!(deps["c"], dep("2", &["z"], true));
}

#[test]
fn the_window_asks_for_the_dependencies_the_workspace_does() {
    let root = root();
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    let shared = manifest_deps(&read("Cargo.toml"), |s| s == "workspace.dependencies");
    let desk = manifest_deps(&read("crates/tokprof-desk/Cargo.toml"), |s| s.ends_with("dependencies"));
    assert!(shared.contains_key("anyhow") && desk.contains_key("anyhow"), "the manifests were not read");
    let mut differ = Vec::new();
    for (name, dep) in &desk {
        if let Some(want) = shared.get(name)
            && want != dep
        {
            differ.push(format!("{name}: the workspace asks for {want:?}, the window for {dep:?}"));
        }
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));
}

/// A package of a lock file and the packages it depends on, each as `name` or `name version`.
struct Locked {
    name: String,
    version: String,
    deps: Vec<String>,
}

fn lock_packages(text: &str) -> Vec<Locked> {
    let mut out: Vec<Locked> = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        if line == "[[package]]" {
            out.push(Locked { name: String::new(), version: String::new(), deps: Vec::new() });
            in_deps = false;
            continue;
        }
        let Some(package) = out.last_mut() else { continue };
        if in_deps {
            if line == "]" {
                in_deps = false;
            } else {
                package.deps.push(line.trim().trim_end_matches(',').trim_matches('"').to_string());
            }
        } else if line == "dependencies = [" {
            in_deps = true;
        } else if let Some(v) = line.strip_prefix("name = ") {
            package.name = v.trim_matches('"').to_string();
        } else if let Some(v) = line.strip_prefix("version = ") {
            package.version = v.trim_matches('"').to_string();
        }
    }
    out
}

/// The versions of every package that `roots` reach in a lock file, by name.
fn lock_closure<'a>(lock: &'a [Locked], roots: &[&str]) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut out: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut todo: Vec<(&str, Option<&str>)> = roots.iter().map(|r| (*r, None)).collect();
    while let Some((name, version)) = todo.pop() {
        let mut found = lock.iter().filter(|p| p.name == name && version.is_none_or(|v| p.version == v));
        let package = found.next().unwrap_or_else(|| panic!("the lock has no {name} {version:?}"));
        assert!(version.is_some() || found.next().is_none(), "{name} is locked at several versions; say which");
        if !out.entry(&package.name).or_default().insert(&package.version) {
            continue;
        }
        for dep in &package.deps {
            let mut parts = dep.split(' ');
            todo.push((parts.next().unwrap(), parts.next()));
        }
    }
    out
}

/// The crates both workspaces build from the same folders.
fn shared_crate(name: &str) -> bool {
    ["tokprof-core", "tokprof-logio", "tokprof-browse", "tokprof-sources"].contains(&name) || name.starts_with("tokprof-source-")
}

#[test]
fn the_window_locks_the_shared_crates_dependencies_as_the_workspace_does() {
    let root = root();
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    let (cli, desk) = (lock_packages(&read("Cargo.lock")), lock_packages(&read("crates/tokprof-desk/Cargo.lock")));
    let roots: Vec<&str> = cli.iter().map(|p| p.name.as_str()).filter(|n| shared_crate(n)).collect();
    assert!(roots.contains(&"tokprof-core"), "the lock was not read");
    let desk_roots: Vec<&str> = roots.iter().copied().filter(|r| desk.iter().any(|p| p.name == *r)).collect();
    let (cli, desk) = (lock_closure(&cli, &roots), lock_closure(&desk, &desk_roots));
    let differ: Vec<String> = cli
        .iter()
        .filter_map(|(name, versions)| {
            let other = desk.get(name)?;
            (other != versions).then(|| format!("{name}: {versions:?} in Cargo.lock, {other:?} in crates/tokprof-desk/Cargo.lock"))
        })
        .collect();
    assert!(
        differ.is_empty(),
        "the CLI and the window would build the shared crates with different dependencies; in crates/tokprof-desk run \
         cargo update -p <name> --precise <the version in Cargo.lock>:\n{}",
        differ.join("\n")
    );
}
