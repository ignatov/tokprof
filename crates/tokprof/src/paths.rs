//! The files tokprof keeps on this machine: the profile directory and the user's price file, and
//! the prices loaded from them.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokprof_core::pricing::Pricing;

/// Where profiles go when no output path is given: `$TOKPROF_DIR`, or `~/tokprof`.
pub fn profile_dir() -> PathBuf {
    std::env::var_os("TOKPROF_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join("tokprof"))
}

/// `$XDG_CONFIG_HOME/tokprof/prices.toml`, by default `~/.config/tokprof/prices.toml`.
pub fn user_prices() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("tokprof")
        .join("prices.toml")
}

/// The built-in prices, then the user's price file if it exists, then `explicit`.
pub fn pricing(explicit: Option<&Path>) -> Result<Pricing> {
    let mut pricing = Pricing::defaults();
    let user = user_prices();
    match std::fs::read_to_string(&user) {
        Ok(text) => pricing.add_layer("user config", Some(user), &text)?,
        Err(e) if e.kind() == ErrorKind::NotFound => pricing.add_missing_layer("user config", user),
        Err(e) => return Err(e).with_context(|| format!("reading {}", user.display())),
    }
    if let Some(path) = explicit {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        pricing.add_layer("--prices", Some(path.to_path_buf()), &text)?;
    }
    Ok(pricing)
}

pub fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}
