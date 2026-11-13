//! The month in review: a card of one month, made to be posted, over Month or Quarter, its image
//! in Month and Quarter, and the same card alone as a 1080×1350 image; and the year's card, Year
//! in review, in the same look with its months in place of the days (see `year`). Neither names
//! a project, title or path. "Save image" draws a card in a frame without a window, by this
//! program run again, and writes it to `~/Downloads`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rune::{Color, El, Key as RuneKey, KeyEvent, TextStyle, Tx, col, el, row, spacer, text};
use tokprof_browse::render_month::{big_numbers, cache, delta_cells, facts, headline, next_month_plain, weeks};
use tokprof_core::calendar::level;
use tokprof_core::month::{Month, Ym};
use tokprof_core::period::Before;
use tokprof_core::year::{MonthKind, Year};

use crate::overlay::{PILL, R_CARD};
use crate::pal::{self, Pal};
use crate::year::cell;
use crate::{Desk, plain};

/// The card's size in points; the image is drawn at twice that, 1080×1350.
pub const CARD: (f32, f32) = (540.0, 675.0);
const PAD: f32 = 32.0;

const BRAND: TextStyle = TextStyle::new(11.5, 700.0).tracking(0.6);
const HEADLINE: TextStyle = TextStyle::new(32.0, 780.0).tracking(-0.8);
const SUB: TextStyle = TextStyle::new(17.0, 500.0).tracking(-0.2);
const BADGE: TextStyle = TextStyle::new(12.5, 750.0).tracking(0.5);
const RULE: TextStyle = TextStyle::new(11.0, 400.0);
const BIG: TextStyle = TextStyle::new(26.0, 760.0).tracking(-0.6).features("'tnum' 1");
const LABEL: TextStyle = TextStyle::new(11.0, 500.0);
const FACT: TextStyle = TextStyle::new(11.5, 400.0).features("'tnum' 1");
/// For names, such as a model's: tabular figures would space its hyphens out.
const FACT_PLAIN: TextStyle = TextStyle::new(11.5, 400.0);
const FACT_HEAD: TextStyle = TextStyle::new(10.0, 700.0).tracking(0.4);
const NEXT: TextStyle = TextStyle::new(12.0, 500.0);
const STAT: TextStyle = TextStyle::new(16.0, 720.0).tracking(-0.3).features("'tnum' 1");

/// The open card: its month, computed, and what the last save said.
pub struct Card {
    pub month: Month,
    pub status: Option<String>,
}

/// What a save in the background sends back: the path written, or why not.
pub type Saved = Arc<Mutex<Option<Result<PathBuf, String>>>>;

/// Where a worker puts a month it computed.
pub type Slot = Arc<Mutex<Option<Month>>>;

/// The card's state in the window.
#[derive(Default)]
pub struct State {
    pub card: Option<Card>,
    /// The frame shows only the card, for the image.
    pub alone: bool,
    /// The month `--month` names, for a frame without a window.
    pub wanted: Option<Ym>,
    /// The command line's filters, for the program that draws the image.
    pub args: Vec<String>,
    pub saving: Option<Saved>,
    /// The month ← or → asked for, computed on a worker; the card shows the month before it until
    /// it arrives.
    pub next: Option<Arc<Mutex<Option<Month>>>>,
    /// The cards Month and Quarter show as images, by month, each with the store it was computed
    /// from, and those being computed.
    pub images: HashMap<Ym, (usize, Arc<Month>)>,
    pub drawing: HashMap<Ym, (usize, Slot)>,
}

/// The address of the calendar's store, which the images are computed from.
fn store_at(s: &Desk) -> Option<(usize, Arc<tokprof_core::daystore::Store>)> {
    s.calendar.data.get().and_then(|c| c.store.clone()).map(|st| (Arc::as_ptr(&st) as usize, st))
}

