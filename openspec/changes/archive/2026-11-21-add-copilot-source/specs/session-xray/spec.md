## ADDED Requirements

### Requirement: Sessions without usage
For a session whose requests have no usage (see cost-attribution), the X-ray SHALL print after the header the line `cost      <usd> by <counter>; the log gives no usage`, or `cost      the log gives no usage` without an own cost, and then only the waits, tools and failures blocks: the others rest on tokens. In `--requests`, such a request's token and cost cells show `–` and its why `the log gives no usage`.

#### Scenario: A Copilot session
- **WHEN** `tokprof --agent copilot scan 5f0c2a9e` X-rays a session whose record billed 120 credits
- **THEN** the line after the header reads `cost      $1.20 by GitHub's billed credits; the log gives no usage`
- **AND** the `waits`, `tools` and `failures` blocks follow, and no spend, work, cache or profile line

#### Scenario: Requests without usage in the requests list
- **WHEN** the same session is X-rayed with `--requests`
- **THEN** the `requests` line reads `11 requests · the log gives no usage`, and each row shows its time with `–` in its token and cost cells and `the log gives no usage` as its why
