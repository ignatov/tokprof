# Tasks

Apply after `add-team-survey` and `add-share-friction`. Every group ends with `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green, in a state that can be committed. Names from the author's logs never go into tests or docs: use made-up names such as `walrus` and `zebra`.

## 1. Before starting

- [x] 1.1 Over the author's 30 days, record in the results table below:
  - the rhythm section of `tokprof --since 30d pauses`;
  - the two agents' tables of `tokprof --since 30d bench`;
  - the last API error of each retried request by class, counted by a script from the raw logs;
  - the shell calls that ran `git commit`, `git push` or `gh pr create` and did not fail, counted by a script from the raw logs, and the commits in those repositories by `git log` over the same days;
  - the median of 3 runs and the size of `tokprof --since 30d share`.

  Also keep the file as it is now: `tokprof --since 30d share -o /tmp/share-before-attention.json`. Verify: the table has the numbers and the date, and the file exists.

## 2. Core rules

- [x] 2.1 Add the version-control actions to the core's command parser: `commit`, `push` and `pull request`, in every part of a command, after `git`'s global options. Verify: unit tests next to the parser for "A commit in a chain", "A commit and a push", "Another folder" and "A pull request", and for `git -c user.name=x commit`, `git --git-dir=.g push` and `gh pr view`.
- [x] 2.2 Add the error classes of a retry's error text. Verify: unit tests for "The classes", "A gateway error" and "A timeout", for `Connection error.`, and for a text with no status and no known word.
- [x] 2.3 Make the bench's defaults (60 requests, 100k context, 6 hours of heartbeats) named constants of the bench, and use them in the CLI. Verify: `tokprof bench --help` shows the same defaults, and `tokprof --since 30d bench` prints the same report as before.

## 3. Sources

- [x] 3.1 In both sources, pass each full shell command to the action parser where the class parser is called, and keep the actions on the call's block. Claude Code: the `Bash` input `command`. Codex: `exec_command` `cmd`, `shell` and `container.exec` `command`, and the `tools.exec_command` calls inside the `exec` tool's code. Verify: unit tests in each source with synthetic lines, one for `git add -A && git commit -m x` and one for an `exec` tool that calls `git push`; both sources' conformance tests and `crates/tokprof/tests/architecture.rs` pass.
- [x] 3.2 Pass the actions with each call's outcome to the sinks. Verify: a unit test in the analysis that a failed call's actions reach the sink marked as failed.
- [x] 3.3 Note the fields in each source's README: the commands read for the actions, with the agent versions. Verify: both READMEs name them.

## 4. The share file

- [ ] 4.1 Count `outcomes` per agent and per client: the calls with actions that did not fail, in main conversations and subagents. Verify: unit tests for "A failed push", a subagent's commit, and two clients.
- [ ] 4.2 Write `retried_by_error` into the `time` part, from the error texts busy time keeps. Verify: a unit test for "The classes add up".
- [ ] 4.3 In `tokprof share`, build the rhythm timelines as `tokprof pauses` does, each with its session's client. Write the file's `rhythm` part and each client's short one. Verify: unit tests for "Two sittings", "An answer", "A median of two", "Agents after the person left", "Sessions nobody typed in", "An IDE and a terminal" and "Two clients at once".
- [ ] 4.4 In the same pass, run the bench scan on the main sessions the client filter keeps, price it with the bench's constants, and write `bench` per agent. Verify: unit tests for "The cheapest way", with a tie, "Breaks that were no return" and "No returns".
- [ ] 4.5 Add the new keys to tokprof's own words. Verify: a unit test for "Nothing new outside the own words", over a file with every new part.
- [ ] 4.6 Update the share section of README: `rhythm`, `bench`, `outcomes` and `retried_by_error` in the field table, and the client additions. Verify: each new key is in the table.

## 5. The pool and the report

- [ ] 5.1 Add the hours at the keyboard to the `activity` line. Verify: unit tests for "One file", "A pool" and "A file without the rhythm part".
- [ ] 5.2 Count `Working with the agent` per hour at the keyboard. Verify: unit tests for "A file without active hours", "Interrupts" and "Ends of turn".
- [ ] 5.3 Add the rows by error class under `requests that waited for retries`. Verify: a unit test for "Mostly gateway errors".
- [ ] 5.4 Add `Coming back after a cold break`. Verify: unit tests for "Two agents" and "No returns".
- [ ] 5.5 Add `Your time with the agents`, with the pooled median as a pause range. Verify: unit tests for "The rows", "One file" and "Ends while the person was there", and a pool of two files whose pooled median falls in `1–5m`.
- [ ] 5.6 Add `Working with the agent by client`. Verify: a unit test for "An IDE against a terminal", and one with 10 clients that ends with `2 more`.
- [ ] 5.7 Add `Commits and pull requests`. Verify: unit tests for "One file" and "No commits".
- [ ] 5.8 Add `At a glance`, from the same pool tables as the sections. Verify: unit tests for "One file" and "A pool", and that a file without the bench parts has no `returns` line.
- [ ] 5.9 Put a note line under every title, move the titles' colon remarks into the notes, and give every count thousands separators. Verify: unit tests for "The notes", "Notes of a pool", "Calls of a tool", "The order", "Format-1 files only" and "Files written before the rhythm parts".
- [ ] 5.10 Check `--json` and `--by-file` over the new tables. Verify: an integration test that `--json` holds them with numeric values, and that `--by-file` adds a column per file to `Your time with the agents`.
- [ ] 5.11 Update README's pool section, and `docs/survey.md`: reading the report, and the prompt's command with `--mcp-names <mcp servers>` and when to delete it. Verify: each new section of the report is described there, and the prompt holds the placeholder.

## 6. Check on the real logs

- [ ] 6.1 Over the window of 1.1, check that `rhythm` equals the rhythm section of `tokprof pauses`, and `bench` equals `tokprof bench`. Verify: the numbers are recorded next to those of 1.1.
- [ ] 6.2 Check that the counts of `retried_by_error` add up to the requests that waited for retries, and compare them with the raw classes of 1.1. Check that `outcomes` equal the script count of 1.1, and that the commits stay at or under `git log`. Verify: the numbers are recorded.
- [ ] 6.3 Time `share` 3 times and record the size. Verify: the median is at most 10% over that of 1.1, the size is under 150 KB, and the words outside tokprof's own are the same as in `/tmp/share-before-attention.json`.
- [ ] 6.4 Read the report of the new file, and pool it with `/tmp/share-before-attention.json`. Verify: each line of `At a glance` matches its section, and each new section says `in 1 of 2 files` in the pool. Record the new file's `At a glance` in the results.
- [ ] 6.5 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`, then run `cargo fmt --all --check`, `cargo test --workspace`, clippy and `openspec validate --all --strict`. Verify: all pass.

