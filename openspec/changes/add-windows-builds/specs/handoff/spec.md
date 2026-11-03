# Spec Delta

## MODIFIED Requirements

### Requirement: Clipboard copy in a terminal
When stdout is a terminal, `tokprof handoff` SHALL also copy the text it printed, the note included, to the clipboard with the first of `pbcopy`, `wl-copy`, `xclip -selection clipboard` and `clip.exe` that runs and succeeds. `clip.exe` gets the text as UTF-16LE after a byte order mark. It then says on stderr `copied to the clipboard: paste it into a new session`, or `no clipboard tool found (pbcopy, wl-copy, xclip, clip.exe): copy the text above` when none worked.

#### Scenario: macOS terminal
- **WHEN** the user runs `tokprof handoff` in a terminal on macOS
- **THEN** `pbcopy` gets the printed text
- **AND** stderr says `copied to the clipboard: paste it into a new session`

#### Scenario: Linux with xclip only
- **WHEN** `pbcopy` and `wl-copy` are missing or fail and `xclip` works
- **THEN** `xclip -selection clipboard` gets the text

#### Scenario: Windows terminal
- **WHEN** the user runs `tokprof handoff` in a terminal on Windows
- **THEN** `clip.exe` gets the printed text, and the clipboard then holds it unchanged, letters beyond ASCII included
- **AND** stderr says `copied to the clipboard: paste it into a new session`

#### Scenario: No clipboard tool
- **WHEN** none of the four tools runs and succeeds
- **THEN** stderr says `no clipboard tool found (pbcopy, wl-copy, xclip, clip.exe): copy the text above`
- **AND** stdout is the same as with a clipboard

#### Scenario: Output not to a terminal
- **WHEN** stdout is piped or captured, as when an agent runs the command
- **THEN** nothing is copied and nothing is printed to stderr
