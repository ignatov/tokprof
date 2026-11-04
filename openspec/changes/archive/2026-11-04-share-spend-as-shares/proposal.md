# Proposal

## Why

A share file holds a person's dollars, so people would send it only anonymously, and an anonymous upload is hard to set up. Dropping parts to make the file open loses what the survey needs to navigate: by client, by tool, by session. The dollars can go without losing anything else: every dollar and token can be written as a share of the person's own spend and tokens, and with those two totals every number of the full file comes back, to the cent.

## What Changes

- **BREAKING** `tokprof share` writes every dollar and token amount as a share of the file's own spend or tokens, to 7 significant digits, and leaves the two totals out of the file. Every other number, name and part stays as it is. This is format 3.
- `share` prints the one-file report as today, dollars included: it computes them from the totals, which never reach the file. A line under the report says the file holds shares, not dollars, and names the two totals that stay on the machine.
- `share --dollars` adds the two totals to the header. With them the file is the full file, and the pool prints dollars from it.
- Both kinds of file are written with one table row per line instead of one number per line: about 81 KB and 568 lines instead of 149 KB and 5,925 lines for the author's 30 days, with nothing left out.
- `tokprof pool` reads format 3. When every file has its totals (format 1, 2, or 3 with `--dollars`), the report is today's. When a file lacks them, each file weighs the same, as if each spent one unit, and every dollar of the report prints as a share of spend.
- `docs/survey.md`: the file holds no dollars and can be sent openly; what it still shows (counts, days, hours, durations) and that those cannot be hidden without losing information.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `share-export`: dollars and tokens as shares; the totals only with `--dollars`; format 3; the row-per-line layout; what `share` prints.
- `share-pool`: reading format 3; the report over files without totals; the survey guide.

## How it is checked

- On the author's 30 days (2026-10-05 – 2026-11-04, $11,465): from the file of `share` and the two totals that `share` prints, every number of the file of `share --dollars` comes back within half a cent and half a token per million, and every name and key is the same.
- `tokprof pool` over the file of `share` and over the file of `share --dollars` print the same shares in every table: `idle > TTL` 7.0% of spend, `sessions longer than a day` 64.5%, `effort max or xhigh` 80.4%, `skill:code-review` 15.0%.
- On the mock team (the author's 30 days cut by project into 8 files): `people` of every pain in `Pains side by side` is the same over the 8 files with and without totals.
- The file of `share` has no key `usd` with a value above 1, and its size is recorded against today's 150 KB.

## Out of scope

- Hiding counts, days, hours or durations. They are small whole numbers or follow from them, so any lossless rescaling still gives them away: two sleeps of the computer and three authentication errors are 2 and 3 in any unit.
- An anonymous upload.
- A shorter report.

## Impact

- `crates/tokprof-core`: writing shares and the totals, the layout, reading format 3 in the pool and weighing files without totals equally.
- `crates/tokprof`: `share --dollars`, the line under the report.
- `ci/smoke.sh` compares the files of `share --dollars`, which hold every number.
- `docs/survey.md`, `README.md`.
- A pool of tokprof 0.1.2 skips a format-3 file with `format 3 needs a newer tokprof`.
