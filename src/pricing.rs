//! Model prices in USD per million tokens, loaded from configuration.
//!
//! Layers, each replacing the earlier entry for the same model:
//! 1. `prices.toml` from the repository, built into the binary;
//! 2. `$XDG_CONFIG_HOME/tokprof/prices.toml`, by default `~/.config/tokprof/prices.toml`;
//! 3. the file given with `--prices`.
//!
//! Prices change over time, so a model can have several periods; a request is priced by the
//! one in effect when it was sent. `prices.toml` documents the format.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::model::Usage;

const DEFAULTS: &str = include_str!("../prices.toml");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    pub input: f64,
    /// Cache write with the short lifetime (Anthropic 5m, OpenAI 30m).
    pub write_short: f64,
    /// Cache write with the long lifetime (Anthropic 1h, OpenAI 12h).
    pub write_long: f64,
    pub read: f64,
    pub output: f64,
}

impl Price {
    /// What a request with this usage costs, in USD.
    pub fn cost(&self, u: &Usage) -> f64 {
        (u.cache_read as f64 * self.read
            + u.write_short as f64 * self.write_short
            + u.write_long as f64 * self.write_long
            + u.uncached as f64 * self.input
            + u.output as f64 * self.output)
            / 1e6
    }
}

/// The prices of a model over a span of time.
#[derive(Clone, Debug)]
pub struct Period {
    /// In effect from `since` (inclusive) to `until` (exclusive), in epoch ms; `None` is open.
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub price: Price,
    /// Prices for requests whose prompt is over the given number of tokens.
    pub long_context: Option<(u64, Price)>,
    pub context_window: Option<u64>,
}

impl Period {
    pub fn covers(&self, ts_ms: i64) -> bool {
        self.since_ms.is_none_or(|s| ts_ms >= s) && self.until_ms.is_none_or(|u| ts_ms < u)
    }
}

#[derive(Clone, Debug)]
pub struct PriceEntry {
    /// Model id prefix, lowercase.
    pub model: String,
    /// Latest start first.
    pub periods: Vec<Period>,
    /// Name of the layer the entry came from.
    pub source: &'static str,
}

