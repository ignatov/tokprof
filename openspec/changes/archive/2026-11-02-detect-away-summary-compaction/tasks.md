# Tasks

## 1. Recognize the second notice

- [x] 1.1 In the Claude Code source, match `content` against both prefixes, `Compacted while idle` and `Compacted using a summary written while you were away`. Use the same pair in the line filter, which still searches only after a boundary, and in the line handler. The comment on the prefixes names both notices and their versions: the first seen in 2.1.302–2.1.310, the second found in the 2.1.316 binary. Verify:
  - `compactions_get_their_kind` passes unchanged;
  - a new test next to it, `an_away_summary_notice_makes_the_compaction_idle`, passes. Its lines follow the second mode in the order the 2.1.316 binary emits them: a reply, a typed prompt after a three-hour break, a `compact_boundary` with trigger `auto`, the compact summary, the notice `Compacted using a summary written while you were away · turn off with "idleCompaction": false in settings.json`, and a reply. The boundary's kind is `idle`, and the reply after it is marked as following a compaction.
- [x] 1.2 Update `sources/claude-code/README.md`. Its session facts name both notices with their versions, as the session-logs spec does. Verify: by reading it next to the spec.
- [x] 1.3 Update the README.md paragraph "Let Claude Code compact the idle session". Add the second mode: Claude Code writes the summary during the break and switches to it at the return, with the notice above. tokprof marks it the same way, as the miss cause `compaction: while idle`. While the session is still idle, the X-ray cannot see the waiting summary and forecasts a full re-write. Verify: the paragraph says all three things, and the rest of it is unchanged.
- [x] 1.4 Run `cargo test --workspace`. Verify: every test passes. Record the count here.
  - Recorded 2026-11-02 in a Branch Deck lease worktree: HEAD d1359b8 plus this change, without the shared checkout's uncommitted work. 156 tests passed and 0 failed, in 16 test binaries. With the second prefix removed, `an_away_summary_notice_makes_the_compaction_idle` fails with `auto` instead of `idle`, so the test checks the new rule.

## 2. Check on the real logs

- [x] 2.1 Count the lines with the new notice in `~/.claude/projects`, main transcripts and subagents, among `system` lines with subtype `informational`. Verify: the count is 0, so unchanged counts in 2.3 are expected. Record it here.
  - Recorded 2026-11-02 with /tmp/idle-check/scripts/count_notices.py. `Compacted using a summary written while you were away`: 0 lines, in main transcripts and in subagents. `Compacted while idle`: 6 lines, all in main transcripts. 14 transcripts mention one of the texts somewhere else, in conversation text.
- [x] 2.2 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof --version` runs from `~/.local/bin`.
  - Deferred at first at the author's choice, while the shared checkout held the unfinished code of `add-setup-view`.
  - Done on 2026-11-02 after both changes landed. It was installed from the lease at eda2b7e, the committed main with nothing uncommitted. `tokprof --version` from `~/.local/bin` prints `tokprof 0.1.0`. `tokprof misses --agent claude --expected` still lists 6 `compaction: while idle` and 25 `compaction: auto`, with the same output as before the rebase, in 7.04 s.
- [x] 2.3 Run `/usr/bin/time -p tokprof misses --agent claude --expected --top 100000` and count the details. Verify against the numbers recorded on 2026-11-02 before the change: 6 rows read `compaction: while idle`, 25 read `compaction: auto`, and the run took 6.7 s. The counts must match, and the time must stay within 10%. Record the counts and the time here.
  - Recorded 2026-11-02. 2.2 was deferred, so the new build ran from the lease's `target/release`, alternating with the installed binary as the old build.
    - Old build: 6.77 s, 6.47 s, 6.62 s, median 6.62 s.
    - New build: 7.11 s, 6.60 s, 6.59 s, median 6.60 s, within 10% of 6.7 s.
    - Both builds list 6 rows as `compaction: while idle` and 25 as `compaction: auto`, and their full outputs are identical.
- [x] 2.4 Run `openspec validate detect-away-summary-compaction --strict`. Verify: it reports the change as valid.

## Workflow follow-up

- Landed on 2026-11-02 as eda2b7e on main, through a Branch Deck lease and `bd pick`.
  - The first try was refused with `uncommitted_overlap` while `add-setup-view` was still uncommitted.
  - Once that change landed (0cc859e, 68ab33e, 6ba7442), the commit was rebased onto 6ba7442. Two conflicts kept both sides: the tests that both changes added after `compactions_get_their_kind`, and neighbouring lines of `sources/claude-code/README.md`.
  - 183 tests pass on the result, and the lease was returned.
- When a real log with the new notice appears, put its line into the test in place of the synthetic one, and name the version it was seen in, in the spec and in both READMEs.
- Archive with `/opsx:archive` after the author's review, then run `openspec validate --all --strict`.
