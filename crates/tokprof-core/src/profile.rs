//! The data of a profile: what each stack of frames spent, by measure, and per minute for JFR.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default)]
pub struct Values {
    pub tokens: f64,
    pub usd: f64,
    pub avoidable: f64,
}

impl Values {
    pub fn add(&mut self, o: Values) {
        self.tokens += o.tokens;
        self.usd += o.usd;
        self.avoidable += o.avoidable;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    Tokens,
    Cost,
    Avoidable,
}

impl Measure {
    /// Integer sample value: tokens, or micro-dollars for money.
    pub fn of(self, v: &Values) -> i64 {
        match self {
            Measure::Tokens => v.tokens.round() as i64,
            Measure::Cost => (v.usd * 1e6).round() as i64,
            Measure::Avoidable => (v.avoidable * 1e6).round() as i64,
        }
    }

    pub fn units(self) -> &'static str {
        match self {
            Measure::Tokens => "tokens",
            Measure::Cost | Measure::Avoidable => "µ$",
        }
    }
}

/// What a frame stands for, which decides its color in the flame graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tint {
    /// Agent, project, session, chain, model, turn.
    Structure,
    /// Content kind, tool, target.
    Content,
    Read,
    Write,
    Uncached,
    Output,
    /// Re-written prefix and miss causes.
    Miss,
}

impl Tint {
    /// async-profiler frame type, an index into its palette: 0 pale green, 1 green, 2 aqua,
    /// 3 red, 5 orange, 6 yellow-green.
    pub(crate) fn frame_type(self) -> u8 {
        match self {
            Tint::Structure => 0,
            Tint::Content => 1,
            Tint::Read => 2,
            Tint::Miss => 3,
            Tint::Uncached | Tint::Output => 5,
            Tint::Write => 6,
        }
    }

    /// Suffix from which async-profiler's converter reads the same frame type. Red frames have
    /// no suffix; their names must not look like Java or C++ symbols.
    pub(crate) fn suffix(self) -> &'static str {
        match self.frame_type() {
            0 => "_[0]",
            1 => "_[j]",
            2 => "_[i]",
            5 => "_[k]",
            6 => "_[1]",
            _ => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Frame {
    pub name: String,
    pub tint: Tint,
    /// The dimension the frame comes from; JFR uses it as the class name.
    pub dim: &'static str,
}

pub type Stacks = HashMap<Vec<Frame>, Values>;

/// (thread, stack, minute start in epoch ms).
pub type TimedKey = (String, Vec<Frame>, i64);

pub type TimedStacks = HashMap<TimedKey, Values>;
