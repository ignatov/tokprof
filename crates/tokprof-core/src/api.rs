//! The prompt-cache rules of the APIs requests are sent to. They belong to the API, not to the
//! agent: an agent that sends requests to several APIs gets each API's rules per request. This is
//! the one place in the library that names vendors.

/// The API a request was sent to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ApiId {
    /// An API whose cache rules tokprof does not know: no lifetime is assumed for it.
    #[default]
    Unknown,
    Anthropic,
    OpenAi,
}

/// How an API's prompt cache behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheRules {
    /// Who runs the API.
    pub vendor: &'static str,
    /// The models the API serves, as reports name them.
    pub models: &'static str,
    /// What the short and the long cache-write prices of the API's models are for.
    pub write_lifetimes: (&'static str, &'static str),
    /// The lifetime of a cache entry written with the short lifetime, in ms, and its name.
    pub short: (i64, &'static str),
    /// The long lifetime, when the API offers one; a conversation is on it when its latest
    /// request with a cache write wrote more tokens with it than with the short one.
    pub long: Option<(i64, &'static str)>,
    /// How many content blocks back the API looks for an earlier cache entry.
    pub lookback_blocks: Option<usize>,
    /// Whether the API documents that reading an entry resets its lifetime; otherwise the bench
    /// assumes it.
    pub read_resets_ttl_documented: bool,
}

const ANTHROPIC: CacheRules =
    CacheRules { vendor: "Anthropic", models: "Claude", write_lifetimes: ("5m", "1h"), short: (300_000, "5m"), long: Some((3_600_000, "1h")), lookback_blocks: Some(20), read_resets_ttl_documented: true };

// OpenAI lifetimes depend on the model and organization; 30 minutes is the default for current
// models, and shorter in-memory retention also expires within it.
const OPENAI: CacheRules = CacheRules { vendor: "OpenAI", models: "GPT", write_lifetimes: ("30m", "12h"), short: (1_800_000, "30m"), long: None, lookback_blocks: None, read_resets_ttl_documented: false };

impl ApiId {
    /// The APIs whose rules are known.
    pub const KNOWN: [ApiId; 2] = [ApiId::Anthropic, ApiId::OpenAi];

    /// The API's cache rules; `None` for an API whose rules are not known.
    pub fn rules(self) -> Option<&'static CacheRules> {
        match self {
            ApiId::Anthropic => Some(&ANTHROPIC),
            ApiId::OpenAi => Some(&OPENAI),
            ApiId::Unknown => None,
        }
    }

    /// How long the prompt cache lives after a request, and its name, given whether the
    /// conversation writes with the long lifetime; `None` when the API's rules are not known.
    pub fn ttl(self, long_ttl: bool) -> Option<(i64, &'static str)> {
        let r = self.rules()?;
        Some(if long_ttl { r.long.unwrap_or(r.short) } else { r.short })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_has_two_lifetimes_and_a_lookback() {
        assert_eq!(ApiId::Anthropic.ttl(true), Some((3_600_000, "1h")));
        assert_eq!(ApiId::Anthropic.ttl(false), Some((300_000, "5m")));
        assert_eq!(ApiId::Anthropic.rules().unwrap().lookback_blocks, Some(20));
    }

    #[test]
    fn openai_has_one_assumed_lifetime_and_no_lookback() {
        assert_eq!(ApiId::OpenAi.ttl(true), Some((1_800_000, "30m")));
        assert_eq!(ApiId::OpenAi.ttl(false), Some((1_800_000, "30m")));
        assert_eq!(ApiId::OpenAi.rules().unwrap().lookback_blocks, None);
    }

    #[test]
    fn an_unknown_api_has_no_lifetime() {
        assert_eq!(ApiId::Unknown.ttl(true), None);
        assert!(ApiId::Unknown.rules().is_none());
    }
}
