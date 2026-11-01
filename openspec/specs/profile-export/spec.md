# Profile Export Specification

## Purpose
`tokprof flame` turns the attributed spend of the selected sessions into profile files for profilers and flame-graph tools: JFR for IntelliJ Profiler and JDK Mission Control, async-profiler's HTML flame graph, collapsed stacks for its jfrconv converter, pprof, and plain folded stacks. This spec covers where the files go, how they are named, and the conventions inside each format that those tools rely on.

## Requirements

### Requirement: Default output file
Without `-o`, `tokprof flame` SHALL write a `.jfr` file into the profile directory: `$TOKPROF_DIR` when it is set and not empty, otherwise `~/tokprof`. The directory is created when it is missing, and an existing file with the same name is overwritten.

#### Scenario: No output option
- **WHEN** the user runs `tokprof flame` and `TOKPROF_DIR` is not set
- **THEN** the profile is written to `~/tokprof/all.jfr`
- **AND** `~/tokprof` is created first if it does not exist

#### Scenario: TOKPROF_DIR is set
- **WHEN** `TOKPROF_DIR=/data/profiles` and the user runs `tokprof flame`
- **THEN** the profile is written to `/data/profiles/all.jfr`

#### Scenario: TOKPROF_DIR is empty
- **WHEN** `TOKPROF_DIR` is set to an empty string
- **THEN** the profile directory is `~/tokprof`

#### Scenario: The file exists
- **WHEN** `~/tokprof/all.jfr` is left from an earlier run
- **THEN** it is replaced without a prompt

### Requirement: Default file name follows the filters
The default file name SHALL be built from the global filters (session-logs), joined with `-`, then `.jfr`: the agent when exactly one is given; the `--session` value; the last path segment of `--project`; `all` when neither `--session` nor `--project` is given; `since-<value>`; `until-<value>`; and `tokens` for `--value tokens` or `misses` for `--value avoidable`. In each part, characters other than ASCII letters, digits and `.` become `-`, trimmed at both ends.

#### Scenario: All sessions
- **WHEN** the user runs `tokprof flame`
- **THEN** the file is `all.jfr`

#### Scenario: One session
- **WHEN** the user runs `tokprof --session 3f951de0 flame`
- **THEN** the file is `3f951de0.jfr`

#### Scenario: A project over the last week
- **WHEN** the user runs `tokprof --project ~/src/billing --since 7d flame`
- **THEN** the file is `billing-since-7d.jfr`

#### Scenario: One agent's avoidable cost
- **WHEN** the user runs `tokprof --agent codex flame --value avoidable`
- **THEN** the file is `codex-all-misses.jfr`

#### Scenario: Tokens until a date
- **WHEN** the user runs `tokprof --until 2026-10-16 flame --value tokens`
- **THEN** the file is `all-until-2026-10-16-tokens.jfr`

### Requirement: The extension picks the format
`tokprof flame -o FILE` SHALL write FILE in the format its name ends with: `.jfr` a JFR recording, `.html` async-profiler's HTML flame graph, `.collapsed` collapsed stacks with frame-type suffixes, `.pb.gz`, `.pprof` or `.pb` a gzipped pprof profile, and any other name plain folded stacks. An existing file is overwritten, and the file's directory is not created.

#### Scenario: Folded stacks for speedscope
- **WHEN** the output is `-o stacks.txt`
- **THEN** the file holds plain folded stacks

#### Scenario: pprof without .gz in the name
- **WHEN** the output is `-o x.pprof` or `-o x.pb`
- **THEN** the file is a gzipped pprof profile, as for `x.pb.gz`

#### Scenario: The directory does not exist
- **WHEN** the output is `-o /tmp/nodir/x.html` and `/tmp/nodir` does not exist
- **THEN** after reading the logs the command fails with `writing /tmp/nodir/x.html`, caused by `No such file or directory`
- **AND** for a pprof output the error reads `creating <path>` instead

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

### Requirement: Measure
`--value` SHALL pick which of the measures that cost-attribution defines every stack carries: `cost` (the default), API-equivalent dollars in micro-dollars; `tokens`; or `avoidable`, the miss premium, in micro-dollars. Each stack's value is rounded to a whole number, and in every format but pprof a stack whose value is zero or less is left out.

