# Proposal

## Why

Commands that pick or show one session can pick the wrong one, and say nothing:
- Codex thread ids are UUIDv7, so sessions started within a minute share their first 8 characters. In `tokprof --agent codex sessions`, six sessions in three projects all show as `01a197e1`, and six more as `01a1978f`. `scan 01a197e1` silently takes the newest, ui exports of those sessions overwrite each other's `~/tokprof/<id>.jfr`, and reports merge them under one `session` frame.
- A target matches text anywhere after a `-` in the file name, so `scan 2026` picks a Codex rollout by its date, and `scan 14bc-4966` matches the middle of a Claude id.
- `--since`, `--until`, `--project` and `--session` are silently ignored by the default command, `scan`, `handoff`, `sessions` and `ui`. `tokprof --session 3f951de0` X-rays the latest session instead. README says every command takes them.
- `--project ~/src/app` without quotes reaches tokprof as `/Users/me/src/app`, matches nothing, and reports $0 with no error. A trailing `/` names the default profile `.jfr`, a hidden file.
- `tokprof handoff logs/x.jsonl` prints the relative path, which a new session started elsewhere cannot open.

## What Changes

- **Short ids.** Codex short ids become the first three groups of the id, 18 characters such as `01a197e1-4d9f-76b1`: the millisecond timestamp of the UUIDv7 and 12 random bits, since parallel launches share the timestamp. Claude Code ids keep 8. This holds everywhere a short id appears: `sessions`, the X-ray, `misses`, flame `session` frames, ui profile file names and the handoff note.
- **Targets.** A target is a file path, or the start of a session id, ignoring case. A prefix that several sessions share fails and lists them with their short ids, instead of taking the newest.
- **Filters everywhere.** The four filters select the sessions that the default command, `scan`, `handoff`, `sessions` and `ui` pick or list: project and id as in reports, a main log last written on or after `--since`, and created before `--until`. An X-ray still covers the whole log of its session. `prices` is the only command that ignores them.
- **`--session` keeps subagents.** It keeps a Codex session's subagent threads too, as it already does for Claude Code subagents.
- **`--project` accepts what the shell gives.** A value under the home directory is read with `~`, and a trailing `/` is dropped.
- **Handoff path.** `tokprof handoff` prints the log's absolute path.

**BREAKING:** Codex sessions get longer ids in folded stacks and profile file names, and an ambiguous prefix that used to work is now an error.

Out of scope:
- Reporting Codex subagent threads under their parent's id in `summary`, `misses` and `flame`. The X-ray and the session list already join them; the reports do not, as session-logs specifies.
- A session's own time window inside the X-ray.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: short ids (new requirement), the target rule, which commands the filters apply to, the project and session filters.
- `session-xray`: the latest session and target resolution follow the filters and fail on an ambiguous prefix.
- `session-browser`: the session list and the ui list only what the filters select, and show the new short ids.
- `handoff`: the absolute path, and the short id in the note header.
- `cost-attribution`: the `session` dimension is the short id.
- `profile-export`: the default file name uses the project as the filter reads it.

## How it is checked on the real logs

- `tokprof --agent codex sessions --limit 40` shows no repeated id; today 6 rows read `01a197e1` and 6 read `01a1978f`.
- `tokprof scan 01a197e1` fails and lists 6 sessions; `tokprof scan 01a197e1-4d9f` X-rays one.
- `tokprof scan 2026` and `tokprof scan 14bc-4966` fail with no match.
- `tokprof --session 3f951de0` prints the same X-ray as `tokprof scan 3f951de0`.
- `tokprof --project $HOME/src/tokprof summary` costs the same as `tokprof --project '~/src/tokprof' summary`; today it is $0.
- `tokprof --since 7d sessions --limit 1000` lists only sessions written in the last 7 days.

## Impact

`src/model.rs` (short id), `src/xray.rs` (target lookup, latest session, session list), `src/main.rs` (filters for every command, project normalization, default profile name), `src/ui.rs` and `src/watch.rs` (filtered list), `src/handoff.rs` (path, note header). README: the filters table and the Codex id examples.
