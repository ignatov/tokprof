//! The window's color themes: the themes of Rex, kept in `themes/themes.json`, which
//! `themes/import.py` writes from Rex's own file. Each theme has an id, a name, an appearance and
//! the colors of its roles as Rex draws them, in sRGB hex.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use rune::Color;
use serde::Deserialize;

/// The theme the window starts in while the appearance is light, and while it is dark.
pub const DEFAULT_LIGHT: &str = "merino-light";
pub const DEFAULT_DARK: &str = "merino-dark";

#[derive(Debug, Deserialize)]
struct File {
    /// Where the themes come from, as the script was told.
    #[allow(dead_code)]
    source: String,
    themes: Vec<Theme>,
}

/// A color theme.
#[derive(Debug, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    appearance: String,
    /// The colors of the theme's roles, by Rex's role names, as `#rrggbb` or `#rrggbbaa`.
    colors: BTreeMap<String, String>,
}

impl Theme {
    pub fn dark(&self) -> bool {
        self.appearance == "dark"
    }

    /// The color of a role; a role the theme lacks is a mistake the tests catch, drawn magenta.
    pub fn color(&self, role: &str) -> Color {
        self.colors.get(role).and_then(|h| rune::color::hex(h)).unwrap_or(Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 })
    }
}

/// Every theme, in Rex's order.
pub fn all() -> &'static [Theme] {
    static THEMES: OnceLock<Vec<Theme>> = OnceLock::new();
    THEMES.get_or_init(|| parse(include_str!("../themes/themes.json")).expect("themes/themes.json parses"))
}

fn parse(text: &str) -> serde_json::Result<Vec<Theme>> {
    serde_json::from_str::<File>(text).map(|f| f.themes)
}

/// The theme with this id.
pub fn find(id: &str) -> Option<&'static Theme> {
    all().iter().find(|t| t.id == id)
}

/// The themes of one appearance, in Rex's order.
pub fn of(dark: bool) -> impl Iterator<Item = &'static Theme> {
    all().iter().filter(move |t| t.dark() == dark)
}

/// The theme with this id when it is of this appearance, else the appearance's default.
pub fn pick(id: &str, dark: bool) -> &'static Theme {
    find(id).filter(|t| t.dark() == dark).unwrap_or_else(|| default(dark))
}

/// The default theme of an appearance.
pub fn default(dark: bool) -> &'static Theme {
    find(if dark { DEFAULT_DARK } else { DEFAULT_LIGHT }).expect("the default themes are in the file")
}

/// The roles the window reads from a theme, which the tests find in every theme.
#[cfg(test)]
pub const ROLES: &[&str] = &[
    "canvas",
    "surfaceSubtle",
    "surfaceRaised",
    "terminalBackground",
    "strokeSubtle",
    "stroke",
    "content",
    "contentSecondary",
    "contentTertiary",
    "controlHover",
    "accent",
    "accentOnSolid",
    "accentDim",
    "danger",
    "dangerContainer",
    "success",
    "warning",
    "info",
    "terminalAnsi3",
    "terminalAnsi4",
    "terminalAnsi5",
    "terminalAnsi6",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_holds_rexs_themes() {
        let themes = all();
        assert_eq!(themes.len(), 41);
        assert_eq!(of(false).count(), 14);
        assert_eq!(of(true).count(), 27);
        let mut ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 41, "ids are unique");
        assert!(themes.iter().all(|t| t.appearance == "light" || t.appearance == "dark"));
    }

    #[test]
    fn every_role_is_in_every_theme() {
        for t in all() {
            for role in ROLES {
                let hex = t.colors.get(*role).unwrap_or_else(|| panic!("{} has no {role}", t.id));
                assert!(rune::color::hex(hex).is_some(), "{} {role} {hex}", t.id);
            }
        }
    }

    #[test]
    fn merino_light_as_rex_draws_it() {
        let t = find("merino-light").unwrap();
        assert_eq!(t.name, "Merino Light");
        assert!(!t.dark());
        assert_eq!(t.colors["canvas"], "#f6f7f9");
        assert_eq!(t.colors["accent"], "#0061f6");
        assert!(find("merino-dark").unwrap().dark());
    }

    #[test]
    fn a_theme_of_the_other_appearance_or_unknown_gives_the_default() {
        assert_eq!(pick("ink-meridian", true).id, "ink-meridian");
        assert_eq!(pick("ink-meridian", false).id, DEFAULT_LIGHT);
        assert_eq!(pick("nosuch", true).id, DEFAULT_DARK);
    }

    #[test]
    fn a_changed_format_does_not_parse() {
        assert!(parse(r#"{"themes": []}"#).is_err());
        assert!(parse(r#"{"source": "x", "themes": [{"id": "a"}]}"#).is_err());
    }
}
