# Proposal

## Why

"Did my change work?" tokprof says what to change: compact before a break, a lower effort, fewer lookups. It never shows whether the change helped. CodeBurn checks each fix on later sessions. The day store (activity-calendar, days-2 since add-month-review) keeps each period's spend, avoidable dollars by miss cause, requests, sessions and cold returns. It can give the period before the one shown at once, without reading the logs again.

## What Changes

- **Reports, key numbers.** Each key number gets a line comparing it with the period of the same length just before it, e.g. `↑ $1,203 (+11%) vs 10 Sep – 9 Oct`.
  - A fall is green and a rise red for the avoidable dollars, their share of the spend, and the cold returns.
  - Spend, sessions and requests stay uncoloured: more spend is not bad by itself.
  - The cold returns compare their count; the store keeps no avoidable dollars of cold returns.
- **Reports, `Could have been saved`.** Each cause row gets its dollars in the period before and the change, as in `idle > TTL $796 · was $1,012 · ↓ 21%`. A cause without dollars before reads `new`.
- **No earlier data.** When the store holds nothing of the period before (it is before the first log), there is no delta, and the key numbers card says `no earlier data`.
- **Console first.** `tokprof summary --since <date> --compare` prints, after the `Total priced` line, a `compared with` line with the same deltas for the spend and the avoidable dollars. Without `--compare` the summary is unchanged, byte for byte.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: adds `Key numbers against the period before` and `Causes against the period before`.
- `cost-attribution`: adds `Summary against the period before`.

## Impact

- **Code:**
  - a comparison over the store's entries in tokprof-core, which calls the store as it is and changes nothing in it;
  - `--compare` on `tokprof summary`;
  - the Reports cards of tokprof-desk, which ask the calendar's worker for the store while Reports shows.
- **Logs:** none. The numbers come from the day store's minutes, hours of misses by cause, and cold returns.
- **Check on the real logs:**
  - The store's numbers for 2026-09-10 to 2026-10-10 must equal `tokprof summary --since 2026-09-10 --until 2026-10-10` (spend and avoidable dollars) and the Reports overview of that window (requests and sessions).
  - The cause dollars must equal the `tokprof misses` cause table over the same window, and the `saved` rows of `tokprof share`.
  - Frames of Reports on 30 days, light and dark.
- **Out of scope:**
  - checking a given fix on given sessions;
  - deltas for the other Reports cards;
  - the avoidable dollars of cold returns (it needs a new store field; see design);
  - deltas with `--client`, which the store cannot filter by.
