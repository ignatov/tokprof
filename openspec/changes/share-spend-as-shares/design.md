# Design

## Context

`tokprof share` builds the file as one JSON value and pretty-prints it, one number per line: about 2,850 lines and 150 KB for the author's 30 days, of which about 790 numbers are dollars (`usd` and `*_usd` keys) and 1,300 are token amounts (`tokens` and `*_tokens`). `tokprof pool` reads files as JSON values; it divides by a file's spend for `median file` and by active hours for rates, and sums dollars for `pooled` columns.

No log field and no agent version is new: the change rescales parts the file already holds (see share-export).

## Goals / Non-Goals

**Goals:**
- Nothing lost: the file and two numbers give back every number of today's file.
- No dollar and no token amount in the file.

**Non-Goals:**
- Hiding counts, days, hours or durations (see proposal, Out of scope).
- A smaller report.

## Decisions

### Shares of two totals, not of each table's total

Each amount is divided by the file's spend or tokens, not by its own table's sum. One base per unit keeps every ratio between any two dollar numbers of the file, so every share, `median file` and pain rule of the pool reads the same, and the file comes back with two numbers. Per-table shares would need one hidden total per table to come back.

### 7 significant digits

The author's largest amount is $11,465 and the report prints cents, so a relative error of 5·10⁻⁸ keeps every printed dollar; tokens print as `101.3M`. At 6 digits the worst dollar of the author's file was off by $0.0057, which can turn a printed cent; at 7 it is under $0.001.

### Context sizes stay in tokens

`median_context`, `min_context` and the largest-context quantiles are sizes of one prompt, not amounts of the window. They say how large the person's contexts are, which the pains need (400k), and they do not add up to spend.

### The pool weighs files the same when one lacks totals

Without totals the pool cannot add dollars. It treats every file as one unit of spend and of tokens. A file with totals is divided by them first, so a mixed pool is consistent. Weighing by unit is also what the median file does today, so `median file` columns do not change; only `pooled` columns turn from dollars into the mean share.

### Row-per-line layout

Writing each row of a table on one line, that is each object whose objects hold no object, keeps the file readable and diffable, row by row, and takes it from 149 KB and 5,925 lines to 81 KB and 568 lines. Indentation alone was 32 KB with one line per number. One line for the whole file (67 KB) was the other way; nobody can review it. Leaving out zero numbers would save another 10%, but the pool tells a missing part from an empty one, so zeros stay.

### `--dollars` adds the totals to the header

The rest of the file is the same with or without it, so the pool needs no second reader, and `ci/smoke.sh` compares the files of `--dollars`, which hold every number.

## Risks / Trade-offs

- [Spend can be estimated from what stays: requests times context sizes gives the cache reads roughly] → the guide says the file hides the dollars, not how much the person works, and that counts and hours stay.
- [Breaking: a script that reads `usd` as dollars gets shares] → format 3 tells them apart; `--dollars` keeps the totals to multiply by; the README says so.
- [A pooled share of files that weigh the same differs from the share of pooled dollars] → the header's `spend` line says which one the report prints.

## Parse time

Dividing about 2,000 numbers and one layout pass over a value in memory: under a millisecond. The full corpus keeps its ~8 s.

## Migration Plan

Ship in the next release with the survey guide. Files of formats 1 and 2 pool as before. Rollback is the previous release, whose pool skips format 3 with a line.