#### Scenario: Cost in micro-dollars
- **WHEN** a stack cost $2.00
- **THEN** its value is 2000000

#### Scenario: A model without a price
- **WHEN** requests use a model that has no price (pricing)
- **THEN** they are missing from cost and avoidable profiles and present in tokens profiles

#### Scenario: Avoidable cost
- **WHEN** the user passes `--value avoidable` with the default stack
- **THEN** only stacks that carry a miss premium remain, and each ends in `rewrite after miss`

#### Scenario: Unknown measure
- **WHEN** the user passes `--value money`
- **THEN** the command fails with a usage error that lists `tokens, cost, avoidable`

### Requirement: Component filter
`--component` SHALL keep only the costs of the listed components, comma separated, out of `read`, `write`, `rewrite`, `uncached` and `output` (cost-attribution); without it every component is kept. The filter applies whether or not `component` is one of the stack dimensions.

#### Scenario: Re-written prefixes only
- **WHEN** the user passes `--component rewrite --stack cause,project,session`
- **THEN** the profile holds only the cost of prefixes re-written after cache misses, by cause

#### Scenario: Unknown component
- **WHEN** the user passes `--component read,foo`
- **THEN** the command fails before reading any log with `unknown component "foo"`

### Requirement: Summary line
After writing, `tokprof flame` SHALL print one line to standard output with what it wrote and where, the path shown with `~` for the home directory: `wrote <n> samples (1 sample = <unit>), <t> threads, <s> stack traces to <path>` for JFR, where the unit reads like `$0.001` or `100 tokens`; `wrote <n> frames to <path>` for HTML; and `wrote <n> stacks to <path>` for every other format.

#### Scenario: A JFR file
- **WHEN** the user runs `tokprof --session 65e9d5a1 flame -o ~/tokprof/x.jfr`
- **THEN** it prints a line such as `wrote 10549 samples (1 sample = $0.001), 1 threads, 429 stack traces to ~/tokprof/x.jfr`

#### Scenario: HTML counts every frame
- **WHEN** the output is HTML
- **THEN** `<n>` counts every frame drawn, the root frame `all` included

#### Scenario: The filters leave no cost
- **WHEN** `--project` matches no session
- **THEN** the file is still written, empty, and the line reads `wrote 0 samples (1 sample = $0.001), 0 threads, 0 stack traces to <path>`, `wrote 1 frames to <path>` for HTML (the root only), or `wrote 0 stacks to <path>`

#### Scenario: No log file matches the session
- **WHEN** `--session` matches no log file
- **THEN** the command fails with `no session logs found for <agents>` and writes no profile

### Requirement: Folded stacks
Plain folded and collapsed output SHALL have one line per stack: its frames root first, separated by `;`, then a space and the stack's whole-number value, with the lines sorted in byte order. Plain folded stacks hold the frame names only, for speedscope and flamegraph.pl.

#### Scenario: A plain folded line
- **WHEN** the output is `-o stacks.txt` with the default stack and measure
- **THEN** a line reads like `claude;~/src/tokprof;65e9d5a1;main;setup;system prompt;cache read 65971`

### Requirement: Frame types carry meaning
In the HTML flame graph and in collapsed output, each frame SHALL have the async-profiler frame type, and so the color, of what it stands for: structure (agent, project, session, chain, model, turn) type 0, pale green, suffix `_[0]`; content (kind, tool, target) type 1, green, `_[j]`; cache read type 2, aqua, `_[i]`; cache write type 6, yellow-green, `_[1]`; uncached input and output type 5, orange, `_[k]`; rewrite after miss and miss causes type 3, red, no suffix.

#### Scenario: A collapsed line
- **WHEN** the output is `-o x.collapsed` with the default stack
- **THEN** a line reads like `claude_[0];~/src/tokprof_[0];65e9d5a1_[0];main_[0];setup_[0];system prompt_[j];cache read_[i] 65971`

#### Scenario: A cache miss
- **WHEN** a stack ends in a prefix re-written after a miss
- **THEN** its last frame is `rewrite after miss` with no suffix, drawn red

