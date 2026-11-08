//! The command palette: a query field over the window and, under it, the sessions, tabs, days
//! and actions whose names hold the typed letters in order, the tightest matches first.

use std::collections::BTreeMap;

use rune::{Color, El, Key as RuneKey, KeyEvent, ScrollSpec, Span, TextStyle, Tx, col, el, rich, row, spacer, text};
use tokprof_browse::browser::{Key, Sort, now_ms};
use tokprof_core::fmt::{local_time, usd};
use tokprof_core::rhythm;

use crate::inspector::Tab;
use crate::overlay::{self, R_CARD, R_ROW};
use crate::pal::{self, Pal};
use crate::reports::Mode;
use crate::{Desk, Focus};

/// The open palette: what is typed, and the place of the highlighted entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    pub query: String,
    pub at: usize,
}

/// The groups of entries, in the order they show with nothing typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Actions,
    Tabs,
    Sessions,
    Days,
}

impl Group {
    pub fn heading(self) -> &'static str {
        match self {
            Group::Actions => "Actions",
            Group::Tabs => "Tabs",
            Group::Sessions => "Sessions",
            Group::Days => "Days",
        }
    }
}

/// What an entry does when it is picked.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// Selects the session with this id.
    Session(String),
    Tab(Tab),
    /// Shows this local date in the lanes, or the day of the latest action.
    Day(Option<i64>),
    Act(Action),
}

/// What the window does from a key or a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Settings,
    KeyHelp,
    DayView,
    Reports,
    Period7,
    Period30,
    Period90,
    Handoff,
    Jfr,
    Html,
    Filter,
    ClearFilter,
    SortTime,
    SortCost,
    SortNext,
    SortWaiting,
    Resort,
    Fit,
    WholeDay,
    DayBack,
    DayOn,
    SessionDay,
    FullSize,
    BackToList,
    ToLanes,
    ToList,
    ToInspector,
    CloseWindow,
    Quit,
}

