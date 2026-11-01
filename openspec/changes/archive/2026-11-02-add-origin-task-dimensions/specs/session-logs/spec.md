# Spec Delta

## ADDED Requirements

### Requirement: Claude Code origin and task fields
tokprof SHALL read the origin, task and title of Claude Code work (see cost-attribution) from:
- `attributionSkill`, `attributionMcpServer` and `attributionPlugin` on assistant lines;
- `<command-name>` and `<task-notification>` with its `<task-id>` at the start of user messages;
- `agentType`, `description` and `toolUseId` in the `agent-<id>.meta.json` next to a subagent transcript;
- `ai-title` and `custom-title` lines.

These fields were seen in Claude Code 2.1.284–2.1.312.

#### Scenario: A subagent transcript without a meta file
- **WHEN** `subagents/agent-a1.jsonl` has no `agent-a1.meta.json` next to it
- **THEN** the subagent has no task, its chain keeps its stream name, and its origin comes from the stream's type

### Requirement: Codex origin and task fields
tokprof SHALL read the origin, task and title of Codex work (see cost-attribution) from:
- `session_meta.source.subagent`: `thread_spawn` with `agent_path` and `agent_role`, or a role such as `{"other": "guardian"}` or `"review"`;
- `id` and `thread_name` in `session_index.jsonl` under `$CODEX_HOME`, or `~/.codex` when it is not set.

These fields were seen in Codex 0.146–0.163. The Codex app and TUI write the index; threads started from IDE plugins and `codex exec` were not in it.

#### Scenario: No index
- **WHEN** `session_index.jsonl` does not exist
- **THEN** Codex sessions have no title and the rest of the analysis is unchanged

## MODIFIED Requirements

### Requirement: Chains
tokprof SHALL split a session's requests into chains, one per cache lineage, each with a name and a kind: main, subagent or side. Claude Code requests with request-audit lines form one chain per `agentId` and `querySource` pair; requests without them form chains by `agentId` and `isSidechain`, apart from the audited ones. Each Codex rollout is one chain. A subagent chain is named `subagent: <task>` when its log names the task (see cost-attribution).

#### Scenario: Claude Code main conversation
- **WHEN** an `api-request` in a main transcript has no `agentId` and a `querySource` of `repl_main_thread`, `sdk`, `main` or none
- **THEN** its chain is `main`, of kind main

#### Scenario: Claude Code subagent stream
- **WHEN** the `querySource` is `agent:builtin:Explore` and the subagent has no meta file with a `description`
- **THEN** the chain is `subagent:Explore`, of kind subagent, named after the part after the last `:`, so `agent:custom` gives `subagent:custom`
- **AND** a main-thread `querySource` with an `agentId`, or in a subagent transcript, gives the chain `subagent`

#### Scenario: Claude Code subagent with a task
- **WHEN** a subagent's meta file has `"description": "Find handlers"`
- **THEN** every subagent chain of its transcript is named `subagent: Find handlers`

#### Scenario: Claude Code side streams
- **WHEN** the `querySource` contains `compact`
- **THEN** the chain is `compact`, of kind side
- **AND** any other `querySource` gives a side chain named after it

#### Scenario: Claude Code requests without audit lines
- **WHEN** a response has no `api-request` record
- **THEN** its chain is `subagent` when the line has an `agentId` or is in a subagent transcript, `sidechain` (kind side) when `isSidechain` is true, and `main` otherwise

#### Scenario: Codex rollout
- **WHEN** a rollout is a subagent thread whose `session_meta.source.subagent` is `{"other": "guardian"}`
- **THEN** its chain is `subagent:guardian`: the role is that field when it is a string, or the first string inside it when it is an object, and the chain is `subagent` when there is no role
- **AND** the chain of any other rollout is `main`

#### Scenario: Codex spawned thread
- **WHEN** `session_meta.source.subagent` is `thread_spawn` with `"agent_path": "/root/fix_flaky_tests"` and no `agent_role`
- **THEN** its role is `default`, and its chain is `subagent: fix flaky tests`
- **AND** without an `agent_path` the chain is `subagent:default`
