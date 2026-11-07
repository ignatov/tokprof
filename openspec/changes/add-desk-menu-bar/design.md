# Design

## Context

The browser already knows, per listed session, when its cache goes cold and what the next request costs: warm before that moment, cold after it (the glance's `next_warm`, `next_cold` and `cold_at_ms`). The list's next cell shows the countdown, and the window's `poll` wakes every second while a warm row is on the screen and once when a listed cache goes cold. Rune runs `poll`, `wake_after` timers and app-handle updates on the main thread also while the window is hidden, minimized or covered; it only skips the view and the drawing then.

Rune has tray icons for Linux and Windows, none for macOS. The window ships for macOS only.

Log fields: none new. The cold price and the moment the cache goes cold are the browser's, read from the fields session-browser names for the next-request cell (Claude Code up to 2.1.316, Codex as its source README lists).

Parse time: unchanged. The menu bar reads the listed rows once a second while a countdown shows; that is a pass over the list without I/O.

## Goals / Non-Goals

**Goals:** the countdown that matters, over any app; one bounce before the cache dies; nothing at rest.

**Non-Goals:** notifications; a process that outlives the window; a tray on other systems.

## Decisions

### Rules apart from AppKit

`src/menubar.rs` holds the rules as plain functions over a list of warm sessions (id, title, last activity, cold moment, cold price) and a time: which count, the soonest, the title, the menu rows, and the nudge. The AppKit part (`native`, macOS only) only sets the title, fills the menu and forwards picks. The rules are unit-tested; the glue is checked by running the window.

The cold price of a warm session is the next-request cost the browser gives at the moment its cache goes cold, so the window needs nothing new from the browser.

### The status item

An `NSStatusItem` of variable length from the system status bar, made on the main thread in `poll` when the setting is on and the desk is live (a window, not a frame). Its button's font is the system font with monospaced digits, so the title does not shift every second. The title glyph `❄` carries the text presentation selector, so it draws as a glyph and not as a color emoji. Turning the setting off removes the item from the status bar.

**Nothing warm: the snowflake stays.** The item does not hide when no session counts. An item that comes and goes moves the other items of the menu bar each time, and the menu is a way back to the window. The user turned it on, so it shows.

### Picks

Menu items need a target: a small `NSObject` subclass (`define_class!`) whose ivars hold the session ids of the items and a callback. A pick calls it on the main thread; the callback queues an update on the app handle (select the session, show Day, close any card) and asks the handle to open the window, which shows it, restores it and activates the app. The handle is taken from the app before it runs and kept by the menu bar.

The menu is kept and changed in place: titles are set again each second, and items are rebuilt only when the number of counted sessions changes, so an open menu is not swapped under the pointer.

### The nudge

`NSApplication requestUserAttention: NSInformationalRequest`: one bounce, no permission needed, nothing while the app is active. The menu bar keeps the (session, cold moment) pairs it nudged for, dropped once that moment passes; a new request moves the cold moment and starts a new period. Only the soonest counted session nudges, as the title shows it. A session that was not the soonest when it passed 5:00 nudges when it becomes the soonest, if it is still warm.

### Timers and App Nap

While the title shows a countdown, `poll` asks to be woken at the next whole second. With nothing counted it asks for nothing of its own: the browser wakes it when the logs change, and the existing wake at the next cold moment updates the title when a cache goes cold.

macOS App Nap slows the timers of an app whose windows are all hidden or covered, by seconds. While a countdown shows, the menu bar holds a `NSProcessInfo` activity (user initiated, idle sleep allowed), and ends it when the countdown goes; at rest the app naps as before.

### The setting

`menu_bar` in `prefs.json`, `off` by default: tonight nobody can look at the real menu bar, so it ships off until the author has. The settings panel gains a fourth part, `Menu bar countdown`, last: after both theme grids, whichever order the window's appearance gives them, as an `Off | On` switch like the bar's. Tab reaches it after the second grid, ← → pick. The panel's foot already names ← → as `choose`, and the key help's Settings line for ← → names the countdown too.

### Constants

$1.00 threshold, 5:00 nudge (and the last minutes that show the price), 8 menu items, 40 characters of title. Named in `menubar.rs` and in the spec.

## Risks / Trade-offs

- The real menu bar and the Dock bounce cannot be seen tonight (no screen recording here). The run of the window logs the title it sets; the author checks the look in the morning.
- A menu open during a countdown may show values up to a second old: AppKit runs the menu in its own run loop mode.
