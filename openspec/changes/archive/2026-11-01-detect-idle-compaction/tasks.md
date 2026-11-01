# Tasks

## 1. Compaction markers with a kind

- [x] 1.1 Add the session's sorted compaction markers (time and kind) to the session model. Fill them in the Codex adapter from `compacted` lines, with no kind. Verify: the new test `a_compaction_marks_the_next_request_and_is_kept_without_a_kind` passes. There was no Codex compaction test before.
- [x] 1.2 Claude Code adapter:
  - the line filter also accepts lines with `Compacted while idle`;
  - a `system`/`informational` line starting with that text records a notice time;
  - each `compact_boundary` keeps its `compactMetadata.trigger`;
  - after parsing, a boundary is `idle` when a notice falls between it and the next answered request of the transcript, else it takes its trigger.

  Verify: `compactions_get_their_kind`, with the recorded 2.1.310 lines, passes. It covers a boundary plus notice → `idle`, a boundary without notice → `auto`, a `manual` trigger → `manual`, and a notice after the next request → `auto`.
- [x] 1.3 Run `cargo test`. Count the kinds over the real logs with a debug print or a test-only command: expect 6 `idle` and 25 `auto` among the 31 Claude Code boundaries. Record the counts here.
  - Recorded 2026-11-01 with a scratch binary over all main logs. Claude Code: 6 `idle` (9428b80a 2026-10-23 21:47; da18a2d8 2026-10-28 00:01, 10-28 18:14, 10-29 03:43, 10-29 12:36, 10-29 19:13) and 25 `auto`. Codex: 791 markers, no kind. `cargo test`: 55 passed.

## 2. The compaction cause detail

- [x] 2.1 In the miss classifier's `compaction` branch, look up the session's latest marker after the previous answered request and set the detail to `while idle`, the trigger or nothing. Verify: unit tests next to the classifier for `idle`, `auto` and a Codex marker (`a_compaction_cause_names_its_kind`, `an_older_compaction_does_not_name_the_cause`).
- [x] 2.2 Run `tokprof misses --expected --top 60` on the real logs. Check that the rebuilds after the 6 idle compactions read `compaction: while idle`, and that misses by cause still show one `compaction` row. Record the lines here.
  - Recorded 2026-11-01. Misses by cause still have one `compaction (expected)` row per agent: claude 31, codex 822. The top 60 are ranked by avoidable cost and hold no compaction rebuild, so the rows below come from `--session <id> misses --expected --top 400` (local time):
    - `2026-10-24 23:25  9428b80a  51.9k  30.1k  $0.23  compaction: while idle`
    - `2026-10-28 16:39  da18a2d8  44.8k  32.7k  $0.26  compaction: while idle`
    - `2026-10-28 22:31  da18a2d8  52.8k  40.8k  $0.32  compaction: while idle`
    - `2026-10-29 09:45  da18a2d8  54.2k  54.2k  $0.42  compaction: while idle`
    - `2026-10-29 14:31  da18a2d8  55.8k  41.3k  $0.32  compaction: while idle`
    - `2026-10-29 20:19  da18a2d8  40.6k  26.2k  $0.20  compaction: while idle`
  - Over all logs, `--top 5000`: 6 rows read `compaction: while idle` and 25 read `compaction: auto`.

## 3. The X-ray after a trailing compaction