/// Asks for the cards of `months` from the store as it is now, on a worker, and takes in those
/// computed. Returns whether one arrived.
pub fn want_images(s: &mut Desk, months: &[Ym]) -> bool {
    let mut arrived = false;
    let done: Vec<Ym> = s.month.drawing.iter().filter(|(_, (_, slot))| slot.lock().map(|m| m.is_some()).unwrap_or(true)).map(|(ym, _)| *ym).collect();
    for ym in done {
        if let Some((at, slot)) = s.month.drawing.remove(&ym)
            && let Some(m) = slot.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            s.month.images.insert(ym, (at, Arc::new(m)));
            arrived = true;
        }
    }
    let Some((at, store)) = store_at(s) else { return arrived };
    for &ym in months {
        let fresh = s.month.images.get(&ym).is_some_and(|(a, _)| *a == at) || s.month.drawing.get(&ym).is_some_and(|(a, _)| *a == at);
        if fresh {
            continue;
        }
        let slot: Arc<Mutex<Option<Month>>> = Arc::default();
        s.month.drawing.insert(ym, (at, slot.clone()));
        let (store, select) = (store.clone(), s.calendar.select.clone());
        std::thread::spawn(move || {
            let month = tokprof_browse::contain::run_on(None, || tokprof_browse::calendar::month(&store, &select, ym, tokprof_browse::browser::now_ms()));
            if let Ok(m) = month {
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
            }
            crate::wake();
        });
    }
    arrived
}

/// Computes the cards of `months` at once, for a frame without a window.
pub fn settle_images(s: &mut Desk, months: &[Ym]) {
    let Some((at, store)) = store_at(s) else { return };
    for &ym in months {
        let m = tokprof_browse::calendar::month(&store, &s.calendar.select, ym, tokprof_browse::browser::now_ms());
        s.month.images.insert(ym, (at, Arc::new(m)));
    }
}

/// The card of `ym` to show as an image, once computed; from an older store while a newer one is.
pub fn image_of(s: &Desk, ym: Ym) -> Option<&Month> {
    s.month.images.get(&ym).map(|(_, m)| m.as_ref())
}

/// What identifies the images computed, for the view's memo.
pub fn images_version(s: &Desk) -> usize {
    s.month.images.values().map(|(_, m)| Arc::as_ptr(m) as usize).fold(s.month.images.len(), |a, b| a.wrapping_mul(31).wrapping_add(b))
}

/// How long the program that draws the image may take.
pub const SAVE_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// This month, by the local clock.
fn this_month() -> Ym {
    Ym::of_date(tokprof_core::rhythm::date_of(tokprof_browse::browser::now_ms(), &tokprof_core::rhythm::local_offset))
}

/// The month a card opens on: `ym`, else the last complete month, as `tokprof month` shows it.
pub fn month_to_open(_: &Desk, ym: Option<Ym>) -> Ym {
    ym.unwrap_or_else(|| this_month().previous())
}

/// Opens the card of `ym` (see `month_to_open`), computed from the calendar's store.
pub fn open(s: &mut Desk, ym: Option<Ym>) {
    let ym = month_to_open(s, ym);
    let Some(store) = s.calendar.data.get().and_then(|c| c.store.clone()) else { return };
    let month = tokprof_browse::calendar::month(&store, &s.calendar.select, ym, tokprof_browse::browser::now_ms());
    s.month.card = Some(Card { month, status: None });
}

/// Asks for the card of `ym` on a worker; the card keeps its month until it arrives.
pub fn step_to(s: &mut Desk, ym: Ym) {
    let Some(store) = s.calendar.data.get().and_then(|c| c.store.clone()) else { return };
    let select = s.calendar.select.clone();
    let slot: Arc<Mutex<Option<Month>>> = Arc::default();
    s.month.next = Some(slot.clone());
    std::thread::spawn(move || {
        // A month that panicked does not arrive: the card keeps the month it shows.
        let month = tokprof_browse::contain::run_on(None, || tokprof_browse::calendar::month(&store, &select, ym, tokprof_browse::browser::now_ms()));
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = month.ok();
        crate::wake();
    });
}

/// The keys while the card is open: ← and → the month before and after, `s` saves the image,
/// esc closes it.
pub fn on_key(s: &mut Desk, e: &KeyEvent, _: &mut Tx) -> bool {
    if e.is_command('s') {
        save(s);
        return true;
    }
    if !plain(e) {
        return false;
    }
    let ym = s.month.card.as_ref().and_then(|c| c.month.ym);
    match &e.key {
        RuneKey::Escape => s.month.card = None,
        RuneKey::Left | RuneKey::Right => {
            // From the month asked for last, so that quick presses step on.
            let pending = s.month.next.as_ref().and_then(|n| n.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.ym)));
            if let Some(ym) = pending.or(ym) {
                let to = if e.key == RuneKey::Left { ym.previous() } else { ym.next() };
                if to <= this_month() {
                    step_to(s, to);
                }
            }
        }
        RuneKey::Text(t) if t == "s" => save(s),
        _ => {}
    }
    true
}

