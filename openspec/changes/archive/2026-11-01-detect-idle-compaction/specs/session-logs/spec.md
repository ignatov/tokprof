# Spec Delta

## ADDED Requirements

### Requirement: Compaction kind
Each Claude Code compaction marker SHALL have a kind. It is `idle` when a `system` line with subtype `informational` whose content starts with `Compacted while idle` comes after the `compact_boundary` and before the next answered request of the transcript (seen in 2.1.302–2.1.310). Otherwise it is the boundary's `compactMetadata.trigger`: `auto` (seen in 2.1.284–2.1.312) or `manual`. Codex markers have no kind.

#### Scenario: Compacted while idle
- **WHEN** a `compact_boundary` with trigger `auto` at 19:13:39 is followed by the notice "Compacted while idle, before the prompt cache expired" at 19:13:40, and the next answered request comes at 19:19
- **THEN** the marker's kind is `idle`

#### Scenario: Compacted because the window filled up
- **WHEN** a `compact_boundary` with trigger `auto` has no such notice before the next answered request
- **THEN** the marker's kind is `auto`

#### Scenario: Compacted on request
- **WHEN** a `compact_boundary` has trigger `manual`
- **THEN** the marker's kind is `manual`

#### Scenario: A notice after the next request
- **WHEN** the notice comes only after the next answered request
- **THEN** it does not change the kind of the earlier marker

#### Scenario: Codex compaction
- **WHEN** a Codex rollout has a `compacted` line
- **THEN** its marker has no kind
