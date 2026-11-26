# Spec Delta

## MODIFIED Requirements

### Requirement: Spend of calls without usage
A row whose calls all come from requests without usage, or from no request, SHALL show `–` as its `rent`, `trips` and `after`, and get no hint, since hints need spend. Such rows come after the rows with spend, by their time, then by their calls. When no call of the table has usage, the total line reads `spend – · the logs give no usage, so these calls have no rent or trips`, and the failures line gives no spend after the failures.

#### Scenario: A Copilot row
- **WHEN** `tokprof --agent copilot-cloud tools` shows the row `bash gradlew test` with 300 calls, 40 of them failed
- **THEN** its `rent`, `trips` and `after` cells read `–`, its `failed` cell reads `40`, and it has no hint

#### Scenario: Copilot calls only
- **WHEN** `tokprof --agent copilot-cloud tools` shows 416 failed calls and 29 that rules denied
- **THEN** the lines under the table read `spend – · the logs give no usage, so these calls have no rent or trips` and `failed 416 calls · rules denied 29`
