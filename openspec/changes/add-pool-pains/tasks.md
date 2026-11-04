# Tasks

Every group ends with `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green. Names from the people's files never go into tests or docs.

## 1. The pain rows

- [x] 1.1 Put the pains in one table in the pool code: each pain's people row, the share from which a file counts (25%, the constant of `files 25%+`, and 50% for effort), its money or time row and its kind. Verify: the table is the only place the thresholds of the section are written.
- [x] 1.2 Build the pain rows after every other part of the pool, from its prevalence rows, its money tables and its problem rows, the most people first, then the most dollars. Verify: the unit test "pains agree with the rows of their sections" asserts each row's people against `files 25%+` and each row's money or time against its source row, the 25% and 50% edges, a file without the parts outside n, and the order.
- [x] 1.3 Keep the rows in `--json`. Verify: `tokprof pool --json` has `pains`.

## 2. The report

- [x] 2.1 Print `Pains side by side` right after `At a glance`, for a pool only, with its note. Verify: the order test and the note test, and the report of one file has no such section.
- [x] 2.2 Print the columns `pain`, `people`, `kind`, `median file per active hour` and `pooled`, with `(in <n> of <files> files)` after a pain some files lack, and `--by-file` columns. Verify: the unit test "pains side by side in a pool".
- [x] 2.3 Update README's pool list and `docs/survey.md`. Verify: both describe the section.

## 3. Check on the survey files

- [x] 3.1 Build the release binary and run `tokprof pool` over the 8 share files of the first survey wave. Verify: every number agrees with the row it comes from, as below.
- [x] 3.2 Run `cargo fmt --all`, `cargo test --workspace`, clippy and `openspec validate --all --strict`, and archive this change after `add-share-spread` in a copy of `openspec/`. Verify: all pass.

## Results

Over the 8 share files of the first survey wave, 2026-10-05 to 2026-11-04:

```
Pains side by side
each pain by the people it touches: in a quarter of their days or sessions or more, or half of their spend for effort; the rows overlap, so money and time are not added up

pain                        people       kind  median file per active hour  pooled
effort max or xhigh         7 of 8      spent                       $15.39  $9,073
cold returns                7 of 8  avoidable                        $0.98    $792
failures                    7 of 8      spent                        $0.28    $226
contexts of 400k+           5 of 8      spent                        $5.16  $3,736
interrupts                  4 of 8          –                            –       –
waits for retries           3 of 8       time                         1.8m   23.8h
the agent asks              2 of 8          –                            –       –
sessions longer than a day  0 of 8      spent                       $13.96  $7,382
```

| check | result |
|---|---|
| people | equal to `files 25%+` of `How often it happens`: cold returns 7, contexts of 400k+ 5, sessions longer than a day 0, spend after a failure 7, retries 3, interrupts 4, questions 2; effort: 7 files at 50% or more of their spend, the eighth at 27% |
| dollars | equal to `idle > TTL` $792, `prompts of 400k+` $3,736, `sessions longer than a day` $7,382 and `effort max or xhigh` $9,073, and to `spend after failures` $226 with its median file $0.28 per active hour |
| time | equal to `time waiting for retries`: 23.8h pooled, 1.8m per active hour in the median file |
| left out | `cache lifetime: subagents` ($403) and `cache lifetime: main` ($0.68) are not in the cold returns (design, decision 5) |
| one file | `tokprof pool` over one of the files prints no `Pains side by side` |
| `--by-file --min-files 3` | the rows stay; each file's column shows the share the people rule reads |

- [x] Let the two size pains touch a file through their share of its spend as well. Verify: over the eight files, `sessions longer than a day` touches 5 of 8 (was 0 of 8), `contexts of 400k+` stays 5 of 8, the other rows are unchanged.