impl Action {
    pub const ALL: [Action; 29] = [
        Action::Settings,
        Action::KeyHelp,
        Action::DayView,
        Action::Reports,
        Action::Period7,
        Action::Period30,
        Action::Period90,
        Action::Handoff,
        Action::Jfr,
        Action::Html,
        Action::Filter,
        Action::ClearFilter,
        Action::SortTime,
        Action::SortCost,
        Action::SortNext,
        Action::SortWaiting,
        Action::Resort,
        Action::Fit,
        Action::WholeDay,
        Action::DayBack,
        Action::DayOn,
        Action::SessionDay,
        Action::FullSize,
        Action::BackToList,
        Action::ToLanes,
        Action::ToList,
        Action::ToInspector,
        Action::CloseWindow,
        Action::Quit,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Action::Settings => "Settings…",
            Action::KeyHelp => "Key help",
            Action::DayView => "Day",
            Action::Reports => "Reports",
            Action::Period7 => "Reports: the last 7 days",
            Action::Period30 => "Reports: the last 30 days",
            Action::Period90 => "Reports: the last 90 days",
            Action::Handoff => "Copy the handoff line",
            Action::Jfr => "Write the JFR for IntelliJ",
            Action::Html => "Write the HTML flame graph and open it",
            Action::Filter => "Filter the sessions",
            Action::ClearFilter => "Clear the filter",
            Action::SortTime => "Sort by Time",
            Action::SortCost => "Sort by Cost",
            Action::SortNext => "Sort by Next request",
            Action::SortWaiting => "Sort by Waiting",
            Action::Resort => "Sort again",
            Action::Fit => "Fit the lanes to the day's actions",
            Action::WholeDay => "Show 24 h in the lanes",
            Action::DayBack => "Previous day",
            Action::DayOn => "Next day",
            Action::SessionDay => "The selected session's day",
            Action::FullSize => "Full-size inspector",
            Action::BackToList => "Back to the list",
            Action::ToLanes => "Keyboard to the lanes",
            Action::ToList => "Keyboard to the list",
            Action::ToInspector => "Keyboard to the inspector",
            Action::CloseWindow => "Close the window",
            Action::Quit => "Quit",
        }
    }

    /// The keys that do the same outside the palette.
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Action::Settings => &["⌘,"],
            Action::KeyHelp => &["?"],
            Action::DayView => &["⌘1"],
            Action::Reports => &["⌘2"],
            Action::Period7 | Action::Period30 | Action::Period90 => &[],
            Action::Handoff => &["c"],
            Action::Jfr => &["f"],
            Action::Html => &["o"],
            Action::Filter => &["/"],
            Action::ClearFilter => &["esc"],
            Action::SortTime | Action::SortCost | Action::SortNext | Action::SortWaiting => &["s"],
            Action::Resort => &["r"],
            Action::Fit | Action::WholeDay => &[],
            Action::DayBack => &["←"],
            Action::DayOn => &["→"],
            Action::SessionDay => &["d"],
            Action::FullSize => &["⏎"],
            Action::BackToList => &["esc"],
            Action::ToLanes | Action::ToList | Action::ToInspector => &["Tab"],
            Action::CloseWindow => &["⌘W"],
            Action::Quit => &["⌘Q"],
        }
    }

    /// Whether the action works on what Day shows (the list, the lanes, the inspector), so that
    /// from Reports it shows Day first.
    pub fn in_day(self) -> bool {
        !matches!(
            self,
            Action::Settings
                | Action::KeyHelp
                | Action::DayView
                | Action::Reports
                | Action::Period7
                | Action::Period30
                | Action::Period90
                | Action::Handoff
                | Action::Jfr
                | Action::Html
                | Action::CloseWindow
                | Action::Quit
        )
    }

    /// Whether the action does something in the window as it is.
    fn offered(self, s: &Desk) -> bool {
        let zoomed = s.browser.zoomed();
        match self {
            Action::ClearFilter => !s.browser.filter().is_empty(),
            Action::SortWaiting => s.browser.waiting(),
            // The full-size inspector has the keyboard.
            Action::FullSize | Action::ToLanes | Action::ToList | Action::ToInspector => !zoomed,
            Action::BackToList => zoomed,
            Action::DayView => s.reports.mode == Mode::Reports,
            Action::Reports => s.reports.mode == Mode::Day,
            _ => true,
        }
    }
}

/// An entry of the palette, as drawn and as ⏎ runs it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub group: Group,
    pub pick: Pick,
    pub title: String,
    pub detail: String,
    /// What the right of the row says: a session's cost, a day's sessions and cost.
    pub right: String,
    pub keys: &'static [&'static str],
    /// A session's agent, by its place in the registry, which picks its dot's color.
    pub agent: Option<usize>,
    /// How tight the match is, and the places of the matched letters in the title or the detail.
    pub score: i32,
    pub in_title: Vec<usize>,
    pub in_detail: Vec<usize>,
    /// A session's last activity or a day's date, for the order with nothing typed.
    recency: i64,
}

impl Entry {
    fn new(group: Group, pick: Pick, title: impl Into<String>) -> Entry {
        Entry {
            group,
            pick,
            title: title.into(),
            detail: String::new(),
            right: String::new(),
            keys: &[],
            agent: None,
            score: 0,
            in_title: Vec::new(),
            in_detail: Vec::new(),
            recency: 0,
        }
    }
}

/// With nothing typed: the most recent sessions and the newest days shown.
pub const RECENT_SESSIONS: usize = 20;
pub const RECENT_DAYS: usize = 14;
/// With a query: the most sessions and days shown.
pub const MATCHED_SESSIONS: usize = 100;
pub const MATCHED_DAYS: usize = 30;
/// The score at or under which a match is loose: its letters are far apart or deep in the name.
pub const LOOSE: i32 = 0;
/// How much lower a match in the detail ranks than one in the title.
const DETAIL_PENALTY: i32 = 2;

