# Bring tokprof's built-in prices in line with the pricing pages

You run unattended in GitHub Actions, every night. Nobody answers questions. The workflow
`.github/workflows/prices.yml` checks your result, runs the tests, and merges it when they pass.
A wrong number you write reaches every tokprof user, so read carefully and never guess.

## The task

Compare `crates/tokprof-core/prices.toml` with these two pages, and change the file so that it
matches them:

- Anthropic: https://platform.claude.com/docs/en/about-claude/pricing
- OpenAI: https://developers.openai.com/api/docs/pricing (the Standard tier)

Read the header of `prices.toml` first. It explains every field, how model ids match by prefix,
and how periods work.

## Rules

1. Only the two pages are sources. No other site, no memory of earlier prices, no estimate.
2. A model the page does not list gets no price. Do not add or keep a guessed one. For example,
   `codex-auto-review` is on neither page and has no entry.
3. Use the model id the page or the provider's model list uses: for Anthropic, `claude-` and the
   name in lowercase with hyphens, `claude-fable-5-2` for "Claude Fable 5.2". OpenAI's page
   prints the ids.
4. Map the columns to fields:
   - Anthropic: base input → `input`, 5m cache writes → `cache_write`, 1h cache writes →
     `cache_write_long`, cache hits and refreshes → `cache_read`, output → `output`.
   - OpenAI Standard tier: input → `input`, cached input → `cache_read`, 30m write →
     `cache_write`, 12h write → `cache_write_long`, output → `output`. Prices for more than
     272K input tokens go to `long_context = { above = 272_000, ... }`. Leave out a field whose
     cell is empty or "–".
   - Set `context_window` only when the page states the model's context window.
5. Scope. Every Claude model in Anthropic's table belongs in the file. From OpenAI, only the
   models Codex runs: the GPT-5 family and later (`gpt-5*`, `gpt-6*` and newer), in the
   Standard tier tables. Leave out older families (`gpt-4o`, `o3` and the like), specialized
   models, and `-latest` aliases. Entries already in the file stay.
6. Mind the prefix. An id also prices every id that adds a date, a variant or a tag to it, so
   `o3` would price `o3-mini`. Before adding an id, check that no model with a different price
   on the page starts with it. If one does, it needs an entry of its own too; if it has no
   price, leave both out and say why in the report.
7. A new model goes next to its family, in the order the file already keeps: newer before older.
8. When a rate of an existing model changed, keep the history. The current entry becomes a
   period with `until = <today>`, and a new period with `since = <today>` holds the new rates,
   in the `[[models."id"]]` form that `gpt-5.6-sol` uses. Today is the UTC date given at the end
   of this prompt.
9. When a model is gone from the page, keep its entry: old sessions still need its price.
10. Write `checked <today>` in the source comment of a provider only when you changed that
   provider's prices. Leave the other comment alone.
11. Edit nothing but `crates/tokprof-core/prices.toml`. Keep its comments and layout.
12. Edit the file with the file-editing tool, not with shell commands. Shell is allowed only for
    `cargo`: after a change, run exactly `cargo test -p tokprof-core --locked pricing` from the
    working directory, with nothing before or after it, and fix your edit until it passes.

## The answer

Reply with a short report: each model you added or changed, with its old and new rates, and the
page it came from. Then end with exactly one of these lines:

- `RESULT: UNCHANGED` when the file already matched both pages;
- `RESULT: CHANGED` when you edited it;
- `RESULT: FAILED <reason>` when a page could not be fetched or read, or a table was unclear.
  Leave the file as it was in that case.
