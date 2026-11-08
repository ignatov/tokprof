//! The window's settings: the appearance and the light and dark themes, kept in the app's
//! settings file, and the panel over the window that picks them.

use std::time::Instant;

use rune::prefs::{Prefs, PrefsError};
use rune::{Color, El, Key as RuneKey, KeyEvent, ScrollSpec, Tx, col, el, row, spacer, text};

use crate::overlay::{self, PILL, R_CARD, R_GROUP, R_TILE};
use crate::pal::{self, Pal};
use crate::themes::{self, Theme};
use crate::{Desk, plain};

/// The app's id, as the bundle names it: its settings file is in a folder of this name.
pub const APP_ID: &str = "io.github.ignatov.tokprof";

/// Whether the window is light or dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Appearance {
    /// As the system is, also when it changes.
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    /// The name in the settings file.
    pub fn name(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn named(name: &str) -> Option<Appearance> {
        Appearance::ALL.into_iter().find(|a| a.name() == name)
    }

    /// The name in the panel.
    pub fn label(self) -> &'static str {
        match self {
            Appearance::System => "As in system",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }

    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Appearance::System => system_dark,
            Appearance::Light => false,
            Appearance::Dark => true,
        }
    }
}

/// What the settings hold.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Settings {
    pub appearance: Appearance,
    /// The theme ids for a light and for a dark window.
    pub light_theme: String,
    pub dark_theme: String,
    /// The cache countdown in the macOS menu bar, off by default.
    pub menu_bar: bool,
    /// The waiting order of the list, off by default.
    pub waiting: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            appearance: Appearance::System,
            light_theme: themes::DEFAULT_LIGHT.into(),
            dark_theme: themes::DEFAULT_DARK.into(),
            menu_bar: false,
            waiting: false,
        }
    }
}

impl Settings {
    /// The settings in a store: a value it does not know is that setting's default.
    pub fn read(prefs: &Prefs) -> Settings {
        let theme = |key: &str, dark: bool| prefs.get::<String>(key).filter(|id| themes::find(id).is_some_and(|t| t.dark() == dark));
        let d = Settings::default();
        Settings {
            appearance: prefs.get::<String>("appearance").and_then(|n| Appearance::named(&n)).unwrap_or(d.appearance),
            light_theme: theme("light_theme", false).unwrap_or(d.light_theme),
            dark_theme: theme("dark_theme", true).unwrap_or(d.dark_theme),
            menu_bar: prefs.get::<String>("menu_bar").as_deref() == Some("on"),
            waiting: prefs.get::<String>("waiting").as_deref() == Some("on"),
        }
    }

    /// Settings that draw in one theme, whatever the system is: those of `--colors`.
    pub fn of_theme(t: &Theme) -> Settings {
        let mut s = Settings { appearance: if t.dark() { Appearance::Dark } else { Appearance::Light }, ..Settings::default() };
        *s.theme_id_mut(t.dark()) = t.id.clone();
        s
    }

    /// The theme the window draws in.
    pub fn theme(&self, system_dark: bool) -> &'static Theme {
        let dark = self.appearance.dark(system_dark);
        themes::pick(self.theme_id(dark), dark)
    }

    pub fn theme_id(&self, dark: bool) -> &str {
        if dark { &self.dark_theme } else { &self.light_theme }
    }

    fn theme_id_mut(&mut self, dark: bool) -> &mut String {
        if dark { &mut self.dark_theme } else { &mut self.light_theme }
    }
}

/// Writes a setting when the store holds another value; whether it wrote.
fn store(prefs: &mut Prefs, key: &str, value: &str) -> Result<bool, PrefsError> {
    if prefs.get::<String>(key).as_deref() == Some(value) {
        return Ok(false);
    }
    prefs.set(key, value).map(|_| true)
}

/// The parts of the panel that take the keyboard, in the order Tab moves through them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Part {
    #[default]
    Appearance,
    Light,
    Dark,
    MenuBar,
    Waiting,
}

impl Part {
    /// The parts in the order the panel draws them and Tab moves through them: the appearance,
    /// then the grid of the appearance the window was in when the panel opened, then the other,
    /// then the menu bar countdown and the waiting order last.
    pub fn order(dark_first: bool) -> [Part; 5] {
        let (first, second) = if dark_first { (Part::Dark, Part::Light) } else { (Part::Light, Part::Dark) };
        [Part::Appearance, first, second, Part::MenuBar, Part::Waiting]
    }

