## Why

The survey is meant to choose which pains of working with coding agents to work on first. A pool of 8 share files already holds the numbers for that choice, but they are spread over a dozen sections in different units: how many people a pain touches is in `How often it happens`, its money in `What could have been saved`, `Habits that cost the most` or `Problems`, and its time in `Problems` too. To compare two pains, the reader has to find the rows and put them side by side by hand.

## What Changes

- **A new section in `tokprof pool`**, `Pains side by side`, right after `At a glance`. It has one row per pain: cold returns, contexts of 400k+, sessions longer than a day, failures, waits for retries, interrupts, the agent asks, and effort max or xhigh.
- Each row says how many people the pain touches, as `<k> of <n>` files: the files where it happened in a quarter of their active days or main sessions or more, or, for effort, with half of their spend at max or xhigh or more.
- Each row gives its money or time: the median file's dollars or minutes per active hour, and the pooled dollars or time. A `kind` column says whether the dollars are `avoidable` or `spent`, or whether the row is `time`.
- The rows are sorted by the people they touch, then by their dollars. A line under the title says that the rows overlap, so money and time are not added up.
- Every number comes from a row that another section already prints, so the two agree.
- The section is in a pool only: one file touches one person.
- The share file does not change.

## Capabilities

### New Capabilities

### Modified Capabilities

- `share-pool`: the new section, its people and money rules, its note, and its place in the order of the sections.

## Impact

- Core: the pool builds the pain rows from its own rows, after all other sections. The thresholds sit in one table in the code.
- CLI: the pool report prints the section; `--json` holds its rows.
- Docs: README's pool list and `docs/survey.md` (reading the report).

How it is checked: `tokprof pool` over the 8 share files of the first survey wave. Each row is compared with the row it comes from: the people with `files 25%+` of `How often it happens` (cold returns 7, contexts of 400k+ 5, sessions longer than a day 0, failures 7, retries 3, interrupts 4, the agent asks 2), the dollars with `idle > TTL` ($792), `prompts of 400k+` ($3,736), `sessions longer than a day` ($7,382), `spend after failures` ($226, median $0.28 per active hour), `effort max or xhigh` ($9,073), and the time with `time waiting for retries` (23.8h, median 1.8m per active hour).

## Out of scope

- New fields in the share file, such as the total time a person took to answer the agent's questions. The files hold only the median, so `the agent asks` has no time.
- A score or rank that adds people, money and time into one number.
- Pains per agent or per client.
- Changing the rows of the sections the pains come from.
