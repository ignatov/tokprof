# Design

## Context

`crates/tokprof-core/prices.toml` is edited by hand, and its comments name the source pages. The CI workflow (`ci.yml`) runs on pushes to `main`, on pull requests, by `workflow_dispatch`, and by `workflow_call` from the release workflow. `main` has no branch protection, and the repository does not allow auto-merge. The default `GITHUB_TOKEN` is read-only, and Actions may not create pull requests. The author has GitHub Copilot, and the Copilot coding agent can be assigned issues in this repository.

The change touches no tokprof code and no log parsing, so parse time does not change.

## Goals / Non-Goals

**Goals:**
- One workflow file, which shows what ran and what it changed from start to end.
- Copilot's part is bounded by what it may fetch, run and write. Plain shell steps check its result before anything is pushed.

**Non-Goals:**
- Parsing the pricing pages with a script. The pages are tables written for people, and they change layout. A model reads them better than a scraper that breaks quietly.
- Prices on other platforms (Bedrock, Vertex, Foundry), fast mode, and data residency.

## Decisions

### Copilot CLI in an ordinary job, not the coding agent or Agentic Workflows
The job installs `@github/copilot` at a pinned version and runs `copilot -p` with a prompt file.
- The coding agent is started by assigning it an issue. That would open an issue and a pull request every night, even when nothing changed. It also runs behind its own firewall, which would need the two pages allowed in the repository settings.
- GitHub Agentic Workflows (`gh-aw`) gives guardrails, but it needs a compiled `.lock.yml`, a CLI extension to rebuild it, and its pull requests do not start CI without another token. It is more machinery than one file that changes.
- In a plain job, an unchanged night costs one Copilot request and leaves nothing behind.

### Copilot's permissions
Flags: `--no-ask-user`, `--allow-tool='write(crates/tokprof-core/prices.toml), shell(cargo:*)'`, and `--allow-url` for `platform.claude.com` and `developers.openai.com`. It has no git, no network beyond the two hosts, and no write to any other path. Copilot runs in a job of its own, `check`, whose `GITHUB_TOKEN` is read-only and whose checkout keeps no credentials. Only later jobs, which run no model, get write permissions. The model is the CLI's default unless the repository variable `COPILOT_MODEL` names one.

### The prompt lives in the repository
`.github/prices-check.md` holds the rules: the two pages, the field meanings from the header of `prices.toml`, "a model the page does not list gets no price", periods on a change (as `gpt-5.6-sol` has), the date in the source comment only when that provider's prices changed, and "touch no other file". Keeping it in the repository makes a change to the rules a reviewed commit.

### A shell guard after Copilot
`git status --porcelain` must list nothing or only `prices.toml`. Otherwise the step fails. Then `cargo fmt --all --check` and `cargo test -p tokprof-core --locked` run. A broken TOML or a typo in a field name fails here, because the built-in prices are parsed in the tests. `check` then hands the new `prices.toml` on as an artifact. The `publish` job (contents and pull requests: write) copies it over the current `main`, commits as `github-actions[bot]`, and force-pushes `bot/prices`.

### Pull request and merge in the same run
- **Pull request:** `gh pr create`, or `gh pr edit` when one is open from `bot/prices`. This needs the repository setting that lets Actions create pull requests.
- **Checks:** the next job calls `./.github/workflows/ci.yml` with `ref: <the pushed commit>`. A pull request opened with `GITHUB_TOKEN` does not start workflows. Calling CI directly avoids a second personal token with write access. `ci.yml` gets an optional `ref` input that each checkout uses. It is empty in every other event, so those checkouts are unchanged.
- **Merge:** the last job `needs` the CI job. It checks that the head of the pull request is still the commit that was tested, then runs `gh pr merge --squash --delete-branch --match-head-commit <sha>`.
- **Alternative not taken:** branch protection with required checks and GitHub auto-merge. It would also gate the author's direct pushes to `main`.

### Schedule and concurrency
`cron: '23 1 * * *'` is 02:23 in Berlin in winter and 03:23 in summer. Off the hour, GitHub delays scheduled runs less. `concurrency: prices`, without cancelling, keeps a manual run and the nightly one from racing on `bot/prices`.

### Token
`COPILOT_GITHUB_TOKEN` is a fine-grained personal access token of the author's account, with only the Copilot Requests permission. It is used only by the Copilot step. Requests are billed to the author's Copilot plan.

## Risks / Trade-offs

- [Copilot misreads a rate, and the tests pass] → The tests check that the file parses, not that the numbers are right. The pull request description lists every changed model with its old and new rates, so a wrong number is visible in the merged history. Accepted by the author in exchange for no manual merge.
- [A page changes layout or blocks the fetch] → Copilot makes no change and reports it. The prompt asks it to fail rather than guess, and the step then fails the run, which sends the failure email.
- [A night with an unchanged table still costs a request] → About 30 requests a month.
- [The merge pushes to `main` with `GITHUB_TOKEN`, so CI does not run on `main` again] → The same commit was already checked on all three systems before the merge.
- [Scheduled workflows stop after 60 days without repository activity] → GitHub emails before it disables them. The author pushes far more often than that.
- [The token expires] → The Copilot step fails, with a failure email. The author renews the token.

## Migration Plan

1. The author creates the token and stores it with `gh secret set COPILOT_GITHUB_TOKEN`. The setting that lets Actions create pull requests is turned on.
2. Land the workflow, the prompt and the `ref` input of `ci.yml`.
3. Dry runs as in the proposal, then one real manual run.

Rollback: disable the workflow in the Actions tab, or delete the file.