/// The entries the palette shows for `query`, in their order.
pub fn entries(s: &Desk, query: &str) -> Vec<Entry> {
    let q: String = query.chars().filter(|c| !c.is_whitespace()).collect();
    let mut all = candidates(s);
    if q.is_empty() {
        let mut out = Vec::new();
        for g in [Group::Actions, Group::Tabs, Group::Sessions, Group::Days] {
            let mut of: Vec<Entry> = all.iter().filter(|e| e.group == g).cloned().collect();
            match g {
                Group::Sessions => {
                    of.sort_by_key(|e| std::cmp::Reverse(e.recency));
                    of.truncate(RECENT_SESSIONS);
                }
                // Today and the newest days after it.
                Group::Days => of.truncate(RECENT_DAYS + 1),
                _ => {}
            }
            out.extend(of);
        }
        return out;
    }
    all.retain_mut(|e| {
        // A day's name is short words and numbers, where letters far apart match by chance: its
        // letters match only in runs from the starts of words that follow one another.
        let day = e.group == Group::Days;
        let fits = |text: &str| fuzzy(&q, text).filter(|(_, at)| !day || in_word_runs(text, at));
        let title = fits(&e.title);
        let detail = match e.group {
            Group::Sessions | Group::Days => fits(&e.detail).map(|(score, at)| (score - DETAIL_PENALTY, at)),
            _ => None,
        };
        match (title, detail) {
            (Some((t, at)), d) if d.as_ref().is_none_or(|(d, _)| t >= *d) => {
                e.score = t;
                e.in_title = at;
            }
            (_, Some((d, at))) => {
                e.score = d;
                e.in_detail = at;
            }
            _ => return false,
        }
        true
    });
    // Letters scattered far apart are left out while tighter matches are there.
    if all.iter().any(|e| e.score > LOOSE) {
        all.retain(|e| e.score > LOOSE);
    }
    let mut groups: Vec<Vec<Entry>> = Vec::new();
    for g in [Group::Actions, Group::Tabs, Group::Sessions, Group::Days] {
        let mut of: Vec<Entry> = all.iter().filter(|e| e.group == g).cloned().collect();
        if of.is_empty() {
            continue;
        }
        // The stable sort keeps the entries' own order among equals.
        of.sort_by_key(|e| (std::cmp::Reverse(e.score), e.title.chars().count(), std::cmp::Reverse(e.recency)));
        of.truncate(match g {
            Group::Sessions => MATCHED_SESSIONS,
            Group::Days => MATCHED_DAYS,
            _ => usize::MAX,
        });
        groups.push(of);
    }
    // The groups in the order of their best match. Among groups as good, the short ones, actions
    // and tabs, go first, then the one whose second match is better, so that a long list of
    // loose matches does not bury the tight ones of the next group; then the order above.
    groups.sort_by_key(|of| {
        let short = matches!(of[0].group, Group::Actions | Group::Tabs);
        std::cmp::Reverse((of[0].score, short, of.get(1).map_or(i32::MIN, |e| e.score)))
    });
    groups.into_iter().flatten().collect()
}

/// Every entry the palette can show, in the order of its groups.
fn candidates(s: &Desk) -> Vec<Entry> {
    let b = &s.browser;
    let now = now_ms();
    let mut out: Vec<Entry> =
        Action::ALL.iter().filter(|a| a.offered(s)).map(|&a| Entry { keys: a.keys(), ..Entry::new(Group::Actions, Pick::Act(a), a.title()) }).collect();
    const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
    out.extend(Tab::ALL.iter().enumerate().map(|(i, &t)| {
        let keys = if t == Tab::Flame { &["0"][..] } else { DIGITS.get(i).map_or(&[][..], std::slice::from_ref) };
        Entry { keys, ..Entry::new(Group::Tabs, Pick::Tab(t), t.name()) }
    }));
    // The sessions, and the days they were last active on, as the list's separators group them.
    let offset = &rhythm::local_offset;
    let mut days: BTreeMap<i64, (usize, f64)> = BTreeMap::new();
    for r in 0..b.len() {
        let c = b.cells(r, now);
        let day = days.entry(rhythm::date_of(c.active_ms, offset)).or_default();
        day.0 += 1;
        day.1 += c.usd.unwrap_or(0.0);
        let (title, detail) = match c.title {
            Some(t) => (t.to_string(), c.project.to_string()),
            None => (c.project.to_string(), String::new()),
        };
        out.push(Entry {
            detail,
            right: c.cost,
            agent: Some(c.agent_place),
            recency: c.active_ms,
            ..Entry::new(Group::Sessions, Pick::Session(b.session(r).to_string()), title)
        });
    }
    let today = rhythm::date_of(now, offset);
    let count = |(n, cost): (usize, f64)| format!("{n} {}, {}", if n == 1 { "session" } else { "sessions" }, usd(cost));
    out.push(Entry {
        detail: "the day of the latest action".into(),
        right: days.get(&today).copied().map(count).unwrap_or_default(),
        recency: i64::MAX,
        ..Entry::new(Group::Days, Pick::Day(None), "Today")
    });
    for (&date, &sessions) in days.iter().rev().filter(|(d, _)| **d != today) {
        let label = crate::view::day_label(date, today);
        // A day named by its weekday also matches by its date.
        let noon = rhythm::midnight(date, offset) + 12 * 3_600_000;
        let detail = if (1..=6).contains(&(today - date)) { local_time(noon, "%-d %B") } else { String::new() };
        out.push(Entry { detail, right: count(sessions), recency: date, ..Entry::new(Group::Days, Pick::Day(Some(date)), label) });
    }
    out
}

