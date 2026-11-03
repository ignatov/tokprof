# Design

## Context

- `tokprof share` already runs three sinks in one scan: the summary's collector, the pause collector and the share collector. It writes only allowed keys, so a new field in a collector cannot reach the file without a change to the writer.
- Most of what format 2 needs is computed in that scan already:
  - the tool table and the what-if replay, by the summary's collector;
  - per-session cold returns and largest prompts, by the pause collector;
  - content kinds, as `kind` on every fact the analysis hands to the sinks. The share collector ignores facts today.
- What is not computed yet: prompt-size ranges, session spans, compaction counts, versions in the request records, and the Codex version at all.
- Names: the architecture test allows only a source to name its agent, its tools and its log fields. Program names such as `git` and `sed` are neutral and already live in the core's command rules (cost-attribution).
- On the author's 30 days, the tool rows held 112 tool names, 65 of them MCP tools of 10 servers, and 6,764 tool rows, almost all of them `Bash` and `exec_command` command heads. The non-MCP names were all built-in tools of the two agents, apart from a few Codex names that do not start with `mcp__` and still need checking against the binary.

## Goals / Non-Goals

**Goals:**
- One file per person answers the survey's questions, and the person sees in their terminal what it says.
- Any name that is not public vocabulary stays out of the file, even after a later change adds a field.
- The pool and the one-file report are the same code, reading the same file, so the person sees what the file holds.

**Non-Goals:**
- Changing any other command's output. `summary`, `tools`, `whatif` and `pauses` print as before.
- Statistics beyond medians and quarters, such as confidence intervals. The groups are small.

## Decisions

1. **Shipped names are allow-lists in the sources.**
   - Each source exposes four lists (tools, slash commands, bundled skills, built-in subagent types) and a rule that finds an MCP tool's server.
   - The lists come from the agent's own binary for the versions in the author's logs: Claude Code 2.1.316's bundled code, where the tool definitions, built-in commands, bundled skills and agent types are declared, and Codex 0.166's tool specs.
   - The author's logs are the cross-check. Every non-MCP tool name of the last 30 days is either on the list or left off with a reason in the source's README, such as a Codex name that the binary does not declare.
   - Why allow-lists: a deny-list leaks every name it does not know. An allow-list fails safe, and a new built-in name shows as `other` until a source adds it.
   - Alternative: hashing unknown names with a shared salt. It lets the pool group the same name across files, but a known salt makes names easy to guess, and the file stops being readable.

2. **Programs in the core.**
   - The program vocabulary consists of the lists cost-attribution already uses (read-only, build, version control, scripts), the shell keywords, the wrappers, and the common programs of share-export.
   - A tool row's command head maps to its first word without the path. A word outside the vocabulary maps to `other`.
   - Tool hints are given to the rows before the mapping, by the same rule that `tokprof tools` uses. So `Bash sed -n` and `Bash sed -i` can get different hints and still add up under `sed`.

3. **New parts from the same scan.**
   - `content`: a fixed array indexed by content kind, so each fact costs one addition.
   - `prompt_sizes`: from each request record's usage (the prompt is cache read plus cache write plus uncached input), split by chain kind.
   - `session_spans` and the client additions: the pause collector's per-session data gains the first and last request time, and per client the cold returns and the main sessions. A cold return stays what pause-report defines: a main-conversation miss with the cause `idle > TTL`.
   - `compactions`: from each selected main session's compaction list, within the window.
   - `versions`: request records gain the version. Claude Code already logs it on every line. Codex gets it from the rollout's first `session_meta` (decision 6).
   - `tools`, `tool_hints` and `whatif`: from the summary collector's tool table and what-if replay. They then equal `tokprof tools` and `tokprof whatif` by construction.

4. **Own words and the leak check work on the finished JSON value.**
   - The writer builds the value, then walks it by position. Under fixed parts, a key or string must be one of tokprof's own words. Under `spend` the keys are models, under `clients` clients, and under `versions` versions; the label and `mcp_names` sit in the header. Anything else is "any other".
   - The leak check searches every non-own word, and tokprof prints none of them.
   - The leak check searches only those, for:
     - the home path;
     - the session ids, short ids and titles that the scan saw;
     - the user name (the last component of the home path) and the host name (`COMPUTERNAME` on Windows, the output of `hostname` elsewhere), as whole words of 4 or more characters.
   - Searching only the non-own words avoids false refusals, such as a user named `user` against the content kind `user prompt`. It still catches a bug that puts a project name into a key, since such a key is not an own word.

5. **The pool reads the file, and so does `share`.**
   - `tokprof share` builds the value, checks it, writes it, then gives the same value to the pool's reader and prints the one-file report.
   - The reader maps formats 1 and 2 to one form. Parts that format 1 lacks are marked missing, not zero, so the report can say `in <k> of <n> files`.
   - Duplicates are found by the SHA-256 of the file's bytes, with the `sha2` crate the workspace already uses.

