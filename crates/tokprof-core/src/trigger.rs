//! What made an agent send a request: a prompt, the results of the previous response's calls,
//! a background task, a compaction. Sources classify each call; the rules here pick the trigger.

/// What made the agent send a request, so that spend can be charged to the reason for another
/// pass over the context.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Trigger {
    /// A message the user typed, or a subagent's brief.
    Prompt,
    /// The result of one read-only call.
    Lookup,
    /// The results of several calls, all read-only.
    Lookups,
    Edit,
    Subagent,
    /// Any other call: a command that is not read-only, a script, an MCP tool.
    Run,
    /// A background task's result.
    Notification,
    Compaction,
    /// The agent went on after an interruption, or after its own message.
    Continuation,
    Advisor,
    #[default]
    Unknown,
}

impl Trigger {
    pub const ALL: [Trigger; 11] = [
        Trigger::Prompt,
        Trigger::Lookup,
        Trigger::Lookups,
        Trigger::Edit,
        Trigger::Subagent,
        Trigger::Run,
        Trigger::Notification,
        Trigger::Compaction,
        Trigger::Continuation,
        Trigger::Advisor,
        Trigger::Unknown,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Trigger::Prompt => "prompt",
            Trigger::Lookup => "lookup",
            Trigger::Lookups => "lookups",
            Trigger::Edit => "edit",
            Trigger::Subagent => "subagent",
            Trigger::Run => "run",
            Trigger::Notification => "notification",
            Trigger::Compaction => "compaction",
            Trigger::Continuation => "continuation",
            Trigger::Advisor => "advisor",
            Trigger::Unknown => "unknown",
        }
    }

    /// The trigger the calls of a response give the request after it: an edit counts before a
    /// subagent, a subagent before a run, a run before lookups. `None` for no calls.
    pub fn of_calls(calls: &[CallClass]) -> Option<Trigger> {
        let has = |k| calls.contains(&k);
        Some(if calls.is_empty() {
            return None;
        } else if has(CallClass::Editing) {
            Trigger::Edit
        } else if has(CallClass::Subagents) {
            Trigger::Subagent
        } else if calls.iter().any(|c| !c.read_only()) {
            Trigger::Run
        } else if calls.len() == 1 {
            Trigger::Lookup
        } else {
            Trigger::Lookups
        })
    }

    /// Whether the calls of the previous response gave this trigger.
    pub fn from_calls(self) -> bool {
        matches!(self, Trigger::Lookup | Trigger::Lookups | Trigger::Edit | Trigger::Subagent | Trigger::Run)
    }
}

impl std::fmt::Display for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// What one tool call did: the activity of the request after it, and through it the trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CallClass {
    Editing,
    Subagents,
    Building,
    VersionControl,
    Scripts,
    System,
    OtherTools,
    Web,
    Reading,
}

impl CallClass {
    /// The order in which a response's calls give its activity: the first class present wins.
    pub const PRECEDENCE: [CallClass; 9] = [
        CallClass::Editing,
        CallClass::Subagents,
        CallClass::Building,
        CallClass::VersionControl,
        CallClass::Scripts,
        CallClass::System,
        CallClass::OtherTools,
        CallClass::Web,
        CallClass::Reading,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CallClass::Editing => "editing",
            CallClass::Subagents => "subagents",
            CallClass::Building => "building and testing",
            CallClass::VersionControl => "version control",
            CallClass::Scripts => "running scripts",
            CallClass::System => "file and system commands",
            CallClass::OtherTools => "other tools",
            CallClass::Web => "web",
            CallClass::Reading => "reading",
        }
    }

    /// Reads, searches and fetches change nothing: they give `lookup` triggers.
    pub fn read_only(self) -> bool {
        matches!(self, CallClass::Reading | CallClass::Web)
    }

    /// The activity of the calls of a response: the first class in [`CallClass::PRECEDENCE`].
    pub fn of_calls(calls: &[CallClass]) -> Option<CallClass> {
        CallClass::PRECEDENCE.into_iter().find(|c| calls.contains(c))
    }
}

