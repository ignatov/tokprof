# Tasks

## 1. The rule

- [x] 1.1 Skip a leading `cd <dir>` up to the first `&&` or `;` in the command head, with unit tests next to it: a `cd` with `;`, a `;` before `&&`, a `;` after the head.

## 2. Checks

- [x] 2.1 Run `tokprof --agent copilot tools` and `tokprof tools` on the real logs before and after; record what changed. (Copilot: the `bash cd /home/runner/…;` row of 1,319 calls is gone, and its calls went to rows such as `bash timeout 3000`, 162 → 206 calls, and `bash python3 -`, 831 → 957. The author's corpus: calls and spend moved only by what live sessions wrote between the two runs (10 calls, $1); `Bash for` entered the top 25 rows.)
- [x] 2.2 Run `cargo fmt --all`, `cargo test --workspace` (the goldens unchanged), `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`.
