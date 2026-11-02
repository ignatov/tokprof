# Proposal

## Why

The X-ray says what fills the context and what cost the most to keep there. It does not say what the session did with the money.

Take a 6-message session that cost $19.86, with one subagent. The X-ray showed:
- `tool results Bash sed -n $2.75` at the top of `what cost the most`;
- `35 turns`, although the user typed 6 messages. The turn count was fixed separately: Claude Code's `system` messages had counted as prompts.

The answer the author wanted was per message. One message cost $8.89, $5.03 of it in a subagent. Another cost $8.53 over 74 requests, most of them reading code and editing it.

The trigger (see cost-attribution) says why each request was sent, but `run` lumps builds, git, scripts and file commands together. On 30 days of the author's Claude Code logs, a rough script splits the spend like this:
- reading 44%;
- answering messages 12%;
- editing 8%;
- version control 8%;
- scripts 6%;
- builds and tests 0.5%;
- commands the script could not place 16%, which the tokprof rule should split further.

## What Changes

- **Activity of a request.** A new request-level dimension: what the agent was doing that led to the request.
  - Its values are `reading`, `editing`, `building and testing`, `version control`, `running scripts`, `file and system commands`, `web`, `subagents` and `other tools`.
  - A request that no call triggered takes its trigger's name: `prompt`, `notification`, `compaction`, `continuation`, `advisor` or `unknown`.
  - Shell commands are placed by their program, with fixed lists in the spec.
- **Where activities show:**
  - `--stack activity` in every profile format;
  - a `Spend by activity` table in `tokprof summary`, after `Spend by trigger`;
  - a `by activity` part in the X-ray's work block.
- **A `prompts` block in the X-ray**, after the work block: what each message the user typed cost. Each line gives the message's number, its cost, its requests and the message itself, cut short. Then come its two costliest activities.
  - A message's cost covers the main conversation's requests from it to the next typed message, plus the subagents started in that time.
  - A message is labeled by its last paragraph, since a long message usually starts with pasted text and ends with the request.
  - At most 8 messages are listed, the most expensive ones in their order, then `<n> more`.
  - The line for the $8.53 message would read:

    `3   $8.53   74 req  одобряю add-request-trigger, применяй…  reading $3.10 · editing $2.90`
- **`what cost the most` becomes clearer.**
  - The title says what is ranked: `what cost the most to keep in the context (each source with every request that re-read it)`.
  - Each tool row gets the number of calls behind it, such as `412 calls`. This shows that `Bash sed -n` is many reads, not one.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cost-attribution`:
  - the activity dimension;
  - the classes of calls and shell commands it uses;
  - a `Spend by activity` table, which changes the order of the summary report.
- `session-xray`:
  - the `prompts` block, which changes the layout;
  - the work block, which gains activities and a new condition for showing;
  - the title and call counts of `what cost the most`.
- `session-logs`:
  - the text of the messages the user typed, which sources now keep for the prompts block.

## How it is checked on the real logs

- **Summary, Claude Code, 30 days.** `Spend by activity` sums to `Total priced`. `reading` sits within 10 points of the `lookup` and `lookups` share of `Spend by trigger`, since reading takes most lookups. The commands left for `file and system commands` are under 10% of the spend.
- **The X-ray of the 6-message example session:**
  - 6 prompt lines, or 5 plus the one the log cannot resolve;
  - costs within 10% of the measurement above: $1.86, $8.89, $8.53, $0.57;
  - the costs of the listed messages and `<n> more` add up to the session's spend, minus requests before the first message.
- **Reconciliation** with Claude Code's own cost counter stays within 1.5%.
- **Parse time** of `tokprof summary` stays within 2% of the baseline, as a median of 3 runs.

## Out of scope

- Telling what a message asked for, beyond its own words.
- Grouping messages into tasks.
- Advice by activity.
- Activities of Codex calls beyond what the trigger already reads from them: shell commands and `apply_patch`.

## Impact

- `tokprof-core`:
  - an activity rule for calls and shell commands, next to the read-only rule;
  - the request model gets an activity;
  - the session model gets the typed messages;
  - the dimension, the summary table and the X-ray blocks.
- `sources/claude-code` and `sources/codex`: each call's class and the typed messages.
- `tokprof` CLI: rendering of the new table and blocks.
- README.md.