/// How tightly `query` matches `text`: all its letters in order, ignoring case. The score and
/// the char places of the letters in their best placement, or `None` when they are not all
/// there in order.
pub fn fuzzy(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    const START: i32 = 10;
    const WORD: i32 = 8;
    const RUN: i32 = 8;
    const GAP_OPEN: i32 = 3;
    const LEAD_MAX: i32 = 10;
    const NONE: i32 = i32::MIN / 4;
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(lower).collect();
    let t: Vec<char> = text.chars().collect();
    let (m, n) = (q.len(), t.len());
    if m == 0 {
        return Some((0, Vec::new()));
    }
    if m > n {
        return None;
    }
    let tl: Vec<char> = t.iter().map(|&c| lower(c)).collect();
    let bonus: Vec<i32> = (0..n)
        .map(|j| {
            if j == 0 {
                return START;
            }
            let (before, at) = (t[j - 1], t[j]);
            if !before.is_alphanumeric() && at.is_alphanumeric()
                || before.is_lowercase() && at.is_uppercase()
                || !before.is_ascii_digit() && at.is_ascii_digit()
            {
                WORD
            } else {
                0
            }
        })
        .collect();
    // score[i][j]: the best placement of the first i + 1 letters with letter i at place j; from
    // [i][j]: the place of letter i - 1 in it.
    let mut score = vec![vec![NONE; n]; m];
    let mut from = vec![vec![usize::MAX; n]; m];
    for j in 0..n {
        if tl[j] == q[0] {
            score[0][j] = 1 + bonus[j] - (j as i32).min(LEAD_MAX);
        }
    }
    for i in 1..m {
        // The best of score[i - 1][k] + k over k < j - 1, for a gap of j - k - 1 places.
        let mut best: (i32, usize) = (NONE, usize::MAX);
        for j in i..n {
            if j >= 2 && score[i - 1][j - 2] > NONE {
                let k = j - 2;
                let v = score[i - 1][k] + k as i32;
                if v > best.0 {
                    best = (v, k);
                }
            }
            if tl[j] != q[i] {
                continue;
            }
            let run = if score[i - 1][j - 1] > NONE { score[i - 1][j - 1] + RUN } else { NONE };
            let gap = if best.0 > NONE { best.0 - (j as i32 - 1) - GAP_OPEN } else { NONE };
            let (prev, k) = if run >= gap { (run, j - 1) } else { (gap, best.1) };
            if prev > NONE {
                score[i][j] = prev + 1 + bonus[j];
                from[i][j] = k;
            }
        }
    }
    let (end, &total) = score[m - 1].iter().enumerate().max_by_key(|&(j, s)| (*s, std::cmp::Reverse(j)))?;
    if total <= NONE {
        return None;
    }
    let mut at = vec![end; m];
    for i in (1..m).rev() {
        at[i - 1] = from[i][at[i]];
    }
    Some((total, at))
}

