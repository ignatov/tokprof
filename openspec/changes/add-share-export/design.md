# Design

## Context

Every number in the file is already computed by a report:
- the spend, client, origin, trigger, activity, setup and miss tables by the summary's collector (cost-attribution, setup-breakdown, cache-misses, and the client table of `add-client-dimension`);
- the pause parts by the pause collector of `add-pause-report`.

Only two parts are new:
- cache writes by lifetime and chain kind, from the requests' usage;
- requests and output by effort level, from the requests' parameters.

Effort is already read from both agents' logs: Claude Code `params.output_config.effort` on `api-request` lines (2.1.308+), and Codex `payload.effort` on `turn_context` lines. It is not yet passed to the sinks.

## Decisions

1. **One pass, three collectors.** `share` runs the summary collector, the pause collector and a small share collector in one scan. It joins the two sink types, as the summary already does. The file then equals the two reports by construction.
2. **Names are dropped where the file is built, not filtered afterwards.**
   - The writer takes only the allowed keys: model, client, cause, trigger, activity, setup source, effort level and the fixed range names.
   - Origins are cut to their kind, the part before `:`.
   - Nothing else is copied from the collectors: no session ids, task labels, titles, projects or tool rows. A new field in a collector therefore cannot leak into the file without a change to the writer.
3. **Setup sources.**
   - The setup table names sources by kind: `instructions`, `skills`, `tool schemas`, and the like.
   - A reminder kind that a source names after a file or a server could carry a name. The writer keeps a fixed list of the source kinds that setup-breakdown defines, and files every other source under `other`.
4. **Numbers.**
   - Dollars are written with 4 decimals and tokens as integers, so a file can be summed with others.
   - Shares are not written; they follow from the counts.
5. **The format number** starts at 1. A later change that renames or removes a field raises it.
6. **Effort reaches the sinks.** The request record that the analysis hands to sinks gains the effort as the log names it. The share collector counts requests and output tokens per level, split by main and subagent chains.

## Risks / Trade-offs

- **Small samples can still identify work.** A user with one session in the window shares that session's spend and pauses. The README says so. It also suggests a window of 30 days or more.
- **Client and model names stay.** They are product names, not the user's names. A client value is the agent's own name for its host, such as `codex_exec` or `sdk-ts`.

## Parse time

`share` reads the logs once, like `summary`. The extra collectors do constant work per request. It should take about as long as `summary`, and no longer than `summary` and `pauses` together.
