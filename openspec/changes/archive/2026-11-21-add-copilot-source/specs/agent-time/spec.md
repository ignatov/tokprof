## MODIFIED Requirements

### Requirement: What the busy time went to
Each moment of busy time SHALL go to the first kind that applies: `model` while a request of a main conversation runs; `API retries` while such a request waits to be sent again after an API error; `subagents` while a subagent of the session works; `environment setup` while a call of that kind runs (see session-logs, Kind of a tool); then what the agent waited on before its next main request (see What the agent waited on between requests); and `nothing` when none applies.

#### Scenario: A request
- **WHEN** a main request is sent at 10:00:00 and its response ends at 10:00:40
- **THEN** those 40 seconds go to `model`
- **AND** a request runs from the send of its answered attempt to the end of its response (see session-logs, Request times)

#### Scenario: Retries
- **WHEN** a request's first attempt is sent at 10:00:00, two attempts fail with API errors, and its answered attempt is sent at 10:00:20 and ends at 10:00:30
- **THEN** 20 seconds go to `API retries` and 10 seconds to `model`

#### Scenario: A request that got no response
- **WHEN** a main request is sent at 10:00 and no response comes, and the user interrupts at 10:03
- **THEN** the 3 minutes go to `model`, since a request with no response runs until the next request or hand-off of its conversation
- **AND** when the request was retried after API errors and no attempt was answered, that time goes to `API retries`

#### Scenario: A subagent while the main conversation waits
- **WHEN** a main response starts a subagent at 10:01, the subagent works until 10:20, and the next main request is sent at 10:21
- **THEN** the 20 minutes go to `subagents`
- **AND** a subagent works from the send of a request to the end of the next one's response while it runs the calls of the first, or when the next comes within 15 minutes

#### Scenario: A subagent resumed later
- **WHEN** a subagent works from 10:04 to 10:10, and a new message resumes it from 12:00 to 12:05
- **THEN** it works for 11 minutes, not for the time between

#### Scenario: A subagent while a main request runs
- **WHEN** a subagent works while a main request runs
- **THEN** that time goes to `model`

#### Scenario: Copilot setup steps
- **WHEN** a Copilot session's typed message is at 10:00:00, its setup steps run from 10:00:00 to 10:06:30, and its first request is sent at 10:06:40 and ends at 10:06:50
- **THEN** 6.5 minutes go to `environment setup`, 10 seconds to `nothing` and 10 seconds to `model`

#### Scenario: A request without usage
- **WHEN** a Copilot request is sent at 10:07:00 and its response ends at 10:07:04
- **THEN** those 4 seconds go to `model`, as for a request with usage
