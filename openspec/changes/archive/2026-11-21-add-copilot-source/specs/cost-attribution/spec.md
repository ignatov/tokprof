## ADDED Requirements

### Requirement: Requests without usage
A request whose log gives no usage, as session-logs says for Copilot requests, SHALL keep its times, model, prompt and calls, and count as answered when its response is in the log. It adds no tokens and no cost to any share of spend, so it never counts as zero tokens: a total or mean of tokens or dollars leaves it out. Its model gets no `No price for` line (see pricing).

#### Scenario: A Copilot request
- **WHEN** a Copilot request made two `bash` calls and its response is in the log
- **THEN** it counts as a request of its model, its calls count in the tool tables, and it adds no tokens and no dollars

#### Scenario: Mixed agents
- **WHEN** `tokprof --agent claude,copilot summary` runs over 100 Claude Code requests that cost $12.00 and 300 Copilot requests
- **THEN** the priced total is $12.00 and the token totals are those of the Claude Code requests

### Requirement: Summary rows without usage
In `tokprof summary`, a row of an agent and model whose requests in the window all have no usage SHALL show its `requests` and `prompt known`, and `–` in the five components and in `total`. Such rows come after the rows with usage. When no request of the window has usage, the `Total priced` line is left out, and after the own-cost line one line replaces the other parts: `The logs give no usage: no spend by client, origin, task, trigger or activity, no setup and no cache misses.`

#### Scenario: Copilot rows
- **WHEN** the window holds 9,000 Copilot requests of `claude-opus-5-5` and 6,000 of `claude-fable-5-2`
- **THEN** the table ends with the rows `copilot claude-opus-5-5 9000` and `copilot claude-fable-5-2 6000`, each with `–` in every component and in `total`

#### Scenario: Copilot sessions only
- **WHEN** `tokprof --agent copilot summary` runs
- **THEN** the model table and the own-cost line are followed by `The logs give no usage: no spend by client, origin, task, trigger or activity, no setup and no cache misses.` and nothing else

### Requirement: Own cost without usage
When reported sessions whose requests have no usage have an own cost (see session-logs, Agent-reported cost), `tokprof summary` SHALL print `<counter> for <n> sessions: <usd>; their logs give no usage, so tokprof has no figure to compare`, with the agent's counter name. The filters select the sessions as Reconciliation with the agent's own cost counter does.

#### Scenario: Copilot sessions
- **WHEN** 40 Copilot sessions in the window have records with credits that sum to $512.34
- **THEN** the line reads `GitHub's billed credits for 40 sessions: $512; their logs give no usage, so tokprof has no figure to compare`, with dollars written as everywhere in tokprof

#### Scenario: No credits
- **WHEN** no reported Copilot session has a record with credits
- **THEN** the line is left out
