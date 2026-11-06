//! A catalog that keeps the logs it read gives what a new one gives while the logs grow: read on
//! from where the last read stopped, with a line cut in the middle, meta files that appear later,
//! and a log written anew.

use std::fs;
use std::path::{Path, PathBuf};

use tokprof_core::catalog::Catalog;
use tokprof_source_claude_code::ClaudeCode;

fn catalog(projects: &Path) -> Catalog {
    Catalog::new(vec![Box::new(ClaudeCode::at(projects.to_path_buf(), Path::new("/h")))], PathBuf::from("/h"))
}

/// What a catalog that kept the logs and a new one give for every log now.
fn both(kept: &Catalog, projects: &Path) -> Vec<(String, String)> {
    let fresh = catalog(projects);
    let mut logs = fresh.discover(&fresh.agents());
    logs.sort_by(|a, b| a.path().cmp(b.path()));
    logs.iter().map(|log| (format!("{:?}", kept.parse(log).unwrap()), format!("{:?}", fresh.parse(log).unwrap()))).collect()
}

fn same(kept: &Catalog, projects: &Path, when: &str) {
    for (i, (kept, fresh)) in both(kept, projects).into_iter().enumerate() {
        if kept != fresh {
            let at = kept.bytes().zip(fresh.bytes()).position(|(a, b)| a != b).unwrap_or(0);
            panic!(
                "log {i} {when}: the kept catalog gives another session:\n{}\n---\n{}",
                &kept[at.saturating_sub(300)..(at + 300).min(kept.len())],
                &fresh[at.saturating_sub(300)..(at + 300).min(fresh.len())]
            );
        }
    }
}

#[test]
fn a_kept_log_read_on_gives_what_a_whole_read_gives() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/-w-app");
    let id = "5e5510f0-1111-4222-8333-944445555666";
    let main = fs::read(fixtures.join(format!("{id}.jsonl"))).unwrap();
    let sub = fs::read(fixtures.join(format!("{id}/subagents/agent-a1.jsonl"))).unwrap();
    let meta = fs::read(fixtures.join(format!("{id}/subagents/agent-a1.meta.json"))).unwrap();

    let projects = std::env::temp_dir().join(format!("tokprof-follow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&projects);
    let dir = projects.join("-w-app");
    let subagents = dir.join(id).join("subagents");
    fs::create_dir_all(&subagents).unwrap();
    let (main_log, sub_log) = (dir.join(format!("{id}.jsonl")), subagents.join("agent-a1.jsonl"));

    let kept = catalog(&projects);
    kept.keep_logs();
    // The logs grow in steps that cut lines anywhere; the meta file shows up halfway.
    let steps = 12;
    for step in 1..=steps {
        let cut = |bytes: &[u8]| bytes[..bytes.len() * step / steps].to_vec();
        fs::write(&main_log, cut(&main)).unwrap();
        fs::write(&sub_log, cut(&sub)).unwrap();
        if step == steps / 2 {
            fs::write(subagents.join("agent-a1.meta.json"), &meta).unwrap();
        }
        same(&kept, &projects, &format!("at step {step}"));
    }
    // Read again unchanged, and once more after its last line lost its newline and got it back.
    same(&kept, &projects, "unchanged");
    fs::write(&main_log, &main[..main.len() - 1]).unwrap();
    same(&kept, &projects, "with a last line without its newline");
    fs::write(&main_log, &main).unwrap();
    same(&kept, &projects, "whole again");

    // Written anew: its second line is gone and lines are appended, so it is longer than before.
    let text = String::from_utf8(main.clone()).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    lines.remove(1);
    let tail: Vec<&str> = text.lines().skip(lines.len().saturating_sub(3)).collect();
    let anew = [lines, tail].concat().join("\n") + "\n";
    assert!(anew.len() > main.len());
    fs::write(&main_log, anew).unwrap();
    same(&kept, &projects, "written anew");
    fs::remove_dir_all(&projects).ok();
}
