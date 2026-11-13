# Proposal

## Why

Calendar is the last screen of the old bar. It shows the store's days from the first log, up to 52 weeks, with no notion of a year, and it is where Month in review used to open. After add-month-quarter-views, Month and Quarter share one template. The year needs the same template, so that ⌘3–⌘5 only change the scale. A year is also the period people share in January, so it gets its own card, `Year in review`.

## What Changes

- **BREAKING (window)**: `Year` replaces `Calendar` in the bar's switch. The switch becomes the final `Day · Week · Month · Quarter · Year` (⌘1–⌘5), with `Sessions` (⌘0) apart. `--scene calendar` and `--scene calendar-day` become `--scene year` and `--scene year-day`.
- Year shows one calendar year (1 Jan – 31 Dec) in the template of Month and Quarter:
  - Key numbers against the same stretch of the year before; with no earlier data, the pace per day.
  - The main chart: the heatmap of the year, 53 columns by 7 (54 in a leap year that starts on a Sunday), drawn as Calendar draws its grid now, with the six metric pills. Days before the logs begin are outlined `no logs`, with a quiet note; days to come are a dot. Under the grid, 12 month bars aligned to the months' columns.
  - The punchcard of weekday by hour, and a Rhythm card: busiest hour, night share, busiest weekday, weekend share.
  - Cards: By project, Agents and models, What to change. All three come from the day store, so Year never reads a year of logs.
  - Records of the year: biggest day, biggest week, longest streak, most sessions at once, most commits in a day. The busiest hour shows once, in the Rhythm card.
  - The `Year in review` card: numbers only, no project names. It opens from a button, from `r` and from the palette, and it saves as a 1080×1350 image, as Month in review does.
- A click on a day opens Day. A click on a month bar opens Month. A click on the biggest week opens Week.
- The year steps with `‹ 2026 ›` in the bar and with ←/→. It never goes past this year, and never before the year the logs begin.
- Year in review is ready when the year is over, or when the year holds 90 days of logs. Before that the preview shows the numbers so far, dimmed, under `Ready on <date>`, and `Save image` is off.
- New CLI `tokprof year [YYYY|this]` prints the year in review in the terminal, as `tokprof month` prints the month. This is the console-first view of the feature. With no argument it prints this year so far when this year has data, else the last year with data.
- Titles of a year are month-review's six rules, with `Marathon` at 100 days in a row instead of 28.
- New counts over the day store, with no new field in it: the month sums of a year, weekend share, busiest weekday, the day the logs begin, and the readiness. Biggest week and most sessions at once are shared with change 3's Quarter records. year-review states the rule for most at once: kept main sessions with work in the same minute.

## Capabilities

### New Capabilities
- `year-review`: one calendar year of the day store summed into numbers, records and rhythm, a readiness rule, a shareable card with no project in it, and `tokprof year`. It is a capability of its own and not a MODIFIED `month-review`. The month's requirements name a month in their text, their CLI and their errors, and a year adds rules a month does not have (records, readiness, the day the logs begin). `year-review` refers to `month-review` by name for the rules it shares: the numbers, the cache, the titles, the line of advice and nothing private.

### Modified Capabilities
- `desk-window`: Calendar is renamed and rewritten into Year: its view, grid, day tooltip, keys, computation and frame. New requirements for the year in the bar, days without logs, month bars, rhythm, key numbers, cards, records, the Year in review card and its frame. The bar's switch, the day controls and the key help drop Calendar, and Week's computation, Week's day numbers and the sources of Month's and Quarter's cards name Year where they named Calendar. Month in review's opening and its `--scene month` move to Month in change 3, so they are not touched here.

## Depends on

