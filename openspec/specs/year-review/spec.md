# year-review Specification

## Purpose
Sums one calendar year of the day store into numbers, records, rhythm and a card someone would post in January, with no project in it, in the terminal and in the window.

## Requirements

### Requirement: The year's window
A year in review SHALL count what the day store (see activity-calendar) holds from local midnight on 1 January to local midnight on the next 1 January, or until now for this year, with the `--agent`, `--project` and `--session` filters. Its days are the calendar's cells of the year's dates, with the calendar's day rule. Its months are the windows of month-review, The month's window.

#### Scenario: The same spend as the summary
- **WHEN** the user runs `tokprof year 2026` and `tokprof summary --since 2026-01-01 --until 2027-01-01` on logs that were not deleted
- **THEN** the year's spend equals `Total priced`, and its avoidable dollars equal `avoidable on cache misses`

#### Scenario: Months add up
- **WHEN** `tokprof month` gives $5,213 for August, $3,729 for September, $11,632 for October and $3,953 for November so far, and no other month of 2026 has data
- **THEN** the year's months read the same, and the year's spend is $24,527

#### Scenario: A night across the new year
- **WHEN** agents worked from 00:00 to 03:00 on 1 January 2027, inside the night of 31 December's day
- **THEN** that spend counts in 2027's spend and not in 2026's, as month-review counts a night across the first of a month

### Requirement: When the logs begin
The logs SHALL begin on the calendar day of the earliest request or action the store holds for the agents asked for, whatever `--project` and `--session` say. Days of the year before it have no logs. They are not idle days, and nothing counts them: not the active days, not the levels, not the pace per day.

#### Scenario: Logs from August
- **WHEN** the store's earliest request is on Thu 6 Aug 2026
- **THEN** 2026's days from 1 January to 5 August have no logs, and 2026 has 99 days of logs on 12 November

#### Scenario: A project that started later
- **WHEN** the window is filtered to a project whose first request is on 1 November, and the logs begin on 6 August
- **THEN** 6 August to 31 October are days with logs and nothing in them, not days without logs

### Requirement: The year's numbers
The year SHALL give what month-review gives for a month, over its window. That is the numbers of The month's numbers and the cache, avoidable, cold returns and top model of The month's cache and model, with the same rules. It also gives the spend, agent hours and commits of each of its twelve months.

#### Scenario: The busiest hour of a year
- **WHEN** 8% of 2026's agent time fell between 00:00 and 01:00, more than in any other hour
- **THEN** the year reads `00:00–01:00, 8% of agent time`

#### Scenario: A month before the logs
- **WHEN** the logs begin on 6 August
- **THEN** January to July have no month numbers and read `no logs`, and August counts from 6 August

### Requirement: Records of the year
The year SHALL name its records:
- the biggest day by spend;
- the biggest week, a Monday-to-Sunday week by spend as Week's totals count it, from Monday 00:00, the year's first and last weeks cut at its bounds;
- the longest streak, the longest run of days with a request, as month-review counts it;
- the most commits in a day;
- the most sessions at once.

A tie goes to the earliest. The busiest hour is not a record: it shows once, in Rhythm of the year.

#### Scenario: Biggest week cut by the new year
- **WHEN** the week of Mon 28 Dec 2026 holds $2,000, of which $900 is on 1–3 January 2027
- **THEN** it counts $1,100 in 2026's records and $900 in 2027's

#### Scenario: A streak still going
- **WHEN** the longest streak ends on the last day of a year in progress
- **THEN** it reads `48 days, since Sat 26 Sep, still going`

#### Scenario: No busiest hour among the records
- **WHEN** the records of 2026 are listed
- **THEN** no record names an hour of the day, and the busiest hour reads only in the rhythm

### Requirement: Most sessions at once
The most sessions at once SHALL be the highest number of main sessions whose agents worked in the same minute, with the first minute it was reached. Only sessions the rhythm keeps count, and a subagent is part of its session. A minute counts when the store has work of the session in it, as the calendar's `agent` metric and month-review's `Conductor` share count.

