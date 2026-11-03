//! `tokprof share` run as a program, on a home directory that holds one synthetic Claude Code
//! session.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A home directory with one Claude Code transcript of two requests, written today.
fn home(name: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!("tokprof-share-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".claude/projects/-w-p");
    std::fs::create_dir_all(&dir).unwrap();
    let now = chrono::Utc::now();
    let at = |min: i64| (now - chrono::Duration::minutes(min)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let assistant = |id: &str, min: i64| {
        format!(
            r#"{{"type":"assistant","sessionId":"5e5510f0-0000-0000-0000-000000000000","cwd":"/w/p","entrypoint":"sdk-ts","requestId":"{id}","timestamp":"{}","message":{{"id":"{id}","model":"claude-opus-5-5","content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":5,"cache_read_input_tokens":20000,"cache_creation_input_tokens":1000,"output_tokens":10}}}}}}"#,
            at(min)
        )
    };
    let user = format!(r#"{{"type":"user","sessionId":"5e5510f0-0000-0000-0000-000000000000","cwd":"/w/p","entrypoint":"sdk-ts","timestamp":"{}","message":{{"role":"user","content":"go"}}}}"#, at(3));
    let lines = [user, assistant("m1", 2), assistant("m2", 1)];
    std::fs::write(dir.join("5e5510f0-0000-0000-0000-000000000000.jsonl"), lines.join("\n") + "\n").unwrap();
    home
}

fn tokprof(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokprof"))
        .args(args)
        .env("HOME", home)
        // Where Windows takes the home directory from.
        .env("USERPROFILE", home)
        .env("TOKPROF_DIR", home.join("tokprof"))
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("CODEX_HOME", home.join(".codex"))
        .output()
        .unwrap()
}

#[test]
fn the_file_goes_to_the_profile_directory_by_default() {
    let home = home("default");
    let out = tokprof(&home, &["share"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    let name = format!("share-{}.json", chrono::Local::now().format("%Y-%m-%d"));
    let sep = std::path::MAIN_SEPARATOR;
    assert!(stdout.starts_with(&format!("wrote ~{sep}tokprof{sep}{name}\n")), "{stdout}");
    assert!(stdout.ends_with("Nothing was sent anywhere: review the file, then send it yourself.\n"), "{stdout}");
    let text = std::fs::read_to_string(home.join("tokprof").join(&name)).unwrap();
    assert!(text.contains("\"format\": 1") && text.contains("\"claude\""), "{text}");
    assert!(!text.contains("5e5510f0") && !text.contains("/w/p"), "{text}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn the_filters_pick_the_requests_and_the_window() {
    let home = home("filters");
    let target = home.join("out/pilot.json");
    let out = tokprof(&home, &["--agent", "claude", "--since", "30d", "share", "-o", target.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&target).unwrap();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert!(text.contains(&format!("\"first\": \"{today}\"")), "{text}");
    assert!(text.contains("\"requests\": 2"), "{text}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn no_selected_request_writes_no_file() {
    let home = home("none");
    let out = tokprof(&home, &["--since", "2099-01-01", "share"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no requests found for these filters; nothing written"), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!home.join("tokprof").exists());
    std::fs::remove_dir_all(&home).unwrap();
}
