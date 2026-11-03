## ADDED Requirements

### Requirement: Request times
Each request SHALL have, when its log tells, the time its first attempt was sent, the time its answered attempt was sent, and the time its response ended. A request that was not retried has one send. When the log tells no time before a request's response, the request was sent at the first line of its response, and a response never ends before its send.

#### Scenario: Claude Code with request-audit lines
- **WHEN** a response's lines carry a `requestRef` that names an `api-request` line (seen in Claude Code 2.1.300–2.1.316)
- **THEN** its answered attempt was sent at that line's `timestamp`
- **AND** its response ended at the latest `timestamp` of the assistant lines of its `message.id`

#### Scenario: Claude Code without request-audit lines
- **WHEN** a transcript, or the part of it written before the audit lines, has no `api-request` lines (Claude Code 2.1.284–2.1.299)
- **THEN** a request was sent at the `timestamp` of the latest `user` line of its chain before the first line of its response
- **AND** `attachment` lines do not count, since Claude Code writes some of them with the response

#### Scenario: Codex
- **WHEN** a Codex request comes from a `token_usage_record` line or a `token_count` event
- **THEN** its response ended at the `timestamp` of the last item the response added (see Codex prompts), or at the request's own line when it added none
- **AND** it was sent at the `timestamp` of the latest `response_item` before those items, in Codex 0.147–0.166

### Requirement: Retried requests
A Claude Code request SHALL count as retried when API errors came before its answered attempt: an `api-request` line followed by a `system` line of `subtype` `api_error` is an attempt that failed, and the next `api-request` of its chain repeats it (seen in 2.1.287–2.1.316). The request keeps how many attempts failed and the `error.formatted` text of the last error.

#### Scenario: Three failed attempts
- **WHEN** `api-request` lines at 07:04:05, 07:04:06 and 07:04:07 are each followed by an `api_error` line with `retryAttempt` 1, 2 and 3, and the `api-request` at 07:04:09 gets the response
- **THEN** the request has 3 retries, its first attempt was sent at 07:04:05 and its answered attempt at 07:04:09
- **AND** its last error is `502 local proxy: upstream request failed` when that is the last line's `error.formatted`

#### Scenario: Without request-audit lines
- **WHEN** a transcript has no `api-request` lines, and `api_error` lines of a request's chain come after its send and before its response
- **THEN** they are its retries, and its answered attempt was sent at the last error's `timestamp` plus its `retryInMs`

#### Scenario: No attempt answered
- **WHEN** every attempt of a request failed and a `<synthetic>` message reports the error, in a transcript with `api-request` lines
- **THEN** the request has no response, and keeps its retries and its last error
- **AND** without `api-request` lines such a request leaves no request at all, as any request without a response does

#### Scenario: Codex
- **WHEN** a Codex rollout is read
- **THEN** its requests have no retries, since Codex 0.147–0.166 logs none

### Requirement: Call outcomes
Each source SHALL tell, for every tool result, whether its call failed, the user refused it, or a permission rule denied it. A result with none of these belongs to a call that worked.

#### Scenario: Claude Code failure
- **WHEN** a `tool_result` item has `is_error` set to `true` (seen in Claude Code 2.1.287–2.1.316), and the call was neither refused nor denied
- **THEN** the call failed

#### Scenario: Claude Code refusal
- **WHEN** a result with `is_error` set has a text that starts with `The user doesn't want to proceed with this tool use`, `User rejected`, `[Request interrupted by user for tool use]` or `MCP tool call has been cancelled likely by a user interaction`
- **THEN** the user refused the call

#### Scenario: Claude Code denial
- **WHEN** the `user` line that holds the result has `permissionDecision.decision` set to `reject` (seen in 2.1.300–2.1.316)
- **THEN** a permission rule denied the call
- **AND** so it did when the text starts with `Permission to use ` and holds ` has been denied`, or starts with `Permission for this command was denied by a built-in`

#### Scenario: A search that found nothing
- **WHEN** a `Bash` result has `toolUseResult.returnCodeInterpretation` `No matches found` and no `is_error`
- **THEN** the call worked

#### Scenario: Codex failure
- **WHEN** a `custom_tool_call_output` starts with `Script failed`, or one of its text items is a JSON object whose `exit_code` is not 0 (seen in Codex 0.147–0.166)
- **THEN** the call failed
- **AND** so did a direct command whose output has a line `Process exited with code <n>` with `n` other than 0, as `function_call_output` reports it (seen in 0.147–0.164)

#### Scenario: Codex refusal
- **WHEN** a Codex tool output starts with `aborted by user`
- **THEN** the user refused the call

### Requirement: Error line of a failed call
A failed call SHALL keep one line that tells why it failed, cut to 200 characters: the first non-empty line of its result's text. A result that tells an exit code gives `exit code <n>: ` followed by the last non-empty line of the command's output instead, and a Codex script error gives the first non-empty line after `Script error:`.

#### Scenario: A missing file
- **WHEN** a `Read` result's text is `File does not exist. Note: your current working directory is /Users/me/src/app.`
- **THEN** that is its error line

#### Scenario: A Claude Code command
- **WHEN** a `Bash` result's text is `Exit code 101` followed by test output whose last non-empty line is `error: test failed, to rerun pass --lib`
- **THEN** its error line is `exit code 101: error: test failed, to rerun pass --lib`

#### Scenario: A Codex command
- **WHEN** an `exec` result prints `"exit_code":1` with an `output` whose last non-empty line is `ModuleNotFoundError: No module named 'bs4'`
- **THEN** its error line is `exit code 1: ModuleNotFoundError: No module named 'bs4'`

#### Scenario: A Codex script error
- **WHEN** an `exec` result starts with `Script failed` and holds `Script error:` followed by `SyntaxError: Unexpected token ')'`
- **THEN** its error line is `SyntaxError: Unexpected token ')'`, the first non-empty line after `Script error:`

### Requirement: What a wake reports
Each source SHALL tell what each wake reports: that a subagent of the session finished or is waiting, that a command the agent ran in the background finished or printed, or something else.

#### Scenario: Claude Code subagent
- **WHEN** the `<summary>` of a `<task-notification>` starts with `Agent "` (seen in Claude Code 2.1.291–2.1.316)
- **THEN** the wake reports a subagent

#### Scenario: Claude Code background command
- **WHEN** the `<summary>` starts with `Background command "` or `Monitor ` (seen in 2.1.293–2.1.316)
- **THEN** the wake reports a background command

#### Scenario: Something else
- **WHEN** the wake is a goal check-in, a notice that Claude Code writes when a session resumes, such as `Background shell command didn't finish before the previous session end`, or a `<cross-session-message`
- **THEN** it reports something else

#### Scenario: Codex
- **WHEN** a Codex rollout is read
- **THEN** it has no wakes to report, since Codex logs none (see Hand-offs between the user and the agent)