/// Whether the letters at char places `at` of `text` lie in runs that each start a word, the
/// words one after another: `8oct` in `Thu 8 Oct` does, `to` (T…O) does not.
pub fn in_word_runs(text: &str, at: &[usize]) -> bool {
    let t: Vec<char> = text.chars().collect();
    // The word of each place, counting the words before it.
    let mut word = Vec::with_capacity(t.len());
    let mut n = 0usize;
    for j in 0..t.len() {
        if j > 0 && !t[j - 1].is_alphanumeric() && t[j].is_alphanumeric() {
            n += 1;
        }
        word.push(n);
    }
    let starts = |j: usize| j == 0 || !t[j - 1].is_alphanumeric();
    let Some(&first) = at.first() else { return true };
    if first >= t.len() || !starts(first) {
        return false;
    }
    at.windows(2).all(|w| w[1] == w[0] + 1 || (w[1] < t.len() && starts(w[1]) && word[w[1]] == word[w[0]] + 1))
}

/// The byte ranges of the chars at `places` in `text`, merged where they touch, drawn in the
/// accent and bold.
fn spans(text: &str, places: &[usize], accent: Color) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (i, (b, c)) in text.char_indices().enumerate() {
        if !places.contains(&i) {
            continue;
        }
        let end = b + c.len_utf8();
        match out.last_mut() {
            Some(last) if last.end as usize == b => last.end = end as u32,
            _ => out.push(Span::new(b, end).color(accent).weight(700.0)),
        }
    }
    out
}

impl Desk {
    /// Opens the palette, or closes it; it does not open over another card. A filter being typed
    /// is kept, as ⏎ keeps it, so that the keys the palette sends are not typed into it.
    pub fn toggle_palette(&mut self) {
        if self.palette.is_some() {
            self.palette = None;
            return;
        }
        if self.panel.is_some() || self.help {
            return;
        }
        if self.browser.editing() {
            let view = self.viewport();
            self.browser.on_key(Key::Enter, view);
        }
        self.palette = Some(Palette::default());
    }

    /// The keys of the open palette: every key stays with it.
    pub fn palette_key(&mut self, e: &KeyEvent, tx: &mut Tx) -> bool {
        let Some(mut palette) = self.palette.clone() else { return false };
        let control = |c: &str| matches!(&e.key, RuneKey::Text(t) if e.mods.ctrl && !e.mods.cmd && t.eq_ignore_ascii_case(c));
        let step = match &e.key {
            RuneKey::Up => Some(-1),
            RuneKey::Down => Some(1),
            _ if control("p") => Some(-1),
            _ if control("n") => Some(1),
            _ => None,
        };
        if let Some(step) = step {
            let count = entries(self, &palette.query).len();
            palette.at = (palette.at as isize + step).clamp(0, count.saturating_sub(1) as isize) as usize;
            if palette.at == 0 {
                tx.scroll_to_y(LIST, 0.0);
            } else {
                tx.scroll_into_view(row_key(palette.at));
            }
            self.palette = Some(palette);
            return true;
        }
        match &e.key {
            RuneKey::Escape => self.palette = None,
            // With nothing matched, ⏎ does nothing and the palette stays.
            RuneKey::Enter => {
                if let Some(entry) = entries(self, &palette.query).into_iter().nth(palette.at) {
                    self.run(entry.pick, tx);
                }
            }
            RuneKey::Backspace => {
                palette.query.pop();
                self.retype(palette, tx);
            }
            RuneKey::Text(t) if !e.mods.cmd && !e.mods.ctrl => {
                palette.query.push_str(t);
                self.retype(palette, tx);
            }
            _ => {}
        }
        true
    }

    /// The palette with a changed query, its first entry highlighted.
    fn retype(&mut self, mut palette: Palette, tx: &mut Tx) {
        palette.at = 0;
        self.palette = Some(palette);
        tx.scroll_to_y(LIST, 0.0);
    }