    fn step(self, back: bool, dark_first: bool) -> Part {
        let order = Part::order(dark_first);
        let n = order.len();
        let at = order.iter().position(|&p| p == self).unwrap_or(0);
        order[if back { (at + n - 1) % n } else { (at + 1) % n }]
    }
}

/// The open settings panel: the part with the keyboard, and whether the dark grid comes first.
/// Until a key or a click fixes them, they follow the window's appearance: the keyboard starts in
/// its grid, which comes right after the appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Panel {
    pub part: Option<Part>,
    pub dark_first: Option<bool>,
}

impl Panel {
    /// The part with the keyboard and whether the dark grid comes first, in a window drawn dark
    /// or not.
    pub fn resolve(self, dark: bool) -> (Part, bool) {
        let first = self.dark_first.unwrap_or(dark);
        (self.part.unwrap_or(if first { Part::Dark } else { Part::Light }), first)
    }

    /// The panel with both fixed as they are now.
    fn fixed(self, dark: bool) -> Panel {
        let (part, first) = self.resolve(dark);
        Panel { part: Some(part), dark_first: Some(first) }
    }
}

impl Desk {
    /// Opens the settings panel, or closes it; it does not open over another card.
    pub fn toggle_settings(&mut self) {
        // One card at a time.
        if self.palette.is_some() || self.help {
            return;
        }
        self.panel = match self.panel {
            Some(_) => None,
            None => Some(Panel::default()),
        };
    }

    pub fn pick_appearance(&mut self, a: Appearance) {
        // The grids keep their order while the appearance changes under them.
        self.panel = self.panel.map(|p| p.fixed(self.theme.get().dark()));
        self.settings.appearance = a;
        self.save("appearance", a.name());
    }

    /// Picks the theme for a dark or a light window.
    pub fn pick_theme(&mut self, dark: bool, id: &str) {
        *self.settings.theme_id_mut(dark) = id.to_string();
        self.save(if dark { "dark_theme" } else { "light_theme" }, id);
    }

    /// Turns the menu bar countdown on or off; the next poll puts the item in place or takes it.
    pub fn pick_menu_bar(&mut self, on: bool) {
        self.settings.menu_bar = on;
        self.save("menu_bar", if on { "on" } else { "off" });
    }

    /// Turns the waiting order on or off; off while the list is sorted by waiting sorts it by
    /// time.
    pub fn pick_waiting(&mut self, on: bool) {
        self.settings.waiting = on;
        self.browser.set_waiting(on);
        self.save("waiting", if on { "on" } else { "off" });
    }

    /// Saves a setting; the bar says so when the file cannot be written.
    fn save(&mut self, key: &str, value: &str) {
        if let Err(e) = store(&mut self.prefs, key, value) {
            self.status = (format!("can't save settings: {e}"), Instant::now());
        }
    }

    /// The keys of the open panel: every key stays with it.
    pub fn settings_key(&mut self, e: &KeyEvent, tx: &mut Tx) -> bool {
        let Some(panel) = self.panel else { return false };
        let panel = panel.fixed(self.theme.get().dark());
        self.panel = Some(panel);
        let (part, first) = panel.resolve(false);
        let text = match &e.key {
            RuneKey::Text(t) if plain(e) => Some(t.as_str()),
            _ => None,
        };
        match (&e.key, text) {
            (RuneKey::Escape | RuneKey::Enter, _) => self.panel = None,
            (RuneKey::Tab, _) => {
                let part = part.step(e.mods.shift, first);
                self.panel = Some(Panel { part: Some(part), ..panel });
                tx.scroll_into_view(part.key());
            }
            _ if part == Part::Appearance => {
                let at = Appearance::ALL.iter().position(|&a| a == self.settings.appearance).unwrap_or(0);
                let to = match (&e.key, text) {
                    (RuneKey::Left, _) | (_, Some("h")) => at.saturating_sub(1),
                    (RuneKey::Right, _) | (_, Some("l")) => (at + 1).min(Appearance::ALL.len() - 1),
                    _ => return true,
                };
                self.pick_appearance(Appearance::ALL[to]);
            }
            _ if part == Part::MenuBar => match (&e.key, text) {
                (RuneKey::Left, _) | (_, Some("h")) => self.pick_menu_bar(false),
                (RuneKey::Right, _) | (_, Some("l")) => self.pick_menu_bar(true),
                _ => {}
            },
            _ if part == Part::Waiting => match (&e.key, text) {
                (RuneKey::Left, _) | (_, Some("h")) => self.pick_waiting(false),
                (RuneKey::Right, _) | (_, Some("l")) => self.pick_waiting(true),
                _ => {}
            },
            _ => {
                let dark = part == Part::Dark;
                let list: Vec<&Theme> = themes::of(dark).collect();
                let at = list.iter().position(|t| t.id == self.settings.theme_id(dark)).unwrap_or(0);
                let Some(to) = grid_step(&e.key, text, at, list.len()) else { return true };
                let id = list[to].id.clone();
                self.pick_theme(dark, &id);
                tx.scroll_into_view(format!("theme-{id}"));
            }
        }
        true
    }
}

