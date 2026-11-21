## ADDED Requirements

### Requirement: Sessions without usage
For a session whose requests have no usage (see cost-attribution), the X-ray SHALL print after the header the line `cost      <usd> by <counter>; the log gives no usage`, or `cost      the log gives no usage` without an own cost, and leave out the blocks that rest on tokens: context fill, cache, forecast, spend, content, cache misses, advice and setup. The work, tools, rhythm, waits, failures and requests blocks stay.

#### Scenario: A Copilot session
- **WHEN** `tokprof --agent copilot scan 5f0c2a9e` X-rays a session whose record billed 120 credits
- **THEN** the line after the header reads `cost      $1.20 by GitHub's billed credits; the log gives no usage`
- **AND** the `tools` and `waits` blocks follow, and no spend or cache block

#### Scenario: Requests without usage in the requests list
- **WHEN** the same session is X-rayed with `--requests`
- **THEN** each row shows its time, model and calls, with `–` in its token and dollar cells