/// `path`, or the same name with `-2`, `-3`… before its extension when it exists.
pub fn free_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().to_string());
    let ext = path.extension().map_or_else(String::new, |x| format!(".{}", x.to_string_lossy()));
    (2..).map(|n| path.with_file_name(format!("{stem}-{n}{ext}"))).find(|p| !p.exists()).unwrap_or_else(|| path.to_path_buf())
}

/// Draws the open card as an image into `~/Downloads/tokprof-<YYYY-MM>.png` by running this
/// program again without a window; the bar says where it went.
pub fn save(s: &mut Desk) {
    let Some(ym) = s.month.card.as_ref().and_then(|c| c.month.ym) else { return };
    save_month(s, ym);
}

/// Draws the card of `ym` as an image, as Save image does, whether the card is open or not.
pub fn save_month(s: &mut Desk, ym: Ym) {
    if s.month.saving.is_some() {
        return;
    }
    let path = free_path(&tokprof_browse::paths::home().join("Downloads").join(format!("tokprof-{}.png", ym.key())));
    let Ok(exe) = std::env::current_exe() else { return };
    let mut args: Vec<String> = vec!["--snap".into(), path.to_string_lossy().to_string(), "--scene".into(), "month-card".into(), "--month".into(), ym.key()];
    args.extend(["--size".into(), format!("{}x{}", CARD.0, CARD.1), "--colors".into(), s.theme.get().id.clone()]);
    args.extend(s.month.args.iter().cloned());
    let saved: Saved = Arc::default();
    s.month.saving = Some(saved.clone());
    if let Some(c) = &mut s.month.card {
        c.status = Some("saving the image…".into());
    }
    args.push("--card-image".into());
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(&args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
        let result = tokprof_browse::contain::run_on(None, || run_for(cmd, SAVE_LIMIT))
            .and_then(|r| r)
            .and_then(|_| if path.exists() { Ok(path) } else { Err("the image was not written".to_string()) });
        *saved.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        crate::wake();
    });
}

/// Runs a program to its end, at most `limit`: its last line of stderr when it fails, and a
/// timeout, after which it is stopped.
pub fn run_for(mut cmd: std::process::Command, limit: std::time::Duration) -> Result<(), String> {
    use std::io::Read;
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut err = String::new();
                if let Some(mut e) = child.stderr.take() {
                    let _ = e.read_to_string(&mut err);
                }
                return if status.success() { Ok(()) } else { Err(err.lines().last().unwrap_or("the image was not written").to_string()) };
            }
            Ok(None) if started.elapsed() >= limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("it took more than {} s, so it was stopped", limit.as_secs()));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Takes in a month a step computed, and a finished save. Returns whether the card changed.
pub fn poll(s: &mut Desk) -> bool {
    let mut changed = false;
    if let Some(slot) = &s.month.next {
        let arrived = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(month) = arrived {
            s.month.next = None;
            if s.month.card.is_some() {
                s.month.card = Some(Card { month, status: None });
            }
            changed = true;
        }
    }
    changed | poll_save(s)
}

fn poll_save(s: &mut Desk) -> bool {
    let Some(saved) = &s.month.saving else { return false };
    let Some(result) = saved.lock().unwrap_or_else(|e| e.into_inner()).take() else { return false };
    s.month.saving = None;
    let line = match result {
        Ok(path) => format!("saved {}", tokprof_core::labels::display_path(&path.to_string_lossy(), None, &tokprof_browse::paths::home())),
        Err(e) => format!("could not save the image: {e}"),
    };
    s.status = (line.clone(), std::time::Instant::now());
    if let Some(c) = &mut s.month.card {
        c.status = Some(line);
    }
    true
}

