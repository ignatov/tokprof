//! The cache countdown in the macOS menu bar: a status item of the window's process that shows,
//! over any app, how long the warm cache of a session that would be costly to resume cold stays
//! warm, with a menu of those sessions, and one Dock bounce in their last minutes.
//!
//! The rules (which sessions count, the title, the menu rows, the nudge) are plain functions here;
//! `native` only sets the title, fills the menu and passes picks back, on macOS.

use std::time::Duration;

use rune::{AppHandle, Tx};
use tokprof_core::fmt::{clip_end, usd};

use crate::view::countdown;
use crate::{Book, Desk, Focus, reports, settings, view};

/// A session counts while one more request would cost at least this much once its cache is cold.
pub const THRESHOLD_USD: f64 = 1.00;
/// The time left at which the title adds the cold price and the Dock bounces once.
pub const NUDGE_MS: i64 = 5 * 60_000;
/// The most sessions the menu lists.
pub const MAX_ITEMS: usize = 8;
/// How much of a session's title the menu shows.
pub const TITLE_CHARS: usize = 40;
/// The title's glyph, a snowflake drawn as text rather than as a color emoji.
pub const GLYPH: &str = "❄\u{FE0E}";
/// The menu's first item while no session counts.
pub const NONE: &str = "No warm session costs $1.00 or more cold";

/// A listed session whose cache is warm.
#[derive(Clone, Debug, PartialEq)]
pub struct Warm {
    pub id: String,
    /// Its title, or its project while it has none.
    pub title: String,
    /// Its last activity, in epoch ms.
    pub active_ms: i64,
    /// When its cache goes cold, in epoch ms.
    pub cold_at: i64,
    /// What one more request costs once it is cold.
    pub cold_usd: f64,
}

/// The listed sessions whose cache is warm at `now`, with their cold price: the next-request cost
/// the browser gives at the moment the cache goes cold. Sessions still being costed are left out.
pub fn warm_sessions(b: &Book, now: i64) -> Vec<Warm> {
    // Nothing listed goes cold later: nothing is warm.
    if b.next_cold(now).is_none() {
        return Vec::new();
    }
    (0..b.len())
        .filter_map(|r| {
            let c = b.cells(r, now);
            let cold_at = c.warm_until?;
            c.usd?;
            let title = c.title.map_or_else(|| c.project.to_string(), str::to_string);
            let active_ms = c.active_ms;
            let cold_usd = b.cells(r, cold_at).next_usd?;
            Some(Warm { id: b.session(r).to_string(), title, active_ms, cold_at, cold_usd })
        })
        .collect()
}

/// The sessions that count at `now`: still warm, and at least the threshold to resume cold.
pub fn counted(warm: &[Warm], now: i64) -> Vec<&Warm> {
    warm.iter().filter(|w| w.cold_at > now && w.cold_usd >= THRESHOLD_USD).collect()
}

/// The counted session that goes cold first.
pub fn soonest<'a>(counted: &[&'a Warm]) -> Option<&'a Warm> {
    counted.iter().copied().min_by_key(|w| w.cold_at)
}

/// The status item's title: the glyph, the soonest session's countdown, and in its last minutes
/// its cold price.
pub fn title(soonest: Option<&Warm>, now: i64) -> String {
    let Some(w) = soonest else { return GLYPH.to_string() };
    let left = w.cold_at - now;
    if left <= NUDGE_MS { format!("{GLYPH} {} · {}", countdown(left), usd(w.cold_usd)) } else { format!("{GLYPH} {}", countdown(left)) }
}

/// An item of the menu: the session it opens and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub text: String,
}

/// The menu's sessions: newest activity first, at most `MAX_ITEMS`, each its title cut, its
/// countdown and its cold price.
pub fn rows(counted: &[&Warm], now: i64) -> Vec<Row> {
    let mut by_activity = counted.to_vec();
    by_activity.sort_by_key(|w| std::cmp::Reverse(w.active_ms));
    by_activity
        .into_iter()
        .take(MAX_ITEMS)
        .map(|w| Row { id: w.id.clone(), text: format!("{}  {} · {} cold", clip_end(&w.title, TITLE_CHARS), countdown(w.cold_at - now), usd(w.cold_usd)) })
        .collect()
}

