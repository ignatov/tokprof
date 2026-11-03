# Design

## Context

The rhythm summary of a stretch of time already holds what the four rules need: the breaks with how soon the agents stood still and how much of each they worked, the turns the user started, the turn ends while the user was there and the time at the keyboard, and the questions with their waits. The day view's cold returns come from the pause report's collector, and the pause report holds its own for its time. The X-ray lays out its advice block in the CLI, from texts the library builds.

## Goals / Non-Goals

**Goals:**
- The rules and their texts in the library, next to the summary they read, so that the day view and the pause report give the same items for the same numbers.

**Non-Goals:**
- Advice in the X-ray's own block. A session's rhythm line stays a line.

## Decisions

1. **One function builds the items** from the summary, the cold returns of the time shown and the instruction files of the agents shown. It returns texts in the fixed order; the CLI lays them out as the X-ray does, with the same width of 82.
2. **The thresholds are constants beside the rules:** $10 saved on a warm cache, a median wait of 10 minutes for questions, 30% of a median break, and 5 minutes for both the median turn and the rate of turn ends. They come from the 31 days in the proposal: lower, the cold-returns item fired on 27 days of 31 at $5.
3. **The median break before the cold returns** is the median of their pauses, which the pause report's collector keeps for each return.
4. **The instruction files** are those the agents of the sessions shown declare: `CLAUDE.md` for Claude Code, `AGENTS.md` for Codex, joined with `or`.
5. **The pause report** builds its block from its rhythm summary and the cold returns of all its agents.

## Risks / Trade-offs

- [Advice that fires most days becomes noise] → the four rules fired on 21, 5, 19 and 15 of 31 days, a median of 2 items a day; the thresholds are named in the spec and can move with data.
- [The cold-returns item depends on prices] → without a price it reads the count only and says nothing of money; the rule then does not hold.

## Parse time

Nothing changes in reading or parsing. The rules read numbers the views already have.