/// What a source has seen in a chain since its latest response: the calls of that response,
/// and the latest prompt, notification or interruption logged after them, which wins.
#[derive(Clone, Debug, Default)]
pub struct Pending {
    calls: Vec<CallClass>,
    event: Option<Trigger>,
}

impl Pending {
    /// A call of the latest response.
    pub fn call(&mut self, kind: CallClass) {
        self.calls.push(kind);
    }

    /// A typed prompt, a notification or an interruption: it replaces everything before it.
    pub fn event(&mut self, trigger: Trigger) {
        self.calls.clear();
        self.event = Some(trigger);
    }

    /// The trigger of the next request, if the chain logged anything that tells.
    pub fn trigger(&self) -> Option<Trigger> {
        self.event.or_else(|| Trigger::of_calls(&self.calls))
    }

    /// The activity of the next request when its trigger comes from calls.
    pub fn activity(&self) -> Option<CallClass> {
        if self.event.is_some() { None } else { CallClass::of_calls(&self.calls) }
    }

    pub fn has_calls(&self) -> bool {
        !self.calls.is_empty()
    }
}

/// Programs that only read, search or print.
const READ_ONLY_PROGRAMS: &[&str] = &[
    "cat", "head", "tail", "nl", "less", "sed", "awk", "grep", "rg", "ag", "ls", "tree", "find", "fd", "wc", "stat", "file", "du", "pwd", "echo", "printf",
    "sort", "uniq", "cut", "tr", "column", "paste", "comm", "jq", "diff", "basename", "dirname", "realpath", "which", "zcat", "zstdcat", "xxd", "test", "[",
    "true", "read",
];

/// Git subcommands that only read.
const READ_ONLY_GIT: &[&str] =
    &["show", "log", "diff", "status", "grep", "ls-files", "blame", "rev-parse", "cat-file", "rev-list", "merge-base", "shortlog", "describe"];

/// Shell keywords that open or close a block; the command after an opening one is checked.
const KEYWORDS: &[&str] = &["do", "then", "else", "if", "elif", "while", "until", "!"];

/// Whether a shell command only reads: every part starts with a read-only program and nothing
/// writes to a file. What the rule cannot read counts as not read-only.
pub fn read_only_command(cmd: &str) -> bool {
    let split = split(cmd);
    split.clean() && !split.parts.is_empty() && split.parts.iter().all(|p| read_only_part(p))
}

/// Programs that build or test.
const BUILD_PROGRAMS: &[&str] =
    &["cargo", "npm", "pnpm", "yarn", "gradle", "gradlew", "mvn", "make", "bazel", "go", "pytest", "tsc", "jest", "vitest", "swift", "xcodebuild", "dotnet"];

/// Interpreters that run scripts.
const SCRIPT_PROGRAMS: &[&str] = &["python", "python3", "node", "deno", "ruby", "perl", "bash", "sh", "zsh", "uv"];

/// Common programs that the lists above leave out, which a share file keeps by name.
const COMMON_PROGRAMS: &[&str] = &[
    "mkdir",
    "rm",
    "cp",
    "mv",
    "ln",
    "touch",
    "chmod",
    "curl",
    "wget",
    "tar",
    "unzip",
    "zip",
    "xargs",
    "sleep",
    "kill",
    "pkill",
    "ps",
    "lsof",
    "date",
    "open",
    "xdg-open",
    "ssh",
    "scp",
    "rsync",
    "docker",
    "kubectl",
    "brew",
    "pip",
    "pip3",
    "npx",
    "java",
    "javac",
    "kotlinc",
    "rustc",
    "rustup",
    "cmake",
    "clang",
    "gcc",
    "ffmpeg",
    "sqlite3",
    "psql",
    "osascript",
];