/// The card over the window, with its buttons under it.
pub fn view(card: &Card, p: &Pal) -> El {
    let button = |key: &'static str, label: &str, primary: bool| {
        row()
            .key(key)
            .h(32.0)
            .px(16.0)
            .center_y()
            .radius(PILL)
            .fill(if primary { p.accent } else { p.panel })
            .border(1.0, if primary { p.accent } else { p.row_line })
            .hover(if primary { p.accent.alpha(0.88) } else { p.hover })
            .role("button")
            .label(label.to_string())
            .child(text(label.to_string()).font(pal::STRONG).color(if primary { p.on_accent } else { p.ink }).single_line())
    };
    let status = card.status.clone().unwrap_or_else(|| "← → another month · s saves the image · esc closes".into());
    let buttons = row()
        .key("month-buttons")
        .w(CARD.0)
        .gap(10.0)
        .center_y()
        .child(button("month-save", "Save image", true).on_press(save))
        .child(button("month-close", "Close", false).on_press(|s: &mut Desk| s.month.card = None))
        .child(spacer().grow(1.0))
        .child(text(status).key("month-status").font(pal::SMALL).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    crate::overlay::scrim("month-scrim", p, |s| s.month.card = None).center().child(
        col()
            .gap(14.0)
            .items_center()
            .child(card_el(&card.month, p, true).shadow(30.0, 70.0, Color::BLACK.alpha(0.3)).on_press(|_: &mut Desk| {}))
            .child(buttons),
    )
}

