# Pricing Specification

## Purpose
Model prices that turn logged token usage into API-equivalent dollars. Prices come from layered TOML files, can change over time, and `tokprof prices` shows which price applies and where it came from.

## Requirements

### Requirement: Prices in USD per million tokens
Every price SHALL be five rates in USD per million tokens: input (prompt tokens neither read from nor written to the cache), cache write with the short lifetime (5 minutes for Anthropic, 30 minutes for OpenAI), cache write with the long lifetime (1 hour for Anthropic, 12 hours for OpenAI), cache read, and output (generated tokens, reasoning included).

#### Scenario: A request priced from its usage
- **WHEN** a request to a model priced at input $5, short write $6.25, read $0.50 and output $25 used 1,000 uncached, 2,000 short-written, 100,000 read and 500 output tokens
- **THEN** it costs (1,000 × 5 + 2,000 × 6.25 + 100,000 × 0.50 + 500 × 25) / 1,000,000 = $0.08

### Requirement: Missing rates fall back
In a price entry `input` and `output` MUST be given. A missing `cache_read` SHALL default to the input rate, a missing `cache_write` SHALL default to the input rate, and a missing `cache_write_long` SHALL default to the short write rate.

#### Scenario: A model that does not charge for cache writes
- **WHEN** an entry gives only `input = 5.00`, `cache_read = 0.50` and `output = 30.00`
- **THEN** both cache write rates are $5.00, the input rate

#### Scenario: No long write rate
- **WHEN** an entry gives `cache_write = 2.50` and no `cache_write_long`
- **THEN** writes with the long lifetime are priced at $2.50

### Requirement: Built-in prices
The binary SHALL contain the repository's `prices.toml` as its built-in prices. They SHALL cover the Anthropic models and the OpenAI models that Codex uses and that have a public price, with the source and the date each was checked written next to them.

#### Scenario: No configuration at all
- **WHEN** neither a user price file nor `--prices` exists
- **THEN** every request to a built-in model is priced from the built-in prices

### Requirement: Price layers
Prices SHALL be read in three layers: the built-in prices; then `$XDG_CONFIG_HOME/tokprof/prices.toml`, or `~/.config/tokprof/prices.toml` when `XDG_CONFIG_HOME` is unset or empty; then the file given with `--prices`. A later layer's entry for a model SHALL replace every period that earlier layers had for that model.

#### Scenario: The user overrides a model with price history
- **WHEN** the built-in prices give `gpt-5.6-sol` two periods and the user config gives it one entry without dates
- **THEN** the user's entry prices `gpt-5.6-sol` requests of every date
- **AND** `tokprof prices` lists the entry once, with source `user config`

#### Scenario: The user config does not exist
- **WHEN** there is no user price file
- **THEN** tokprof runs with the remaining layers, and `tokprof prices` shows the user config path marked `(not found)`

#### Scenario: The --prices file does not exist
- **WHEN** `--prices /tmp/nope.toml` names a missing file
- **THEN** the command fails with `reading /tmp/nope.toml` and the cause

### Requirement: Price files are validated
A price file with a parse error, an unknown field, a negative or non-finite rate, an invalid date, or a period whose `since` is not before its `until` MUST make the command fail. The error SHALL name the file, and the model when the problem is in one entry.

#### Scenario: A misspelled field
- **WHEN** a price file has `cache_reads = 0.1` under `[models.x]`
- **THEN** the command fails with an error naming the file and `x`, caused by an unknown field `cache_reads` and listing the expected fields

#### Scenario: A negative rate
- **WHEN** a price file has `input = -1`
- **THEN** the command fails with `prices must be non-negative numbers`

### Requirement: Model ids match by prefix
A price entry SHALL price every model id that starts with the entry's id, ignoring case, when the rest is empty, or starts with `-`, `@`, `[`, `:` or `/` and the alphanumeric run right after that character is not a version number of 1–5 digits. When several entries match, the longest id SHALL win.

#### Scenario: A dated model id
- **WHEN** a request uses `claude-sonnet-4-5-20250929`
- **THEN** it is priced by the `claude-sonnet-4-5` entry

#### Scenario: A context-size tag
- **WHEN** a request uses `claude-opus-4-6[1m]`
- **THEN** it is priced by the `claude-opus-4-6` entry

#### Scenario: Another version is not matched
- **WHEN** a request uses `claude-opus-5-7` or `gpt-5.7-sol` and no entry has that id
- **THEN** the request has no price, even though `claude-opus-5` and `gpt-5` exist

#### Scenario: The most specific entry wins
- **WHEN** a request uses `claude-opus-5-5`
- **THEN** it is priced by `claude-opus-5-5`, not by `claude-opus-5`

### Requirement: Prices are time-bound
A model entry MAY be an array of periods (`[[models."id"]]`), each with optional `since` and `until`. A request SHALL be priced by the period in effect when it was sent: `since` included, `until` excluded. Where periods overlap, the one with the latest `since` SHALL win.