    /// Does what an entry says, with the palette closed.
    pub fn run(&mut self, pick: Pick, tx: &mut Tx) {
        self.palette = None;
        // A session, a tab, a day and what works on them are shown in Day.
        if !matches!(pick, Pick::Act(a) if !a.in_day()) {
            crate::reports::show(self, Mode::Day);
        }
        match pick {
            Pick::Session(id) => {
                let zoomed = self.browser.zoomed();
                if self.browser.open_session(&id) {
                    if zoomed {
                        self.restore_list(tx);
                    }
                    self.pane = Focus::List;
                    self.show_tab(Tab::Summary, tx);
                    tx.scroll_into_view(id.clone());
                    tx.scroll_into_view(format!("lane-{id}"));
                }
            }
            Pick::Tab(t) => self.show_tab(t, tx),
            Pick::Day(None) => self.browser.show_latest_day(),
            Pick::Day(Some(date)) => {
                self.browser.show_day(date);
                // A step of no days makes the latest date the latest day, as the steps reach it.
                self.browser.step_day_by(0);
            }
            Pick::Act(a) => self.act(a, tx),
        }
    }

    fn act(&mut self, a: Action, tx: &mut Tx) {
        let key = |c: char| Key::Char(c);
        match a {
            Action::Settings => self.toggle_settings(),
            Action::KeyHelp => self.open_help(),
            Action::DayView => crate::reports::show(self, Mode::Day),
            Action::Reports => crate::reports::show(self, Mode::Reports),
            Action::Period7 | Action::Period30 | Action::Period90 => {
                crate::reports::show(self, Mode::Reports);
                let days = match a {
                    Action::Period7 => 7,
                    Action::Period30 => 30,
                    _ => 90,
                };
                crate::reports::pick_period(self, days, tx);
            }
            Action::Handoff => self.browser_key(key('c'), tx),
            Action::Jfr => self.browser_key(key('f'), tx),
            Action::Html => self.browser_key(key('o'), tx),
            Action::Filter => self.browser_key(key('/'), tx),
            Action::ClearFilter => self.browser.clear_filter(),
            Action::SortTime | Action::SortCost | Action::SortNext | Action::SortWaiting => {
                self.browser.sort_by(match a {
                    Action::SortTime => Sort::Time,
                    Action::SortCost => Sort::Cost,
                    Action::SortNext => Sort::Next,
                    _ => Sort::Waiting,
                });
                self.scroll_to_selected(tx);
            }
            Action::Resort => self.browser_key(key('r'), tx),
            Action::Fit => self.fit = true,
            Action::WholeDay => self.fit = false,
            Action::DayBack => self.browser.step_day_by(-1),
            Action::DayOn => self.browser.step_day_by(1),
            Action::SessionDay => self.browser_key(key('d'), tx),
            Action::FullSize => self.browser_key(Key::Enter, tx),
            Action::BackToList => self.browser_key(Key::Esc, tx),
            Action::ToLanes => self.pane = Focus::Lanes,
            Action::ToList => self.pane = Focus::List,
            Action::ToInspector => self.pane = Focus::Inspector,
            Action::CloseWindow => tx.close_window(),
            Action::Quit => tx.quit(),
        }
    }

    /// Passes a key to the browser as `on_key` does, quitting where the browser says so.
    fn browser_key(&mut self, key: Key, tx: &mut Tx) {
        if !crate::browser_key(self, key, tx) {
            tx.quit();
        }
    }

    fn scroll_to_selected(&self, tx: &mut Tx) {
        if let Some(row) = self.browser.selected() {
            tx.scroll_into_view(self.browser.session(row).to_string());
        }
    }
}

/// The palette's width, the height of a row, and the most rows it shows before it scrolls.
const CARD_W: f32 = 640.0;
const ROW_H: f32 = 34.0;
const ROWS: f32 = 10.0;
/// The most room a row's detail takes, so that the name keeps the rest.
const DETAIL_W: f32 = 240.0;
/// The key of the scrolling list of entries.
pub const LIST: &str = "palette-list";
/// The query's type.
const QUERY: TextStyle = TextStyle::new(16.0, 450.0);

/// The key of the entry at `i`.
pub fn row_key(i: usize) -> String {
    format!("pick-{i}")
}