/// The card itself, `CARD` points: the month and its title, the big numbers, the days beside
/// the facts, the cache, the change from the month before, and the line for next month.
pub fn card_el(m: &Month, p: &Pal, rounded: bool) -> El {
    let head = row()
        .w_full()
        .center_y()
        .child(text("TOKPROF").font(BRAND).color(p.accent).single_line())
        .child(spacer().grow(1.0))
        .child(text("from local agent logs · nothing uploaded").font(RULE).color(p.ink3).single_line());
    let name = m.ym.map_or_else(String::new, |ym| ym.name());
    let sub = headline(m).trim_start_matches(name.as_str()).trim().to_string();
    let title = row()
        .gap(10.0)
        .items_end()
        .child(text(name).key("month-headline").font(HEADLINE).color(p.ink).single_line())
        .child(text(sub).font(SUB).color(p.ink2).single_line().pb(4.0));
    let radius = if rounded { R_CARD } else { 0.0 };
    let mut body = col().key("month-card").w(CARD.0).h(CARD.1).padding(PAD).gap(11.0).radius(radius).fill(p.bg).clip_rounded(radius).child(head).child(title);
    if !m.any() {
        return body.child(text("No requests and no messages in this month.").font(SUB).color(p.ink2).single_line());
    }
    // The title, as a badge with its rule beside it.
    if let Some(t) = &m.title {
        body = body.child(
            row()
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    row()
                        .key("month-title")
                        .h(26.0)
                        .px(12.0)
                        .center_y()
                        .radius(PILL)
                        .fill(p.accent)
                        .child(text(t.name.to_uppercase()).font(BADGE).color(p.on_accent).single_line()),
                )
                .child(text(t.rule.clone()).key("month-rule").grow(1.0).min_w(0.0).wrap().max_lines(2).font(RULE).color(p.ink2)),
        );
    }
    // The big numbers, three by two.
    let third = (CARD.0 - 2.0 * PAD - 20.0) / 3.0;
    let big = big_numbers(m);
    let big_cell = |(v, what): &(String, &'static str)| {
        col().w(third).gap(0.0).child(text(v.clone()).font(BIG).color(p.ink).single_line()).child(text(*what).font(LABEL).color(p.ink2).single_line())
    };
    let grid = col()
        .key("month-big")
        .w_full()
        .gap(8.0)
        .child(row().w_full().gap(10.0).children(big[..3].iter().map(big_cell)))
        .child(row().w_full().gap(10.0).children(big[3..].iter().map(big_cell)));
    body = body.child(grid);
    // The days of the month beside the facts.
    let (sq, gap) = (18.0, 4.0);
    let mut cal = col().key("month-days").gap(gap);
    cal = cal.child(row().gap(gap).children(
        ["M", "T", "W", "T", "F", "S", "S"].iter().map(|d| row().w(sq).justify_center().child(text(*d).font(FACT_HEAD).color(p.ink3).single_line())),
    ));
    let busiest = m.busiest_day.map(|b| b.0);
    for week in weeks(m) {
        cal = cal.child(row().gap(gap).children(week.iter().map(|slot| match slot {
            Some(i) => {
                let d = &m.days[*i];
                let sq_el = cell(level(d.amounts.usd, m.cuts), p.accent, p, sq, sq, 4.0);
                if busiest == Some(d.date) { sq_el.border(1.5, p.ink) } else { sq_el }
            }
            None => el().w(sq).h(sq),
        })));
    }
    // What the squares say: spend per day, from less to more, and the cuts between.
    let swatch = |lvl: usize| cell(lvl, p.accent, p, 10.0, 10.0, 2.5);
    let mut key = col().key("month-days-legend").pt(2.0).gap(2.0).child(
        row()
            .gap(3.0)
            .center_y()
            .child(text("spend per day · less").font(RULE).color(p.ink3).single_line().pr(2.0))
            .children((0..5).map(swatch))
            .child(text("more").font(RULE).color(p.ink3).single_line().pl(2.0)),
    );
    if let Some(c) = m.cuts {
        let cut = |v: f64| tokprof_browse::render_calendar::cut_amount(v, tokprof_core::calendar::Unit::Usd);
        key = key.child(text(format!("cuts at {}, {}, {}", cut(c[0]), cut(c[1]), cut(c[2]))).font(RULE).color(p.ink3).single_line());
    }
    cal = cal.child(key);
    let fact_line = |label: &str, value: String| {
        col()
            .gap(1.0)
            .child(text(label.to_uppercase()).font(FACT_HEAD).color(p.ink3).single_line())
            .child(text(value).w_full().wrap().max_lines(2).font(if label == "top model" { FACT_PLAIN } else { FACT }).color(p.ink))
    };
    let mut side = col().key("month-facts").grow(1.0).min_w(0.0).gap(5.0);
    for (label, value) in facts(m) {
        side = side.child(fact_line(label, value));
    }
    body = body.child(row().w_full().gap(24.0).items_start().child(cal).child(side));
    // Small numbers in a row: what one is, under it.
    let quarter = |n: usize| (CARD.0 - 2.0 * PAD - 12.0 * (n as f32 - 1.0)) / n as f32;
    let stats = |key: &'static str, head: String, cells: Vec<(String, String)>| {
        let w = quarter(cells.len().max(1));
        col().key(key).w_full().gap(5.0).child(text(head).font(FACT_HEAD).color(p.ink3).single_line()).child(row().w_full().gap(12.0).children(
            cells.into_iter().map(move |(v, what)| {
                col().w(w).gap(0.0).child(text(v).font(STAT).color(p.ink).single_line()).child(text(what).font(RULE).color(p.ink2).w_full().wrap().max_lines(2))
            }),
        ))
    };
    let (cache, note) = cache(m);
    // The card's room asks for the short names.
    let short = |what: &'static str| -> &'static str {
        match what {
            "of input read from cache" => "read from cache",
            "saved by the cache" => "saved by the cache",
            "avoidable on misses" => "lost to misses",
            w => w,
        }
    };
    let cache: Vec<(String, String)> = cache.into_iter().map(|(v, what)| (v.split(' ').next().unwrap_or("").to_string(), short(what).to_string())).collect();
    // Without the month before, the room goes between the parts, not into one empty band.
    let deltas = delta_cells(m);
    let spread = deltas.is_none();
    if spread {
        body = body.child(spacer().grow(1.0));
    }
    body = body.child(stats("month-cache", "CACHE".to_string(), cache));
    if let Some((name, cells)) = deltas {
        // A true minus sign for a fall.
        let cells = cells.into_iter().map(|(v, what)| (v, what.replace(", -", ", −"))).collect();
        body = body.child(stats("month-deltas", format!("VS {}", name.to_uppercase()), cells));
    }
    body = body.child(spacer().grow(1.0));
    if let Some(line) = next_month_plain(m) {
        body = body.child(
            col()
                .key("month-next")
                .w_full()
                .px(14.0)
                .py(9.0)
                .gap(2.0)
                .radius(12.0)
                .fill(pal::mix_srgb(p.bg, p.accent, 0.12))
                .child(text("FOR NEXT MONTH").font(FACT_HEAD).color(p.accent).single_line())
                .child(text(line).w_full().wrap().max_lines(2).font(NEXT).color(p.ink)),
        );
    }
    // The footnotes: what the month cannot say, and the requests without a price.
    for (k, n) in tokprof_browse::render_month::notes(m).into_iter().chain(note).enumerate() {
        body = body.child(text(n).key(format!("month-note-{k}")).font(RULE).color(p.ink3).single_line());
    }
    body
}

