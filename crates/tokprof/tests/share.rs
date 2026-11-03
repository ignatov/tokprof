//! `tokprof share` and `tokprof pool` run as programs, on a home directory that holds one
//! synthetic Claude Code session.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A home directory with one Claude Code transcript of two requests, written today.
fn home(name: &str) -> PathBuf {
    home_with_model(name, |_| "claude-opus-5-5".to_string())
}

/// The same home, with the model its requests name made from the home's path.
fn home_with_model(name: &str, model: impl Fn(&Path) -> String) -> PathBuf {
    let home = std::env::temp_dir().join(format!("tokprof-share-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".claude/projects/-w-p");
    std::fs::create_dir_all(&dir).unwrap();
    let model = model(&home);
    let now = chrono::Utc::now();
    let at = |min: i64| (now - chrono::Duration::minutes(min)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let assistant = |id: &str, min: i64| {
        format!(
            r#"{{"type":"assistant","sessionId":"5e5510f0-0000-0000-0000-000000000000","cwd":"/w/p","entrypoint":"sdk-ts","requestId":"{id}","timestamp":"{}","message":{{"id":"{id}","model":"{model}","content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":5,"cache_read_input_tokens":20000,"cache_creation_input_tokens":1000,"output_tokens":10}}}}}}"#,
            at(min),
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
    assert!(text.contains("\"format\": 2") && text.contains("\"claude\""), "{text}");
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

#[test]
fn share_prints_the_report_of_its_file_then_the_words_to_review() {
    let home = home("prints");
    let target = home.join("out/f.json");
    let out = tokprof(&home, &["--since", "30d", "share", "--survey", "team-2026-11", "-o", target.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    let wrote = stdout.find("wrote ").unwrap();
    let report = stdout.find("share   ").unwrap();
    let words = stdout.find("words to review: claude-opus-5-5, sdk-ts").unwrap_or_else(|| panic!("{stdout}"));
    let nothing = stdout.find("Nothing was sent anywhere").unwrap();
    assert!(wrote < report && report < words && words < nothing, "{stdout}");
    assert!(stdout.contains(" · survey team-2026-11\n") && stdout.contains("Where the money went"), "{stdout}");
    // The report is what `tokprof pool` prints for the file.
    let pooled = tokprof(&home, &["pool", target.to_str().unwrap()]);
    assert!(stdout.contains(&*String::from_utf8_lossy(&pooled.stdout)), "{}", String::from_utf8_lossy(&pooled.stdout));
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_label_with_a_space_fails_before_the_logs_are_read() {
    let home = home("label");
    let out = tokprof(&home, &["share", "--survey", "team 11"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--survey takes 1 to 40 letters, digits, '.', '_' or '-'") && !stderr.contains("read claude"), "{stderr}");
    assert!(!home.join("tokprof").exists());
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_home_path_in_a_model_name_writes_nothing() {
    let home = home_with_model("leak", |home| format!("{}/models/m1", home.display()));
    let out = tokprof(&home, &["share"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("the file would hold the home path in agents.claude.spend; nothing written"), "{stderr}");
    assert!(!home.join("tokprof").exists());
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn the_pool_reads_share_files_and_skips_the_others() {
    let home = home("pool");
    let dir = home.join("survey");
    let a = dir.join("a.json");
    assert!(tokprof(&home, &["share", "-o", a.to_str().unwrap()]).status.success());
    std::fs::copy(&a, dir.join("a (1).json")).unwrap();
    std::fs::write(dir.join("notes.txt"), "not read").unwrap();
    std::fs::write(dir.join("bad.json"), "{}").unwrap();
    std::fs::write(dir.join("newer.json"), r#"{"format": 3, "agents": {}}"#).unwrap();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::copy(&a, dir.join("sub/b.json")).unwrap();
    let out = tokprof(&home, &["pool", dir.to_str().unwrap()]);
    let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{stderr}");
    assert!(stdout.starts_with("share   "), "one file read: {stdout}");
    assert!(stderr.contains("a (1).json: the same as ") && stderr.contains("bad.json: not a tokprof share file") && stderr.contains("newer.json: format 3 needs a newer tokprof"), "{stderr}");
    assert!(!stderr.contains("notes.txt") && !stderr.contains("b.json"), "{stderr}");
    // Two different files make a pool.
    let b = dir.join("b.json");
    assert!(tokprof(&home, &["--since", "30d", "share", "--survey", "team-2026-11", "-o", b.to_str().unwrap()]).status.success());
    let two = tokprof(&home, &["pool", "--json", a.to_str().unwrap(), b.to_str().unwrap()]);
    let v: serde_json::Value = serde_json::from_slice(&two.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&two.stdout)));
    assert_eq!(v["files"].as_array().unwrap().len(), 2);
    assert!(v["tables"][0]["rows"][0]["share"].is_f64(), "{v}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn the_pool_takes_no_filters_and_needs_a_share_file() {
    let home = home("pool-empty");
    let dir = home.join("empty");
    std::fs::create_dir_all(&dir).unwrap();
    let out = tokprof(&home, &["pool", dir.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no share files in "), "{}", String::from_utf8_lossy(&out.stderr));
    let filtered = tokprof(&home, &["--since", "7d", "pool", dir.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&filtered.stderr).contains("pool takes no filters: each file was filtered when it was written"));
    std::fs::remove_dir_all(&home).unwrap();
}
