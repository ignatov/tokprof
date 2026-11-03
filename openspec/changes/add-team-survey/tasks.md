# Tasks

Apply after `add-release-builds`. Every group ends with `cargo test --workspace` green, in a state that can be committed. Names from the author's logs never go into tests, docs or fixtures: use made-up names such as `walrus` and `zebra`.

## 1. Before starting

- [x] 1.1 With the current binary, write a format-1 file: `tokprof --since 30d share -o /tmp/share-format1.json`. Record the baseline in the results table below:
  - the median of 3 runs and the size;
  - the total line of `tokprof --since 30d tools`;
  - the rows of `tokprof --since 30d whatif`;
  - `Total priced` of `tokprof --since 30d summary`;
  - the Codex rows of `Cache misses by cause`.

  Verify: the table has the numbers and the date, and the file exists.

## 2. Sources: names that ship with the agent, and the Codex version

- [x] 2.1 Claude Code source: list the built-in tools, slash commands, bundled skills and built-in subagent types, read from the 2.1.316 binary. Cross-check them against the non-MCP tool names and the `command:`, `skill:` and `agent:` origins of the author's last 30 days, and give a reason in the README for each name left off. Add the lists and the versions checked to `sources/claude-code/README.md`. Verify: a unit test that `Bash`, `general-purpose` and `/goal` are on the lists and `walrus-review` is not.
- [x] 2.2 Codex source: the same for the 0.166 binary's tool specs, with `exec_command`, `apply_patch` and `view_image` checked, and the Codex names to check from design.md settled. Verify: a unit test on the lists, and `sources/codex/README.md` names them and the version.
- [x] 2.3 Both sources: expose the server of an MCP tool (`mcp__<server>__<tool>`). Verify: unit tests for `mcp__docs__search` and for a name without the prefix.
- [x] 2.4 Codex source: read `cli_version` from the first `session_meta` of a rollout into every request. Verify:
  - decoder tests for "A Codex rollout", "A later session_meta" and "A log without a version";
  - on the real logs, the Codex rows of `Cache misses by cause` equal the baseline from 1.1.

## 3. Core: the new parts

- [x] 3.1 Pass the version to the request records. Add the `versions` part with the version pattern. Verify: unit tests for "Versions of two agents" and "A value that is not a version".
- [x] 3.2 Add `content` from the facts, as an array by content kind. Verify: unit tests for "The kinds" and "Content adds up to the spend".
- [x] 3.3 Add `prompt_sizes` by chain kind. Verify: unit tests for "A prompt at a bound" and "Prompt sizes add up to the spend".
- [x] 3.4 Extend the pause collector's per-session data with the first and last request time and the client. Add `session_spans` and the client additions (main sessions, cold returns, cache writes by lifetime). Verify: unit tests for "A session over days", "Spans add up" and "Clients and the cache lifetime"; the existing pause tests still pass.
- [x] 3.5 Add `compactions` by trigger. Verify: a unit test for "Compactions of one session", and one with a logged trigger the model does not know, which counts under `other`.
- [x] 3.6 Add the program vocabulary and the mapping of command heads. Build `tools` from the summary collector's tool table with the shipped names, `mcp`, `mcp:<server>` and `other`. Build `tool_hints` from the unmapped rows. Verify: unit tests for:
  - "Common programs", "Programs of the other lists" and "The user's own scripts";
  - "A shell tool" and "Reads one per response";
  - "The same totals as the tools command".
- [x] 3.7 Write `origins` with the kind totals and their `names`. Add dollars to `effort`, and fold the levels outside the list into `other`. Verify: unit tests for "Origins without names", "Names under their kind", "Effort", "Effort levels", "A built-in subagent type", "A user's own names" and "A name that a newer agent added".
- [x] 3.8 Add `whatif` to the Claude Code object. Verify: a unit test for "The same numbers as the whatif report", and that the Codex object has no `whatif`.
- [x] 3.9 Write the header: format 2, `platform`, `survey` and `mcp_names`. Add `--survey`, with its check before reading the logs, and `--mcp-names`. Verify: unit tests for "A survey file", "Without a label", "A label", "A label with a space", "Without the option" and "With the option".