6. **The Codex version comes from the first `session_meta` only.**
   - `cli_version` sits on the first line of every rollout the author has (41 plain rollouts, 0.147–0.166).
   - Taking only the first keeps every request of a rollout on one version. The cache rule that compares the versions of consecutive requests (`agent updated`, see cache-misses) then finds no new Codex misses, and the Codex miss table stays as it is.
   - Using every `session_meta` would be more exact for resumed rollouts, but it would also move Codex misses between causes. That is a separate change, if it is ever wanted.

7. **The platform** is `<os>-<arch>` from the standard library's constants, such as `macos-aarch64` or `linux-x86_64`.

8. **The survey prompt lives in `docs/survey.md`, not in the binary.**
   - The prompt names a release tag, so it is written for a release that already exists. A command that prints it could not know its archives' checksums, which exist only after the release is built.
   - The text below is the one the guide starts from:

   ```
   Please run a survey of my coding-agent logs with tokprof <tag>, an open-source profiler
   (https://github.com/ignatov/tokprof). It reads the Claude Code and Codex logs on this machine
   and writes one JSON file of totals with no names in it. It makes no network calls.

   1. Make a new temporary folder.
   2. Pick the archive for this machine (see release-builds):
      - Mac with Apple silicon: tokprof-aarch64-apple-darwin.tar.gz
      - Mac with Intel: tokprof-x86_64-apple-darwin.tar.gz
      - Linux on x86-64: tokprof-x86_64-unknown-linux-gnu.tar.gz
      - Linux on ARM64: tokprof-aarch64-unknown-linux-gnu.tar.gz
      - Windows on x86-64: tokprof-x86_64-pc-windows-msvc.zip
      - Windows on ARM64: tokprof-aarch64-pc-windows-msvc.zip
   3. Download it and SHA256SUMS from https://github.com/ignatov/tokprof/releases/download/<tag>/
      into the folder. If a sandbox blocks the network, ask me to allow these two downloads.
   4. Check the archive's SHA-256 against its line in SHA256SUMS. If they differ, stop and tell me.
      If gh is installed, also run: gh attestation verify <archive> -R ignatov/tokprof
   5. Unpack the archive in the folder, and from there run
      ./tokprof --since 30d share --survey <label> -o <folder>/tokprof-share.json
      On Windows, run .\tokprof.exe with the same arguments.
      It can take a few minutes on a long history, so allow it that long.
   6. Show me tokprof's output as it is.
   7. Tell me where the file is, and that I should look through it and send it to <recipient> myself.

   Do not install tokprof, change any settings or send anything anywhere. Do not read the file
   or my logs into this chat.
   ```

## Log fields and versions

- Claude Code: `version` on every line (2.1.284–2.1.316); tool names in `tool_use` items; `mcp__<server>__<tool>`; origins from slash commands, skills and subagent types (see cost-attribution, Origin of a request); compaction triggers in `compact_boundary` `compactMetadata.trigger` and the idle notices (see session-logs).
- Codex: `session_meta.payload.cli_version` (0.147–0.166); `originator` as the client (see session-logs); tool names in function calls, `mcp__<server>__<tool>`.

## Parse time

- The extra work is constant per fact, per request and per session: one addition per fact, two map updates per request, and a few per session.
- The leak check walks a value of about 50 KB once. The pool reads a few dozen such files.
- Expected: within 1–3% of today's 24.4 s for `tokprof --since 30d share` over the author's 8.5 GB. The budget is 5%.
- `summary`, `tools`, `whatif` and `pauses` are unchanged, apart from Codex request records carrying a version, which costs one shared string per rollout.

## Risks / Trade-offs

- [A small group can still be told apart] → The files carry no person, and the pool shows per-file columns only with `--by-file`. The guide asks for `--min-files 3` before a report goes beyond the people who sent the files. It also says plainly that a pooled report of five people shows each person's range.
- [Whoever collects the files knows who sent which] → The file is anonymous in its content, not in its sending. The guide says so, and suggests a shared folder that does not show the sender when that matters.
- [The lists of shipped names go stale as the agents release] → They fail safe to `other`. The source READMEs name the versions checked, and the tool rows of a new version show what is missing.
- [A false refusal by the leak check] → Only non-own words are searched, and user and host names only as whole words of 4 or more characters. The message names the kind of name and the field, so the person can report it without showing it.
- [Dollars from different tokprof versions] → Each file is priced by the tokprof that wrote it. A survey pins one release tag in its prompt.
- [The survey costs the colleague money] → The agent's session to download and run tokprof is measured in the dry run. The target is under $1.
- [An agent's tool timeout] → Claude Code's shell tool waits 2 minutes by default. The prompt warns that the run can take a few minutes, and tokprof prints the read line on stderr when it starts reading.

## Migration Plan

1. Before applying, keep a format-1 file from the current binary (`tokprof --since 30d share -o /tmp/share-format1.json`) to test the pool against.
2. Apply after `add-release-builds`, whose dry-run archives the prompt's dry run uses.
3. No data migrates. Format-1 files stay readable by the pool.

## Open Questions

- Which skills are bundled with Claude Code 2.1.316 is read from its binary during apply. The specs do not depend on the answer, since the list is an allow-list either way.
