//! `tokprof share` and `tokprof pool` run as programs, on a home directory that holds one
//! synthetic Claude Code session.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

/// A home directory with one Claude Code transcript of two requests, written just before the
/// latest noon that has passed (see `written_at`).
fn home(name: &str) -> PathBuf {
    home_with_model(name, |_| "claude-opus-5-5".to_string())
}

/// The date of a moment in the zone tokprof runs in under `common::tokprof`: UTC, as its `TZ`
/// says, except on Windows, which ignores `TZ` and keeps the machine's zone.
fn tokprof_date(t: chrono::DateTime<chrono::Utc>) -> chrono::NaiveDate {
    if cfg!(windows) { t.with_timezone(&chrono::Local).date_naive() } else { t.date_naive() }
}

/// When the transcript's requests end: the latest 12:00 in tokprof's zone at least five minutes
/// ago, so that the requests are in the past, on one date, whatever the time of the test.
fn written_at() -> chrono::DateTime<chrono::Utc> {
    let now = chrono::Utc::now();
    let noon = |date: chrono::NaiveDate| {
        let t = date.and_hms_opt(12, 0, 0).unwrap();
        if cfg!(windows) { t.and_local_timezone(chrono::Local).earliest().unwrap().with_timezone(&chrono::Utc) } else { t.and_utc() }
    };
    let today = noon(tokprof_date(now));
    if today + chrono::Duration::minutes(5) <= now { today } else { noon(tokprof_date(now) - chrono::Duration::days(1)) }
}