/// The palette over the window: the query, the entries under small headings, and the keys.
pub fn view(s: &Desk, palette: &Palette, p: &Pal) -> El {
    let list = entries(s, &palette.query);
    let at = palette.at.min(list.len().saturating_sub(1));
    let mut field = row().key("palette-query").w_full().h(52.0).px(18.0).gap(1.0).center_y();
    let caret = || el().key("palette-caret").w(1.5).h(19.0).radius(0.75).fill(p.accent);
    if palette.query.is_empty() {
        field = field
            .child(caret())
            .child(text("Search sessions, tabs, days and actions").font(QUERY).color(p.ink3).single_line().truncate().shrink().min_w(0.0).pl(2.0));
    } else {
        field = field
            .child(text(palette.query.clone()).key("palette-text").font(QUERY).color(p.ink).single_line().truncate_head().shrink().min_w(0.0))
            .child(caret());
    }
    field = field.child(spacer().grow(1.0)).child(overlay::keycap("⌘K", p));
    let body = if list.is_empty() {
        row()
            .key("palette-empty")
            .w_full()
            .h(64.0)
            .px(20.0)
            .center_y()
            .child(text(format!("Nothing matches “{}”", palette.query)).font(pal::BODY).color(p.ink3).single_line().truncate().min_w(0.0))
    } else {
        let mut rows = col().w_full().px(8.0).py(6.0);
        let mut group = None;
        for (i, e) in list.iter().enumerate() {
            if group != Some(e.group) {
                group = Some(e.group);
                rows = rows.child(
                    text(e.group.heading())
                        .key(format!("palette-group-{}", e.group.heading().to_lowercase()))
                        .font(pal::SMALL_STRONG)
                        .color(p.ink3)
                        .single_line()
                        .px(10.0)
                        .pt(if i == 0 { 4.0 } else { 10.0 })
                        .pb(4.0),
                );
            }
            rows = rows.child(entry_row(i, e, i == at, p));
        }
        let list_el = col().key(LIST).w_full().max_h(ROWS * ROW_H + 12.0).scroll(ScrollSpec { stick_bottom: false }).child(rows);
        // More rows than it shows: the last one shown fades, so that it reads as more below.
        if list.len() > ROWS as usize { overlay::faded(list_el, p.panel) } else { list_el }
    };
    let count = match list.len() {
        0 => String::new(),
        1 => "1 match".to_string(),
        n if palette.query.is_empty() => format!("{n} entries"),
        n => format!("{n} matches"),
    };
    let foot = row()
        .w_full()
        .px(18.0)
        .py(11.0)
        .gap(14.0)
        .center_y()
        .child(overlay::hint(&["↑", "↓"], "move", p))
        .child(overlay::hint(&["⏎"], "open", p))
        .child(overlay::hint(&["esc"], "close", p))
        .child(spacer().grow(1.0))
        .child(text(count).key("palette-count").font(pal::SMALL).color(p.ink3).single_line());
    let card = overlay::card("palette", "Command palette", CARD_W, R_CARD, p)
        .child(field)
        .child(el().w_full().h(1.0).fill(p.row_line))
        .child(body)
        .child(el().w_full().h(1.0).fill(p.row_line))
        .child(foot);
    // A click outside the card closes the palette.
    overlay::scrim("palette-scrim", p, |s| s.palette = None).child(col().w_full().h_full().items_center().pt(72.0).child(card))
}