/// The year's card, `CARD` points, in the month card's look: the year and its title, the big
/// numbers, the twelve months as bars of spend beside the facts, the cache, the change from the
/// year before, and the line for next year. A card that is not ready is dimmed under a band that
/// says when it will be.
pub fn year_card_el(y: &Year, p: &Pal, rounded: bool) -> El {
    let head = row()
        .w_full()
        .center_y()
        .child(text("TOKPROF").font(BRAND).color(p.accent).single_line())
        .child(spacer().grow(1.0))
        .child(text("from local agent logs · nothing uploaded").font(RULE).color(p.ink3).single_line());
    let sub = if y.sums.complete { "with agents" } else { "with agents, so far" };
    let title = row()
        .gap(10.0)
        .items_end()
        .child(text(format!("{}", y.year)).key("year-headline").font(HEADLINE).color(p.ink).single_line())
        .child(text(sub).font(SUB).color(p.ink2).single_line().pb(4.0));
    let radius = if rounded { R_CARD } else { 0.0 };
    let frame = el().key("year-card").w(CARD.0).h(CARD.1).radius(radius).fill(p.bg).clip_rounded(radius);
    let mut body = col().w(CARD.0).h(CARD.1).padding(PAD).gap(10.0).child(head).child(title);
    if !y.any() {
        return frame.child(body.child(text("No requests and no messages in this year.").font(SUB).color(p.ink2).single_line()));
    }
    let m = &y.sums.span;
    if let Some(t) = &y.title {
        body = body.child(
            row()
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    row()
                        .key("year-title")
                        .h(26.0)
                        .px(12.0)
                        .center_y()
                        .radius(PILL)
                        .fill(p.accent)
                        .child(text(t.name.to_uppercase()).font(BADGE).color(p.on_accent).single_line()),
                )
                .child(text(t.rule.clone()).key("year-rule").grow(1.0).min_w(0.0).wrap().max_lines(2).font(RULE).color(p.ink2)),
        );
    }
    let third = (CARD.0 - 2.0 * PAD - 20.0) / 3.0;
    let big = big_numbers(m);
    let big_cell = |(v, what): &(String, &'static str)| {
        col().w(third).gap(0.0).child(text(v.clone()).font(BIG).color(p.ink).single_line()).child(text(*what).font(LABEL).color(p.ink2).single_line())
    };
    body = body.child(
        col()
            .key("year-big")
            .w_full()
            .gap(8.0)
            .child(row().w_full().gap(10.0).children(big[..3].iter().map(big_cell)))
            .child(row().w_full().gap(10.0).children(big[3..].iter().map(big_cell))),
    );
    // The twelve months as bars of spend; a month without logs, a hairline.
    let (bw, gap, bar_h) = (12.0, 5.0, 84.0);
    let rows = y.months();
    let most = rows.iter().filter_map(|r| r.month).map(|x| x.total.usd).fold(0.0, f64::max);
    let mut strip = row().key("year-months").gap(gap).items_end();
    for r in &rows {
        let v = r.month.map_or(0.0, |x| x.total.usd);
        let h = if most > 0.0 && v > 0.0 { ((v / most) as f32 * bar_h).max(2.0) } else { 0.0 };
        let bar = if r.month.is_some() && h > 0.0 {
            el().w(bw).h(h).corners([3.0, 3.0, 0.0, 0.0]).fill(p.accent)
        } else {
            el().w(bw).h(1.0).fill(if r.kind == MonthKind::NotBegun { Color::TRANSPARENT } else { p.row_line })
        };
        let letter = r.ym.month_name()[..1].to_string();
        strip = strip.child(
            col().w(bw).gap(4.0).items_center().child(col().h(bar_h).justify_end().child(bar)).child(text(letter).font(FACT_HEAD).color(p.ink3).single_line()),
        );
    }
    let strip = col().gap(4.0).child(strip).child(text("spend per month").font(RULE).color(p.ink3).single_line());
    let mut side = col().key("year-facts").grow(1.0).min_w(0.0).gap(8.0).pt(6.0);
    for (label, value) in year_facts(y) {
        side = side.child(
            row()
                .w_full()
                .gap(8.0)
                .items_start()
                .child(row().w(104.0).pt(1.5).child(text(label.to_uppercase()).font(FACT_HEAD).color(p.ink3).single_line()))
                .child(text(value).grow(1.0).min_w(0.0).single_line().truncate().font(FACT).color(p.ink)),
        );
    }
    body = body.child(row().w_full().gap(22.0).items_start().child(strip).child(side));
    let quarter = |n: usize| (CARD.0 - 2.0 * PAD - 12.0 * (n as f32 - 1.0)) / n as f32;
    let stats = |key: &'static str, head: String, cells: Vec<(String, String)>| {
        let w = quarter(cells.len().max(1));
        col().key(key).w_full().gap(5.0).child(text(head).font(FACT_HEAD).color(p.ink3).single_line()).child(row().w_full().gap(12.0).children(
            cells.into_iter().map(move |(v, what)| {
                col().w(w).gap(0.0).child(text(v).font(STAT).color(p.ink).single_line()).child(text(what).font(RULE).color(p.ink2).w_full().wrap().max_lines(2))
            }),
        ))
    };
    let (cache_cells, unpriced) = cache(m);
    let short = |what: &'static str| -> &'static str {
        match what {
            "of input read from cache" => "read from cache",
            "avoidable on misses" => "lost to misses",
            w => w,
        }
    };
    let cache_cells: Vec<(String, String)> =
        cache_cells.into_iter().map(|(v, what)| (v.split(' ').next().unwrap_or("").to_string(), short(what).to_string())).collect();
    let deltas = year_deltas(y);
    if deltas.is_none() {
        body = body.child(spacer().grow(1.0));
    }
    body = body.child(stats("year-cache", "CACHE".to_string(), cache_cells));
    if let Some((name, cells)) = deltas {
        body = body.child(stats("year-deltas", format!("VS {}", name.to_uppercase()), cells));
    }
    body = body.child(spacer().grow(1.0));
    if let Some(line) = next_month_plain(m) {
        // `… cost $1,120. Compact …` reads `… cost $1,120 this year. Compact …`.
        let line = match line.split_once(". ") {
            Some((a, b)) => format!("{a} this year. {b}"),
            None => line,
        };
        body = body.child(
            col()
                .key("year-next")
                .w_full()
                .px(14.0)
                .py(9.0)
                .gap(2.0)
                .radius(12.0)
                .fill(pal::mix_srgb(p.bg, p.accent, 0.12))
                .child(text("FOR NEXT YEAR").font(FACT_HEAD).color(p.accent).single_line())
                .child(text(line).w_full().wrap().max_lines(2).font(NEXT).color(p.ink)),
        );
    }
    let mut notes: Vec<String> = y.ready.since_text().into_iter().collect();
    notes.extend(tokprof_browse::render_year::notes(y));
    notes.extend(unpriced);
    if !notes.is_empty() {
        body = body.child(text(notes.join(" · ")).key("year-notes").w_full().wrap().max_lines(2).font(RULE).color(p.ink3));
    }
    body = body.child(
        row()
            .key("year-brand")
            .w_full()
            .center_y()
            .child(text(format!("tokprof year {}", y.year)).font(RULE).color(p.ink2).single_line())
            .child(spacer().grow(1.0))
            .child(text("API-equivalent dollars").font(RULE).color(p.ink3).single_line()),
    );
    if y.ready.ready {
        return frame.child(body);
    }
    // Not ready: the numbers so far, dimmed, under the day it gets ready.
    let band = col()
        .key("year-lockband")
        .w(CARD.0 - 2.0 * PAD)
        .px(18.0)
        .py(14.0)
        .gap(4.0)
        .radius(14.0)
        .fill(p.panel)
        .border(1.0, p.row_line)
        .shadow(12.0, 30.0, Color::BLACK.alpha(0.16))
        .child(text(y.ready.ready_on_text().map_or("No logs".to_string(), |d| format!("Ready on {d}"))).font(SUB).color(p.ink).single_line())
        .child(
            text(format!("{}. The card opens at the end of the year, or once the year holds 90 days of logs.", y.ready.logs_text(y.year)))
                .w_full()
                .wrap()
                .font(FACT)
                .color(p.ink2),
        );
    frame.child(body.dim(0.4)).child(band.absolute(PAD, CARD.1 * 0.42))
}

