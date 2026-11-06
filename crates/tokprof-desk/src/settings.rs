//! The window's settings: the appearance and the light and dark themes, kept in the app's
//! settings file, and the panel over the window that picks them.

use std::time::Instant;

use rune::prefs::{Prefs, PrefsError};
use rune::{Color, El, Key as RuneKey, KeyEvent, ScrollSpec, Tx, col, el, row, spacer, text};

use crate::pal::{self, Pal};
use crate::themes::{self, Theme};
use crate::view;
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
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { appearance: Appearance::System, light_theme: themes::DEFAULT_LIGHT.into(), dark_theme: themes::DEFAULT_DARK.into() }
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
}

impl Part {
    const ORDER: [Part; 3] = [Part::Appearance, Part::Light, Part::Dark];

    fn step(self, back: bool) -> Part {
        let at = Part::ORDER.iter().position(|&p| p == self).unwrap_or(0);
        Part::ORDER[if back { (at + 2) % 3 } else { (at + 1) % 3 }]
    }
}

/// The open settings panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Panel {
    /// The part with the keyboard.
    pub part: Part,
}

impl Desk {
    /// Opens the settings panel, or closes it.
    pub fn toggle_settings(&mut self) {
        self.panel = match self.panel {
            Some(_) => None,
            None => Some(Panel::default()),
        };
    }

    pub fn pick_appearance(&mut self, a: Appearance) {
        self.settings.appearance = a;
        self.save("appearance", a.name());
    }

    /// Picks the theme for a dark or a light window.
    pub fn pick_theme(&mut self, dark: bool, id: &str) {
        *self.settings.theme_id_mut(dark) = id.to_string();
        self.save(if dark { "dark_theme" } else { "light_theme" }, id);
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
        let text = match &e.key {
            RuneKey::Text(t) if plain(e) => Some(t.as_str()),
            _ => None,
        };
        match (&e.key, text) {
            (RuneKey::Escape | RuneKey::Enter, _) => self.panel = None,
            (RuneKey::Tab, _) => self.panel = Some(Panel { part: panel.part.step(e.mods.shift) }),
            _ if panel.part == Part::Appearance => {
                let at = Appearance::ALL.iter().position(|&a| a == self.settings.appearance).unwrap_or(0);
                let to = match (&e.key, text) {
                    (RuneKey::Left, _) | (_, Some("h")) => at.saturating_sub(1),
                    (RuneKey::Right, _) | (_, Some("l")) => (at + 1).min(Appearance::ALL.len() - 1),
                    _ => return true,
                };
                self.pick_appearance(Appearance::ALL[to]);
            }
            _ => {
                let dark = panel.part == Part::Dark;
                let list: Vec<&Theme> = themes::of(dark).collect();
                let at = list.iter().position(|t| t.id == self.settings.theme_id(dark)).unwrap_or(0);
                let last = list.len() - 1;
                let to = match (&e.key, text) {
                    (RuneKey::Up, _) | (_, Some("k")) => at.saturating_sub(1),
                    (RuneKey::Down, _) | (_, Some("j")) => (at + 1).min(last),
                    (RuneKey::Home, _) => 0,
                    (RuneKey::End, _) => last,
                    _ => return true,
                };
                let id = list[to].id.clone();
                self.pick_theme(dark, &id);
                tx.scroll_into_view(format!("theme-{id}"));
            }
        }
        true
    }
}

/// The key of a theme's row in the panel.
pub fn row_key(t: &Theme) -> String {
    format!("theme-{}", t.id)
}

/// The settings panel over the window: a card over a dimmed window; a click outside the card
/// closes it.
pub fn panel(s: &Desk, panel: Panel, p: &Pal, height: f32) -> El {
    let card_h = (height - 120.0).clamp(300.0, 640.0);
    let appearance = Appearance::ALL.iter().map(|&a| {
        view::segment(format!("appearance-{}", a.name()), a.label(), s.settings.appearance == a, p).on_press(move |s: &mut Desk| s.pick_appearance(a))
    });
    let card = col()
        .key("settings")
        .w(640.0)
        .h(card_h)
        .padding(20.0)
        .gap(14.0)
        .radius(12.0)
        .fill(p.panel)
        .border(1.0, p.line)
        .shadow(12.0, 40.0, Color::BLACK.alpha(0.25))
        .role("dialog")
        .label("Settings")
        // A click on the card stays on it; one outside it closes the panel. Rune's modal would
        // take Tab itself, so the scrim stands in for it.
        .on_press(|_: &mut Desk| {})
        .child(text("Settings").key("settings-title").font(pal::TITLE).color(p.ink).single_line())
        .child(col().gap(6.0).child(heading("Appearance", panel.part == Part::Appearance, p)).child(row().child(view::segments(
            "appearance",
            appearance.collect(),
            p,
        ))))
        .child(row().key("theme-lists").w_full().grow(1.0).min_h(0.0).gap(16.0).child(theme_list(s, false, panel.part == Part::Light, p)).child(theme_list(
            s,
            true,
            panel.part == Part::Dark,
            p,
        )))
        .child(row().w_full().child(spacer().grow(1.0)).child(view::button("settings-done", "Done", true, p).on_press(|s: &mut Desk| s.panel = None)));
    el().key("settings-scrim").absolute(0.0, 0.0).w_full().h_full().fill(p.bg.alpha(0.5)).center().on_press(|s: &mut Desk| s.panel = None).child(card)
}

/// A part's title, with the line that marks it as the one with the keyboard under it.
fn heading(label: &'static str, on: bool, p: &Pal) -> El {
    col().gap(3.0).child(text(label).font(pal::HEAD).color(p.ink2).single_line()).child(el().w(28.0).h(2.0).radius(1.0).fill(if on {
        p.accent
    } else {
        Color::TRANSPARENT
    }))
}

/// The list of the light or the dark themes, the chosen one marked.
fn theme_list(s: &Desk, dark: bool, keyboard: bool, p: &Pal) -> El {
    let chosen = s.settings.theme_id(dark).to_string();
    let rows = themes::of(dark).map(|t| theme_row(t, t.id == chosen, dark, p));
    let list = col()
        .key(if dark { "dark-themes" } else { "light-themes" })
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .radius(8.0)
        .border(1.0, p.line)
        .role("listbox")
        .label(if dark { "Dark theme" } else { "Light theme" })
        .child(col().w_full().padding(4.0).gap(2.0).children(rows));
    col().w(0.0).grow(1.0).h_full().min_h(0.0).gap(6.0).child(heading(if dark { "Dark theme" } else { "Light theme" }, keyboard, p)).child(list)
}

/// A theme's row: its name and a strip of its background, text, accent, green, yellow and red.
fn theme_row(t: &'static Theme, chosen: bool, dark: bool, p: &Pal) -> El {
    let swatches =
        ["canvas", "content", "accent", "success", "warning", "danger"].map(|role| el().w(12.0).h(12.0).radius(3.0).fill(t.color(role)).border(1.0, p.line));
    row()
        .key(row_key(t))
        .w_full()
        .h(30.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .radius(6.0)
        .fill(if chosen { p.accent } else { Color::TRANSPARENT })
        .hover(if chosen { p.accent } else { p.hover })
        .role("option")
        .checked(chosen)
        .label(t.name.clone())
        .on_press(move |s: &mut Desk| s.pick_theme(dark, &t.id))
        .child(text(t.name.clone()).font(pal::BODY).color(if chosen { p.on_accent } else { p.ink }).single_line().truncate().grow(1.0).min_w(0.0))
        .child(row().gap(3.0).children(swatches))
}

#[cfg(test)]
mod tests {
    use super::*;

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
