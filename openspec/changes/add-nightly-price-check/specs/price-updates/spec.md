# Spec Delta

## Purpose

Keeps the built-in prices in line with the providers' pricing pages. A nightly check brings a new model or a changed price into `main` without waiting for someone to notice that it is missing.

## ADDED Requirements

### Requirement: Nightly check
The repository SHALL compare the built-in prices with the Anthropic pricing page (platform.claude.com/docs/en/about-claude/pricing) and the OpenAI pricing page (developers.openai.com/api/docs/pricing) every night at 01:23 UTC. A maintainer SHALL also be able to start the check by hand.

#### Scenario: A quiet night
- **WHEN** the built-in prices already match both pages
- **THEN** the check ends successfully, with no commit and no pull request

#### Scenario: A manual run
- **WHEN** a maintainer starts the check from the Actions tab
- **THEN** it runs exactly as the nightly check does

### Requirement: What the check may change
The check SHALL change only `crates/tokprof-core/prices.toml`, and it SHALL read only the two pricing pages. A run whose result changes any other file MUST fail without a commit.

#### Scenario: Another file changed
- **WHEN** the check's result also changes a source file
- **THEN** the run fails, and nothing is committed or merged

### Requirement: Rules for the prices it writes
The check SHALL add a model only with the rates its provider's page prints, and SHALL leave a model that the page does not list without a price. A rate that changed SHALL become a new period whose `since` is the day of the check. The previous rates SHALL stay as a period ending that day. The source comment of a provider whose prices changed SHALL carry that day's date.

#### Scenario: A new model
- **WHEN** the Anthropic page lists `claude-fable-5-2` at $8 input, $10 and $16 for cache writes, $0.20 for cache reads and $40 output, and the built-in prices lack it
- **THEN** the check adds an entry for `claude-fable-5-2` with exactly these rates

#### Scenario: A changed price
- **WHEN** on 2026-12-01 the OpenAI page gives `gpt-5.6-sol` an input rate other than the built-in $4.00
- **THEN** the old rates end with `until = 2026-12-01`, and the new rates start with `since = 2026-12-01`

#### Scenario: A model without a public price
- **WHEN** Codex logs requests to `codex-auto-review`, which neither page lists
- **THEN** the check adds no price for it

### Requirement: Pull request of a change
When the check changes the prices, it SHALL run the formatting check and the core tests, and commit the change to the branch `bot/prices` on top of the current `main`. It SHALL then open one pull request into `main`, or update the one already open, with the providers and models it changed in its description.

#### Scenario: A pull request already open
- **WHEN** an earlier night's pull request is still open and the prices change again
- **THEN** that same pull request is updated, and no second one is opened

### Requirement: Merge after the checks
The pull request of a change SHALL be merged into `main` only after the checks of every push (see release-builds) pass on its commit. When they fail, the pull request SHALL stay open, and the run SHALL fail.

#### Scenario: Green checks
- **WHEN** the tests, formatting, clippy and the OpenSpec specs pass on the commit of `bot/prices`
- **THEN** the pull request is merged into `main`

#### Scenario: A failing test
- **WHEN** a test fails on Windows on that commit
- **THEN** the pull request stays open, the run fails, and nothing reaches `main`

### Requirement: Dry run of the check
A manual run with `dry_run` set SHALL show the diff it would commit in the run summary. It SHALL push, open and merge nothing.

#### Scenario: Trying the check
- **WHEN** a maintainer starts the check with `dry_run` on a branch whose `prices.toml` lacks `claude-fable-5-2`
- **THEN** the run summary shows the entry it adds, and the repository is unchanged