#### Scenario: Two sessions in one minute
- **WHEN** one session worked from 10:00:05 to 10:00:20 and another from 10:00:40 to 10:00:50
- **THEN** minute 10:00 counts 2 sessions at once, while `tokprof day` gives a peak of 1 for that moment

#### Scenario: The same as the day's peak or more
- **WHEN** `tokprof day 2026-11-01` prints `peak 11 at 17:08`
- **THEN** 2026's most at once is 11 or more, and reads like `12 sessions, Sun 1 Nov 17:08` when another session worked in the minute of 17:08 without overlapping the 11

### Requirement: Rhythm of the year
The year SHALL give the busiest hour and the night share of The year's numbers. It also gives the weekday with the most spend, with its spend over the year, as the calendar's `weekday` line counts it. And it gives the weekend share: the spend of Saturday and Sunday days, as a share of the year's spend.

#### Scenario: The busiest weekday
- **WHEN** `tokprof calendar --since 2026-01-01` prints `weekday   Wednesdays, $4,335`
- **THEN** the year's busiest weekday reads `Wednesday`, `$4,335 on Wednesdays`

#### Scenario: No spend
- **WHEN** the year has requests but none with a price
- **THEN** the busiest weekday and the weekend share are left out

### Requirement: The year before
The year SHALL compare its spend, agent hours and avoidable share with the year before, as month-review's The month before does. A year in progress compares with the same number of days and hours from 1 January of the year before, and names it, as `vs 1 Jan – 12 Nov 2025`. A year before that is incomplete by desk-window's An incomplete period before gives a note instead, and so does one with no request and no message.

#### Scenario: No logs in the year before
- **WHEN** 2026 is shown and the store holds nothing in 2025
- **THEN** there is no comparison, and the note reads `no comparison with 2025: no logs`

#### Scenario: A leap year
- **WHEN** `tokprof year this` runs on 1 March 2028 at 10:00
- **THEN** it compares with 1 January 2027 00:00 to 2 March 2027 10:00, the same 60 days and 10 hours

#### Scenario: An incomplete year before
- **WHEN** 2027 is shown and the store's first claude request is on 29 Sep 2026
- **THEN** the note reads `no comparison with 2026: claude's logs start 29 Sep`

### Requirement: Pace per day
When the year has no comparison, its spend, agent hours, hours at the keyboard, sessions and commits SHALL each also be given per day of logs. That is the number divided by the year's days that have logs, up to the last day.

#### Scenario: A year with logs from August
- **WHEN** 2026 has $24,527 of spend over 99 days of logs, from 6 Aug to 12 Nov
- **THEN** its pace reads `$248 a day`

### Requirement: Titles of a year
The year SHALL take a title by month-review's Titles: the same six rules, in the same order, over the year's window, with the same thresholds but one: `Marathon` takes 100 or more days in a row with a request. A year with none prints `no title this year: no rule of the six held`.

#### Scenario: Night owl for a year
- **WHEN** 33% of 2026's agent time fell between 22:00 and 05:00
- **THEN** 2026's title is `Night owl`, with the rule `33% of agent time 22:00–05:00 (title from 33%)`

#### Scenario: Marathon for a year
- **WHEN** 2026's longest run of days with a request is 48 days, and 2027 has a run of 104 days
- **THEN** 2026 is no `Marathon`, while a month with a 28-day run is; and 2027 can be, with the rule `104 days in a row with the agents (title from 100)`, as month-review words it

### Requirement: The line for next year
The year SHALL end with the line of month-review's The line for next month, over the year, as `$1,120 went to idle > TTL. Compact or wrap up a big session before a long break.`. It is left out when no miss was avoidable.

#### Scenario: Compaction calls first
- **WHEN** compaction calls lost the most avoidable dollars of 2026, $640
- **THEN** the line names compaction calls and their $640, with tokprof's advice for them

