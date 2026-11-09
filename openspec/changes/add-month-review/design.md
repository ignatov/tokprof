# Design

## Context

The day store (add-activity-calendar) keeps every log's minutes, work and actions. The calendar sums them by work-rhythm day in about 0.1 s. A month is a window of that: from local midnight on its first day to the next month's. `Calendar::of` already cuts by window, so the month reuses it, and the summary's numbers for the same dates follow.

What the store lacked for a card:
- the cache's share of input and what the reads saved: `Usage::cache_read`, `Usage::prompt()` and `Price::{input, read}`, from the request records;
- avoidable dollars by cause: `MissEvent::cause` and `premium_usd`;
- spend by model: `RequestRecord::model` and the price;
- cold returns: `Pauses` over the same log, `AgentPauses::returns[].ts_ms`.

All of these come from fields the analysis already reads: Claude Code's `usage` and `model`, and Codex's token counts, for the versions their source READMEs name. No new log field is read.

## Decisions

### Store format 2

Each minute row gains `read_tokens`, `prompt_tokens`, `saved_usd` and `unpriced`, so it is an array of 11. Each entry gains `misses` (hour, cause name, usd), `models` (hour, model, usd) and `cold` (ms). The file is `days-2.json`; a `days-1.json` from an unreleased build is left alone. On the author's logs the store grows from 2.9 MB to 4.8 MB.

Saved by cache reads is `cache_read × (input − read) / 1e6` with the request's price. A request without a price adds nothing and counts in `unpriced`, which the card names.

### The month

`Month::of(entries, filters, ym, now)`:
1. Run `Calendar::of` with the month's window. Sum all its days for the big numbers. This includes a sliver of the last day of the month before, when agents worked past midnight, so spend equals the summary's for the same dates. Take the cells of the month's dates for the mini calendar.
2. The punchcard gives agent and keyboard time by local hour: the busiest hour, the night share (22:00–05:00) and the morning share (05:00–09:00).
3. Concurrency: per minute of the window, count the main, kept entries whose work bit is set; the conductor share is minutes with 3 or more over minutes with 1 or more.
4. Sessions with a request, cold returns in the window, and models and causes summed over the hours in the window.
5. The same for the month before, for the deltas.

### Titles

They are tried in this order: night owl, early bird, conductor, marathon, cache keeper, shipper. The thresholds come from the author's months:

| month | night | morning | 3+ at once | streak | avoidable | commits/agent h | title |
|---|---|---|---|---|---|---|---|
| Aug 2026 | 34.8% | 0.4% | 7.5% | 26 | 11.8% | 0.00 | Night owl |
| Sep 2026 | 29.4% | 0.3% | 2.0% | 12 | 11.5% | 0.77 | Shipper |
| Oct 2026 | 30.7% | 0.2% | 9.4% | 31 | 12.5% | 2.19 | Marathon |
| Nov 2026 so far | 27.8% | 3.1% | 14.0% | 8 | 4.3% | 4.01 | Cache keeper |

The night owl takes 33% or more, so August gets it and October does not. The marathon takes 28 days or more, so October gets it. The shipper takes 0.75 or more commits per agent hour, so September gets it. The early bird (20%) and the conductor (25%) are rare on these logs, and are kept for other people.

### The card in the window, and its image

`src/month.rs` draws the card as one 540×675 pt column: the brand line, the headline with "with agents" on its baseline, the badge with its rule, the big numbers three by two, the mini calendar beside five facts, the cache and the deltas as rows of small numbers, and the next-month box. It uses the theme's canvas, ink and accent, and the calendar's level ramp.

`Save image` runs the same binary again: `--snap <path> --scene month-card --month YYYY-MM --size 540x675 --colors <theme>`, plus the window's filters. The process draws the card alone, at scale 2, into 1080×1350 pixels. This reuses Rune's frame without a window, which has no in-process API for a picture of one element. The child skips the session list's settling for this scene and takes about 3 s. The parent picks the free name (`-2`, `-3`…) before it starts, and gets the result back through a mutex the poll reads.

### Hardening the store (code review 1)

- **Errors other than "not found"** give an in-memory store that is never written, with a warning.
- **A store that cannot be parsed** is kept as `days-2.json.broken-<UTC>`, with `-2`, `-3`… after an earlier backup of that second. `load` takes `now_ms` for the stamp, since the library reads no clock.
- **Saves** write and `sync_all` the temporary file before the rename. A failed write removes the temporary file. A failed save is a warning after the output, never an error.
- **The fingerprint** names the git commit: `build.rs` runs `git rev-parse --short=12 HEAD` and reruns when HEAD or its ref moves. A build without git uses the version alone.
- **The window's worker** reads the price files again when their mtime changes, so its fingerprint agrees with the CLI's. It waits at least 60 s between refreshes while logs change, instead of 20 s.
- **A moved log** drops the old entry only once the new key has an entry after this refresh.
- **A minute without a time** (minute ≤ 0) is in no day. Calendars without a window count it on an `untimed` line.

## Effect on parse time

The first run of the new store parses every log once with `Pauses` added to the store's sink, the same pass `tokprof pauses` makes. On the author's logs that took 27.2 s under load ~20; the format-1 store's first run took 25–45 s. `tokprof summary` is unchanged.

## Risks / Trade-offs

- **Every commit re-reads all logs once** (about 45 s), including commits that only change docs. Correctness of the totals wins, by the PM's decision.
- **The image comes from a child process**, which needs the binary on disk. An app bundle has it.
- **The month's spend includes the night sliver** that the calendar puts on the last day of the month before. The mini calendar can then sum to less than the big number: $138.84 in October.
