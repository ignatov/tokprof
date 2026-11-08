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

/// The folder of the files tokprof keeps to answer faster: `$TOKPROF_CACHE_DIR`, else
/// `~/Library/Caches/tokprof` on macOS, `%LOCALAPPDATA%\tokprof` on Windows, and
/// `$XDG_CACHE_HOME/tokprof` or `~/.cache/tokprof` elsewhere.
pub fn cache_dir() -> PathBuf {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = var("TOKPROF_CACHE_DIR") {
        return dir;
    }
    if cfg!(target_os = "macos") {
        return home().join("Library").join("Caches").join("tokprof");
    }
    if cfg!(windows)
        && let Some(local) = var("LOCALAPPDATA")
    {
        return local.join("tokprof");
    }
    var("XDG_CACHE_HOME").unwrap_or_else(|| home().join(".cache")).join("tokprof")
}

/// The day store's file in the cache folder.
pub fn day_store() -> PathBuf {
    cache_dir().join(tokprof_core::daystore::FILE_NAME)
}

pub fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}