This is change 4 of 4. It is written on top of add-sessions-screen (change 1), add-week-view (change 2) and add-month-quarter-views (change 3), as the reconcile pass of 12 Nov left them. Archiving the four in order was simulated in a copy of `openspec/`. The shared texts:
- `View switch`: change 3 renamed it from `Reports view`. This delta MODIFIES `View switch`, so `openspec validate` notes that archive needs change 3 archived first. The switch becomes `Day | Week | Month | Quarter | Year`, with change 1's `Sessions` button and ⌘0.
- `Day controls`: built on change 3's version, which holds change 1's (`All sessions`, the zoom in the lanes' header) and change 2's (`In Week`, with the filter). `In Calendar` is now about Year.
- `Key help`: built on change 3's version, with change 1's groups (`The day`, `Sessions`), change 2's `Week` group and its scenario, and change 3's `Month and Quarter`. `Calendar` becomes `Year`.
- `Calendar view` and `Keys in Calendar`: changes 1 and 3 modify them (⌘0, ⌘5). This change renames them to `Year view` and `Keys in Year` and keeps their scenarios, including change 1's `Sessions from Calendar`.
- `Month in review in the window`: change 3 still lets Calendar open it. This change drops Calendar from it and keeps all five scenarios.
- These come from change 3 and are referred to by name: `Keys in Month and Quarter` (←/→ and `[`/`]` step the period while no card has the keyboard, `t` goes to the current one, esc in steps), `Key numbers against the period before`, `An incomplete period before`, `A small agent in the period before`, `Pace per day`, `The filter in Month and Quarter`, and Month's By project card.
- Requirements that earlier changes ADDED and that name Calendar get a MODIFIED delta here: change 3's `Sources of the period's cards` ("refreshed as Calendar refreshes it"), change 2's `Week computation` ("as Calendar computation does") and change 2's `Numbers of a day` ("as Calendar's tooltip"). They now name Year.
- Change 3's `Records of the quarter` also lists the most sessions at once and the biggest week. year-review's `Most sessions at once` gives the exact rule (by the minute, kept main sessions). Quarter can refer to it.

## How it is checked on the real logs

These are the installed `tokprof` 0.1.6 numbers on the author's logs, run on 12 Nov 2026. Year 2026 and `tokprof year this` must show the same:
- Spend: `tokprof calendar --since 2026-01-01` total `$24,527` = `tokprof summary --since 2026-01-01 --until 2027-01-01` `Total priced: $24,527`. Avoidable `$2,715` (11.1%) in both. Spend grows through the day: change 1's later run read `Total priced` $24,529, and a run at the reconcile pass read $24,564, so compare the two commands at the same moment.
- Month bars = `tokprof month` per month: Aug `$5,213`, Sep `$3,729`, Oct `$11,632`, Nov so far `$3,953`. They sum to the year's spend. Oct also gives 221h agents, 78h at the keyboard, 1,431 messages and 486 commits.
- Grid and numbers = `tokprof calendar` (data from Thu 6 Aug, 99 days): active 92 of 99 days, longest streak 48 days (Sat 26 Sep – Thu 12 Nov), busiest Sun 25 Oct `$1,221`, weekday Wednesdays `$4,335`. Agent time 590.1h, keyboard 209.3h, 3,815 messages, 1,103 commits. Most commits in a day: Wed 11 Nov, 175.
- Logs begin Thu 6 Aug 2026. So 1 Jan – 5 Aug is drawn as `no logs`, and 13 Nov – 31 Dec as days to come.
- The year holds 99 days of logs, so Year in review for 2026 is ready now, as `2026 so far`. (The sketch assumed logs from 30 Sep, which would not be ready until Mon 28 Dec 2026, its 90th day of logs.)
- 2025 has no logs, so the key numbers show the pace per day: `$248 a day` over 99 days.
- Most at once: 12 sessions at Sun 1 Nov 17:08, counted from the store's minutes. `tokprof day 2026-11-01` prints `peak 11 at 17:08`. The record can be higher than the day's peak, but only by sessions that worked in the same minute without overlapping.
- Biggest week: Mon 12 – Sun 18 Oct, `$3,925` = `tokprof summary --since 2026-10-12 --until 2026-10-19` `Total priced`.
- Agents: claude `$13,763`, codex `$10,766`, summed over the clients of the summary's agent table.

## Out of scope

- The "Since logs began" alternative of periods.html.
- A punchcard cell that opens Sessions filtered by weekday and hour.
- A forecast of the rest of the year.
- New fields in the day store. Everything comes from format 2.
- By model with the cost components (cache read, write, input, output) for a year. Those need a year of logs. Quarter keeps them.
- `Coming back after a cold break` and `Habits that cost the most` for a year, for the same reason.
- Changes to `tokprof calendar` itself.

## Open questions

1. Default chosen, author to confirm: `t` goes to this year, as `t` goes to today's day or period in every view (change 1 now uses `t` in Sessions too). Keys follow change 3: ←/→ (`[`, `]`) step the year while no card has the keyboard, and they move the grid's cursor by a week once Tab gives the grid the keyboard.
2. Default chosen, author to confirm: with no year before, or an incomplete one, the key numbers show the pace per day, as Month and Week do.

## Decided

- Year in review is ready when the year holds 90 days of logs or the year is over. 2026 is ready now, with 99 days.
- `Marathon` over a year takes 100 days in a row, not the month's 28. On the author's calendar (`tokprof calendar`, 12 Nov 2026), 99 days of logs already hold two runs over 28 days: 6 Aug – 6 Sep (32 days) and 26 Sep – 12 Nov (48, still going). At 28 nearly every year of daily use would be a Marathon. 100 is twice the longest run so far, and 2026 cannot reach it (26 Sep to 31 Dec is 97 days), so it marks a rare year.
- `tokprof year` with no argument prints this year so far when this year has a request or a message, else the last year that has one.
- The busiest hour shows once in Year, in the Rhythm card, not in the records.
- A project row in By project sets the window's filter and stays on the year, as in Week, Month and Quarter. The filter matches agent, project and session id, not titles.
- `Agent hours` in the key numbers counts each minute some agent worked once (590.1h in 2026). Rhythm lines that add up over sessions keep their sum.
- With `--client`, Year counts every client and says so in one quiet line: the day store has no client.

## Impact

- `crates/tokprof-core`: year sums and records over the day store, next to the month in review; the readiness rule.
- `crates/tokprof`: the `year` subcommand and its text. README "Your calendar" and "Your month in review" sections, and a new "Your year in review".
- `crates/tokprof-desk`: the Calendar mode becomes Year; the year grid, month bars, rhythm, cards, records, the year card, saving it, the scenes and the key help. `tokprof-browse` only if the year card's text renderer lives there, as the month's does.
- No change to the day store format, to log reading or to parse time.