/// The words a command can start with that frame or wrap the programs it runs: shell keywords,
/// `cd`, and the wrappers that the class of a command skips.
const FRAMES: &[&str] = &["for", "while", "until", "if", "case", "do", "done", "then", "elif", "else", "fi", "!", "cd", "time", "timeout", "env", "nice"];

/// A word without its path and without a `.cmd` or `.exe` suffix.
fn bare(word: &str) -> &str {
    let word = word.rsplit('/').next().unwrap_or(word);
    word.strip_suffix(".exe").or_else(|| word.strip_suffix(".cmd")).unwrap_or(word)
}

/// Whether a share file keeps a program by name: the programs the rules of this module name,
/// the common ones, shell keywords and wrappers. A program given with a path or a `.cmd` or
/// `.exe` suffix counts as the program.
pub fn known_program(word: &str) -> bool {
    let word = bare(word);
    [READ_ONLY_PROGRAMS, BUILD_PROGRAMS, SCRIPT_PROGRAMS, COMMON_PROGRAMS, FRAMES].iter().any(|list| list.contains(&word))
        || matches!(word, "git" | "gh" | "bun")
}

/// The program a share file files a shell command head under: its first word without a path
/// or suffix when [`known_program`] keeps it, `other` otherwise.
pub fn program_of(head: &str) -> &str {
    let word = bare(head.split_whitespace().next().unwrap_or(""));
    if known_program(word) { word } else { "other" }
}

/// Every program [`known_program`] keeps, for the words a share file may hold.
pub fn known_programs() -> impl Iterator<Item = &'static str> {
    [READ_ONLY_PROGRAMS, BUILD_PROGRAMS, SCRIPT_PROGRAMS, COMMON_PROGRAMS, FRAMES, &["git", "gh", "bun"]].into_iter().flatten().copied()
}

/// What a shell command does: it only reads; it writes a file with a program that reads or
/// prints; or the class of its first program.
pub fn command_class(cmd: &str) -> CallClass {
    if read_only_command(cmd) {
        return CallClass::Reading;
    }
    let split = split(cmd);
    let Some((path, program, args)) = split.parts.iter().find_map(|p| first_program(p)) else { return CallClass::System };
    let in_place = (program == "sed" && sed_in_place(&args))
        || (program == "perl" && args.iter().any(|a| a.starts_with("-i") || (a.starts_with("-p") && a.contains('i'))));
    let tee = split.parts.iter().any(|p| first_program(p).is_some_and(|(_, prog, _)| prog == "tee"));
    if (split.writes || split.heredoc || tee || in_place)
        && (READ_ONLY_PROGRAMS.contains(&program.as_str()) || matches!(program.as_str(), "tee" | "sed" | "perl"))
    {
        return CallClass::Editing;
    }
    match program.as_str() {
        "bun" if args.first().is_some_and(|a| a == "-e") => CallClass::Scripts,
        "bun" => CallClass::Building,
        "git" | "gh" => CallClass::VersionControl,
        p if BUILD_PROGRAMS.contains(&p) => CallClass::Building,
        p if SCRIPT_PROGRAMS.contains(&p) => CallClass::Scripts,
        _ if path => CallClass::Scripts,
        _ => CallClass::System,
    }
}

/// The program a part starts with, after keywords, assignments and wrappers: whether it was
/// given by a path, its name without path or `.cmd`/`.exe` suffix, and its arguments.
fn first_program(part: &str) -> Option<(bool, String, Vec<String>)> {
    let mut words: Vec<&str> = part.split_whitespace().collect();
    loop {
        let &w = words.first()?;
        let assignment = w.split_once('=').is_some_and(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        if KEYWORDS.contains(&w) || assignment || matches!(w, "time" | "env" | "nice") {
            words.remove(0);
        } else if w == "timeout" {
            words.drain(..words.len().min(2));
        } else if w.starts_with('-') && words.len() > 1 {
            // An option of a wrapper, such as `env -i` or `nice -n`.
            words.remove(0);
        } else {
            break;
        }
    }
    let first = words[0];
    if matches!(first, "cd" | "for" | "done" | "fi" | "{" | "}" | "(" | ")") {
        return None;
    }
    let name = first.rsplit('/').next().unwrap_or(first);
    let name = name.strip_suffix(".cmd").or_else(|| name.strip_suffix(".exe")).unwrap_or(name);
    Some((first.contains('/'), name.to_string(), words[1..].iter().map(|w| w.to_string()).collect()))
}

/// What a shell command does to version control, in any of its parts: a commit, a push, a pull
/// request opened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VcsActions {
    pub commit: bool,
    pub push: bool,
    pub pull_request: bool,
}

