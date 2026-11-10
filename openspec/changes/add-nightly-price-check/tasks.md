# Tasks

## 1. Setup by the author

- [ ] 1.1 The author creates a fine-grained personal access token with only the Copilot Requests permission and stores it with `gh secret set COPILOT_GITHUB_TOKEN -R ignatov/tokprof`. Verify that `gh secret list` shows it.
- [x] 1.2 Let Actions create pull requests: `gh api -X PUT repos/ignatov/tokprof/actions/permissions/workflow -f default_workflow_permissions=read -F can_approve_pull_request_reviews=true`. Verify that the GET of the same endpoint returns `true`.

## 2. CI on a given ref

- [ ] 2.1 Add the optional `ref` input to `workflow_call` in `ci.yml`, and pass it to every `actions/checkout`. Verify that the run of the next push to `main` and a run of the release workflow's dry run still check out their own commit.

## 3. The check

- [x] 3.1 Write `.github/prices-check.md`, the prompt with the rules from the spec: the two pages, official rates only, no price for unlisted models, periods on a change, the source date only for a changed provider, and no other file. Verify that it states each rule of "Rules for the prices it writes".
- [x] 3.2 Change the header comments of `prices.toml`: the source lines say the date is when the prices were last brought in line, and that a nightly check keeps them in line. Verify that `cargo test -p tokprof-core` passes.
- [x] 3.3 Write `.github/workflows/prices.yml`:
  - triggers: the schedule `23 1 * * *`, and `workflow_dispatch` with a `dry_run` boolean;
  - `concurrency: prices`;
  - job `check` (read-only): Rust, Node, the pinned `@github/copilot`, `copilot -p` with the flags from the design, the file guard, `cargo fmt --all --check`, `cargo test -p tokprof-core --locked`, the diff in the run summary, the file as an artifact, and an output saying whether it changed;
  - job `publish`: commit onto `main`, force-push `bot/prices`, create or edit the pull request with the changed models in its description;
  - job `ci`: `uses: ./.github/workflows/ci.yml` with the pushed commit;
  - job `merge`: `gh pr merge --squash --delete-branch --match-head-commit`.

  `publish`, `ci` and `merge` run only when something changed and `dry_run` is off. Verify with `actionlint` if it is installed, otherwise by the first dry run.
- [x] 3.4 Add a "Prices" paragraph to the README's prices section: the nightly check, and the rule that a model without a public price stays without one. Verify that the paragraph names the workflow file.

## 4. Runs on GitHub

- [ ] 4.1 Dry run on `main`: `gh workflow run prices.yml -f dry_run=true`. Verify that the run succeeds with an empty diff and that no branch or pull request appears.
- [ ] 4.2 Dry run on a scratch branch whose `prices.toml` lacks `claude-fable-5-2`. Verify that the summary's diff adds it at 8 / 10 / 16 / 0.20 / 40, and that nothing is pushed. Delete the branch.
- [ ] 4.3 A real manual run on `main`. Verify that it ends without a pull request, because the prices are current. Record the run's Copilot request count from the run log.
- [ ] 4.4 Run `npx --yes @fission-ai/openspec@1.16.0 validate --all --strict` and verify that it passes.

## Workflow follow-up

- After the first real pull request merges, check that `main` got exactly the `prices.toml` change, then archive the change.
