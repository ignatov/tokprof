# Tasks

Every group ends with `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green. Names from the people's files never go into tests or docs.

## 1. The answers in the share file

- [x] 1.1 Put the questions and their answers in one table in the core, with the parsing of `--about` and its errors. Verify: the unit tests "answers about the person go into the header in the order of the questions" and "answers outside the lists fail with the allowed ones" (a wrong role, three things that bother, a repeated one, `nothing` with another, an unknown key, a clause without `=`, a key given twice).
- [x] 1.2 Write `about` into the header with only the keys given, and check `--about` before any log is read. Verify: the same test; `tokprof share --about role=dev` fails and writes no file.
- [x] 1.3 Add the keys and answers to tokprof's own words, and let the leak check walk `about`. Verify: the unit test "the answers are own words and other strings under about are searched".
- [x] 1.4 Print `about   <answers>` in the report of one file. Verify: the unit test "what people said in a pool" checks the second line of one file's report.

## 2. The pool

- [x] 2.1 Read each file's answers, counting a value outside the lists as not answered. Verify: the unit test "a file with answers outside the lists counts them as not answered".
- [x] 2.2 Add `--where about.<key>=<value>`, repeatable, and say in the first line how many files it kept; fail when it keeps none. Verify: the unit tests "where keeps the files with every answer given" and "a filtered pool says how many files it kept".
- [x] 2.3 Build the rows of `What people said`, with `not answered`, the touched counts of `bother` by the people rule of `Pains side by side`, and the folding of `--min-files`. Verify: the unit tests "what people said counts each answer and those who gave none", "rare answers fold into more but not answered never does" and "the pain touches those who said it as pains side by side counts them" (with the agents standing still in 12 of 40 breaks).
- [x] 2.4 Print the section after `Pains side by side`, for a pool only, with its note; keep the rows in `--json` as `said` and the filter as `filter`. Verify: the order test and the note test.

## 3. Docs

- [x] 3.1 Ask the three questions in the prompt of `docs/survey.md` and pass the answers as `--about`; describe the header field, `--where` and the new section. Verify: the page has all four.
- [x] 3.2 Update README's share and pool parts. Verify: `--about`, the `about` field, the new section and `--where` are described.

## 4. Checks

- [x] 4.1 Run `openspec validate --all --strict`, and archive `add-share-spread`, `add-pool-pains` and this change in that order in a copy of `openspec/`. Verify: both pass (17 specs after the archives).
- [x] 4.2 Pool the 8 files of the first survey wave with synthetic `about` headers, two of them without, with and without `--where about.role=developer`. Verify: the numbers below.
- [x] 4.3 Run `tokprof --since 7d share --about role=developer,agents=over-1y,bother=waiting+cost` on the author's logs. Verify: the header holds the answers, the report prints them, and the leak check passes.

## Results

Over the 8 share files of the first survey wave, 2026-10-05 to 2026-11-04, six of them with a synthetic `about` and two without:

```
What people said
the answers to tokprof share --about; the pain touches: of the people who gave an answer of bother, those its measured pains touch, as Pains side by side counts them

question  answer         people  the pain touches
role      developer     4 (50%)
          qa            1 (12%)
          manager       1 (12%)
          not answered  2 (25%)
agents    under-3m      1 (12%)
          3-12m         3 (38%)
          over-1y       2 (25%)
          not answered  2 (25%)
bother    cost          2 (25%)            2 of 2
          waiting       2 (25%)            1 of 2
          quality       1 (12%)                 –
          context       1 (12%)            1 of 1
          setup         1 (12%)                 –
          not answered  2 (25%)
```

| check | result |
|---|---|
| answers | each count equals the headers put in; `not answered` is 2 for each question |
| `cost` | 2 of 2: one file is touched by effort (79% of its spend at max or xhigh), the other by cold returns (70% of its days) |
| `waiting` | 1 of 2: one file is touched only by the agents standing still, under 5 minutes in 12 of its 12 breaks; the other has no retries, no questions and no breaks, so it is in n but not in k |
| `context` | 1 of 1: contexts of 400k+ in 50% of the file's sessions |
| `--where about.role=developer` | first line `pool    4 of 8 files where about.role=developer · 2026-10-05 – 2026-11-03`; `Pains side by side` and every section count those 4 files; `role` shows `developer 4 (100%)` and no `not answered` |
| `--where about.role=qa` | the report of one file: `share   2026-10-07 – 2026-11-01 · 1 of 8 files where about.role=qa`, then `about   role=qa,agents=under-3m,bother=setup` |
| `--where about.role=designer` | fails with `no share file has about.role=designer` |
| `--min-files 2` | `qa` and `manager` fold into `2 more` 2 (25%); `under-3m` into `1 more`; `quality`, `context` and `setup` into `3 more` 3 (38%) with `–` |

The author's logs, `tokprof --since 7d share --about role=developer,agents=over-1y,bother=waiting+cost` (2026-10-28 to 2026-11-04, 8.7 GB read in 24.5 s):
- the header holds `"about": {"agents": "over-1y", "bother": ["waiting", "cost"], "role": "developer"}`, the keys in the file's alphabetical order;
- the report's second line is `about   role=developer,agents=over-1y,bother=waiting+cost`;
- the leak check passed and the file was written; `--about role=dev` failed with the allowed roles and wrote nothing.