### Requirement: HTML flame graph
`.html` output SHALL be async-profiler 4.6's interactive flame graph page, filled from the template vendored under `assets/async-profiler` with its Apache License 2.0 and built into the binary, so the page is one self-contained file that loads nothing from the network. Values in the page read in `µ$` for cost and avoidable, and in `tokens` for tokens.

#### Scenario: Opening the page offline
- **WHEN** an `.html` profile is opened in a browser without a network
- **THEN** it shows the flame graph with async-profiler's search, zoom, inverted view, tree view and dark mode

#### Scenario: Units
- **WHEN** the profile was written with `--value tokens`
- **THEN** the page reads values in `tokens`
- **AND** with `cost` or `avoidable` it reads them in `µ$`

### Requirement: HTML title and highlight
`--title` and `--highlight` SHALL apply to the HTML page only. `--title` sets its heading, by default `tokprof: <measure> by <stack>`, where the measure reads `cost, µ$`, `tokens` or `avoidable cost of cache misses, µ$` and the stack is the `--stack` list joined with ` > `. `--highlight` takes a regular expression, and the page opens with the matching frames highlighted, as if it were typed into its search.

#### Scenario: Default title
- **WHEN** the user runs `tokprof flame -o x.html` with the default stack and measure
- **THEN** the heading reads `tokprof: cost, µ$ by agent > project > session > chain > turn > kind > tool > target > component`

#### Scenario: Highlighting misses
- **WHEN** the user passes `--highlight miss`
- **THEN** the page opens with every frame whose name matches `miss` highlighted

#### Scenario: Other formats
- **WHEN** `--title` or `--highlight` is given for a JFR, collapsed, pprof or folded output
- **THEN** it has no effect

### Requirement: pprof profile
pprof output SHALL be a gzipped protobuf profile with three sample types, `tokens` (unit `count`), `cost` (`microdollars`) and `avoidable` (`microdollars`), whose default sample type is the `--value` measure. A stack is left out only when all three of its values are zero or less, and each distinct frame name is one function.

#### Scenario: Switching measures in pprof
- **WHEN** the user runs `tokprof flame --value tokens -o x.pb.gz` and opens it with `go tool pprof -http=: x.pb.gz`
- **THEN** pprof shows tokens first and can switch to cost and avoidable

#### Scenario: A stack with tokens but no cost
- **WHEN** a stack has tokens but no cost, as for a model without a price
- **THEN** it stays in the pprof file with a cost of 0

### Requirement: JFR threads
In JFR output, each distinct set of values of the `--threads` dimensions SHALL be one thread, named by those values joined with spaces, whose stacks hold the remaining `--stack` dimensions in order. The default is `agent,project,session`, so each session is a thread. `--threads` has no effect on other formats.

#### Scenario: Default threads
- **WHEN** the user runs `tokprof flame -o x.jfr`
- **THEN** a thread is named like `claude ~/src/tokprof 65e9d5a1` and its stacks start at the chain

#### Scenario: One thread per agent
- **WHEN** the user passes `--threads agent --stack kind,tool,target,component`
- **THEN** the threads are named `claude` and `codex`, each with stacks of kind, tool, target and component

#### Scenario: Unknown thread dimension
- **WHEN** the user passes `--threads bogus -o x.jfr`
- **THEN** the command fails with the unknown dimension error
- **AND** with `-o x.html` the option is ignored

### Requirement: JFR sample unit
Each JFR sample SHALL stand for one round unit of the measure: the smallest of $0.001, $0.01, $0.1 and so on for cost and avoidable, or of 100, 1,000 and so on tokens for tokens, at which the total makes at most 2,000,000 samples. Each thread, stack and minute gets one sample per whole unit, and its remainder gives one more sample or none, with a chance equal to the remainder's share of a unit, drawn the same way on every run.

#### Scenario: One session
- **WHEN** the profile is one session that cost about $10
- **THEN** one sample is $0.001 and the file holds about 10,000 samples

#### Scenario: Many sessions
- **WHEN** the profile totals $5,000
- **THEN** one sample is $0.01 and the file holds about 500,000 samples

