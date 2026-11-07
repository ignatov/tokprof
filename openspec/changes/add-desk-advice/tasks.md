# Tasks

## 1. The core

- [x] 1.1 Name the advice texts that the rows reuse: `advice::miss_hint` public, `LOOKUP_HINT`, `THINKING_HINT`, `rhythm::RETURNS_HINT`, `AgentInfo::ttl_line` (used by `tokprof whatif`).
- [x] 1.2 Add `savings`: the `BySession` sink (effort max or xhigh, prompts of 400k+, single lookups, thinking, setup per session), and `changes` building the share file and `pool::build` over it, keeping `saved`, `habits` and the bench table, the sessions behind each split row, and each row's advice. Unit-test: the sessions of a row sum to the row, the ways name the cold return and the cheapest, advice per row, an empty period.
- [x] 1.3 Feed `Share`, `BySession` and the bench scan in the overview pass; keep `Changes` in the overview.
- [x] 1.4 Check that `tokprof share`, `whatif`, `pauses`, `summary` and the share file are byte for byte the same before and after.
  - 2026-11-07: over `--since 2026-10-08 --until 2026-11-07`, built at 7a3a50b and with this change: `cmp` finds the four outputs and the share file identical.

## 2. The browser

- [x] 2.1 `Browser::row_of_short`: a listed session by its agent and short id.

## 3. The window

- [x] 3.1 `src/changes.rs`: the three cards, the advice under the rows, the open row's sessions, the cheapest way marked; in `reports.rs` the parts `Saved`, `Ways`, `Habits` after the period, the open row in the state and the memo, closed when the period changes.
- [x] 3.2 Keys and clicks: ↑↓ over rows and sessions, ⏎ and a click open and close a row and open a session in Day; a session not listed says so in the bar. The key help's Reports group names ⏎'s new meaning.
- [x] 3.3 The scene `reports-advice`.
- [x] 3.4 Runtime tests: the rows, amounts and advice on the screen, Tab through the cards, a row opening its sessions and a session opening in Day, a session not listed; the old Reports tests updated for the new Tab order and the taller page.
- [x] 3.5 Fix from code review 2: opening a session from a card sent the key `d` to the browser, which a filter being typed took. It now asks the browser for the selected session's day directly; `a_session_opened_from_what_to_change_leaves_a_filter_being_typed_alone` tests it.

## 4. Check and document

- [x] 4.1 `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`; `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` at the root.
  - 2026-11-07: the window's 111 tests and the root's 508 pass; fmt and clippy clean.
- [x] 4.2 On the real logs: the window with `--until 2026-11-07 --scene reports-advice` against `tokprof share --since 30d --until 2026-11-07` at the same moment; record the rows. Render light, dark and Neon Arcade into `shots/advice/` and look at them.
  - 2026-11-07, `shots/advice/advice-light.png` against `share`: savings $1,712 (14.7%): idle > TTL $788 6.8% $1.85/h, cache lifetime: subagents $394 3.4% $0.92, after assistant message $264 2.3% $0.62, history edited $227 1.9% $0.53, images removed $28.22, unknown $8.33, params changed $0.98, model switch $0.77, tools changed $0.39, cache lifetime: main $0.00 — all equal. Ways: resume $1,734 100.0% 0 of 222, /compact $1,335 77.0%, handoff note $562 32.4% 202 of 222 (cheapest), heartbeats then resume $1,366 78.8% 20 of 222, heartbeats then compaction $886 51.1% — equal. Habits: effort max or xhigh $9,491 81.5%, sessions longer than a day $7,410 63.6%, prompts of 400k+ $3,904 33.5%, single lookups $3,903 33.5%, … — equal.
  - Light, dark (`advice-dark.png`) and Neon Arcade (`advice-neon-arcade.png`) looked at: the idle > TTL sessions open with their titles; the cheapest way marked.
- [x] 4.3 README of the window: what to change, its drill-down and the scene.