## 4. Names and the leak check

- [x] 4.1 Add the walk over the finished value that splits own words from words to review by position. Verify: unit tests for "The words the file holds", "Models, clients and versions" and "A string of no known kind".
- [x] 4.2 Add the leak check before writing. Verify: unit tests for "A home path in a model name", "A name that is also one of tokprof's words" and "Nothing found".
- [x] 4.3 Extend the distinctive-names test with `/walrus-sync`, a user skill, the MCP server `narwhal`, the subagent type `ocelot` and `./yak-deploy.sh --fast`. Verify: it passes. It fails when a skill name is written into the file on purpose, and passes again once that is taken out.
- [x] 4.4 Print the `words to review:` line after `wrote <path>`. Verify: the integration test in `crates/tokprof/tests/share.rs` sees it, and still sees the last line `Nothing was sent anywhere: review the file, then send it yourself.`

## 5. The pool and the report

- [x] 5.1 Add the share-file reader for formats 1 and 2, with missing parts, and the skip rules: not a share file, a newer format, duplicates by SHA-256. Verify: unit tests for "A folder", "Nothing to pool", "A file sent twice", "Both formats" and "A newer format".
- [x] 5.2 Add the report: the header, the pooled-table columns, `Where the money went`, `What could have been saved`, `Habits that cost the most`, `Tools and commands` and `Versions`. Verify: unit tests over synthetic files for:
  - "Five files", "Files of two surveys" and "A row that two files have";
  - "Origins with names" and "Ranges in order";
  - the three scenarios of What could have been saved;
  - "The habits" and "A format-1 file";
  - "Rows of a shell tool", "Many versions" and "A format-1 file among format-2 files".
- [x] 5.3 Add `tokprof pool`, and refuse the global filters. Add `--min-files`, `--by-file` and `--json`. Verify: unit tests for "A filter", "A rare model", "A rare name", "Three files" and "Shares as numbers".
- [x] 5.4 Print the one-file report from `tokprof share`, from the value it wrote. Verify: integration tests for "The report that share prints" and "What it prints".

## 6. Docs

- [x] 6.1 Write `docs/survey.md`:
  - the prompt from design.md, with `<tag>`, `<label>` and `<recipient>`;
  - how to add `--mcp-names`;
  - how to collect the files, and that the sender is known to whoever collects them;
  - `tokprof pool` with `--min-files 3` before a report is shown to others;
  - what the file holds, and what it never holds.

  Verify: the guide covers each scenario of the Survey guide requirement, and its commands run as written on the author's Mac.
- [x] 6.2 Update README:
  - the share section: format 2's fields, the words to review, and the report it prints;
  - a short pool section, with a link to `docs/survey.md`.

  Verify: the README examples run as written.

## 7. Check on the real logs

- [x] 7.1 Run `tokprof --since 30d share -o /tmp/share-format2.json` 3 times. Verify:
  - `content` and `prompt_sizes` add up to the spend;
  - `tools` gives the rent and trips of `tokprof tools`;
  - `whatif` equals `tokprof whatif`;
  - `session_spans` adds up to `main_sessions`;
  - record the size (budget 150 KB, first estimated at 60 KB) and the median time (budget 5% over the baseline of 1.1).
- [x] 7.2 Search `/tmp/share-format2.json` for `/Users/`, `~/`, the author's project folder names, the first 8 characters of 20 session ids, and the author's own skill, command and MCP server names, from `tokprof flame --stack agent,origin` and `--stack agent,kind,tool`. Verify: nothing is found, and `general-purpose`, `Bash` and `git` are there. Record the words to review that the run printed, without project names.
- [x] 7.3 Pool the files. Verify, and record the numbers:
  - `tokprof pool /tmp/share-format2.json` prints the same report as `share` did;
  - three copies give three times the dollars and the same shares;
  - with `/tmp/share-format1.json` beside it, the tables say `in 1 of 2 files` where format 1 lacks the part;
  - 50 copies under new names, made to differ by one number each, pool in under a second.
