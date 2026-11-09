//! The month in review: a card of one month, made to be posted, over the Calendar view, and the
//! same card alone as a 1080×1350 image. It names no project, title or path. "Save image" draws
//! it in a frame without a window, by this program run again, and writes it to `~/Downloads`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rune::{Color, El, Key as RuneKey, KeyEvent, TextStyle, Tx, col, el, row, spacer, text};
use tokprof_browse::render_month::{big_numbers, cache, delta_cells, facts, headline, next_month, weeks};
use tokprof_core::calendar::level;
use tokprof_core::month::{Month, Ym};

use crate::calendar::level_color;
use crate::overlay::{PILL, R_CARD};
use crate::pal::{self, Pal};
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
}

/// The month of the cell cursor, or of the last day.
fn cursor_month(s: &Desk) -> Option<Ym> {
    crate::calendar::cursor_date(s).map(Ym::of_date)
}

/// Opens the card of `ym`, or of the cursor's month, computed from the calendar's store.
pub fn open(s: &mut Desk, ym: Option<Ym>) {
    let Some(ym) = ym.or_else(|| cursor_month(s)) else { return };
    let Some(store) = s.calendar.data.get().and_then(|c| c.store.clone()) else { return };
    let month = tokprof_browse::calendar::month(&store, &s.calendar.select, ym, tokprof_browse::browser::now_ms());
    s.month.card = Some(Card { month, status: None });
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
            if let Some(ym) = ym {
                let to = if e.key == RuneKey::Left { ym.previous() } else { ym.next() };
                let this = Ym::of_date(tokprof_core::rhythm::date_of(tokprof_browse::browser::now_ms(), &tokprof_core::rhythm::local_offset));
                if to <= this {
                    open(s, Some(to));
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
    std::thread::spawn(move || {
        let out = std::process::Command::new(exe).args(&args).stdin(std::process::Stdio::null()).output();
        let result = match out {
            Ok(o) if o.status.success() && path.exists() => Ok(path),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("the image was not written").to_string()),
            Err(e) => Err(e.to_string()),
        };
        *saved.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        crate::wake();
    });
}

/// Takes in a finished save. Returns whether the card changed.
pub fn poll(s: &mut Desk) -> bool {
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
    let mut body = col().key("month-card").w(CARD.0).h(CARD.1).padding(PAD).gap(13.0).radius(radius).fill(p.bg).clip_rounded(radius).child(head).child(title);
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
    let cell = |(v, what): &(String, &'static str)| {
        col().w(third).gap(0.0).child(text(v.clone()).font(BIG).color(p.ink).single_line()).child(text(*what).font(LABEL).color(p.ink2).single_line())
    };
    let grid = col()
        .key("month-big")
        .w_full()
        .gap(8.0)
        .child(row().w_full().gap(10.0).children(big[..3].iter().map(cell)))
        .child(row().w_full().gap(10.0).children(big[3..].iter().map(cell)));
    body = body.child(grid);
    // The days of the month beside the facts.
    let (sq, gap) = (20.0, 4.0);
    let mut cal = col().key("month-days").gap(gap);
    cal = cal.child(row().gap(gap).children(
        ["M", "T", "W", "T", "F", "S", "S"].iter().map(|d| row().w(sq).justify_center().child(text(*d).font(FACT_HEAD).color(p.ink3).single_line())),
    ));
    let busiest = m.busiest_day.map(|b| b.0);
    for week in weeks(m) {
        cal = cal.child(row().gap(gap).children(week.iter().map(|cell| match cell {
            Some(i) => {
                let d = &m.days[*i];
                let sq_el = el().w(sq).h(sq).radius(4.0).fill(level_color(level(d.amounts.usd, m.cuts), p));
                if busiest == Some(d.date) { sq_el.border(1.5, p.ink) } else { sq_el }
            }
            None => el().w(sq).h(sq),
        })));
    }
    let fact_line = |label: &str, value: String| {
        col()
            .gap(1.0)
            .child(text(label.to_uppercase()).font(FACT_HEAD).color(p.ink3).single_line())
            .child(text(value).w_full().wrap().max_lines(2).font(FACT).color(p.ink))
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
    let head = match note {
        Some(n) => format!("CACHE · {n}"),
        None => "CACHE".to_string(),
    };
    body = body.child(stats("month-cache", head, cache));
    if let Some((name, cells)) = delta_cells(m) {
        body = body.child(stats("month-deltas", format!("VS {}", name.to_uppercase()), cells));
    }
    body = body.child(spacer().grow(1.0));
    if let Some(line) = next_month(m) {
        body = body.child(
            col()
                .key("month-next")
                .w_full()
                .px(14.0)
                .py(11.0)
                .gap(3.0)
                .radius(12.0)
                .fill(pal::mix_srgb(p.bg, p.accent, 0.12))
                .child(text("FOR NEXT MONTH").font(FACT_HEAD).color(p.accent).single_line())
                .child(text(line).w_full().wrap().max_lines(2).font(NEXT).color(p.ink)),
        );
    }
    body
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