/// The warm periods the Dock bounced for: a session's id and when its cache was to go cold.
#[derive(Debug, Default)]
pub struct Nudges {
    done: Vec<(String, i64)>,
}

impl Nudges {
    /// Whether to bounce now: the soonest session has the nudge time or less left, and no bounce
    /// was made for this warm period of it. A new request moves the cold moment, a new period.
    pub fn due(&mut self, soonest: Option<&Warm>, now: i64) -> bool {
        self.done.retain(|(_, cold_at)| *cold_at > now);
        let Some(w) = soonest.filter(|w| w.cold_at > now && w.cold_at - now <= NUDGE_MS) else { return false };
        if self.done.iter().any(|(id, at)| *id == w.id && *at == w.cold_at) {
            return false;
        }
        self.done.push((w.id.clone(), w.cold_at));
        true
    }
}

/// What a pick in the menu asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Session(String),
    Open,
    Settings,
}

/// The menu bar's part of the window's state.
#[derive(Default)]
pub struct MenuBar {
    nudges: Nudges,
    /// The running app, where picks go; none in a frame without a window and in tests.
    pub handle: Option<AppHandle<Desk>>,
    /// The second of the last update while it counted down: the frames of that second skip the
    /// pass over the list, since the next second wakes it anyway.
    counted_at: Option<i64>,
    #[cfg(target_os = "macos")]
    item: Option<native::Item>,
}

impl MenuBar {
    /// Whether the status item is in the menu bar.
    pub fn shown(&self) -> bool {
        #[cfg(target_os = "macos")]
        return self.item.is_some();
        #[cfg(not(target_os = "macos"))]
        false
    }
}