/// How many theme cards a row of the grid holds.
pub const COLUMNS: usize = 4;

/// The card a key goes to in a grid of `count` cards from card `at`: the one before or after,
/// above or below (staying put at an edge), or the first or the last.
fn grid_step(key: &RuneKey, text: Option<&str>, at: usize, count: usize) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match (key, text) {
        (RuneKey::Left, _) | (_, Some("h")) => at.saturating_sub(1),
        (RuneKey::Right, _) | (_, Some("l")) => (at + 1).min(last),
        (RuneKey::Up, _) | (_, Some("k")) => at.checked_sub(COLUMNS).unwrap_or(at),
        (RuneKey::Down, _) | (_, Some("j")) => Some(at + COLUMNS).filter(|&i| i <= last).unwrap_or(at),
        (RuneKey::Home, _) => 0,
        (RuneKey::End, _) => last,
        _ => return None,
    })
}

impl Part {
    /// The key of the part's group in the panel.
    fn key(self) -> &'static str {
        match self {
            Part::Appearance => "part-appearance",
            Part::Light => "part-light",
            Part::Dark => "part-dark",
            Part::MenuBar => "part-menu-bar",
            Part::Waiting => "part-waiting",
        }
    }
}

/// The key of a theme's card in the panel.
pub fn row_key(t: &Theme) -> String {
    format!("theme-{}", t.id)
}

/// The card's width and its padding, and the room a group takes around its content.
const CARD_W: f32 = 760.0;
const CARD_PAD: f32 = 24.0;
const GROUP_PAD: f32 = 14.0;
const GAP: f32 = 12.0;

/// The settings panel over the window: a rounded card over the window dimmed and blurred; a
/// click outside the card closes it.
pub fn panel(s: &Desk, panel: Panel, p: &Pal, height: f32) -> El {
    let card_h = (height - 80.0).clamp(320.0, 780.0);
    let shown = s.theme.get();
    let (keyboard, first) = panel.resolve(shown.dark());
    let badge = row()
        .key("settings-shown")
        .h(26.0)
        .px(12.0)
        .gap(6.0)
        .center_y()
        .radius(PILL)
        .fill(p.bar)
        .child(text(if shown.dark() { "☾" } else { "☀" }).font(pal::BODY).color(p.ink2).single_line())
        .child(text(shown.name.clone()).font(pal::SMALL_STRONG).color(p.ink2).single_line());
    let head = row()
        .w_full()
        .px(CARD_PAD)
        .pt(20.0)
        .pb(16.0)
        .center_y()
        .child(text("Settings").key("settings-title").font(pal::TITLE).color(p.ink).single_line())
        .child(spacer().grow(1.0))
        .child(badge);
    let body = col().key("settings-body").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(
        col().w_full().px(CARD_PAD).py(18.0).gap(22.0).children(Part::order(first).map(|part| match part {
            Part::Appearance => section(part, "Appearance", appearance_tiles(s, p), keyboard, p),
            Part::Light => section(part, "Light theme", theme_grid(s, false, p), keyboard, p),
            Part::Dark => section(part, "Dark theme", theme_grid(s, true, p), keyboard, p),
            Part::MenuBar => section(part, "Menu bar countdown", menu_bar_choice(s, p), keyboard, p),
            Part::Waiting => section(part, "Waiting order", waiting_choice(s, p), keyboard, p),
        })),
    );
    let done = row()
        .key("settings-done")
        .h(32.0)
        .px(20.0)
        .center_y()
        .radius(PILL)
        .fill(p.accent)
        .hover(p.accent.alpha(0.88))
        .role("button")
        .label("Done")
        .on_press(|s: &mut Desk| s.panel = None)
        .child(text("Done").font(pal::STRONG).color(p.on_accent).single_line());
    let foot = row()
        .w_full()
        .px(CARD_PAD)
        .py(14.0)
        .gap(14.0)
        .center_y()
        .child(overlay::hint(&["Tab"], "section", p))
        .child(overlay::hint(&["←", "→", "↑", "↓"], "choose", p))
        .child(overlay::hint(&["⏎"], "done", p))
        .child(spacer().grow(1.0))
        .child(done);
    let card = overlay::card("settings", "Settings", CARD_W, R_CARD, p)
        .h(card_h)
        .child(head)
        .child(el().w_full().h(1.0).fill(p.row_line))
        .child(body)
        .child(el().w_full().h(1.0).fill(p.row_line))
        .child(foot);
    // A click outside the card closes the panel; Rune's modal would take Tab itself, so the scrim
    // stands in for it.
    overlay::scrim("settings-scrim", p, |s| s.panel = None).center().child(card)
}

