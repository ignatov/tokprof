//! Interactive session browser in the terminal, `tokprof ui`: the browser of `tokprof-browse`
//! drawn with ratatui, the sessions on the left and the X-ray of the selected one on the right.
//! The browser holds the sessions, their costs, X-rays and days; this draws them and passes the
//! terminal's keys on.

use std::sync::Arc;
use std::time::Duration;

use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};

use tokprof_browse::browser::{Browser, Frontend, Key, Pane, Viewport, now_ms};
use tokprof_core::catalog::Catalog;
use tokprof_core::pricing::Pricing;
use tokprof_core::query::Query;
use tokprof_core::rhythm;

const LIST_WIDTH: u16 = 74;

/// The colors of the agent column, by the agent's place in the registry.
const AGENT_COLORS: [Color; 4] = [Color::Magenta, Color::Green, Color::Cyan, Color::Yellow];

type App = Browser<Text<'static>>;

/// How the terminal shows the browser's texts: their color codes as ratatui styles.
fn terminal() -> Frontend<Text<'static>> {
    Frontend { show, lines, day_layout }
}

fn show(text: String) -> Text<'static> {
    text.into_text().unwrap_or_else(|_| Text::raw(text))
}

fn lines(text: &Text<'static>) -> usize {
    text.lines.len()
}

pub fn run(catalog: Arc<Catalog>, pricing: Pricing, dump: Option<(u16, u16)>, select: Query) -> Result<()> {
    // The terminal draws at least every 100 ms, so the background work needs not wake it.
    let mut app = Browser::start(catalog, pricing, select, dump.is_none(), Arc::new(|| {}), terminal())?;
    if let Some((width, height)) = dump {
        return dump_frame(&mut app, width, height);
    }
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        app.tick();
        terminal.draw(|f| draw(f, app))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && !on_key(app, key, terminal.size()?.height)
        {
            return Ok(());
        }
    }
}

/// Renders one frame without a terminal, for checking the layout.
fn dump_frame(app: &mut App, width: u16, height: u16) -> Result<()> {
    app.settle();
    let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, height))?;
    terminal.draw(|f| draw(f, app))?;
    let buffer = terminal.backend().buffer();
    for y in 0..height {
        let line: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
}

/// The browser's key for a key of the terminal, when it takes one.
fn key_of(key: KeyEvent) -> Option<Key> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Key::Interrupt);
    }
    Some(match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Backspace => Key::Backspace,
        _ => return None,
    })
}

/// Handles a key on a terminal `height` rows high; returns false to quit.
fn on_key(app: &mut App, key: KeyEvent, height: u16) -> bool {
    let Some(key) = key_of(key) else { return true };
    // A page of the list is the terminal less the footer, the borders and a row; the right pane
    // is the terminal less the footer and the borders.
    let view = Viewport { page: height.saturating_sub(4).max(1) as usize, lines: height.saturating_sub(3) as usize };
    app.on_key(key, view)
}

fn draw(frame: &mut Frame, app: &mut App) {
    let [body, footer] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
    let report = if app.zoomed() {
        body
    } else {
        let [left, right] = Layout::horizontal([Constraint::Length(LIST_WIDTH), Constraint::Fill(1)]).areas(body);
        draw_list(frame, app, left);
        right
    };
    app.set_pane((report.width, report.height));
    if app.showing_day() {
        draw_day(frame, app, report);
    } else {
        draw_report(frame, app, report);
    }
    draw_footer(frame, app, footer);
}

/// The day's layout in a pane of this size: labels a fifth of the width, from 20 to 40 characters;
/// the strip in the rest, less a space and the borders; and the lanes in the rows the text above
/// and below the strip leaves.
fn day_layout((width, height): (u16, u16)) -> rhythm::Layout {
    let label = (width as usize / 5).clamp(20, 40);
    rhythm::Layout {
        label,
        columns: (width as i64 - label as i64 - 3).max(12),
        rows: (height as usize).saturating_sub(DAY_TEXT_ROWS - 1).max(rhythm::STRIP_LANES + 1),
        space: true,
    }
}

/// The rows of the screen that the day view's text takes besides its lanes: the borders, the lines
/// above and below the strip, and its hours, `you` and legend rows, with the footer.
const DAY_TEXT_ROWS: usize = 18;