/// From `poll`: puts the status item in place or takes it away as the setting says, sets its
/// title and menu, bounces the Dock when the nudge is due, and asks to be woken at the next second
/// while a countdown shows.
pub fn poll(s: &mut Desk, now: i64, tx: &mut Tx) {
    if !(s.live && s.settings.menu_bar) {
        #[cfg(target_os = "macos")]
        {
            s.menu_bar.item = None;
        }
        s.menu_bar.counted_at = None;
        return;
    }
    let next_second = Duration::from_millis((1000 - now.rem_euclid(1000)) as u64 + 5);
    if s.menu_bar.shown() && s.menu_bar.counted_at == Some(now / 1000) {
        tx.wake_after(next_second);
        return;
    }
    let warm = warm_sessions(&s.browser, now);
    let counted = counted(&warm, now);
    let soon = soonest(&counted);
    let nudge = s.menu_bar.nudges.due(soon, now);
    s.menu_bar.counted_at = soon.map(|_| now / 1000);
    if soon.is_some() {
        tx.wake_after(next_second);
    }
    #[cfg(target_os = "macos")]
    {
        let bar = &mut s.menu_bar;
        if bar.item.is_none() {
            let handle = bar.handle.clone();
            bar.item = native::Item::new(Box::new(move |p| {
                if let Some(h) = &handle {
                    h.update(move |s, tx| pick(s, p, tx));
                    h.open();
                }
            }));
        }
        if let Some(item) = bar.item.as_mut() {
            item.show(&title(soon, now), &rows(&counted, now), soon.is_some(), nudge);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = nudge;
}

/// A pick in the menu, once the window comes forward: a session is selected in Day as a click on
/// its row selects it, over no card; `Settings…` opens the settings panel.
pub fn pick(s: &mut Desk, p: Pick, tx: &mut Tx) {
    match p {
        Pick::Open => {}
        Pick::Settings => {
            s.palette = None;
            s.help = false;
            if s.panel.is_none() {
                s.panel = Some(settings::Panel::default());
            }
        }
        Pick::Session(id) => {
            if s.browser.row_of(&id).is_none() {
                return;
            }
            (s.panel, s.palette, s.help) = (None, None, false);
            reports::show(s, reports::Mode::Day);
            let zoomed = s.browser.zoomed();
            if s.browser.open_session(&id) {
                if zoomed {
                    s.restore_list(tx);
                }
                s.pane = Focus::List;
                s.cursor = 0;
                tx.scroll_to_y(view::PANE, 0.0);
                tx.scroll_into_view(id.clone());
                tx.scroll_into_view(format!("lane-{id}"));
            }
        }
    }
}

/// The status item in AppKit. Every call runs on the main thread, where Rune runs `poll` and the
/// app handle's updates; off it, no item is made.
#[cfg(target_os = "macos")]
mod native {
    use std::cell::RefCell;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject, Sel};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSApplication, NSFont, NSFontWeightRegular, NSMenu, NSMenuItem, NSRequestUserAttentionType, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
    };
    use objc2_foundation::{NSActivityOptions, NSObject, NSProcessInfo, NSString};

    use super::{NONE, Pick, Row};

    pub struct Ivars {
        /// The sessions of the menu's items, by their tags.
        ids: RefCell<Vec<String>>,
        on_pick: Box<dyn Fn(Pick)>,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and `Target` does not implement Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "TokprofMenuBarTarget"]
        #[ivars = Ivars]
        struct Target;

        impl Target {
            #[unsafe(method(pickSession:))]
            fn pick_session(&self, sender: &NSMenuItem) {
                let id = usize::try_from(sender.tag()).ok().and_then(|i| self.ivars().ids.borrow().get(i).cloned());
                if let Some(id) = id {
                    (self.ivars().on_pick)(Pick::Session(id));
                }
            }

            #[unsafe(method(openWindow:))]
            fn open_window(&self, _sender: &NSMenuItem) {
                (self.ivars().on_pick)(Pick::Open);
            }

            #[unsafe(method(openSettings:))]
            fn open_settings(&self, _sender: &NSMenuItem) {
                (self.ivars().on_pick)(Pick::Settings);
            }
        }

        unsafe impl NSObjectProtocol for Target {}
    );

    impl Target {
        fn new(mtm: MainThreadMarker, on_pick: Box<dyn Fn(Pick)>) -> Retained<Target> {
            let this = Target::alloc(mtm).set_ivars(Ivars { ids: RefCell::default(), on_pick });
            // SAFETY: NSObject's init, on a new object.
            unsafe { msg_send![super(this), init] }
        }
    }

    /// The status item, its menu and what was last put in them.
    pub struct Item {
        mtm: MainThreadMarker,
        item: Retained<NSStatusItem>,
        menu: Retained<NSMenu>,
        target: Retained<Target>,
        title: String,
        /// How many session items the menu was built with, once built.
        built: Option<usize>,
        /// Held while a countdown shows, so that App Nap does not delay its seconds.
        activity: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    }

    impl Item {
        /// A new status item at the end of the menu bar, off the main thread none.
        pub fn new(on_pick: Box<dyn Fn(Pick)>) -> Option<Item> {
            let mtm = MainThreadMarker::new()?;
            let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
            if let Some(button) = item.button(mtm) {
                // Digits of one width: the title does not shift as it counts down.
                // SAFETY: a constant of AppKit's.
                let font = NSFont::monospacedDigitSystemFontOfSize_weight(NSFont::systemFontSize(), unsafe { NSFontWeightRegular });
                button.setFont(Some(&font));
            }
            let menu = NSMenu::new(mtm);
            menu.setAutoenablesItems(false);
            item.setMenu(Some(&menu));
            Some(Item { mtm, item, menu, target: Target::new(mtm, on_pick), title: String::new(), built: None, activity: None })
        }

        /// Shows this title and these sessions; `counting` while a countdown shows, `nudge` to
        /// bounce the Dock once.
        pub fn show(&mut self, title: &str, rows: &[Row], counting: bool, nudge: bool) {
            if self.title != title {
                if let Some(button) = self.item.button(self.mtm) {
                    button.setTitle(&NSString::from_str(title));
                }
                self.title = title.to_string();
            }
            self.fill(rows);
            let info = NSProcessInfo::processInfo();
            match (counting, self.activity.take()) {
                (true, None) => {
                    let reason = NSString::from_str("the menu bar counts down a warm prompt cache");
                    self.activity = Some(info.beginActivityWithOptions_reason(NSActivityOptions::UserInitiatedAllowingIdleSystemSleep, &reason));
                }
                // SAFETY: the activity that beginActivity returned.
                (false, Some(a)) => unsafe { info.endActivity(&a) },
                (_, kept) => self.activity = kept,
            }
            if nudge {
                NSApplication::sharedApplication(self.mtm).requestUserAttention(NSRequestUserAttentionType::InformationalRequest);
            }
        }

        /// The menu: the sessions, a separator, `Open tokprof` and `Settings…`. Built again only
        /// when the count of sessions changes, else its titles set in place, so that an open menu
        /// stays under the pointer.
        fn fill(&mut self, rows: &[Row]) {
            *self.target.ivars().ids.borrow_mut() = rows.iter().map(|r| r.id.clone()).collect();
            if self.built == Some(rows.len()) {
                for (i, r) in rows.iter().enumerate() {
                    if let Some(item) = self.menu.itemAtIndex(i as isize)
                        && item.title().to_string() != r.text
                    {
                        item.setTitle(&NSString::from_str(&r.text));
                    }
                }
                return;
            }
            self.menu.removeAllItems();
            if rows.is_empty() {
                let none = self.entry(NONE, None, 0);
                none.setEnabled(false);
                self.menu.addItem(&none);
            }
            for (i, r) in rows.iter().enumerate() {
                self.menu.addItem(&self.entry(&r.text, Some(sel!(pickSession:)), i as isize));
            }
            self.menu.addItem(&NSMenuItem::separatorItem(self.mtm));
            self.menu.addItem(&self.entry("Open tokprof", Some(sel!(openWindow:)), 0));
            self.menu.addItem(&self.entry("Settings…", Some(sel!(openSettings:)), 0));
            self.built = Some(rows.len());
        }

        fn entry(&self, title: &str, action: Option<Sel>, tag: isize) -> Retained<NSMenuItem> {
            // SAFETY: a new item with a title, an action of the target's or none, and no key.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(self.mtm), &NSString::from_str(title), action, &NSString::from_str(""))
            };
            if action.is_some() {
                let target: &AnyObject = &self.target;
                // SAFETY: the target answers the action and lives as long as the item's menu.
                unsafe { item.setTarget(Some(target)) };
            }
            item.setTag(tag);
            item
        }
    }

    impl Drop for Item {
        fn drop(&mut self) {
            if let Some(a) = self.activity.take() {
                // SAFETY: the activity that beginActivity returned.
                unsafe { NSProcessInfo::processInfo().endActivity(&a) };
            }
            NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn warm(id: &str, active_ago: i64, left_ms: i64, cold_usd: f64) -> Warm {
        Warm { id: id.into(), title: format!("Session {id}"), active_ms: NOW - active_ago, cold_at: NOW + left_ms, cold_usd }
    }

    #[test]
    fn a_session_counts_from_the_threshold_on_while_it_is_warm() {
        let all = [warm("cheap", 0, 600_000, 0.84), warm("even", 0, 600_000, 1.00), warm("dear", 0, 600_000, 7.19), warm("cold", 0, 0, 9.0)];
        let ids: Vec<&str> = counted(&all, NOW).iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["even", "dear"], "below $1.00 does not count, $1.00 does, a cache cold now does not");
        assert!(counted(&all[..1], NOW).is_empty());
    }

    #[test]
    fn the_soonest_goes_cold_first_whatever_the_order() {
        let all = [warm("a", 0, 900_000, 2.0), warm("b", 60_000, 300_000, 2.0), warm("c", 120_000, 1_200_000, 2.0)];
        assert_eq!(soonest(&counted(&all, NOW)).map(|w| w.id.as_str()), Some("b"));
        assert_eq!(soonest(&[]), None);
    }

    #[test]
    fn the_title_counts_down_and_adds_the_price_in_the_last_five_minutes() {
        let w = warm("a", 0, 12 * 60_000 + 40_000, 7.19);
        assert_eq!(title(Some(&w), NOW), format!("{GLYPH} 12:40"));
        assert_eq!(title(Some(&w), NOW + 1000), format!("{GLYPH} 12:39"));
        assert_eq!(title(Some(&w), w.cold_at - NUDGE_MS - 1000), format!("{GLYPH} 5:01"));
        assert_eq!(title(Some(&w), w.cold_at - NUDGE_MS), format!("{GLYPH} 5:00 · $7.19"), "5:00 is in the last minutes");
        assert_eq!(title(Some(&w), w.cold_at - 252_000), format!("{GLYPH} 4:12 · $7.19"));
        assert_eq!(title(None, NOW), GLYPH);
        assert_eq!(GLYPH.chars().next(), Some('❄'));
    }

    #[test]
    fn the_title_follows_the_next_session_once_the_soonest_goes_cold() {
        let all = [warm("a", 0, 60_000, 2.0), warm("b", 0, 600_000, 3.0)];
        let at = NOW + 60_000;
        assert_eq!(title(soonest(&counted(&all, at)), at), format!("{GLYPH} 9:00"));
        let gone = NOW + 600_000;
        assert_eq!(title(soonest(&counted(&all, gone)), gone), GLYPH, "nothing warm: the glyph alone");
    }

    #[test]
    fn the_menu_lists_the_eight_newest_with_titles_cut() {
        let all: Vec<Warm> = (0..9).map(|i| warm(&format!("s{i}"), (8 - i) * 60_000, 600_000 + i * 1000, 2.0)).collect();
        let rows = rows(&counted(&all, NOW), NOW);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["s8", "s7", "s6", "s5", "s4", "s3", "s2", "s1"], "newest first, the oldest of nine left out");
        assert_eq!(rows[0].text, "Session s8  10:08 · $2.00 cold");
        let long = Warm { title: "Find why the order export is slow on large accounts".into(), ..warm("x", 0, 760_000, 7.19) };
        assert_eq!(super::rows(&[&long], NOW)[0].text, "Find why the order export is slow on la…  12:40 · $7.19 cold");
    }

    #[test]
    fn the_nudge_fires_once_per_session_per_warm_period() {
        let mut n = Nudges::default();
        let w = warm("a", 0, 6 * 60_000, 2.0);
        assert!(!n.due(Some(&w), NOW), "6:00 left: not yet");
        assert!(n.due(Some(&w), w.cold_at - NUDGE_MS), "at 5:00");
        assert!(!n.due(Some(&w), w.cold_at - NUDGE_MS + 1000), "not again at 4:59");
        assert!(!n.due(Some(&w), w.cold_at - 1000));
        // A request at 3:00 left keeps the cache warm an hour more: a new period.
        let again = Warm { cold_at: w.cold_at - 180_000 + 3_600_000, ..w.clone() };
        assert!(!n.due(Some(&again), w.cold_at - 180_000));
        assert!(n.due(Some(&again), again.cold_at - NUDGE_MS), "its own 5:00");
        assert!(!n.due(None, NOW), "nothing counted: no nudge");
    }

    #[test]
    fn a_start_in_the_last_minutes_nudges_once_and_a_cold_cache_never() {
        let mut n = Nudges::default();
        let w = warm("a", 0, 150_000, 2.0);
        assert!(n.due(Some(&w), NOW), "2:30 left at the start");
        assert!(!n.due(Some(&w), NOW + 1000));
        let cold = warm("b", 0, 0, 2.0);
        assert!(!n.due(Some(&cold), NOW));
        assert!(n.done.iter().all(|(id, _)| id == "a"));
        n.due(None, w.cold_at);
        assert!(n.done.is_empty(), "a period that ended is forgotten");
    }
}
