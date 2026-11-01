# Design

## Context

The advice rules return one string per rule, and the X-ray wraps each string at 82 characters under a `▸` marker, 12 columns in. The rules, their inputs and their numbers stay as they are (see proposal.md, What Changes). Only the strings they build and the number of items the carry-on rule returns change.

## Goals / Non-Goals

**Goals:**
- Every item reads as plain sentences and fits in two lines of 82 characters on the author's usual sessions, with the numbers the old text had.
- One place sets the 82-character width of the wrap.

**Non-Goals:**
- Changing the rules' inputs or how the X-ray finds the latest prompt. The change detect-idle-compaction works there.
- Styling inside an item, such as coloring the hint.

## Decisions

- **Sentences, one thought per item, at most two lines.** The first version wrote one telegraphic line per item (`cold: resume +$6.38 (re-writes 817.6k), then $0.16/request`). It halved the block, but the author could not read it without knowing the code. Full sentences with the old repetition removed cost about two lines per item. The fixed texts are measured so that every item fits in two lines with ordinary tool names.
- **One string per item, and the carry-on rule pushes up to three.** The X-ray already prints one `▸` item per string, so splitting resume, new session and `/compact` into items needs no change in the layout code. The other option was to keep one string and split it in the X-ray, but then the layout would have to know the advice wording.
- **A hint is one clause after a colon.** The numbers come first, then `: ` and what to do, such as `Tool results take 45% of the context (Bash 299.9k, Read 63.9k): pipe long command output through tail or grep.` The old hints had a second sentence on why ("is re-read on every request"). README now carries that reason, once for all items.
- **Up to two sources.** The third source was almost always small (ListAgents 1.3k next to Bash 299.9k) and pushed items onto a third line. With two lines allowed, there is no need to drop the second source when the line is long.
- **The width lives with the advice.** One constant of 82 characters sets the wrap. Today the X-ray holds the number by itself. The wrap counts characters, not bytes: `≈`, `→`, `×` and `▸` take several bytes in UTF-8 and one column in the terminal.
- **Pasted prompts: the three largest, shown in turn order.** The largest blocks cost the most to re-read. Turn order keeps the list easy to read next to the conversation. `and <n> more` gives the count of the rest.
- **The hints are fixed strings chosen by the leading source, as now.** Only the text changes. The hint table keeps its keys: tool names, reminder sources and miss causes.

## Risks / Trade-offs

- [Short hints say less: the hint for Bash output drops "is re-read on every request"] → README lists what each item means, with the reasons.
- [Anyone who matched the old sentences breaks] → No command or test outside the advice and its unit tests reads them. A repository search for the old and the one-line phrases checks this before the work is done.
- [Concurrent edits: other sessions change the same functions in the X-ray] → Only the format strings in the advice and the width constant in the X-ray change. The cache state and the latest-prompt code are left to detect-idle-compaction.

No log fields are read, and no agent version matters. Parse time of the full log corpus does not change: the advice runs once per X-ray, after parsing, on a handful of strings.
