# Proposal

## Why

Providers release models and change prices without notice. Until someone edits `prices.toml` by hand, tokprof shows such a model in tokens with "no price". On 2026-11-10 Claude Fable 5.2 had 27 requests in the author's logs, all without a price. Models tend to launch in the US afternoon, which is the European evening. A check every night means the table is current by the next morning.

## What Changes

- A new GitHub Actions workflow runs every night at 01:23 UTC, and also on demand. It has GitHub Copilot CLI compare `crates/tokprof-core/prices.toml` with the Anthropic and OpenAI pricing pages.
- Copilot may fetch only those two pages and may edit only `prices.toml`. It follows the rules the file already keeps: official prices only, no price for a model the page does not list, and a dated period when a price changes.
- If nothing changed, the run ends and opens nothing.
- If `prices.toml` changed, the workflow checks that no other file changed. It runs the formatting check and the core tests, and commits to the branch `bot/prices`. Then it opens a pull request, or updates the one already open.
- The same run then runs the full checks of every push (the CI workflow) on that commit. If they pass, it merges the pull request into `main`. If they fail, the pull request stays open and the run fails, and GitHub emails the author.
- A `dry_run` input of the manual run stops after showing the diff in the run summary.
- The CI workflow gets an optional input, the ref to check, so that this workflow can call it on the bot's commit.

## Capabilities

### New Capabilities
- `price-updates`: the nightly check of the built-in prices against the providers' pricing pages, and the pull request that brings them in line.

### Modified Capabilities
- `pricing`: the Built-in prices requirement says which date sits next to each source: the day the prices were last brought in line with it, not the day of each check.

## Impact

- New `.github/workflows/prices.yml` and the prompt it gives Copilot, `.github/prices-check.md`. `ci.yml` gets the `ref` input of `workflow_call`.
- Each run uses Copilot requests of the account whose token the secret `COPILOT_GITHUB_TOKEN` holds. That is a fine-grained personal access token with the Copilot Requests permission, which the author creates.
- Repository settings: GitHub Actions must be allowed to create pull requests. Merging needs no branch protection, and `main` has none today.
- Out of scope: cutting a release after a price change, rebuilding the author's local binaries, and models without a public price, such as `codex-auto-review`.
- Check: a manual dry run on today's `main` shows no diff. A dry run on a branch without the Fable 5.2 entry shows that entry added with the page's rates. The first real change is merged only after the CI run passes.