/// A part of the panel: its title over a soft group, ringed in the accent while it has the
/// keyboard.
fn section(part: Part, title: &'static str, content: El, keyboard: Part, p: &Pal) -> El {
    let on = part == keyboard;
    let group = col().key(part.key()).w_full().padding(GROUP_PAD).radius(R_GROUP).fill(p.bar).child(content);
    let group = if on { group.outline(2.0, 2.0, p.accent) } else { group };
    col().w_full().gap(10.0).child(text(title).font(pal::HEAD).color(if on { p.accent } else { p.ink2 }).single_line()).child(group)
}

/// The three appearances as tiles with a small window in each: the system's shows the light
/// theme and the dark one side by side.
fn appearance_tiles(s: &Desk, p: &Pal) -> El {
    let light = themes::pick(s.settings.theme_id(false), false);
    let dark = themes::pick(s.settings.theme_id(true), true);
    let tile_w = (CARD_W - 2.0 * CARD_PAD - 2.0 * GROUP_PAD - 2.0 * GAP) / 3.0;
    let tiles = Appearance::ALL.iter().map(|&a| {
        let chosen = s.settings.appearance == a;
        let picture = match a {
            Appearance::System => row().w_full().h_full().child(window_picture(light).w(tile_w / 2.0)).child(window_picture(dark).w(tile_w / 2.0)),
            Appearance::Light => window_picture(light).w_full(),
            Appearance::Dark => window_picture(dark).w_full(),
        };
        // The picture is clipped to its corners; the ring goes on the box around it, which the
        // clip leaves alone.
        let picture = el().w_full().h_full().radius(R_TILE).clip_rounded(R_TILE).child(picture);
        // The edge over the picture, so that a tile as dark as the group around it still has one.
        let edge = el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(R_TILE).border(1.0, if s.theme.get().dark() { p.line } else { p.row_line });
        let frame = el().w_full().h(74.0).radius(R_TILE).child(picture).child(edge);
        let frame = if chosen { frame.outline(2.0, 2.0, p.accent) } else { frame };
        col()
            .key(format!("appearance-{}", a.name()))
            .w(tile_w)
            .gap(8.0)
            .items_center()
            .role("radio")
            .checked(chosen)
            .label(a.label())
            .on_press(move |s: &mut Desk| s.pick_appearance(a))
            .child(frame)
            .child(
                text(a.label())
                    .key(format!("appearance-label-{}", a.name()))
                    .font(if chosen { pal::STRONG } else { pal::BODY })
                    .color(if chosen { p.accent } else { p.ink })
                    .single_line(),
            )
    });
    row().w_full().gap(GAP).children(tiles)
}