## Results

| check | before | after |
|---|---|---|
| 1.1 rhythm section | closed window 2026-10-04 to 11-03: 317 sittings, 3.4d at the keyboard, 1,460 messages to 228 sessions; median sitting 6.7m, median break 33m; the agents stood still a median 11m after the author left and worked through 42% of a median break; a turn ended every 3.9m while the author was there, 29% started by the agent; turns a median 2.9m, answers 2.7m; 45 interrupts, 35 messages typed while the agent worked, 73 questions answered in a median 2.3m; 232 sessions left out | |
| 1.1 bench | Claude Code 205 returns: resume $1,650, /compact first $1,286, handoff note $518, heartbeats $1,276, heartbeats then compaction $803; the note cheaper in 205 of 205; heartbeats kept 124 warm for $84.99 of pings; breaks that were no return: 966 pings for $82.35, 128 compactions. Codex 18 returns: resume $77.89, /compact first $55.82, note $32.22, heartbeats $90.29, heartbeats then compaction $62.98; the note cheaper in 18 of 18 | |
| 1.1 retry errors by class | Claude Code's raw `api_error` lines, the last of each run of attempts: 102 runs; server 44, timeout 28, auth 11, connection 5, and 14 `Connection lost while your computer was asleep` | |
| 1.1 agents' commits, pushes, pull requests | Claude Code: 424 `git commit` calls (5 failed), 139 `git push` (4 failed), 15 `gh pr create`; Codex: 2 commits. The author's own commits in the 6 repositories the calls ran in: 525 | |
| 1.1 share time and size | closed window: 26.75, 27.13, 26.83 s, median 26.83 s; 132,504 bytes (`/tmp/share-before-attention.json`) | |

## Workflow follow-up

- Archive in this order: `openspec archive add-team-survey -y`, `openspec archive add-share-friction -y`, then `openspec archive add-share-attention-outcomes -y`, and run `openspec validate --specs --strict`.
- Push and tag the survey release when the author decides.
