## MODIFIED Requirements

### Requirement: Default output file
Without `-o`, `tokprof flame` SHALL write a `.jfr` file into the profile directory: `$TOKPROF_DIR` when it is set and not empty, otherwise `~/tokprof`. The directory is created when it is missing, but only once the options are valid, and an existing file with the same name is overwritten.

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

#### Scenario: An invalid option
- **WHEN** the user runs `tokprof flame --stack foo` and `~/tokprof` does not exist
- **THEN** the command fails with the unknown dimension error and `~/tokprof` is not created

### Requirement: The extension picks the format
`tokprof flame -o FILE` SHALL write FILE in the format its name ends with, ignoring case: `.jfr` a JFR recording, `.html` async-profiler's HTML flame graph, `.collapsed` collapsed stacks with frame-type suffixes, `.pb.gz`, `.pprof` or `.pb` a gzipped pprof profile, and any other name plain folded stacks. An existing file is overwritten, and the file's directory is not created.

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

#### Scenario: An upper-case extension
- **WHEN** the output is `-o x.HTML` or `-o x.JFR`
- **THEN** the file is an HTML flame graph or a JFR recording, as for `x.html` or `x.jfr`

### Requirement: Component filter
`--component` SHALL keep only the costs of the listed components, comma separated, out of `read`, `write`, `rewrite`, `uncached` and `output` (cost-attribution); without it every component is kept. The filter applies whether or not `component` is one of the stack dimensions.

#### Scenario: Re-written prefixes only
- **WHEN** the user passes `--component rewrite --stack cause,project,session`
- **THEN** the profile holds only the cost of prefixes re-written after cache misses, by cause

#### Scenario: Unknown component
- **WHEN** the user passes `--component read,foo`
- **THEN** the command fails before reading any log with `unknown component "foo"; use read, write, rewrite, uncached, output`

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

#### Scenario: No dimension left below the threads
- **WHEN** every `--stack` dimension is also a `--threads` dimension, as with `--threads agent --stack agent -o x.jfr`
- **THEN** the command fails before reading any log with `--stack needs a dimension that --threads does not take`, instead of writing a recording with no samples

### Requirement: Summary line
After writing, `tokprof flame` SHALL print one line to standard output with what it wrote and where, the path shown with `~` for the home directory: `wrote <n> samples (1 sample = <unit>), <t> threads, <s> stack traces to <path>` for JFR, where the unit reads like `$0.001` or `100 tokens`; `wrote <n> frames to <path>` for HTML; and `wrote <n> stacks to <path>` for every other format. A count of 1 takes the singular: `1 thread`, `1 frame`, `1 stack`, `1 sample`.

#### Scenario: A JFR file
- **WHEN** the user runs `tokprof --session 65e9d5a1 flame -o ~/tokprof/x.jfr`
- **THEN** it prints a line such as `wrote 10549 samples (1 sample = $0.001), 1 thread, 429 stack traces to ~/tokprof/x.jfr`

#### Scenario: HTML counts every frame
- **WHEN** the output is HTML
- **THEN** `<n>` counts every frame drawn, the root frame `all` included

#### Scenario: The filters leave no cost
- **WHEN** `--project` matches no session
- **THEN** the file is still written, empty, and the line reads `wrote 0 samples (1 sample = $0.001), 0 threads, 0 stack traces to <path>`, `wrote 1 frame to <path>` for HTML (the root only), or `wrote 0 stacks to <path>`

#### Scenario: No log file matches the session
- **WHEN** `--session` matches no log file
- **THEN** the command fails with `no session logs found for <agents>` and writes no profile