/// `Off` and `On` for the menu bar countdown, with what it does beside them.
fn menu_bar_choice(s: &Desk, p: &Pal) -> El {
    let on = s.settings.menu_bar;
    let choice = crate::view::segments(
        "menu-bar",
        vec![
            crate::view::segment("menu-bar-off".into(), "Off", !on, p).on_press(|s: &mut Desk| s.pick_menu_bar(false)),
            crate::view::segment("menu-bar-on".into(), "On", on, p).on_press(|s: &mut Desk| s.pick_menu_bar(true)),
        ],
        p.panel,
    );
    let about = "How long the warm caches stay warm, of sessions that cost $1.00 or more to resume cold";
    row().w_full().gap(GAP).center_y().child(choice).child(text(about).font(pal::SMALL).color(p.ink2).single_line().truncate().shrink().min_w(0.0))
}

/// `Off` and `On` for the waiting order, with what it does beside them.
fn waiting_choice(s: &Desk, p: &Pal) -> El {
    let on = s.settings.waiting;
    let choice = crate::view::segments(
        "waiting",
        vec![
            crate::view::segment("waiting-off".into(), "Off", !on, p).on_press(|s: &mut Desk| s.pick_waiting(false)),
            crate::view::segment("waiting-on".into(), "On", on, p).on_press(|s: &mut Desk| s.pick_waiting(true)),
        ],
        p.panel,
    );
    let about = "A Waiting order in the list: the sessions that wait for you, the longest wait first";
    row().w_full().gap(GAP).center_y().child(choice).child(text(about).font(pal::SMALL).color(p.ink2).single_line().truncate().shrink().min_w(0.0))
}

/// A window in a theme, drawn small: its bar, a few lines of text and an accent.
fn window_picture(t: &Theme) -> El {
    let line = |w: f32, c: Color| el().w(w).h(5.0).radius(2.5).fill(c);
    col().h_full().fill(t.color("canvas")).child(el().w_full().h(14.0).fill(t.color("surfaceSubtle"))).child(
        col()
            .padding(9.0)
            .gap(6.0)
            .child(line(46.0, t.color("content").alpha(0.7)))
            .child(line(30.0, t.color("contentTertiary")))
            .child(row().gap(4.0).child(line(18.0, t.color("accent"))).child(line(12.0, t.color("success")))),
    )
}

/// The light or the dark themes as a grid of cards, the chosen one ringed.
fn theme_grid(s: &Desk, dark: bool, p: &Pal) -> El {
    let chosen = s.settings.theme_id(dark).to_string();
    let card_w = (CARD_W - 2.0 * CARD_PAD - 2.0 * GROUP_PAD - (COLUMNS as f32 - 1.0) * GAP) / COLUMNS as f32;
    let cards = themes::of(dark).map(|t| theme_card(t, t.id == chosen, dark, card_w, p));
    row().w_full().gap(GAP).flex_wrap().role("listbox").label(if dark { "Dark theme" } else { "Light theme" }).children(cards)
}