impl PriceEntry {
    /// The period in effect at `ts_ms`; the latest start wins where periods overlap.
    pub fn at(&self, ts_ms: i64) -> Option<&Period> {
        self.periods.iter().find(|p| p.covers(ts_ms))
    }
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: &'static str,
    pub path: Option<PathBuf>,
    pub loaded: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Pricing {
    /// Longest prefix first, so the most specific entry wins.
    entries: Vec<PriceEntry>,
    layers: Vec<Layer>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceFile {
    /// A table of rates per model, or an array of them for prices that changed over time.
    #[serde(default)]
    models: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rates {
    since: Option<toml::Value>,
    until: Option<toml::Value>,
    input: f64,
    output: f64,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
    cache_write_long: Option<f64>,
    long_context: Option<LongContextRates>,
    context_window: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LongContextRates {
    above: u64,
    input: f64,
    output: f64,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
    cache_write_long: Option<f64>,
}

fn price(input: f64, output: f64, read: Option<f64>, write: Option<f64>, write_long: Option<f64>) -> Price {
    let write_short = write.unwrap_or(input);
    Price { input, write_short, write_long: write_long.unwrap_or(write_short), read: read.unwrap_or(input), output }
}

/// A TOML date or date-time, or a string with one, in epoch ms; UTC unless it has an offset.
fn moment(v: &toml::Value) -> Result<i64> {
    match v {
        toml::Value::Datetime(dt) => {
            let date = dt.date.context("a date is needed")?;
            let (h, m, s) = dt.time.map_or((0, 0, 0), |t| (t.hour, t.minute, t.second.unwrap_or(0)));
            let naive = chrono::NaiveDate::from_ymd_opt(date.year.into(), date.month.into(), date.day.into())
                .and_then(|d| d.and_hms_opt(h.into(), m.into(), s.into()))
                .context("not a valid date")?;
            let offset_min = match dt.offset {
                Some(toml::value::Offset::Custom { minutes }) => i64::from(minutes),
                _ => 0,
            };
            Ok(naive.and_utc().timestamp_millis() - offset_min * 60_000)
        }
        toml::Value::String(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|t| t.timestamp_millis())
            .or_else(|_| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map(|d| d.and_time(chrono::NaiveTime::MIN).and_utc().timestamp_millis()))
            .with_context(|| format!("{s:?} is not a date")),
        _ => bail!("expected a date such as 2026-08-21"),
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn user_config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::adapters::home().join(".config"))
        .join("tokprof")
        .join("prices.toml")
}

impl Pricing {
    /// The built-in defaults, then the user config if it exists, then `explicit`.
    pub fn load(explicit: Option<&Path>) -> Result<Pricing> {
        let mut pricing = Pricing::defaults();
        let user = user_config_path();
        match std::fs::read_to_string(&user) {
            Ok(text) => pricing.add_layer("user config", Some(user), &text)?,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                pricing.layers.push(Layer { name: "user config", path: Some(user), loaded: false });
            }
            Err(e) => return Err(e).with_context(|| format!("reading {}", user.display())),
        }
        if let Some(path) = explicit {
            let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            pricing.add_layer("--prices", Some(path.to_path_buf()), &text)?;
        }
        Ok(pricing)
    }

    pub fn defaults() -> Pricing {
        let mut pricing = Pricing::default();
        pricing.add_layer("built-in", None, DEFAULTS).expect("the built-in prices.toml is valid");
        pricing
    }

    /// Adds a layer; its entry for a model replaces every period an earlier layer had for it.
    pub fn add_layer(&mut self, name: &'static str, path: Option<PathBuf>, text: &str) -> Result<()> {
        let origin = path.as_ref().map_or_else(|| name.to_string(), |p| p.display().to_string());
        let file: PriceFile = toml::from_str(text).with_context(|| format!("parsing {origin}"))?;
        for (model, value) in file.models {
            let model = model.to_ascii_lowercase();
            let items = match value {
                toml::Value::Array(items) => items,
                one => vec![one],
            };
            let mut periods = Vec::with_capacity(items.len());
            for item in items {
                let r: Rates = item.try_into().with_context(|| format!("{origin}: {model}"))?;
                periods.push(period(&r).with_context(|| format!("{origin}: {model}"))?);
            }
            periods.sort_by_key(|p| Reverse(p.since_ms.unwrap_or(i64::MIN)));
            self.entries.retain(|e| e.model != model);
            self.entries.push(PriceEntry { model, periods, source: name });
        }
        self.entries.sort_by_key(|e| Reverse(e.model.len()));
        self.layers.push(Layer { name, path, loaded: true });
        Ok(())
    }

    /// The price of a request to `model` sent at `ts_ms` with a prompt of `prompt_tokens`.
    pub fn get(&self, model: &str, prompt_tokens: u64, ts_ms: i64) -> Option<Price> {
        let period = self.entry(model)?.at(ts_ms)?;
        Some(match period.long_context {
            Some((above, long)) if prompt_tokens > above => long,
            _ => period.price,
        })
    }

    pub fn entry(&self, model: &str) -> Option<&PriceEntry> {
        let model = model.to_ascii_lowercase();
        self.entries.iter().find(|e| prefix_matches(&model, &e.model))
    }

    pub fn context_window(&self, model: &str) -> Option<u64> {
        let entry = self.entry(model)?;
        entry.at(now_ms()).or(entry.periods.first())?.context_window
    }

    pub fn entries(&self) -> &[PriceEntry] {
        &self.entries
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }
}

fn period(r: &Rates) -> Result<Period> {
    let mut numbers = vec![Some(r.input), Some(r.output), r.cache_read, r.cache_write, r.cache_write_long];
    if let Some(l) = &r.long_context {
        numbers.extend([Some(l.input), Some(l.output), l.cache_read, l.cache_write, l.cache_write_long]);
    }
    if numbers.into_iter().flatten().any(|x| !x.is_finite() || x < 0.0) {
        bail!("prices must be non-negative numbers");
    }
    let since_ms = r.since.as_ref().map(moment).transpose().context("since")?;
    let until_ms = r.until.as_ref().map(moment).transpose().context("until")?;
    if let (Some(s), Some(u)) = (since_ms, until_ms)
        && s >= u {
            bail!("since must be before until");
        }
    Ok(Period {
        since_ms,
        until_ms,
        price: price(r.input, r.output, r.cache_read, r.cache_write, r.cache_write_long),
        long_context: r.long_context.as_ref().map(|l| (l.above, price(l.input, l.output, l.cache_read, l.cache_write, l.cache_write_long))),
        context_window: r.context_window,
    })
}

/// `prefix` prices `model` when the rest of the id is a date, a variant or a tag
/// ("-20250929", "-codex-max", "[1m]"), not another version ("-7", ".7-sol").
fn prefix_matches(model: &str, prefix: &str) -> bool {
    let Some(rest) = model.strip_prefix(prefix) else { return false };
    let mut chars = rest.chars();
    match chars.next() {
        None => true,
        Some('-' | '@' | '[' | ':' | '/') => {
            let segment: String = chars.take_while(char::is_ascii_alphanumeric).collect();
            let version = !segment.is_empty() && segment.len() < 6 && segment.chars().all(|c| c.is_ascii_digit());
            !version
        }
        Some(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(date: &str) -> i64 {
        moment(&toml::Value::String(date.to_string())).unwrap()
    }

    #[test]
    fn defaults_cover_the_models_in_use() {
        let p = Pricing::defaults();
        let now = now_ms();
        assert_eq!(p.get("claude-opus-5-5", 0, now).unwrap().read, 0.20);
        assert_eq!(p.get("claude-opus-5", 0, now).unwrap().read, 0.50);
        assert_eq!(p.get("claude-sonnet-4-5-20250929", 0, now).unwrap().input, 3.0);
        assert_eq!(p.get("claude-opus-4-5-20251101", 0, now).unwrap().input, 5.0);
        assert_eq!(p.get("gpt-6-astra", 0, now).unwrap().write_long, 25.0);
        assert_eq!(p.get("gpt-5.1-codex-max", 0, now).unwrap().input, 1.25);
        // No write charge before GPT-5.6: writes cost the input rate.
        assert_eq!(p.get("gpt-5.5", 0, now).unwrap().write_short, 5.0);
    }

    #[test]
    fn requests_are_priced_by_the_period_they_were_sent_in() {
        let p = Pricing::defaults();
        let before = p.get("gpt-5.6-sol", 0, at("2026-08-20")).unwrap();
        let after = p.get("gpt-5.6-sol", 0, at("2026-09-01")).unwrap();
        assert_eq!((before.input, before.output, before.write_short), (5.0, 30.0, 6.25));
        assert_eq!((after.input, after.output, after.write_short), (4.0, 20.0, 5.0));
        assert_eq!(p.get("gpt-5.6-sol", 300_000, at("2026-08-20")).unwrap().output, 45.0);
    }

    #[test]
    fn periods_parse_dates_and_date_times() {
        let mut p = Pricing::default();
        let text = "[[models.m]]\nuntil = 2026-08-21T17:00:00Z\ninput = 5\noutput = 30\n\n\
                    [[models.m]]\nsince = 2026-08-21T19:00:00+02:00\ninput = 4\noutput = 20\n";
        p.add_layer("test", None, text).unwrap();
        assert_eq!(p.get("m", 0, at("2026-08-21T16:59:59Z")).unwrap().input, 5.0);
        assert_eq!(p.get("m", 0, at("2026-08-21T17:00:00Z")).unwrap().input, 4.0);
        assert!(p.add_layer("test", None, "[models.m]\nsince = 2026-09-01\nuntil = 2026-08-01\ninput = 1\noutput = 1\n").is_err());
        // A gap between periods has no price rather than a guessed one.
        p.add_layer("test", None, "[[models.g]]\nuntil = 2026-01-01\ninput = 1\noutput = 1\n[[models.g]]\nsince = 2026-02-01\ninput = 2\noutput = 2\n").unwrap();
        assert!(p.get("g", 0, at("2026-01-15")).is_none());
    }

    #[test]
    fn long_context_tier_applies_above_the_threshold() {
        let p = Pricing::defaults();
        let now = now_ms();
        assert_eq!(p.get("claude-haiku-5-5", 100_000, now).unwrap().input, 0.10);
        assert_eq!(p.get("claude-haiku-5-5", 100_001, now).unwrap().input, 0.50);
        assert_eq!(p.get("gpt-6-astra", 300_000, now).unwrap().output, 75.0);
    }

    #[test]
    fn prefixes_do_not_price_other_versions() {
        let p = Pricing::defaults();
        let now = now_ms();
        assert!(p.get("claude-opus-5-7", 0, now).is_none());
        assert!(p.get("gpt-5.7-sol", 0, now).is_none());
        assert!(p.get("codex-auto-review", 0, now).is_none());
        assert!(prefix_matches("claude-opus-4-6[1m]", "claude-opus-4-6"));
    }

    #[test]
    fn later_layers_replace_entries() {
        let mut p = Pricing::defaults();
        p.add_layer("user config", None, "[models.\"gpt-5.6-sol\"]\ninput = 1\noutput = 2\n").unwrap();
        let e = p.entry("gpt-5.6-sol").unwrap();
        assert_eq!((e.periods.len(), e.periods[0].price.input, e.source), (1, 1.0, "user config"));
        assert_eq!(p.get("gpt-5.6-sol", 0, at("2026-08-01")).unwrap().read, 1.0, "the override covers every date");
        p.add_layer("--prices", None, "[models.\"codex-auto-review\"]\ninput = 1\noutput = 4\ncache_read = 0.1\n").unwrap();
        assert_eq!(p.get("codex-auto-review", 0, now_ms()).unwrap().read, 0.1);
    }

    #[test]
    fn typos_are_errors() {
        let mut p = Pricing::default();
        let err = p.add_layer("test", None, "[models.x]\ninput = 1\noutput = 2\ncache_reads = 0.1\n").unwrap_err();
        assert!(format!("{err:#}").contains("cache_reads"), "{err:#}");
        assert!(p.add_layer("test", None, "[models.x]\ninput = -1\noutput = 2\n").is_err());
        assert!(p.add_layer("test", None, "[models.x]\nsince = \"soon\"\ninput = 1\noutput = 2\n").is_err());
    }
}