impl VcsActions {
    pub fn any(self) -> bool {
        self.commit || self.push || self.pull_request
    }
}

/// The options of `git` before its subcommand whose value is the next word.
const GIT_VALUE_OPTIONS: [&str; 5] = ["-C", "-c", "--git-dir", "--work-tree", "--namespace"];

/// The version-control actions of a shell command: `git commit`, `git push` and `gh pr create`
/// in any of its parts, after `git`'s options before the subcommand.
pub fn vcs_actions(cmd: &str) -> VcsActions {
    let mut a = VcsActions::default();
    // A here-string's word is data too, and `split` reads `<<` as the start of a heredoc.
    for part in &split(&without_heredoc_bodies(cmd).replace("<<<", " ")).parts {
        let Some((_, program, args)) = first_program(part) else { continue };
        match program.as_str() {
            "git" => {
                let mut i = 0;
                while args.get(i).is_some_and(|w| w.starts_with('-')) {
                    i += if GIT_VALUE_OPTIONS.contains(&args[i].as_str()) { 2 } else { 1 };
                }
                match args.get(i).map(String::as_str) {
                    Some("commit") => a.commit = true,
                    Some("push") => a.push = true,
                    _ => {}
                }
            }
            "gh" if args.first().is_some_and(|w| w == "pr") && args.get(1).is_some_and(|w| w == "create") => a.pull_request = true,
            _ => {}
        }
    }
    a
}