#### Scenario: Tokens
- **WHEN** the user passes `--value tokens` for a session of 25 million tokens
- **THEN** one sample is 100 tokens

#### Scenario: A remainder
- **WHEN** one thread, stack and minute add up to $0.0004 at a unit of $0.001
- **THEN** it gives one sample in about 4 of 10 such cases, and the same logs always give the same samples

### Requirement: JFR sample times
JFR samples SHALL be timed by the date that cost-attribution gives each share of spend, when its content entered the context, cut to the minute: the samples of each thread, stack and minute are spread evenly over that minute, and the recording runs from the first such minute to the end of the last. A profiler's timeline thus shows when costly content came in, not when the requests that re-read it were billed.

#### Scenario: A file that stays in the context
- **WHEN** a file is read at 10:00 and re-read from the cache by every request until 14:00
- **THEN** all of its samples lie in the 10:00 minute of the timeline

#### Scenario: Samples within a minute
- **WHEN** one thread, stack and minute make 30 samples
- **THEN** they lie 2 s apart, from the start of that minute

#### Scenario: Recording span
- **WHEN** content first entered the context at 16:46 UTC and last at 21:04 UTC
- **THEN** `jfr summary` shows `Start: … 16:46:00 (UTC)` and a duration of 15540 s, which ends at 21:05:00

### Requirement: JFR file
JFR output SHALL be a one-chunk JFR 2.0 recording of `jdk.ExecutionSample` events in thread state `STATE_DEFAULT`, the state async-profiler gives CPU samples, that the JDK's `jfr` tool and async-profiler 4.6's jfrconv read. It is meant for IntelliJ IDEA's Profiler (Open snapshot) and JDK Mission Control.

#### Scenario: The JDK jfr tool
- **WHEN** the user runs `jfr summary x.jfr`
- **THEN** it reports version 2.0, one chunk, and as many `jdk.ExecutionSample` events as tokprof printed
- **AND** `jfr print --events jdk.ExecutionSample x.jfr` shows each sample with `state = "STATE_DEFAULT"`

#### Scenario: CPU samples, not wall-clock
- **WHEN** `jfrconv --cpu` converts the file
- **THEN** it keeps every sample, as none is in `STATE_RUNNABLE` or `STATE_SLEEPING`, the states async-profiler uses for wall-clock samples

#### Scenario: An empty profile
- **WHEN** the filters leave no cost
- **THEN** the file still reads in `jfr summary`, with no samples

### Requirement: JFR frames
Each JFR frame SHALL be a Java method whose class is the dimension's name and whose name is the value without a leading dimension name, so profilers show frames such as `tool.Read`, `turn.12` and `miss.idle > TTL`; the class of the cause dimension is `miss`. Frames use Java frame types only, so every viewer shows them: Interpreted for structure, Inlined for cache read, C1 compiled for cache write, and JIT compiled for the rest.

#### Scenario: Dimension prefixes
- **WHEN** a turn frame reads `turn 12` and a cause frame `miss: idle > TTL`
- **THEN** the JFR methods are `turn.12` and `miss.idle > TTL`
- **AND** a tool frame `Read` becomes `tool.Read`

#### Scenario: Colors after jfrconv
- **WHEN** jfrconv turns the JFR into an HTML flame graph
- **THEN** structure is pale green, cache reads aqua, cache writes yellow-green, and every other frame green, misses included

### Requirement: Round-trip through jfrconv
tokprof's collapsed and HTML files SHALL convert through async-profiler 4.6's jfrconv without loss: the converter reads back the same stacks, values and frame types, so the HTML it makes from tokprof's `.collapsed` file matches tokprof's own `.html` for the same profile.

#### Scenario: Collapsed to HTML
- **WHEN** the user runs `jfrconv x.collapsed y.html` on a tokprof profile
- **THEN** `y.html` shows the same flame graph, colors included, as tokprof's `x.html` for the same profile

#### Scenario: Comparing two profiles
- **WHEN** the user runs `jfrconv --diff before.collapsed after.collapsed diff.html` on two tokprof profiles
- **THEN** it draws the difference with the frame colors kept