- [x] 7.4 Dry-run the prompt on the author's Mac against the dry-run archives of `add-release-builds`, in a new Claude Code session and a new Codex session. Verify:
  - each agent picks `tokprof-aarch64-apple-darwin.tar.gz`, checks it, runs tokprof, shows the output, and sends nothing;
  - record each session's time and cost with `tokprof scan <id>` (aim: under 5 minutes and $1).
- [x] 7.5 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`, then run `cargo test --workspace` and `openspec validate --all --strict`. Verify: all pass.

## Results

| check | baseline | after |
|---|---|---|
| 1.1 share: median of 3 runs and size, 30 days | the build of main (aa59541), format 1: 27.15, 24.57, 25.18 s, median 25.18 s; 27,495 bytes (2026-11-03) | |
| 1.1 tools total line | `spend $11,201 · rent of these tools $6,324 · trips of these tools $8,619` | |
| 1.1 whatif rows | main conversation 32,522 requests: actual $4,375, 5m $5,223, 1h $4,156; subagents and side requests 39,218: $2,803, $2,462, $2,867 | |
| 1.1 summary `Total priced` | $11,199 | |
| 1.1 Codex misses by cause | after assistant message 173, $266; idle > TTL 48, $30.98; params changed 3, $0.98; model switch 3, $0.77; unknown (server side?) 2, $0.40 | |
| 2.4 Codex misses by cause after the version | | the same five rows, to the cent; no `agent updated` |
| 7.1 sums and time | main: 25.18 s | 2026-10-04 to 11-03 (`--until 2026-11-03`): content, prompt sizes, versions and spans each add up to the spend, $9,109.91 for Claude Code and $1,931.60 for Codex; spans count 290 and 170 sessions, the `main_sessions`; tools rent $6,100 and trips $8,363, as `tokprof tools`; whatif $4,187 / $5,034 / $3,966 and $2,851 / $2,540 / $2,915, as `tokprof whatif`; total $11,042, as `Total priced`. 30 days: 3 runs 26.49, 24.52, 25.35 s, median 25.35 s (+0.7%); 105,108 bytes with the names filled in (41,578 before) |
| 7.2 names in the file | | 538 names searched: 328 session id prefixes, 156 project folders, 48 origin names, 6 MCP servers; no `/Users/` or `~/`. Two words match only inside two client names that Codex logs, which the words to review list; `general-purpose`, `Bash`, `git`, `code-review` and `/goal` are in the file. 44 words to review |
| 7.3 pool checks | | the pool of the file equals the report `share` printed (150 lines); 3 copies give 3.0× the dollars and the same shares in every row; with the format-1 file, the content, prompt-size, span, tool and version tables say `in 1 of 2 files`; 50 files pool in 0.03 s |
| 7.5 reinstall and checks | | after a rebase onto 8587e0f (add-failures-and-waits and add-windows-builds landed meanwhile): 385 tests pass, clippy clean, `openspec validate --all --strict` 18 of 18; ~/.local/bin/tokprof reinstalled from e64c325, `share` over 30 days in 25.0 s |
| 7.4 dry-run sessions: time and cost | | the prompt of docs/survey.md against `tokprof-aarch64-apple-darwin.tar.gz` and `SHA256SUMS` served from 127.0.0.1 (no attestation, so that line was left out). Claude Code: 68 s, 5 turns, $0.33; checked the SHA-256, ran tokprof, showed its output, named the file, did not open it. Codex: 2 min, 8 requests, $0.68; the same. Neither sent anything or installed tokprof |

## Workflow follow-up

- The author tags a release that holds this change, fills in the prompt from `docs/survey.md` and sends it to the first group.
- Archive the change with `openspec archive add-team-survey -y` after `add-release-builds`, then run `openspec validate --specs --strict`.