/// The facts of the year's card, short enough for a line each: the biggest day and week, the
/// longest streak, the most at once, the busiest hour and the night share.
pub fn year_facts(y: &Year) -> Vec<(&'static str, String)> {
    let r = &y.sums.records;
    let day = |d: i64| tokprof_browse::render_calendar::day_name(d);
    let mut out = Vec::new();
    if let Some((d, v)) = r.biggest_day {
        out.push(("biggest day", format!("{}, {}", day(d), tokprof_core::fmt::usd(v))));
    }
    if let Some(w) = y.biggest_week() {
        out.push(("biggest week", format!("{}, {}", tokprof_core::week::dates(w.first, w.last, false), tokprof_core::fmt::usd(w.amounts.usd))));
    }
    if let Some((streak, going)) = r.streak {
        let days = tokprof_core::fmt::count(streak.days() as u64, "day", "days");
        out.push(("longest streak", if going { format!("{days}, still going") } else { format!("{days}, to {}", day(streak.to)) }));
    }
    if let Some((n, ts)) = r.most_at_once {
        out.push((
            "most at once",
            format!("{}, {}", tokprof_core::fmt::count(u64::from(n), "session", "sessions"), tokprof_core::fmt::local_time(ts, "%-d %b %H:%M")),
        ));
    }
    let m = &y.sums.span;
    if let Some((h, share)) = m.busiest_hour {
        out.push(("busiest hour", format!("{h:02}:00–{:02}:00, {:.0}%", (h + 1) % 24, 100.0 * share)));
    }
    if let Some(n) = m.night_share {
        out.push(("night share", format!("{:.0}% of agent time", 100.0 * n)));
    }
    out
}