/// A command without the bodies of its heredocs, which are data: the text before each `<<`, the
/// rest of that line after the delimiter, and what follows the line that ends the body.
fn without_heredoc_bodies(cmd: &str) -> String {
    let mut out = String::new();
    let mut rest = cmd;
    while let Some(at) = heredoc_at(rest) {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let after = after.strip_prefix('-').unwrap_or(after).trim_start_matches([' ', '\t']);
        let word: String = after.chars().take_while(|c| !c.is_whitespace() && !matches!(c, ';' | '|' | '&' | ')')).collect();
        let delimiter = word.trim_matches(['\'', '"']);
        let line = &after[word.len()..];
        let Some(end) = line.find('\n') else {
            out.push_str(line);
            return out;
        };
        out.push_str(&line[..end]);
        out.push('\n');
        let body = &line[end + 1..];
        let mut at_end = None;
        let mut pos = 0;
        for l in body.split_inclusive('\n') {
            pos += l.len();
            if l.trim() == delimiter {
                at_end = Some(pos);
                break;
            }
        }
        match at_end {
            Some(p) => rest = &body[p..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Where the next heredoc's `<<` stands, outside quotes; a here-string's `<<<` is none.
fn heredoc_at(cmd: &str) -> Option<usize> {
    let b = cmd.as_bytes();
    let (mut single, mut double) = (false, false);
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' if !single => i += 1,
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'<' if !single && !double && b.get(i + 1) == Some(&b'<') => {
                if b.get(i + 2) == Some(&b'<') {
                    i += 3;
                    continue;
                }
                return Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// `-i`, `-i.bak`, `-ni` or `--in-place` make `sed` edit the file.
fn sed_in_place<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().map(AsRef::as_ref).any(|a: &str| {
        a.starts_with("--in-place") || (a.starts_with('-') && !a.starts_with("--") && a[1..].chars().take_while(char::is_ascii_alphabetic).any(|c| c == 'i'))
    })
}

/// A command split at `&&`, `||`, `;`, `|`, `&` and line breaks outside quotes, with what the
/// split saw on the way.
struct Split {
    parts: Vec<String>,
    /// `>` or `>>` to a file other than `/dev/null`, `&1` or `&2`.
    writes: bool,
    heredoc: bool,
    /// `$(…)` or a backquote.
    substitution: bool,
    unclosed: bool,
}

impl Split {
    fn clean(&self) -> bool {
        !(self.writes || self.heredoc || self.substitution || self.unclosed)
    }
}

fn split(cmd: &str) -> Split {
    let chars: Vec<char> = cmd.chars().collect();
    let mut out = Split { parts: Vec::new(), writes: false, heredoc: false, substitution: false, unclosed: false };
    let mut parts = Vec::new();
    let mut cur = String::new();
    let (mut single, mut double) = (false, false);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if single {
            single = c != '\'';
            cur.push(c);
            i += 1;
            continue;
        }
        match c {
            '\\' => {
                cur.push(c);
                if let Some(n) = next {
                    cur.push(n);
                }
                i += 2;
                continue;
            }
            '\'' if !double => single = true,
            '"' => double = !double,
            '`' => out.substitution = true,
            '$' if next == Some('(') => out.substitution = true,
            _ if double => {}
            '<' if next == Some('<') => {
                // The heredoc's body is data, not commands.
                out.heredoc = true;
                parts.push(std::mem::take(&mut cur));
                break;
            }
            '>' => {
                let rest: String = chars[i + 1..].iter().collect();
                let rest = rest.trim_start_matches('>').trim_start();
                let quiet = rest.starts_with("/dev/null") || rest.starts_with("&1") || rest.starts_with("&2");
                out.writes |= !quiet;
            }
            ';' | '|' | '&' | '\n' => {
                // `2>&1` and `&>` belong to a redirect, not a separator.
                let redirect = c == '&' && (cur.ends_with('>') || next == Some('>'));
                if !redirect {
                    parts.push(std::mem::take(&mut cur));
                    i += 1;
                    while i < chars.len() && matches!(chars[i], '&' | '|') {
                        i += 1;
                    }
                    continue;
                }
            }
            _ => {}
        }
        cur.push(c);
        i += 1;
    }
    out.unclosed = single || double;
    parts.push(cur);
    out.parts = parts.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
    out
}

fn read_only_part(part: &str) -> bool {
    let mut words: Vec<&str> = part.split_whitespace().collect();
    while let Some(w) = words.first() {
        let assignment = w.split_once('=').is_some_and(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        if KEYWORDS.contains(w) || assignment {
            words.remove(0);
        } else {
            break;
        }
    }
    let Some(&first) = words.first() else { return true };
    if matches!(first, "done" | "fi" | "cd" | "{" | "}" | "(" | ")") {
        return true;
    }
    if first == "for" {
        return words.get(2) == Some(&"in") || words.len() <= 2;
    }
    let program = first.rsplit('/').next().unwrap_or(first);
    let program = program.strip_suffix(".cmd").or_else(|| program.strip_suffix(".exe")).unwrap_or(program);
    let args = &words[1..];
    match program {
        "git" => git_reads(args),
        "sed" => !sed_in_place(args),
        "find" => !args.iter().any(|a| matches!(*a, "-delete" | "-exec" | "-execdir" | "-ok" | "-okdir" | "-fprint" | "-fprint0" | "-fprintf" | "-fls")),
        p => READ_ONLY_PROGRAMS.contains(&p),
    }
}

/// `git [-C <dir>] [-c <k=v>] [--no-pager] <subcommand>` with a subcommand that only reads.
fn git_reads(args: &[&str]) -> bool {
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-C" | "-c" => i += 2,
            a if a.starts_with('-') => i += 1,
            sub => return READ_ONLY_GIT.contains(&sub),
        }
    }
    false
}

#[cfg(test)]
mod vcs_tests {
    use super::*;

    fn acts(cmd: &str) -> (bool, bool, bool) {
        let a = vcs_actions(cmd);
        (a.commit, a.push, a.pull_request)
    }

    #[test]
    fn a_commit_in_a_chain() {
        assert_eq!(acts("git add -A && git commit -m \"fix\""), (true, false, false));
        assert_eq!(acts("cd walrus && git commit -am wip"), (true, false, false));
    }

    #[test]
    fn a_commit_and_a_push() {
        assert_eq!(acts("git commit -m x && git push"), (true, true, false));
        assert_eq!(acts("git commit -m \"a && git push\""), (true, false, false), "a quoted && is no part");
    }

    #[test]
    fn options_before_the_subcommand() {
        assert_eq!(acts("git -C ../zebra commit -m x"), (true, false, false));
        assert_eq!(acts("git -c user.name=x commit -m y"), (true, false, false));
        assert_eq!(acts("git --git-dir=.g push origin main"), (false, true, false));
        assert_eq!(acts("GIT_AUTHOR_NAME=x git commit -m y"), (true, false, false));
    }

    #[test]
    fn heredoc_bodies_are_data() {
        let file = "cat > notes.md <<'EOF'\ngit push and git commit are words here\nEOF\ngit add notes.md && git commit -m x";
        assert_eq!(acts(file), (true, false, false), "the commit after the body counts, the words in it do not");
        let message = "git commit -q -F - <<'EOF'\nSay why, not git push\nEOF\ngit push origin main";
        assert_eq!(acts(message), (true, true, false));
        let tabs = "cat <<-END > x\n\tgh pr create\n\tEND\ngh pr create --fill";
        assert_eq!(acts(tabs), (false, false, true));
        assert_eq!(acts("cat <<'EOF'\ngit commit"), (false, false, false), "a body that never ends");
        assert_eq!(acts("grep x <<< \"git push\" && git commit -m y"), (true, false, false), "a here-string is no heredoc");
    }

    #[test]
    fn a_pull_request() {
        assert_eq!(acts("gh pr create --fill"), (false, false, true));
        assert_eq!(acts("gh pr view 12"), (false, false, false));
    }

    #[test]
    fn other_commands_do_nothing() {
        for cmd in ["git status", "git log --oneline", "echo git commit", "git", "", "commit -m x"] {
            assert_eq!(acts(cmd), (false, false, false), "{cmd}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_file_keeps_common_programs_by_name() {
        for p in [
            "mkdir",
            "rm",
            "cp",
            "mv",
            "ln",
            "touch",
            "chmod",
            "curl",
            "wget",
            "tar",
            "unzip",
            "zip",
            "xargs",
            "sleep",
            "kill",
            "pkill",
            "ps",
            "lsof",
            "date",
            "open",
            "xdg-open",
            "ssh",
            "scp",
            "rsync",
            "docker",
            "kubectl",
            "brew",
            "pip",
            "pip3",
            "npx",
            "java",
            "javac",
            "kotlinc",
            "rustc",
            "rustup",
            "cmake",
            "clang",
            "gcc",
            "ffmpeg",
            "sqlite3",
            "psql",
            "osascript",
        ] {
            assert_eq!(program_of(&format!("{p} -x")), p);
        }
    }

    #[test]
    fn programs_of_the_other_lists_keep_their_names_without_a_path() {
        let heads = ["sed -n", "python3 -I", "git grep", "/usr/bin/grep -n", "for f", "rg.cmd -n", "timeout 60", "cargo test"];
        assert_eq!(heads.map(program_of), ["sed", "python3", "git", "grep", "for", "rg", "timeout", "cargo"]);
    }

    #[test]
    fn the_users_own_scripts_are_other() {
        assert_eq!(program_of("./yak-deploy.sh --fast"), "other");
        assert_eq!(program_of("zebra-build all"), "other");
        assert_eq!(program_of(""), "other");
    }

    #[test]
    fn read_only_programs() {
        for cmd in [
            "cat src/main.rs",
            "sed -n 1,80p src/lib.rs",
            "rg -n 'fn main' crates",
            "find . -name '*.rs'",
            "wc -l src/*.rs",
            "ls -la",
            "jq .version package.json",
            "rg.cmd -n foo",
            "echo ---",
        ] {
            assert!(read_only_command(cmd), "{cmd}");
        }
    }

    #[test]
    fn git_that_reads() {
        assert!(read_only_command("git show HEAD:src/lib.rs"));
        assert!(read_only_command("git -C /repo log --oneline -5"));
        assert!(read_only_command("git --no-pager diff --stat"));
        assert!(!read_only_command("git commit -m x"));
        assert!(!read_only_command("git checkout main"));
        assert!(!read_only_command("git -C /repo push"));
    }

    #[test]
    fn a_filtered_read() {
        assert!(read_only_command("cd /repo && git show HEAD:src/lib.rs | sed -n 1,120p 2>/dev/null"));
        assert!(read_only_command("grep -rn foo src 2>&1 | head -20"));
        assert!(read_only_command("LC_ALL=C sort a.txt | uniq -c"));
        assert!(read_only_command("grep -n 'a|b;c' src/x.rs"));
    }

    #[test]
    fn a_write_hidden_in_a_chain() {
        assert!(!read_only_command("cat a.txt > b.txt"));
        assert!(!read_only_command("grep -n foo src | tee hits.txt"));
        assert!(!read_only_command("echo x >> log.txt"));
        assert!(!read_only_command("sed -i 's/a/b/' f.rs"));
        assert!(!read_only_command("sed -i.bak 's/a/b/' f.rs"));
        assert!(!read_only_command("find . -name '*.tmp' -delete"));
        assert!(!read_only_command("find . -name x -exec rm {} ;"));
        assert!(!read_only_command("ls && rm -rf build"));
    }

    #[test]
    fn scripts_substitutions_and_heredocs() {
        assert!(!read_only_command("python3 -I script.py"));
        assert!(!read_only_command("node -e 'console.log(1)'"));
        assert!(!read_only_command("cargo test"));
        assert!(!read_only_command("cat <<'EOF' > x.py\nprint(1)\nEOF"));
        assert!(!read_only_command("cat $(git ls-files)"));
        assert!(!read_only_command("echo `date`"));
        assert!(!read_only_command("echo \"$(rm -rf x)\""));
        assert!(!read_only_command(""));
    }

    #[test]
    fn loops_and_conditions_around_reads() {
        assert!(read_only_command("for f in a.rs b.rs; do echo \"== $f\"; sed -n 1,20p \"$f\"; done"));
        assert!(read_only_command("if [ -f Cargo.toml ]; then cat Cargo.toml; fi"));
        assert!(!read_only_command("for f in *.rs; do rm \"$f\"; done"));
        assert!(!read_only_command("while read f; do python3 x.py \"$f\"; done < list.txt"));
    }

    #[test]
    fn quotes_hide_separators_and_redirects() {
        assert!(read_only_command("awk '{ if ($1 > 3) print $2 }' data.tsv"));
        assert!(read_only_command("echo 'a > b'"));
        assert!(!read_only_command("echo 'unclosed"));
    }

    #[test]
    fn the_calls_of_a_response_give_the_trigger() {
        use CallClass::*;
        assert_eq!(Trigger::of_calls(&[]), None);
        assert_eq!(Trigger::of_calls(&[Reading]), Some(Trigger::Lookup));
        assert_eq!(Trigger::of_calls(&[Reading, Web]), Some(Trigger::Lookups));
        assert_eq!(Trigger::of_calls(&[Reading, Building, Editing]), Some(Trigger::Edit));
        assert_eq!(Trigger::of_calls(&[Scripts, Subagents]), Some(Trigger::Subagent));
        assert_eq!(Trigger::of_calls(&[Reading, System]), Some(Trigger::Run));
        for (class, trigger) in [
            (Editing, Trigger::Edit),
            (Subagents, Trigger::Subagent),
            (Building, Trigger::Run),
            (VersionControl, Trigger::Run),
            (Scripts, Trigger::Run),
            (System, Trigger::Run),
            (OtherTools, Trigger::Run),
            (Web, Trigger::Lookup),
            (Reading, Trigger::Lookup),
        ] {
            assert_eq!(Trigger::of_calls(&[class]), Some(trigger), "{class:?}");
        }
    }

    #[test]
    fn shell_commands_by_their_program() {
        use CallClass::*;
        for (cmd, class) in [
            ("cargo test --workspace", Building),
            ("cd /repo && RUST_LOG=1 cargo build --release", Building),
            ("timeout 600 ./gradlew test", Building),
            ("/opt/tools/bun.cmd run build", Building),
            ("bun -e 'console.log(1)'", Scripts),
            ("bazel.cmd build //x", Building),
            ("git commit -m x", VersionControl),
            ("gh pr view 12", VersionControl),
            ("python3 -I - <<'EOF'\nprint(1)\nEOF", Scripts),
            ("scripts/report --since 7d", Scripts),
            ("time node x.js", Scripts),
            ("cat > notes.md <<'EOF'\n# x\nEOF", Editing),
            ("echo x >> log.txt", Editing),
            ("sed -i 's/a/b/' f.rs", Editing),
            ("printf x | tee out.txt", Editing),
            ("mkdir -p /tmp/x", System),
            ("rm -rf build", System),
            ("curl -s https://example.com", System),
            ("sed -n 1,20p f.rs", Reading),
            ("for f in a b; do cargo test -p $f; done", Building),
        ] {
            assert_eq!(command_class(cmd), class, "{cmd}");
        }
    }

    #[test]
    fn the_first_class_in_order_gives_the_activity() {
        use CallClass::*;
        assert_eq!(CallClass::of_calls(&[Reading, Building, Reading]), Some(Building));
        assert_eq!(CallClass::of_calls(&[Web, Reading]), Some(Web));
        assert_eq!(CallClass::of_calls(&[]), None);
        let mut p = Pending::default();
        p.call(Editing);
        assert_eq!(p.activity(), Some(Editing));
        p.event(Trigger::Prompt);
        assert_eq!(p.activity(), None);
    }

    #[test]
    fn advisor_calls_and_compactions_say_so_themselves() {
        use crate::model::Request;
        let seen = Request { trigger: Some(Trigger::Lookup), ..Request::default() };
        assert_eq!(seen.trigger(), Trigger::Lookup);
        assert_eq!(Request { compacted_before: true, ..seen.clone() }.trigger(), Trigger::Compaction);
        assert_eq!(Request { compacted_before: true, advisor: true, ..seen.clone() }.trigger(), Trigger::Advisor);
        assert_eq!(Request::default().trigger(), Trigger::Unknown);
    }

    #[test]
    fn the_activity_follows_the_trigger() {
        use crate::model::Request;
        let built = Request { trigger: Some(Trigger::Run), activity: Some(CallClass::Building), ..Request::default() };
        assert_eq!(built.activity(), "building and testing");
        assert_eq!(Request { trigger: Some(Trigger::Prompt), ..Request::default() }.activity(), "prompt");
        assert_eq!(Request { advisor: true, ..built.clone() }.activity(), "advisor");
        assert_eq!(Request { compacted_before: true, ..built }.activity(), "compaction");
    }

    #[test]
    fn the_latest_event_wins() {
        let mut p = Pending::default();
        p.call(CallClass::Reading);
        assert_eq!(p.trigger(), Some(Trigger::Lookup));
        p.event(Trigger::Continuation);
        p.event(Trigger::Prompt);
        assert_eq!(p.trigger(), Some(Trigger::Prompt));
        assert!(Pending::default().trigger().is_none());
    }
}
