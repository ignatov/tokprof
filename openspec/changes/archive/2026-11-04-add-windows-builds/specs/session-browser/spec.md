# Spec Delta

## MODIFIED Requirements

### Requirement: Profiles from the browser
`f` SHALL write the selected session's JFR to `<profile dir>/<id>.jfr` and `o` its HTML flame graph to `<profile dir>/<id>.html`, in the background, where the profile directory is the one profile-export uses (`$TOKPROF_DIR`, or `~/tokprof`) and is created when missing. The profile covers the whole session with its subagents, with profile-export's default stack and threads, measured in cost. A subagent log that cannot be read is left out of the profile, as `tokprof flame` leaves it out.

#### Scenario: Writing the JFR
- **WHEN** `f` is pressed
- **THEN** the footer reads `writing ~/tokprof/<id>.jfr…` and then `wrote <n> samples (1 = $<unit>) to ~/tokprof/<id>.jfr`

#### Scenario: Writing and opening the HTML
- **WHEN** `o` is pressed
- **THEN** the footer reads `wrote <n> frames to ~/tokprof/<id>.html`, and the page is titled `tokprof: <agent> <project> <id>`
- **AND** tokprof then tries to open the page with `open` on macOS, with `explorer` on Windows, and with `xdg-open` elsewhere

#### Scenario: No way to open the page
- **WHEN** the command that opens the page is missing or fails
- **THEN** the file is still written and no error is shown

#### Scenario: The export fails
- **WHEN** the profile directory cannot be created, or the session's main log cannot be parsed
- **THEN** the footer reads `export failed: <reason>`
- **AND** a subagent log that cannot be parsed does not fail the export; the footer adds `, <n> unreadable logs left out`

#### Scenario: An existing file
- **WHEN** the file is left from an earlier export
- **THEN** it is overwritten

### Requirement: Handoff key
`c` SHALL copy the selected session's handoff line (handoff) to the clipboard with the first of `pbcopy`, `wl-copy`, `xclip` and `clip.exe` that works, as handoff copies, after which the footer reads `copied the handoff line for <id>: paste it into a new session`.

#### Scenario: No clipboard tool
- **WHEN** none of the four tools works
- **THEN** nothing is copied and the footer reads `no clipboard tool (pbcopy, wl-copy, xclip, clip.exe); run: tokprof handoff <id>`
