# Spec Delta

## MODIFIED Requirements

### Requirement: Summary report
`tokprof summary` SHALL print `Spend by model (API-equivalent USD; token counts where no price is known)` and a table, one row per agent and model, most expensive first: `agent`, `model`, `requests`, `prompt known`, five components and `total`. Then come the totals line, the unpriced models (see pricing), the reconciliation line if any, the spend by origin, task, trigger and activity, the setup table (see setup-breakdown), and `Cache misses by cause` (see cache-misses) minus expected rebuilds.

#### Scenario: A model row
- **WHEN** the reported window holds 16,281 requests to `claude-opus-5-5`, all with a rebuilt prompt
- **THEN** its row starts with `claude`, `claude-opus-5-5`, `16281` and `100%`, followed by the dollars of each component and the total, such as `$1,192` or `$0.14`

#### Scenario: Requests and prompt known
- **WHEN** a model had 200 requests with usage in the window, advisor calls included, and 150 of them had a rebuilt prompt
- **THEN** its row shows `200` requests and `75%` prompt known

#### Scenario: Order of the parts
- **WHEN** the window holds priced requests and cache misses
- **THEN** the spend by origin and task comes after the totals and reconciliation lines, then the spend by trigger, the spend by activity and the setup table, all before `Cache misses by cause`