### Requirement: When the card is ready
The year's card SHALL be ready when the year is over, or when the year has 90 or more days of logs. A year not ready is shown with the numbers so far and the day it gets ready: the day of its 90th day of logs, or 1 January after it, whichever comes first. A year with no logs is never ready.

#### Scenario: Logs from 10 October
- **WHEN** the logs begin on 10 Oct 2026 and it is 12 November
- **THEN** 2026 is not ready, and it reads `ready on Fri 1 Jan 2027: 2026 has 34 days of logs, from 10 Oct`, as its 90th day of logs would be 7 Jan 2027

#### Scenario: Logs from 6 August
- **WHEN** the logs begin on 6 Aug 2026 and it is 12 November
- **THEN** 2026 is ready, as 2026 so far, with 99 days of logs

#### Scenario: A year that is over
- **WHEN** the logs begin on 20 Dec 2026 and it is January 2027
- **THEN** 2026 is ready with 12 days of logs, and its card says `logs since 20 Dec`

### Requirement: Nothing private in a year
A year in review SHALL name no project, session id, session title, path or user name, as month-review's Nothing private says. It names models, miss causes, dates and hours.

#### Scenario: A project filter
- **WHEN** the user runs `tokprof --project billing year`
- **THEN** the year counts only that project's sessions, and the word `billing` does not appear in it

### Requirement: tokprof year
`tokprof year [YYYY]` SHALL print the year in review to the terminal. By default it prints this year so far when this year holds a request or a message, else the last year that does, else this year so far. With `this` it prints this year so far. It refreshes the day store as `tokprof calendar` does and prints the same line on what it read. `--since`, `--until` and `--client` fail as they fail for `tokprof month`, with `year` in the message.

#### Scenario: The default in the first year
- **WHEN** the user runs `tokprof year` on 12 Nov 2026 and the store holds nothing in 2025
- **THEN** it prints 2026 so far

#### Scenario: Early in a new year
- **WHEN** the user runs `tokprof year` at 09:00 on 1 Jan 2027, before any request or message of 2027, and 2026 holds data
- **THEN** it prints 2026

#### Scenario: A bad year
- **WHEN** the user runs `tokprof year 26`
- **THEN** it fails with `bad year "26", expected YYYY or this`

#### Scenario: A year that has not begun
- **WHEN** the user runs `tokprof year 2027` in November 2026
- **THEN** it fails with `2027 has not begun; tokprof year this shows 2026 so far`

#### Scenario: A year with nothing
- **WHEN** the store holds nothing in 2024
- **THEN** it prints `No requests and no messages in this year.` under the headline

#### Scenario: Since and until
- **WHEN** the user runs `tokprof year 2026 --since 7d`
- **THEN** it fails with `year picks its time by YYYY; drop --since and --until`

### Requirement: The text of tokprof year
`tokprof year` SHALL print a headline `tokprof · year · 2026 with agents`, with `, so far` for this year. Then the title and its rule, the six big numbers of `tokprof month`, and a row per month: its short name and a bar of `█` as long as its share of the biggest month, with its spend, `no logs`, `so far` or `not begun`. Then the records, the rhythm, the cache, the comparison or its note, whether the card is ready, and the line for next year.

#### Scenario: The months
- **WHEN** the user runs `tokprof year this` on 12 Nov 2026 with logs from 6 Aug
- **THEN** `Jan` to `Jul` read `no logs`, `Aug` reads `$5,213  from 6 Aug`, `Oct` has the longest bar, `Nov` reads `$3,953  so far` and `Dec` reads `not begun`

#### Scenario: Without color
- **WHEN** the output is not a terminal
- **THEN** the bars are the same characters and no escape code is written

#### Scenario: Not ready
- **WHEN** the year's card is not ready
- **THEN** the `ready` line reads like `ready          on Fri 1 Jan 2027: 2026 has 34 days of logs, from 10 Oct`, and everything else prints as for a ready year
