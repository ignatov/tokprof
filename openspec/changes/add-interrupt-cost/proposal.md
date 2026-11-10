# Proposal

## Why

tokprof counts when the person stops the agent, but not what the stopped turn cost, what the agent was doing or what came next, so `interrupts` is the one pain in `Pains side by side` with no money. It also does not say how the agent's turns end: did it change files and wait, ask, fail or just answer? Nor does it say whether a well-specified long run is cheaper than step-by-step ping-pong. Two of four people in the pilot said `quality` bothers them, and no measured pain stands behind that answer.

A prototype over the author's last 30 days of Claude Code logs gives the shape:
- 1,511 turns the author started: 52% ended with an answer only, 30% after changing files without a commit (36% of the spend, a median 6.9m long), 9% after a commit, 4.5% with a question in the text, 1.9% stopped (1.5% of the spend).
- 22 of 36 stopped turns were stopped during a shell call. 19 of them were followed by a new message within 5 minutes.
- Turns of 15m–1h made 75 edit calls per 1% of the spend; turns under 1m made 18, and turns of 1–5m made 37.
- Of 2,358 logged turn ends with text, 27% ask the person something, 38% wait for the agent's own background work, and 35% report. Ending with `?` finds only 13%, less than half of the asks.

## What Changes

- **Turns get their requests and money.** Each turn of work-rhythm owns the requests sent during it, its subagents' included. It also owns their dollars and their tool calls.
- **How a turn ends.** Each turn ends in one of seven ways, the first that applies: `stopped`, `failed`, `asked`, `waiting` (for its own background work), `committed`, `changed` or `answered`. `asked` comes from a small rule on the last paragraph of the agent's last response: a `?`, a request, permission or an offer, in Russian and English. It also records what came next: how soon the person typed again in that session, or `none` when the session ended there.
- **Stopped turns.** For each stopped turn: its dollars, the time until the stop, the activity of the agent's last call, its edit calls, and how soon the person typed again.
- **Run length.** The turns the person started, by length (`under 1m` to `1h+`), with dollars, edit calls, commits and stops. This tests whether long runs are cheaper per edit. `/goal` turns are counted on their own.
- **Where it shows:**
  - `tokprof` X-ray: an `ends` line under `rhythm`.
  - `tokprof pauses`: a section `How your turns end`, with the tables `Stopped turns` and `Run length`.
  - `tokprof share`: a `turns` part. Its dollars become shares like every other amount, and it holds no text.
  - `tokprof pool`: the sections `How turns end`, `Stopped turns` and `Run length`.
  - In `Pains side by side`, `interrupts` gets money (`spent`, the dollars of the stopped turns).
  - The answer `quality` of `What people said` gets `interrupts` as its pain.

## Capabilities

### New Capabilities
- `turn-outcomes`: what each turn of the agent cost, how it ended, what the person did next, the stopped turns, and turns by length.

### Modified Capabilities
- `session-xray`: adds the `ends` line.
- `pause-report`: adds the section `How your turns end`.
- `share-export`: adds the `turns` part.
- `share-pool`:
  - adds three sections;
  - changes `Order of the sections`, `Section notes`, `Pains side by side`, `Money and time of a pain` and `The pains of each answer`.

## Impact

- **Code:**
  - the rhythm module: turns get requests;
  - a new turn-outcomes module in tokprof-core;
  - X-ray, pauses, share and pool rendering.
- **Logs:**
  - no new fields beyond those session-logs already reads for hand-offs, call outcomes and assistant text;
  - Claude Code 2.1.310–2.1.316, Codex 0.147–0.166.
- **Share files:**
  - a new part, and the format stays 3;
  - older files lack it, and the pool marks the new sections `in <k> of <n> files` as for any missing part.
- **Check on the real logs:**
  - `tokprof --since 30d pauses` must count as many stopped turns as the rhythm counts interrupts (52 today, over all agents), and as many turns as it counts ends and interrupts;
  - the kinds' shares should land near the prototype's numbers above;
  - the dollars of all turns must equal the spend of the main sessions the rhythm keeps.
- **Out of scope:**
  - keeping the text of any message: only the source reads the last paragraph, and it passes on whether it asks;
  - judging whether the person reviewed a diff;
  - new advice text;
  - an `At a glance` line;
  - features in Air;
  - Junie and OpenCode.