/// A pane's text as the terminal draws it; what is still being computed is dim.
fn pane_text(pane: Pane<'_, Text<'static>>) -> Text<'static> {
    match pane {
        Pane::Text(text) => text.clone(),
        Pane::Computing(note) => Text::raw(note).dim(),
        Pane::Empty(note) => Text::raw(note),
    }
}

/// The first line of the right pane on the screen.
fn scroll(app: &App) -> u16 {
    app.scroll().min(u16::MAX as usize) as u16
}

fn draw_day(frame: &mut Frame, app: &App, area: Rect) {
    let text = pane_text(app.day_pane());
    frame.render_widget(Paragraph::new(text).block(Block::bordered().title(app.day_title())).scroll((scroll(app), 0)), area);
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect) {
    // date 11, agent 6, cost 9, next 12, gaps 4, highlight 2, borders 2
    let project_width = area.width.saturating_sub(46) as usize;
    let now = now_ms();
    let items: Vec<ListItem> = (0..app.len())
        .map(|row| {
            let c = app.cells(row, now);
            let agent_color = AGENT_COLORS[c.agent_place % AGENT_COLORS.len()];
            let next = match c.next {
                Some((cell, cold)) => Span::styled(format!("{cell:>12}"), Style::new().fg(if cold { Color::Red } else { Color::Green })),
                None => Span::raw(" ".repeat(12)),
            };
            ListItem::new(Line::from(vec![
                Span::raw(c.time).dim(),
                Span::raw(" "),
                Span::styled(format!("{:<6}", c.agent), Style::new().fg(agent_color)),
                Span::raw(" "),
                Span::raw(format!("{:<width$}", clip_left(c.project, project_width), width = project_width)),
                Span::raw(" "),
                Span::raw(format!("{:>9}", c.cost)).bold(),
                next,
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(Block::bordered().title(app.list_title(now)))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▸ ");
    // The list scrolls to keep the selection on the screen; the browser keeps the rows on it in
    // place from there.
    let mut state = ListState::default().with_selected(app.selected()).with_offset(app.top());
    frame.render_stateful_widget(list, area, &mut state);
    app.set_top(state.offset());
}

fn draw_report(frame: &mut Frame, app: &App, area: Rect) {
    let text = pane_text(app.report());
    frame.render_widget(Paragraph::new(text).block(Block::bordered().title(app.report_title())).scroll((scroll(app), 0)), area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = if app.editing() {
        Line::from(vec![Span::raw(" filter: ").dim(), Span::raw(format!("{}▌", app.filter())), Span::raw("   ⏎ done · esc clear").dim()])
    } else {
        let mut spans = vec![Span::raw(app.footer_keys()).dim()];
        if !app.filter().is_empty() {
            spans.push(Span::raw(format!("   filter: {}", app.filter())));
        }
        if !app.status().is_empty() {
            spans.push(Span::raw(format!("   {}", app.status())).fg(Color::Yellow));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn clip_left(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n + 1 - width).collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use tokprof_core::source::{AgentId, Locator, SessionRef};
    use tokprof_core::xray::SessionRow;

    /// Session `i`, last active `at` seconds after the epoch.
    fn row(i: usize, at: u64) -> SessionRow {
        SessionRow {
            file: SessionRef {
                source: "test",
                agent: AgentId::unknown("claude"),
                locator: Locator::File(format!("/x/{i}.jsonl").into()),
                session_id: format!("id{i}"),
                main_id: format!("id{i}"),
                subagent: false,
                size: 1 + at,
                modified_ns: at as i128 * 1_000_000_000,
                created_ms: None,
            },
            subagents: Vec::new(),
            project: "~/src/a".to_string(),
            id: format!("id{i}"),
            session_id: format!("id{i}"),
            bytes: 1 + at,
            active_ms: at as i64 * 1000,
            client: None,
        }
    }

    /// `n` sessions, listed 0, 1, 2…
    fn app(n: usize) -> App {
        let rows = (0..n).map(|i| row(i, 1000 - i as u64)).collect();
        Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), terminal())
    }

    /// The lines of the ui drawn 160 columns by 20 rows.
    fn screen(app: &mut App) -> Vec<String> {
        let mut t = Terminal::new(ratatui::backend::TestBackend::new(160, 20)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let b = t.backend().buffer();
        (0..20).map(|y| (0..160).map(|x| b[(x, y)].symbol()).collect::<String>()).collect()
    }

    fn press(app: &mut App, code: KeyCode) -> bool {
        on_key(app, KeyEvent::new(code, KeyModifiers::NONE), 30)
    }

    #[test]
    fn terminal_keys_are_the_browser_s_keys() {
        assert_eq!(key_of(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), Some(Key::Interrupt));
        assert_eq!(key_of(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)), Some(Key::Char('c')));
        let keys = [
            (KeyCode::Up, Key::Up),
            (KeyCode::Down, Key::Down),
            (KeyCode::Left, Key::Left),
            (KeyCode::Right, Key::Right),
            (KeyCode::PageUp, Key::PageUp),
            (KeyCode::PageDown, Key::PageDown),
            (KeyCode::Home, Key::Home),
            (KeyCode::End, Key::End),
            (KeyCode::Enter, Key::Enter),
            (KeyCode::Esc, Key::Esc),
            (KeyCode::Backspace, Key::Backspace),
        ];
        for (code, key) in keys {
            assert_eq!(key_of(KeyEvent::new(code, KeyModifiers::NONE)), Some(key), "{code:?}");
        }
        assert_eq!(key_of(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)), None);
    }

    #[test]
    fn the_list_title_tops_the_screen_and_the_keys_end_it() {
        let mut a = app(30);
        let lines = screen(&mut a);
        assert!(lines[0].starts_with("┌ 30 sessions · ") && lines[0].contains(" · by time · costing 30…"), "{}", lines[0]);
        assert!(lines[1].starts_with("│▸ "), "the newest session is selected: {}", lines[1]);
        assert!(lines[19].contains("s sort: time, cost, next · r re-sort") && lines[19].contains(" · q quit · d day"), "{}", lines[19]);
        assert_eq!(a.pane(), (160 - LIST_WIDTH, 19), "the X-ray pane is right of the list, above the footer");
    }

    #[test]
    fn the_list_keeps_the_selection_on_the_screen() {
        let mut a = app(30);
        screen(&mut a);
        for _ in 0..25 {
            press(&mut a, KeyCode::Down);
        }
        let lines = screen(&mut a);
        assert!(lines.iter().any(|l| l.starts_with("│▸ ")), "the selected row is on the screen");
        // 19 rows above the footer, less the two borders.
        assert_eq!(a.top(), 25 - 16, "the list scrolled to show it in the last of its 17 rows");
    }

    #[test]
    fn a_filter_shows_in_the_footer() {
        let mut a = app(3);
        for c in "/id1".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        assert!(screen(&mut a)[19].starts_with(" filter: id1▌   ⏎ done · esc clear"), "while it is typed");
        press(&mut a, KeyCode::Enter);
        assert!(screen(&mut a)[19].contains("   filter: id1"), "and after");
    }

    #[test]
    fn the_day_takes_the_whole_screen() {
        let mut a = app(1);
        press(&mut a, KeyCode::Char('d'));
        let lines = screen(&mut a);
        assert!(lines[0].starts_with("┌ day · "), "{}", lines[0]);
        assert!(lines[1].contains("computing the day of "), "{}", lines[1]);
        assert!(lines[19].starts_with(" ↑↓ PgUp PgDn scroll · ← → days · esc back"), "{}", lines[19]);
        assert_eq!(a.pane(), (160, 19), "the day is laid out for the whole screen above the footer");
        assert!(press(&mut a, KeyCode::Esc), "esc goes back to the list");
        assert!(!a.showing_day());
        assert!(!press(&mut a, KeyCode::Char('q')));
    }

    #[test]
    fn the_day_fits_the_screen() {
        // 190x50: the pane is 190x49 above the footer.
        assert_eq!(day_layout((190, 49)), rhythm::Layout { label: 38, columns: 149, rows: 32, space: true });
        assert_eq!(day_layout((120, 29)), rhythm::Layout { label: 24, columns: 93, rows: 12, space: true });
        assert_eq!(day_layout((80, 20)).label, 20, "at least 20 characters");
        assert_eq!(day_layout((300, 60)).label, 40, "at most 40");
        assert_eq!(day_layout((80, 10)).rows, 9, "at least the 8 lanes and the more line");
    }
}
