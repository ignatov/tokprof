## Context

See proposal.md for why. `tokprof share` writes a header and the parts of each agent, and searches every word that is not one of tokprof's own for this machine's names before it writes. `tokprof pool` reads the files and builds every section from them; `Pains side by side` already says, per pain, which files it touches (see share-pool, People a pain touches).

This change reads no log and no new log field, so it depends on no agent version.

## Goals / Non-Goals

**Goals:**
- Three short questions whose answers can be pooled, cut and compared with the measured pains.
- Answers that cannot hold a name, so the leak check stays as strict as now.
- A pool that can be cut by an answer, and says that it was.

**Non-Goals:**
- Free text, or any other question.
- A score that weighs what people said against what the logs measured.

## Decisions

1. **Closed lists in one table.** The keys and their answers are written once in the code and read by `share`, by `pool --where` and by the section. A wrong key or value fails before any log is read, with the allowed ones in the message, such as `--about role=dev: role takes developer, qa, devops, manager, designer, researcher or other`.
2. **The answers are tokprof's own words.** Every answer comes from the lists, so none can be a name of the machine. They join the own words, and the leak check walks the `about` part as it walks `rhythm`: a string there that is not an own word is still searched.
   - Alternative: free text for `other`. Rejected: it could hold a name, and the leak check would only catch this machine's names.
3. **`bother` takes one or two.** One answer forces a choice between two pains a person often feels together, such as waiting and cost; three or more make every answer count for little. The values are joined with `+`, since `,` separates the keys. `nothing` goes alone, and a value may not repeat. The file keeps them in the order given.
4. **Only the keys given.** A skipped question leaves its key out; without `--about` the file has no `about`. The format stays 2: an older tokprof that pools the file ignores the part.
5. **The pool reads only known answers.** A value that is not in the lists, such as one from an edited file, counts as `not answered`, so a row of the section is always one of tokprof's words.
6. **`--where` is a filter of files.** The files are chosen before the pool is built, so every section, the header and `Pains side by side` describe only them. Several `--where` must all hold. For `bother`, a file holds `about.bother=waiting` when `waiting` is one of its answers. When no file is kept, the command fails with `no share file has about.role=qa`. The first line says `5 of 8 files where about.role=developer`; when one file is kept, the report is that of one file, and its first line ends with ` · 1 of 8 files where about.role=qa`.
7. **The touched count reuses the people rule of `Pains side by side`.** A file is touched by a pain when that pain's row counts it among its people. Each `bother` answer maps to pains:
   - `cost`: any pain whose kind is `avoidable` or `spent`;
   - `waiting`: `waits for retries`, `the agent asks`, or the agents stood still after the person left;
   - `context`: `contexts of 400k+` or `sessions longer than a day`;
   - `quality`, `setup`, `limits` and `nothing`: no measured pain, so `–`.
   A person counts in n when the file has the fields of at least one of the mapped pains, and in k when at least one of them touches the person.
8. **The agents stood still is not a row of `Pains side by side`.** The brief maps `waiting` to it, but that section has no row for it, so its rule is written next to the map: a file is touched when, in a quarter of its breaks or more, the agents stood still within 5 minutes after the person left. The quarter is the share that `Pains side by side` uses for days and sessions, and the 5 minutes are the bound of `… within 5 minutes` in `Your time with the agents`. It reads the file's `rhythm.stood_still`, which holds one wait per break. Adding a row to `Pains side by side` would change that section, which is out of scope here.
9. **Rows in the order of the lists.** Answers are shown in the order the questions list them, not by count, so that two cuts of a pool show their rows in the same order. Answers no file gave are left out; `not answered` comes last.
10. **Folding.** With `--min-files k`, the answers of a question that fewer than k files gave fold into one `<n> more` row, whose people are the files that gave any of them and whose touched count is `–`, so that it does not show the pain of one or two people. `not answered` is never folded: it describes nobody.
11. **Pool only.** One file is one person; the report of one file shows the answers in an `about` line after its first line instead, in the form `--about` takes them.
12. **Archive order.** The deltas modify `Order of the sections` and `Section notes` as `add-pool-pains` leaves them, and `Rows few files share` as `add-share-spread` leaves it. This change is archived after `add-share-spread` and `add-pool-pains`.

## Parse time

None: `share` adds one small part to the header after the logs are read, and the pool reads one more part of each file.

## Risks / Trade-offs

- [Answers are opinions] A person may tick `cost` without knowing what they spend. → That is the point of the touched column: it shows how many of them the measured pain touches.
- [Few answers per value] A cut of a small pool by an answer can describe one person. → `--min-files` folds rare answers, and `docs/survey.md` already asks for `--min-files 3` before a report is shown to others. A `--where` cut that keeps fewer files shows the reader the count in its first line.
- [A rule for standing still] Decision 8 is a new rule, not one of `Pains side by side`. → It is written in the spec and in one place in the code, next to the map.