/// The same home, with the model its requests name made from the home's path.
fn home_with_model(name: &str, model: impl Fn(&Path) -> String) -> PathBuf {
    let home = std::env::temp_dir().join(format!("tokprof-share-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".claude/projects/-w-p");
    std::fs::create_dir_all(&dir).unwrap();
    // As a JSON string: a Windows home path holds `\`, which JSON escapes.
    let model = serde_json::to_string(&model(&home)).unwrap();
    let end = written_at();
    let at = |min: i64| (end - chrono::Duration::minutes(min)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let assistant = |id: &str, min: i64| {
        format!(
            r#"{{"type":"assistant","sessionId":"5e5510f0-0000-0000-0000-000000000000","cwd":"/w/p","entrypoint":"sdk-ts","requestId":"{id}","timestamp":"{}","message":{{"id":"{id}","model":{model},"content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":5,"cache_read_input_tokens":20000,"cache_creation_input_tokens":1000,"output_tokens":10}}}}}}"#,
            at(min),
        )
    };
    let user = format!(
        r#"{{"type":"user","sessionId":"5e5510f0-0000-0000-0000-000000000000","cwd":"/w/p","entrypoint":"sdk-ts","timestamp":"{}","message":{{"role":"user","content":"go"}}}}"#,
        at(3)
    );
    let lines = [user, assistant("m1", 2), assistant("m2", 1)];
    std::fs::write(dir.join("5e5510f0-0000-0000-0000-000000000000.jsonl"), lines.join("\n") + "\n").unwrap();
    home
}

fn tokprof(home: &Path, args: &[&str]) -> Output {
    common::tokprof(home).args(args).output().unwrap()
}

#[test]
fn the_file_goes_to_the_profile_directory_by_default() {
    let home = home("default");
    // The file is named for the date tokprof writes it on, in its zone: the date before the run or
    // after it, when the run crosses midnight.
    let before = tokprof_date(chrono::Utc::now());
    let out = tokprof(&home, &["share"]);
    let after = tokprof_date(chrono::Utc::now());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    let sep = std::path::MAIN_SEPARATOR;
    let name = [before, after]
        .map(|d| format!("share-{}.json", d.format("%Y-%m-%d")))
        .into_iter()
        .find(|name| stdout.starts_with(&format!("wrote ~{sep}tokprof{sep}{name}\n")))
        .unwrap_or_else(|| panic!("{stdout}"));
    assert!(stdout.ends_with("Nothing was sent anywhere: review the file, then send it yourself.\n"), "{stdout}");
    let text = std::fs::read_to_string(home.join("tokprof").join(&name)).unwrap();
    assert!(text.contains("\"format\": 3") && text.contains("\"claude\"") && !text.contains("\"totals\""), "{text}");
    assert!(!text.contains("5e5510f0") && !text.contains("/w/p"), "{text}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn the_filters_pick_the_requests_and_the_window() {
    // The date of the requests, read on both sides of the home's making, should it cross 12:05.
    let first = tokprof_date(written_at());
    let home = home("filters");
    let last = tokprof_date(written_at());
    let target = home.join("out/pilot.json");
    let out = tokprof(&home, &["--agent", "claude", "--since", "30d", "share", "-o", target.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&target).unwrap();
    assert!([first, last].iter().any(|d| text.contains(&format!("\"first\":\"{}\"", d.format("%Y-%m-%d")))), "{text}");
    assert!(text.contains("\"requests\":2"), "{text}");
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
fn share_prints_the_report_of_its_file_and_no_list_of_words() {
    let home = home("prints");
    let target = home.join("out/f.json");
    let out = tokprof(&home, &["--since", "30d", "share", "--survey", "team-2026-11", "-o", target.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    let wrote = stdout.find("wrote ").unwrap();
    let report = stdout.find("share   ").unwrap();
    let totals = stdout.find("the file holds shares of $").unwrap();
    let nothing = stdout.find("Nothing was sent anywhere").unwrap();
    assert!(wrote < report && report < totals && totals < nothing, "{stdout}");
    assert!(stdout.contains(", not dollars; these two totals stay on this machine\n"), "{stdout}");
    assert!(!stdout.contains("words to review"), "{stdout}");
    assert!(stdout.contains(" · survey team-2026-11\n") && stdout.contains("Where the money went"), "{stdout}");
    // The report of the file in shares prints no dollars.
    let shares = tokprof(&home, &["pool", target.to_str().unwrap()]);
    let shares = String::from_utf8_lossy(&shares.stdout);
    assert!(shares.contains("spend   in shares: the file holds no dollars\n") && !shares.contains('$'), "{shares}");
    // With the totals, the report is what `share` printed.
    let dollars = home.join("out/d.json");
    let out = tokprof(&home, &["--since", "30d", "share", "--survey", "team-2026-11", "--dollars", "-o", dollars.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("the file holds dollars: $"), "{}", String::from_utf8_lossy(&out.stdout));
    let pooled = tokprof(&home, &["pool", dollars.to_str().unwrap()]);
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
    std::fs::write(dir.join("newer.json"), r#"{"format": 4, "agents": {}}"#).unwrap();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::copy(&a, dir.join("sub/b.json")).unwrap();
    let out = tokprof(&home, &["pool", dir.to_str().unwrap()]);
    let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{stderr}");
    assert!(stdout.starts_with("share   "), "one file read: {stdout}");
    assert!(
        stderr.contains("a (1).json: the same as ")
            && stderr.contains("bad.json: not a tokprof share file")
            && stderr.contains("newer.json: format 4 needs a newer tokprof"),
        "{stderr}"
    );
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

/// tokprof over Claude Code's and Codex's fixture logs, with a home of its own and UTC.
fn tokprof_fixtures(home: &Path, args: &[&str]) -> Output {
    common::tokprof_fixtures(home, false).args(args).output().unwrap()
}

#[test]
fn the_busy_time_in_the_file_is_the_pause_reports() {
    let home = std::env::temp_dir().join(format!("tokprof-share-busy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let file = home.join("f.json");
    assert!(tokprof_fixtures(&home, &["share", "-o", file.to_str().unwrap()]).status.success());
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let pauses = String::from_utf8_lossy(&tokprof_fixtures(&home, &["pauses"]).stdout).to_string();
    // The sections of the report come in agent order, Claude Code first, as the file's agents do.
    let sections: Vec<&str> = pauses.split("Where the agent's busy time went").skip(1).collect();
    let agents = v["agents"].as_object().unwrap();
    assert_eq!(sections.len(), agents.len(), "{pauses}");
    for ((name, a), section) in agents.iter().zip(&sections) {
        let t = &a["time"];
        assert!(t["kinds"].as_object().is_some_and(|k| !k.is_empty()), "{name}: {t}");
        for (kind, k) in t["kinds"].as_object().unwrap() {
            let ms = k["ms"].as_i64().unwrap();
            let shown = if ms >= 3_600_000 { format!("{:.1}h", ms as f64 / 3_600_000.0) } else { tokprof_core::rhythm::duration(ms) };
            assert!(section.lines().any(|l| l.starts_with(kind.as_str()) && l.contains(&shown)), "{name} {kind} {shown}: {section}");
        }
        let slow = format!("{} of {} requests took a minute or more", t["slow"], t["requests"]);
        assert!(section.contains(&slow), "{name}: {slow} in {section}");
    }
    assert!(agents["claude"]["time"]["retried"].as_u64().unwrap() > 0, "the fixture has a retried request");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn the_pool_gives_the_new_tables_as_json_and_by_file() {
    let home = std::env::temp_dir().join(format!("tokprof-share-json-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let (a, b) = (home.join("a.json"), home.join("b.json"));
    assert!(tokprof_fixtures(&home, &["share", "--survey", "one", "-o", a.to_str().unwrap()]).status.success());
    assert!(tokprof_fixtures(&home, &["share", "--survey", "two", "-o", b.to_str().unwrap()]).status.success());
    let out = tokprof_fixtures(&home, &["pool", "--json", a.to_str().unwrap(), b.to_str().unwrap()]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["activity"]["active_hours"].is_f64(), "{}", v["activity"]);
    let problems = v["problems"].as_array().unwrap();
    assert!(problems.iter().any(|r| r["name"] == "failed tool calls" && r["value"].is_f64()), "{problems:?}");
    assert!(v["time"].as_array().is_some_and(|t| t.iter().all(|r| r["ms"].is_f64())));
    assert!(v["working"].as_array().is_some_and(|w| !w.is_empty()));
    // The tables of the rhythm, the agents' commits and the bench, with numbers.
    let rhythm = v["rhythm"].as_array().unwrap();
    assert!(rhythm.iter().any(|r| r["name"] == "time at the keyboard" && r["value"].is_f64()), "{rhythm:?}");
    assert!(rhythm.iter().all(|r| r["files"].is_u64()), "{rhythm:?}");
    assert!(v["outcomes"]["rows"].as_array().is_some_and(|o| o.len() == 3 && o.iter().all(|r| r["value"].is_f64())), "{}", v["outcomes"]);
    assert!(v["bench"].is_null() || v["bench"]["rows"].as_array().is_some_and(|b| b.iter().all(|r| r["usd"].is_f64())), "{}", v["bench"]);
    let text = String::from_utf8_lossy(&tokprof_fixtures(&home, &["pool", "--by-file", a.to_str().unwrap(), b.to_str().unwrap()]).stdout).to_string();
    let head = text.lines().find(|l| l.starts_with("measure")).unwrap_or_else(|| panic!("{text}"));
    assert!(head.trim_end().ends_with("a  b") || head.split_whitespace().rev().take(2).collect::<Vec<_>>() == ["b", "a"], "{head}");
    let head = text.lines().find(|l| l.starts_with("item")).unwrap_or_else(|| panic!("{text}"));
    assert_eq!(head.split_whitespace().rev().take(2).collect::<Vec<_>>(), ["b", "a"], "a column per file in the rhythm table: {head}");
    std::fs::remove_dir_all(&home).unwrap();
}
