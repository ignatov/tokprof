## MODIFIED Requirements

### Requirement: Profiles from the browser
`f` SHALL write the selected session's JFR to `<profile dir>/<id>.jfr` and `o` its HTML flame graph to `<profile dir>/<id>.html`, in the background, where the profile directory is the one profile-export uses (`$TOKPROF_DIR`, or `~/tokprof`) and is created when missing. The profile covers the whole session with its subagents, with profile-export's default stack and threads, measured in cost. A subagent log that cannot be read is left out of the profile, as `tokprof flame` leaves it out.

#### Scenario: Writing the JFR
- **WHEN** `f` is pressed
- **THEN** the footer reads `writing ~/tokprof/<id>.jfr…` and then `wrote <n> samples (1 = $<unit>) to ~/tokprof/<id>.jfr`

#### Scenario: Writing and opening the HTML
- **WHEN** `o` is pressed
- **THEN** the footer reads `wrote <n> frames to ~/tokprof/<id>.html`, and the page is titled `tokprof: <agent> <project> <id>`
- **AND** tokprof then tries to open the page with `open` on macOS, and with `xdg-open` elsewhere

#### Scenario: No way to open the page
- **WHEN** the `open` command is missing or fails
- **THEN** the file is still written and no error is shown

#### Scenario: The export fails
- **WHEN** the profile directory cannot be created, or the session's main log cannot be parsed
- **THEN** the footer reads `export failed: <reason>`
- **AND** a subagent log that cannot be parsed does not fail the export; the footer adds `, <n> unreadable logs left out`

#### Scenario: An existing file
- **WHEN** the file is left from an earlier export
- **THEN** it is overwritten

### Requirement: One frame as text
The hidden option `tokprof ui --dump <width>x<height>` SHALL render one frame of that size as plain text on standard output and exit, without a terminal and without following the logs. It waits up to 20 s for the X-ray of the newest session, then 0.5 s more for costs, and prints every row with trailing spaces trimmed.

#### Scenario: Checking the layout
- **WHEN** the user runs `tokprof ui --dump 160x45`
- **THEN** it prints 45 lines with the list, the X-ray of the newest session and the footer, and costs not known yet read `…`

#### Scenario: A bad size
- **WHEN** the user runs `tokprof ui --dump 160` or `tokprof ui --dump 160xabc`
- **THEN** the command fails with `--dump takes WIDTHxHEIGHT`

#### Scenario: Not in the help
- **WHEN** the user runs `tokprof ui --help`
- **THEN** `--dump` is not listed

### Requirement: Full-screen X-ray
`⏎`, `→` or `l` SHALL show the selected session's X-ray full screen. There `↑` or `k` and `↓` or `j` scroll it by a line, PgUp, PgDn or space by a page, and `←`, `h`, Backspace or esc go back to the list. `f`, `o`, `c` and `q` work there as in the list.

#### Scenario: Scrolling
- **WHEN** `⏎` and then `↓` are pressed
- **THEN** the X-ray scrolls by one line and the same session stays selected

#### Scenario: Back to the list
- **WHEN** esc is pressed in the full-screen X-ray
- **THEN** the list shows again and the ui keeps running

#### Scenario: The end of the X-ray
- **WHEN** the X-ray is scrolled down to its last line
- **THEN** further `↓`, PgDn or space keep the last page on screen instead of scrolling past the end
