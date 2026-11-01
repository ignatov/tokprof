# Spec Delta

## MODIFIED Requirements

### Requirement: Stack dimensions
`--stack` SHALL set the frames of every stack, root first, as a comma-separated list of the dimensions that cost-attribution defines; the default is `agent,project,session,chain,turn,kind,tool,target,component`. Empty entries in the list are ignored, and a dimension that has no value for a cost adds no frame to that cost's stack.

#### Scenario: Default stack
- **WHEN** no `--stack` is given
- **THEN** each stack runs agent, project, session, chain, turn, kind, tool, target, and ends with the cost component

#### Scenario: Unknown dimension
- **WHEN** the user passes `--stack agent,foo`
- **THEN** the command fails before reading any log with `unknown dimension "foo"; use ` followed by every dimension name, comma separated

#### Scenario: A dimension without a value
- **WHEN** the stack has `tool` and the cost is for the system prompt, which no tool produced
- **THEN** that stack has no tool frame

#### Scenario: A stack with no frames left
- **WHEN** none of the `--stack` dimensions has a value for a cost, as with `--stack tool` and the system prompt, or in JFR when every stack dimension is also a thread dimension
- **THEN** that cost is left out of the profile

#### Scenario: Separators inside values
- **WHEN** a value, such as a command line in `target`, contains `;` or a line break
- **THEN** each of them is replaced by a space in every format, so folded lines still split into the right frames

#### Scenario: Stacks by origin and task
- **WHEN** the user passes `--stack origin,task,kind,component`
- **THEN** every stack starts with the request's origin, such as `skill:code-review`, then its task, such as `Angle C cross-file tracer`, and every cost has both frames
