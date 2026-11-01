# Design

## Short ids: a fixed length per agent

Two options were weighed:

1. The shortest unique prefix, as git does. It needs every session known before any id is printed, and an id printed today can become ambiguous tomorrow.
2. A fixed length per agent. Claude Code ids are UUIDv4: 8 hex characters are 32 random bits, so 1,000 sessions collide with a chance of about 1 in 10,000. Codex ids are UUIDv7: the first 12 hex digits, 13 characters with the dash, are the creation time in milliseconds, and the third group holds the version and 12 random bits.

Option 2 is chosen: it is stable, needs no global view, and keeps prefix semantics.

The first draft took 13 characters for Codex, the whole timestamp. Measured on the author's 793 Codex rollouts, sessions that share a short id:

| Codex short id | Sessions sharing one | Groups |
|---|---:|---:|
| 8 characters, as before | 220 | 72 |
| 13 characters, the timestamp | 11 | 3 |
| 18 characters, three groups | 0 | 0 |

The 11 come from launches in one millisecond, such as the reference bench starting `codex exec` runs in parallel. So the short id is the first three groups. Four threads launched together then share one with a chance of about 0.15%, and the ambiguity error catches the rest.

## Ambiguous prefixes fail

When two selected main sessions match a prefix, tokprof fails and lists up to 10 of them, newest first, with short id, time, agent and project. Silently taking the newest is what made the Codex collisions invisible.

## Filters for commands that pick sessions

Reports filter requests by timestamp. Session commands need a session-level rule that is cheap, since the session list must stay fast:
- `--since`: the main log was modified on or after it. This is the "last active" time the list already sorts by.
- `--until`: the main log was created before it. The file's birth time is the start of the session; when the file system has none, the modification time is used.
- `--project` and `--session`: the project and id that the list already reads from the first lines.

Target lookup searches only the selected sessions, so `--project x scan <id>` fails when `<id>` belongs to another project. An explicit file path bypasses the filters, since it names its file exactly.

## Project normalization

`--project` is normalized once, at startup: a value that starts with the home directory gets `~` in its place, and a trailing `/` is dropped. Every consumer then sees the same text: the filter, the session commands and the default profile name.