/// The changes from the year before, as the month card's cells give them; none without a
/// comparison.
fn year_deltas(y: &Year) -> Option<(String, Vec<(String, String)>)> {
    let Before::Against { numbers, .. } = &y.sums.before else { return None };
    let k = &y.sums.key;
    let arrow = |d: f64| {
        if d > 0.0 {
            "↑"
        } else if d < 0.0 {
            "↓"
        } else {
            "="
        }
    };
    let rel = |d: f64, base: f64| if base > 0.0 { format!(", {}{:.0}%", if d < 0.0 { "−" } else { "+" }, (100.0 * d / base).abs()) } else { String::new() };
    let mut out = Vec::new();
    let d = k.usd - numbers.usd;
    out.push((format!("{} {}", arrow(d), tokprof_core::fmt::usd(d.abs())), format!("spend{}", rel(d, numbers.usd))));
    let d = k.agent_minutes as f64 - numbers.agent_minutes as f64;
    out.push((format!("{} {}", arrow(d), tokprof_browse::render_month::hours(d.abs())), format!("agent time{}", rel(d, numbers.agent_minutes as f64))));
    if let (Some(now), Some(was)) = (k.avoidable_share(), numbers.avoidable_share()) {
        let d = 100.0 * (now - was);
        out.push((format!("{} {:.1} pt", arrow(d), d.abs()), format!("avoidable, {:.1}% from {:.1}%", 100.0 * now, 100.0 * was)));
    }
    Some((format!("{}", y.year - 1), out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_image_never_overwrites_another() {
        let dir = std::env::temp_dir().join(format!("tokprof-month-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tokprof-2026-10.png");
        assert_eq!(free_path(&path), path);
        std::fs::write(&path, b"x").unwrap();
        assert_eq!(free_path(&path), dir.join("tokprof-2026-10-2.png"));
        std::fs::write(dir.join("tokprof-2026-10-2.png"), b"x").unwrap();
        assert_eq!(free_path(&path), dir.join("tokprof-2026-10-3.png"));
    }
}