#### Scenario: A price cut
- **WHEN** `gpt-5.6-sol` has launch prices `until = 2026-08-21` and cut prices `since = 2026-08-21`
- **THEN** a request sent on 2026-08-20 pays input $5 and output $30
- **AND** a request sent on 2026-09-01 pays input $4 and output $20

#### Scenario: The boundary instant
- **WHEN** one period ends `until = 2026-08-21T17:00:00Z` and the next starts at that instant
- **THEN** a request at 16:59:59Z gets the first period and a request at 17:00:00Z gets the second

### Requirement: A gap between periods has no price
A request sent when no period of its model is in effect SHALL have no price rather than a guessed one.

#### Scenario: A request inside a gap
- **WHEN** a model has periods `until = 2026-01-01` and `since = 2026-02-01`
- **THEN** a request sent on 2026-01-15 has no price

### Requirement: Period dates
`since` and `until` SHALL accept a TOML date (`2026-08-21`), a TOML date-time, or a string with either. A value without an offset SHALL be read as UTC.

#### Scenario: A date-time with an offset
- **WHEN** a period has `since = 2026-08-21T19:00:00+02:00`
- **THEN** it takes effect at 2026-08-21T17:00:00Z

#### Scenario: A word instead of a date
- **WHEN** a period has `since = "soon"`
- **THEN** the price file is rejected

### Requirement: Long-context tier
A period MAY give `long_context` rates with a threshold `above`, in tokens. A request whose prompt (uncached, cache read and cache write tokens together) is over `above` SHALL be priced at the long-context rates. A prompt at the threshold or under it SHALL get the normal rates.

#### Scenario: At and over the threshold
- **WHEN** `claude-haiku-5-5` has input $0.10 and long-context input $0.50 above 100,000 tokens
- **THEN** a 100,000-token prompt pays $0.10 and a 100,001-token prompt pays $0.50 per million input tokens

### Requirement: Context window from the price file
A period MAY give `context_window` in tokens. tokprof SHALL use it for the context fill of a session only when the agent's log does not give the window itself. It SHALL take it from the period in effect now, or from the latest period when none is.

#### Scenario: A Claude Code session
- **WHEN** a Claude Code session uses `claude-opus-5-5`, whose entry gives `context_window = 1_000_000`
- **THEN** the X-ray measures the context fill against 1,000,000 tokens

### Requirement: Logged requests use the prices of their time
Analysis of logged requests SHALL price each request by its own timestamp, so older sessions keep the prices of their time. A forecast of a request not yet sent, such as the next-request cost in the X-ray, SHALL use the prices in effect now.

#### Scenario: Reports after a price cut
- **WHEN** the logs hold `gpt-5.6-sol` requests from before and after 2026-08-21
- **THEN** reports charge the earlier requests the launch prices and the later ones the cut prices

### Requirement: Requests without a price
A request whose model has no price SHALL still count its tokens. Reports SHALL show token counts where no price is known, and SHALL name each unpriced model with its tokens and the path of the user price file to add it to.

#### Scenario: A model with no public price
- **WHEN** the logs hold requests to `codex-auto-review`, which has no built-in price
- **THEN** `tokprof summary` says `No price for codex-auto-review (<n> tokens). Add them to <user price file path>`

### Requirement: The prices command
`tokprof prices` SHALL print the layers in order, each with its path, `prices.toml in the binary` for the built-in one, or the path marked `(not found)` for a missing user config. It SHALL then print a table with one row per period sorted by model: model, input, write short, write long, read, output, when it is in effect, and the source layer. The when column reads `always`, `since <date>`, `until <date>` or `<date> to <date>`.

#### Scenario: A model with two periods
- **WHEN** `tokprof prices` lists `gpt-5.6-sol`
- **THEN** it prints two rows, `since 2026-08-21` and `until 2026-08-21`, with the model name and source only on the first

#### Scenario: A long-context tier
- **WHEN** a period has long-context rates above 272,000 tokens
- **THEN** the next row reads `  over 272k tokens` with those rates

### Requirement: Prices on a given day
`tokprof prices --at YYYY-MM-DD` SHALL list only the periods in effect at the start of that day in local time and SHALL say `Showing the prices in effect on <date>.` A date that is not YYYY-MM-DD MUST be rejected with `bad date "<value>", expected YYYY-MM-DD`.

#### Scenario: Before the price cut
- **WHEN** the user runs `tokprof prices --at 2026-08-15`
- **THEN** `gpt-5.6-sol` shows only its launch prices, `until 2026-08-21`

#### Scenario: An invalid date
- **WHEN** the user runs `tokprof prices --at 2026-13-01`
- **THEN** the command fails with `bad date "2026-13-01", expected YYYY-MM-DD`
