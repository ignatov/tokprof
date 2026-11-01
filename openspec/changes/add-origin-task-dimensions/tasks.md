# Tasks

## 1. Reading origin, task and title from the logs

- [x] 1.1 Claude Code:
  - responses carry `attributionSkill`, `attributionMcpServer` or `attributionPlugin`, or else the origin of the main conversation's turn;
  - slash commands and notifications start a turn;
  - interruptions, compaction summaries and shell commands carry the turn on.

  Verify: `requests_carry_the_origin_of_their_work` passes.
- [x] 1.2 Claude Code subagents:
  - read `agent-<id>.meta.json` for the type and description;
  - name the chain `subagent: <description>`;
  - link a notification's task id through `toolUseId` to the response that started the subagent.

  Verify: `subagents_are_named_by_their_task_and_credited_to_what_started_them` passes.
- [x] 1.3 Claude Code titles: the latest `custom-title` wins over `ai-title`, and `[archived]` is stripped. Verify: the title assertion in 1.1's test.
- [x] 1.4 Codex:
  - a spawned thread's role is `default` and its task is the end of its `agent_path`;
  - `guardian` and `review` threads keep their role;
  - the title comes from `session_index.jsonl`, the parent's for a subagent thread, and the index is read again when its size changes.

  Verify: `subagent_threads_get_their_task_and_origin` passes.

## 2. Spend by origin and task

- [x] 2.1 The attribution walk:
  - splits each chain's sums by request origin;
  - every share of spend carries origin and task;
  - untitled sessions get `untitled <id>`.

  Verify: `spend_goes_to_the_origin_of_each_request` passes, including the check that whole-request sums equal the shares.
- [x] 2.2 `origin` and `task` stack dimensions. Verify: `tokprof --agent claude --since 2026-10-02 flame --stack origin,task,kind,tool,component -o x.collapsed` writes stacks that start with origins such as `skill:code-review`.
- [x] 2.3 `tokprof summary` prints `Spend by origin` and `Most expensive tasks` after the reconciliation line.
  - Verify: `a_task_shows_the_origin_most_of_its_spend_came_from` passes.
  - Verify: the spend-by-model table is unchanged against the previous binary (`claude-opus-5-5` total $9,218 in both runs).
- [x] 2.4 README: the summary line, the `--stack` dimensions with a `work.html` example, and an "Origin and task" section. Verify: the section names every field and version used.

## 3. X-ray

- [x] 3.1 A `title` line in the header, and a `work` block after `spend`:
  - origins as bars, with the sixth and later folded into `<n> more`;
  - up to 6 tasks;
  - left out when there is one origin and one task.

  Verify: `tokprof scan 03aee587` shows `skill:code-review` at 96.5% ($508 of $526), with `Angle C cross-file tracer` as the top task.
- [x] 3.2 README's X-ray list names the title and the work block. The UI pane shows the X-ray text unchanged. Verify: the README list.

## 4. Checks on the real logs

- [x] 4.1 30 days of Claude Code (`--since 2026-10-02`), against the measurement script. Verify: skills about 27% (code-review $1,720), `/goal` $120, `/model` $65, `notification` $448 for background shell commands. Recorded in proposal.md.
- [x] 4.2 Parse time and memory against the previous binary. Verify: 23.5/23.9 s against 23.6/25.1 s, and 250 MB against 244 MB peak. Recorded in design.md.
- [x] 4.3 `cargo test` and `cargo clippy --tests` pass with no warnings.
- [x] 4.4 Reinstall the binary the author runs with `cargo install --path . --root ~/.local`. Verify: `tokprof scan 03aee587` shows the `work` block.

## Workflow follow-up

- The author reviews this change. After approval, archive it with `/opsx:archive` and run `openspec validate --all --strict`.