/// An entry's row: its name with the matched letters in the accent, its detail, and on the right
/// its cost, its sessions or its keys.
fn entry_row(i: usize, e: &Entry, on: bool, p: &Pal) -> El {
    let pick = e.pick.clone();
    let mut r = row()
        .key(row_key(i))
        .w_full()
        .h(ROW_H)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(R_ROW)
        .fill(if on { p.sel } else { Color::TRANSPARENT })
        .hover(if on { p.sel } else { p.hover })
        .role("option")
        .checked(on)
        .label(e.title.clone())
        .on_press_tx(move |s: &mut Desk, tx| s.run(pick.clone(), tx));
    if let Some(place) = e.agent {
        r = r.child(crate::view::dot(p.agents[place % p.agents.len()], 7.0));
    }
    r = r.child(rich(e.title.clone(), spans(&e.title, &e.in_title, p.accent)).key("pick-title").font(pal::BODY).color(p.ink).truncate().shrink().min_w(0.0));
    if !e.detail.is_empty() {
        r = r.child(
            rich(e.detail.clone(), spans(&e.detail, &e.in_detail, p.accent))
                .key("pick-detail")
                .font(pal::SMALL)
                .color(p.ink3)
                .truncate()
                .shrink()
                .min_w(0.0)
                .max_w(DETAIL_W),
        );
    }
    r = r.child(spacer().grow(1.0));
    if !e.right.is_empty() {
        let font = if e.group == Group::Sessions { pal::STRONG_NUM } else { pal::SMALL_NUM };
        r = r.child(text(e.right.clone()).key("pick-right").font(font).color(if e.group == Group::Sessions { p.ink } else { p.ink2 }).single_line());
    }
    if !e.keys.is_empty() {
        r = r.child(row().gap(3.0).children(e.keys.iter().map(|k| overlay::keycap(k, p))));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn places(q: &str, t: &str) -> Vec<usize> {
        fuzzy(q, t).unwrap().1
    }

    fn score(q: &str, t: &str) -> i32 {
        fuzzy(q, t).map_or(i32::MIN, |m| m.0)
    }

    #[test]
    fn all_letters_in_order_ignoring_case_and_spaces() {
        assert!(fuzzy("sum", "Summary").is_some());
        assert!(fuzzy("SUM", "summary").is_some());
        assert!(fuzzy("s c", "Sort by Cost").is_some());
        assert!(fuzzy("ums", "Summary").is_none(), "out of order");
        assert!(fuzzy("summaryy", "Summary").is_none());
        assert_eq!(fuzzy("", "anything"), Some((0, Vec::new())));
        assert!(fuzzy("x", "").is_none());
    }

    #[test]
    fn a_prefix_beats_a_word_start_beats_a_run_inside_a_word_beats_scattered_letters() {
        assert!(score("to", "Tools") > score("to", "Keyboard to the lanes"));
        assert!(score("to", "Keyboard to the lanes") > score("to", "Photos"));
        assert!(score("nd", "Next day") > score("nd", "Copy the handoff line"));
        assert!(score("cost", "Sort by Cost") > score("cost", "Copy the handoff line, sort"));
        assert!(score("tab", "Tabs") > score("tab", "The tail of a bar"));
        assert!(score("to", "Tokprof survey") > score("to", "Test OAuth"), "a run beats two word starts");
    }

    #[test]
    fn the_places_are_those_of_the_best_placement() {
        // The run at the word's start, not the first letters that fit.
        assert_eq!(places("cost", "a cat, sort by Cost"), vec![15, 16, 17, 18]);
        assert_eq!(places("nd", "Next day"), vec![0, 5]);
        assert_eq!(places("sum", "Summary"), vec![0, 1, 2]);
        assert_eq!(places("é", "Café"), vec![3]);
    }

    #[test]
    fn days_match_only_in_runs_from_the_starts_of_words() {
        let day = |q: &str, t: &str| fuzzy(q, t).is_some_and(|(_, at)| in_word_runs(t, &at));
        assert!(day("to", "Today") && day("tod", "Today"), "a run from the start");
        assert!(!day("to", "Thu 8 Oct 2026"), "T in Thu, o in Oct: two words apart");
        assert!(!day("to", "8 October"), "t inside a word");
        assert!(day("8oct", "Thu 8 Oct 2026") && day("oct", "Thu 8 Oct 2026") && day("thu", "Thu 8 Oct 2026"));
        assert!(day("ye", "Yesterday") && !day("ys", "Yesterday"));
        // Sessions keep the loose matches.
        assert!(fuzzy("to", "Thu 8 Oct 2026").is_some());
    }

    #[test]
    fn spans_merge_where_the_letters_touch() {
        let s = spans("Sort by Cost", &[0, 1, 8, 9, 10], Color::BLACK);
        assert_eq!(s.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>(), vec![(0, 2), (8, 11)]);
        let s = spans("Café", &[3], Color::BLACK);
        assert_eq!((s[0].start, s[0].end), (3, 5), "byte ranges of the chars");
    }
}