/// A theme's card: its background with a prompt in its text and dots of its accent, green,
/// yellow, red and blue, and its name under it.
fn theme_card(t: &'static Theme, chosen: bool, dark: bool, w: f32, p: &Pal) -> El {
    let canvas = t.color("canvas");
    let dots = ["accent", "success", "warning", "danger", "info"]
        .iter()
        .enumerate()
        .map(|(i, role)| el().absolute(i as f32 * 11.0, 0.0).w(16.0).h(16.0).radius(8.0).fill(t.color(role)).border(2.0, canvas));
    let prompt = row()
        .gap(6.0)
        .center_y()
        .child(
            row()
                .h(18.0)
                .px(6.0)
                .center_y()
                .radius(6.0)
                .fill(t.color("surfaceRaised"))
                .border(1.0, t.color("strokeSubtle"))
                .child(text("~/").font(pal::SMALL_STRONG).color(t.color("content")).single_line()),
        )
        .child(text("tokprof").font(pal::SMALL).color(t.color("contentSecondary")).single_line());
    let picture = col()
        .w_full()
        .h(78.0)
        .padding(10.0)
        .radius(R_TILE)
        .fill(canvas)
        .border(1.0, if dark { t.color("stroke") } else { t.color("strokeSubtle") })
        .child(prompt)
        .child(spacer().grow(1.0))
        .child(el().w(16.0 + 4.0 * 11.0).h(16.0).children(dots));
    let picture = if chosen { picture.outline(2.0, 2.0, p.accent) } else { picture };
    col()
        .key(row_key(t))
        .w(w)
        .gap(7.0)
        .role("option")
        .checked(chosen)
        .label(t.name.clone())
        .on_press(move |s: &mut Desk| s.pick_theme(dark, &t.id))
        .child(picture)
        .child(
            text(t.name.clone())
                .key(format!("theme-name-{}", t.id))
                .font(if chosen { pal::STRONG } else { pal::BODY })
                .color(if chosen { p.accent } else { p.ink })
                .single_line()
                .truncate()
                .pl(2.0),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_step_through_a_grid_and_stop_at_its_edges() {
        // 14 cards in rows of 4: the last row holds 12 and 13.
        let step = |key: RuneKey, at| grid_step(&key, None, at, 14);
        assert_eq!(step(RuneKey::Right, 0), Some(1));
        assert_eq!(step(RuneKey::Left, 0), Some(0));
        assert_eq!(step(RuneKey::Right, 13), Some(13));
        assert_eq!(step(RuneKey::Down, 1), Some(5));
        assert_eq!(step(RuneKey::Down, 11), Some(11), "no card below 11");
        assert_eq!(step(RuneKey::Down, 9), Some(13));
        assert_eq!(step(RuneKey::Up, 13), Some(9));
        assert_eq!(step(RuneKey::Up, 2), Some(2));
        assert_eq!(step(RuneKey::Home, 7), Some(0));
        assert_eq!(step(RuneKey::End, 7), Some(13));
        assert_eq!(grid_step(&RuneKey::Text("j".into()), Some("j"), 0, 14), Some(4));
        assert_eq!(step(RuneKey::PageDown, 0), None);
    }

    #[test]
    fn appearances_by_name_both_ways() {
        for a in Appearance::ALL {
            assert_eq!(Appearance::named(a.name()), Some(a));
        }
        assert_eq!(Appearance::named("sepia"), None);
    }

    #[test]
    fn an_appearance_says_whether_the_window_is_dark() {
        assert!(Appearance::System.dark(true));
        assert!(!Appearance::System.dark(false));
        assert!(!Appearance::Light.dark(true));
        assert!(Appearance::Dark.dark(false));
    }

    #[test]
    fn unknown_values_give_the_defaults() {
        let mut prefs = Prefs::memory();
        prefs.set("appearance", "sepia").unwrap();
        prefs.set("light_theme", "nosuch").unwrap();
        // A dark theme is no light theme.
        prefs.set("dark_theme", "merino-light").unwrap();
        assert_eq!(Settings::read(&prefs), Settings::default());
        assert_eq!(Settings::default().theme(false).id, "merino-light");
        assert_eq!(Settings::default().theme(true).id, "merino-dark");
    }

    #[test]
    fn a_saved_choice_is_read_back() {
        let mut prefs = Prefs::memory();
        prefs.set("appearance", "dark").unwrap();
        prefs.set("dark_theme", "ink-meridian").unwrap();
        let s = Settings::read(&prefs);
        assert_eq!(s.appearance, Appearance::Dark);
        assert_eq!(s.theme(false).id, "ink-meridian", "dark whatever the system is");
        assert_eq!(s.light_theme, "merino-light");
    }

    #[test]
    fn the_menu_bar_countdown_is_off_unless_saved_on() {
        let mut prefs = Prefs::memory();
        assert!(!Settings::read(&prefs).menu_bar, "off by default");
        prefs.set("menu_bar", "on").unwrap();
        assert!(Settings::read(&prefs).menu_bar);
        prefs.set("menu_bar", "yes").unwrap();
        assert!(!Settings::read(&prefs).menu_bar, "a value it does not know is off");
    }

    #[test]
    fn colors_of_a_theme_draw_in_it_whatever_the_system_is() {
        let t = themes::find("alpine-milk").unwrap();
        let s = Settings::of_theme(t);
        assert_eq!(s.theme(true).id, "alpine-milk");
    }

    #[test]
    fn a_value_is_written_only_when_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let mut prefs = Prefs::at(&path);
        assert!(store(&mut prefs, "appearance", "dark").unwrap());
        let written = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!store(&mut prefs, "appearance", "dark").unwrap());
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), written);
        assert!(
            std::fs::read_to_string(&path).unwrap().contains(r#""appearance": "dark""#)
                || std::fs::read_to_string(&path).unwrap().contains(r#""appearance":"dark""#)
        );
    }
}
