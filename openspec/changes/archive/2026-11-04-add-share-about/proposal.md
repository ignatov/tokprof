## Why

The survey goes to tens of people first, then to thousands, to choose which pains of working with coding agents to work on. The logs say what happened, but not who the person is or what they feel hurts. Without that, a company-wide pool is one blended average: a QA engineer's month and a developer's month are added together, and nobody can see whether the pain people complain about is the pain the logs measure.

## What Changes

- **`tokprof share --about <key>=<value>[,<key>=<value>...]`**, optional, with three questions and closed lists of answers:
  - `role`: `developer`, `qa`, `devops`, `manager`, `designer`, `researcher`, `other`;
  - `agents`, how long the person has used coding agents: `under-3m`, `3-12m`, `over-1y`;
  - `bother`, what bothers them most when they work with agents, one or two values joined with `+`: `cost`, `waiting`, `quality`, `context`, `setup`, `limits`, `nothing`.
  Any other key or value is an error that lists the allowed ones. The file's header gets `about` with only the keys given. The answers are tokprof's own words, so the leak check does not search them. The report that `share` prints shows them in an `about` line.
- **The survey prompt** in `docs/survey.md` asks the person the three questions first, each of which can be skipped, and passes the answers as `--about`.
- **`tokprof pool --where about.<key>=<value>`**, repeatable: only the files with that answer are pooled, so a report can be cut by role, by experience or by what bothers people. The first line of the header says how many files the filter kept.
- **A new pool section, `What people said`**, right after `Pains side by side`: how many people gave each answer, with `not answered` for the files without one. Next to each `bother` answer, how many of those people the related measured pain touches, by the people rule of `Pains side by side`. With `--min-files`, answers that fewer files gave fold into one `<n> more` row.

## Capabilities

### New Capabilities

### Modified Capabilities

- `share-export`: the `--about` option, the `about` part of the header, and its words among tokprof's own.
- `share-pool`: `--where`, the header line of a filtered pool, the `about` line of the report of one file, the new section with its note, order and folding, and the questions in the survey prompt.

## Impact

- Core: the questions and their answers in one table, shared by `share` and `pool`; the header part; the own words; the filter and the rows of the new section in the pool.
- CLI: `share --about`, `pool --where`, the header lines and the new section.
- Docs: README (share and pool) and `docs/survey.md` (the prompt, the header field and the new section).

How it is checked: the 8 share files of the first survey wave get synthetic `about` headers, two of them none. `tokprof pool` over them must count each answer as it was put in, with `not answered` 2 for each question, and the touched counts of `bother` must agree with `people` of `Pains side by side` for the same files. `--where about.role=developer` must keep exactly the files given that role, and say so in its first line. One real `tokprof --since 7d share --about role=developer,agents=over-1y,bother=waiting+cost` must write the answers into the header and print them, and the leak check must pass.

## Out of scope

- Free text answers, or any answer outside the lists: free text could hold names.
- Questions beyond these three, such as team, product or IDE.
- Weighting or scoring the answers against the measured pains.
- `--where` on anything but `about`, such as the survey label or the agents.
