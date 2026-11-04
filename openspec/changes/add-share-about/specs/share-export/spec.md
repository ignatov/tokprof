# Spec Delta

## ADDED Requirements

### Requirement: About the person
`--about <key>=<value>[,<key>=<value>...]` SHALL write the person's answers into the header as `about`, with only the keys given: `role`, a string; `agents`, how long the person has used coding agents, a string; and `bother`, what bothers them most when they work with agents, a list of one or two answers in the order given (see Answers about the person).

#### Scenario: Three answers
- **WHEN** `tokprof share --about role=developer,agents=over-1y,bother=waiting+cost` runs
- **THEN** the header has `"about": {"role": "developer", "agents": "over-1y", "bother": ["waiting", "cost"]}`

#### Scenario: A skipped question
- **WHEN** `tokprof share --about role=qa` runs
- **THEN** `about` holds `role` only

### Requirement: Answers about the person
The answers of `--about` SHALL come from closed lists: `role` takes `developer`, `qa`, `devops`, `manager`, `designer`, `researcher` or `other`; `agents` takes `under-3m`, `3-12m` or `over-1y`; `bother` takes one or two of `cost`, `waiting`, `quality`, `context`, `setup`, `limits` and `nothing`, joined with `+`, with `nothing` alone and no answer twice. Anything else fails before any log is read, names the clause and the allowed answers, and writes no file.

#### Scenario: An answer outside the list
- **WHEN** `tokprof share --about role=dev` runs
- **THEN** the command fails with `--about role=dev: role takes developer, qa, devops, manager, designer, researcher or other`

#### Scenario: Three things that bother
- **WHEN** `tokprof share --about bother=cost+waiting+quality` runs
- **THEN** the command fails with `--about bother=cost+waiting+quality: bother takes one or two of cost, waiting, quality, context, setup, limits or nothing, joined with '+'`

#### Scenario: An unknown key
- **WHEN** `tokprof share --about team=core` runs
- **THEN** the command fails with `--about team=core: give <key>=<value>, with the keys role, agents and bother`

### Requirement: Words of the about part
The keys and answers of `about` SHALL be tokprof's own words (see Own words), so the leak check does not search them. A string under `about` that is not one of them is searched.

#### Scenario: The answers about the person
- **WHEN** a file has `about` with `role`, `agents` and `bother`
- **THEN** its words outside tokprof's own are the same as those of the file without `about`

#### Scenario: An edited part
- **WHEN** a string under `about` is not an answer of the lists
- **THEN** the leak check searches it

## MODIFIED Requirements

### Requirement: File header
The file SHALL start with `format` (the number 2), `tokprof` (the version that wrote it), `platform` (the system and CPU it ran on, such as `macos-aarch64` or `linux-x86_64`), `window` (the first and last day of the selected requests, as `YYYY-MM-DD`), `survey`, `mcp_names` and `about` when they were given, and `agents`, one object per agent with requests, keyed by agent name.

#### Scenario: Two agents
- **WHEN** the selected requests come from Claude Code and Codex
- **THEN** `agents` has the keys `claude` and `codex`

#### Scenario: A survey file
- **WHEN** `tokprof share --survey team-2026-11 --mcp-names docs` runs on an Apple silicon Mac
- **THEN** the header has `"format": 2`, `"platform": "macos-aarch64"`, `"survey": "team-2026-11"` and `"mcp_names": ["docs"]`

#### Scenario: Without a label
- **WHEN** neither `--survey`, `--mcp-names` nor `--about` is given
- **THEN** the header has no `survey`, no `mcp_names` and no `about`