- [x] 3.1 Find the trailing compaction: the session's latest marker is later than the live prompt's time. Compute the estimate (the advice's post-compaction prompt, else setup + 20,000) and setup (the live prompt's tool-schema and system-prompt tokens). Verify: unit tests for the case with an earlier compaction and for the first compaction (`a_trailing_compaction_sets_the_next_prompt`).
- [x] 3.2 Cache line: `compacted while idle at HH:MM · the next request re-builds ~<estimate>`, or `compacted at HH:MM (<kind>) · …`, or `compacted at HH:MM · …` with no kind, with no color. Context line: `~<estimate> of <window>`. Verify: a test renders both lines for an idle and an auto compaction (`the_x_ray_names_a_trailing_compaction`).
- [x] 3.3 Next line: setup at the read price, plus (estimate − setup + mean new tokens) at the write price of the conversation's lifetime, plus mean output, with no warm figure. Verify: a test reproduces the spec's $0.37 scenario (`the_forecast_after_a_compaction_reads_the_setup_and_writes_the_rest`).
- [x] 3.4 Advice gets a derived live prompt with the estimate's size, the setup blocks only and zero turns. Leave the advice format strings untouched. Verify: tests show the carry-on, context-cost and content-hint items do not fire, and the thinking and miss items still do (`advice_after_a_compaction_sees_the_next_prompt`).
- [x] 3.5 The `in context` heading ends with `, before the compaction at HH:MM`. Verify: the rendering test from 3.2.
- [x] 3.6 On the real logs, X-ray each of the 6 sessions cut right after its idle compaction (the cut files come from the transcripts in proposal.md). Record a table: date, today's forecast, the new forecast and the logged next request. At least 5 of the 6 new forecasts must be within 2× of the logged cost, and all within 2.2× (the criterion the author accepted on 2026-11-01; it was every forecast within 2×). Also X-ray the full sessions, which have requests after the compaction, and check that they read as before.
  - Recorded 2026-11-01. Done: 5 of 6 forecasts are within 2×, and the 2026-10-29 19:13 one is 2.17×. The author accepted this and kept the forecast as specified.

    | Idle compaction (UTC) | Session | X-ray before | X-ray after | Logged next request | After ÷ logged |
    |---|---|---:|---:|---:|---:|
    | 2026-10-23 21:47 | 9428b80a | $1.85 | $0.26 | $0.25 | 1.05× |
    | 2026-10-28 00:01 | da18a2d8 | $6.60 | $0.48 | $0.27 | 1.79× |
    | 2026-10-28 18:14 | da18a2d8 | $3.18 | $0.33 | $0.35 | 0.93× |
    | 2026-10-29 03:43 | da18a2d8 | $4.46 | $0.43 | $0.48 | 0.90× |
    | 2026-10-29 12:36 | da18a2d8 | $3.24 | $0.29 | $0.37 | 0.79× |
    | 2026-10-29 19:13 | da18a2d8 | $3.73 | $0.47 | $0.22 | 2.17× |

  - The outlier comes from the conversation's pace. Its latest 20 requests wrote 24.2k new tokens each, mostly screenshots, while the logged first request after the compaction wrote 26.2k in all. Without the pace, the forecasts are 0.45–1.25× the logged costs, further off on average and 2.2× in one case.
  - Full sessions: the X-rays of 9428b80a and da18a2d8 differ from the baseline binary only in the advice wording, which comes from the `compact-xray-advice` change that landed in between. The context, cache, next, in-context and misses lines are identical.

## 4. Docs and checks

- [x] 4.1 README.md:
  - "Other ways to avoid a cold return" says tokprof labels idle compactions;
  - the X-ray description mentions the compacted cache line.

  Verify: the sections read right next to the new output.
- [x] 4.2 Run `cargo test` and `cargo clippy`, with no new warnings. (61 tests passed; `cargo clippy --release --all-targets` printed no warnings.)
- [x] 4.3 Check that `tokprof summary` totals are unchanged on the real logs, and that the Claude Code total still matches Claude Code's own cost counter within 1.5%.
  - Recorded 2026-11-01. `tokprof --until 2026-11-01 summary` prints exactly the same from the baseline binary and the new one. The full `tokprof summary` matches Claude Code's own counter at $10,327 against $10,435 (1.05%).
- [x] 4.4 Compare the time of a full `tokprof summary` before and after the change. Expected: under 1% slower. Record both times.
  - Recorded 2026-11-01, `--until 2026-11-01 summary`, the two binaries alternating.
    - The first build searched every line for the notice: 23.60 s against 24.07 s mean (+2%). So the notice is now looked for only between a `compact_boundary` and the next response.
    - After that, over 4 runs: baseline 23.93, 23.79, 23.66, 23.69 s; new 26.68, 24.04, 23.71, 23.63 s. Medians 23.74 against 23.88 s (+0.6%). Runs 2–4 average 23.71 against 23.79 s (+0.3%), and the first new run is an outlier.
- [x] 4.5 Run `openspec validate --all --strict`, then reinstall the author's binary with `cargo install --path . --root ~/.local`. (17 items passed; reinstalled 2026-11-01 23:25.)

## Workflow follow-up

- Archive with `/opsx:archive` after the author has checked the X-ray on a live idle-compacted session.
- If the advice should name the idle compaction ("already compacted while idle"), ask the `compact-xray-advice` change to add the line in its format once that change is archived.
