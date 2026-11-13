## MODIFIED Requirements

### Requirement: Sessions screen
The bar SHALL have a `Sessions` button right of the switch, apart from it, and ⌘0 and the palette's `Sessions` show the Sessions screen from every screen. Under the bar it shows a toolbar, the table of sessions with its footer, and, while a session is selected, a narrow inspector at the right. `Day` in the switch, or ⌘1, shows Day as it was. The session selected in Sessions stays selected when Day shows.

#### Scenario: Switching
- **WHEN** Day is shown and the user presses ⌘0
- **THEN** the lanes and the details give way to the toolbar, the table and its footer, and `Sessions` is marked in the bar while no part of the switch is

#### Scenario: Back to Day
- **WHEN** Sessions is shown and the user presses ⌘1
- **THEN** Day shows the day and the selection it showed before

#### Scenario: From Reports
- **WHEN** Month or Quarter, which took Reports' place, is shown and the user clicks `Sessions`
- **THEN** the Sessions screen is shown
