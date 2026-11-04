## Context

See proposal.md for why. The pool already has every number the section needs, as rows of other sections:
- **How often it happens** keeps, per count, each file's share of its active days or of its main sessions someone worked in, and counts `files 25%+` with a threshold of 25%.
- **What could have been saved** keeps each cause's dollars, and each file's share of its spend.
- **Habits that cost the most** keeps `prompts of 400k+`, `sessions longer than a day` and `effort max or xhigh` the same way.
- **Problems** keeps `spend after failures` and `time waiting for retries`, pooled, and each file's value per active hour with their median.

The section reads no log and no new field of the share file. It depends on the prevalence parts that `add-share-spread` added to format 2; older files count only in the effort row.

## Goals / Non-Goals

**Goals:**
- One table to choose pains from: people, money or time, in one unit per column.
- Every number equal to a number another section prints, so the reader can check it and tests can assert it.
- The thresholds in one place in the code and in the spec.

**Non-Goals:**
- A combined score. People, money and time stay in their own columns.
- Any change to the share file.

## Decisions

1. **Built from the pool's own rows.** The pain rows are built after every other part of the pool, from its prevalence rows, its tables and its problem rows. No pain reads the files again. So `people` equals `files 25%+`, and `pooled` equals the row of the section the money comes from.
2. **One table of pains in the code.** Each pain names the row its people come from, the share from which a file counts, the row its money or time comes from, and the kind of that money. The 25% is the same constant that `files 25%+` uses; the 50% of effort is a constant next to it.
3. **Who counts in n.** A file counts in `n` when it has the fields of the pain's people: for a prevalence count, the prevalence part with a base above zero, as `How often it happens` counts it; for effort, dollars per effort level, which format 1 lacks. When `n` is below the pooled files, the row's name says `(in <n> of <files> files)`, as the rows of `Problems` do.
4. **Effort from half of the spend.** Effort is a share of spend, not of days or sessions. Half of the spend says that the person works at the highest effort most of the time; a quarter would also count people who use it now and then.
5. **The money of cold returns is `idle > TTL` alone.**
   - Alternative: add the cache-lifetime rows. Rejected: they are a different pain. `cache lifetime: subagents` is the premium of writing subagents' prompts with the 1-hour lifetime when they end within minutes, and on the 8 files it is $403 of the $404 in the two rows. Adding it would make cold returns look $403 dearer for a reason that has nothing to do with coming back to a session. It stays in `What could have been saved`.
6. **Kinds.** `avoidable` for dollars that could have been saved (cold returns); `spent` for dollars spent in the pain, of which only part could be saved (large contexts, long sessions, spend after failures, effort); `time` for waits (retries); `–` for interrupts and questions, which the files count but do not price.
7. **The agent asks has no time.** The files hold the median of the person's answers to questions and their count, not their total, so the time waited for answers cannot be pooled without a guess. The row has `–`.
8. **Median file per active hour.** For a row of a money table, each file's dollars (its share times its spend) over its active hours, over the files that have the row's part and active hours, as `per active hour` of `What could have been saved` counts them. For a row of `Problems`, its `median file`. Time is printed as `Problems` prints it: `1.8m` per active hour, `23.8h` pooled.
9. **Order.** The most people first, then the most dollars; a time row or a row without money comes after the dollar rows that touch as many people. People is a count, not a share, so a row of fewer files does not rise above one of more.
10. **Names.** `contexts of 400k+` and `waits for retries` instead of "large contexts" and "retries", so the name says the threshold and what was waited for.
11. **Pool only.** One file touches one person, and `k of 1` says nothing that the other sections do not say.
12. **`--by-file`** adds each file's share of its days, sessions or spend that the people rule reads, so the reader sees which side of the threshold each file is on.
13. **`--min-files`** leaves the rows as they are: they count files, like `files 25%+`.
14. **Archive order.** The delta modifies `Order of the sections` and `Section notes` as `add-share-spread` leaves them, so this change is archived after it.

## Parse time

None: the pool reads only share files, and the section is a few loops over rows the pool already built.

## Risks / Trade-offs

- [A threshold hides a pain of few days] `sessions longer than a day` touches 0 of 8 people at 25% of their sessions, but holds $7,382 of spend. → The row still shows its dollars, and the sort puts it last, which is what the people column means. The dollars stay visible.
- [Rows overlap] A request in a 400k+ prompt of a session longer than a day, at max effort, counts in three rows. → The note under the title says so, and the section has no total.
- [Older files] A file without the prevalence parts counts only in the effort row. → Each row says in how many files it is.


## Added after the first pool of eight files

- On the first pool, `sessions longer than a day` touched 0 of 8 files although it held 65% of the spend: such sessions are few (15% of sessions) but expensive. `contexts of 400k+` and `sessions longer than a day` now also touch a file whose spend in their habit row is 25% or more of its spend (`or_spend` in `PAINS`). Then the row touches 5 of 8 files.
