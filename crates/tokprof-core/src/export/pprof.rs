//! The pprof writer: a gzipped `profile.proto`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use prost::Message;

use super::{Measure, Stacks};

// The subset of github.com/google/pprof/proto/profile.proto this writer needs.
#[derive(Clone, PartialEq, Message)]
struct Profile {
    #[prost(message, repeated, tag = "1")]
    sample_type: Vec<ValueType>,
    #[prost(message, repeated, tag = "2")]
    sample: Vec<Sample>,
    #[prost(message, repeated, tag = "4")]
    location: Vec<Location>,
    #[prost(message, repeated, tag = "5")]
    function: Vec<Function>,
    #[prost(string, repeated, tag = "6")]
    string_table: Vec<String>,
    #[prost(int64, tag = "14")]
    default_sample_type: i64,
}

#[derive(Clone, PartialEq, Message)]
struct ValueType {
    #[prost(int64, tag = "1")]
    r#type: i64,
    #[prost(int64, tag = "2")]
    unit: i64,
}

#[derive(Clone, PartialEq, Message)]
struct Sample {
    #[prost(uint64, repeated, tag = "1")]
    location_id: Vec<u64>,
    #[prost(int64, repeated, tag = "2")]
    value: Vec<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct Location {
    #[prost(uint64, tag = "1")]
    id: u64,
    #[prost(message, repeated, tag = "4")]
    line: Vec<Line>,
}

#[derive(Clone, PartialEq, Message)]
struct Line {
    #[prost(uint64, tag = "1")]
    function_id: u64,
}

#[derive(Clone, PartialEq, Message)]
struct Function {
    #[prost(uint64, tag = "1")]
    id: u64,
    #[prost(int64, tag = "2")]
    name: i64,
    #[prost(int64, tag = "3")]
    system_name: i64,
}

/// All three measures go in as sample types, so pprof can switch between them; the chosen one
/// is the default.
pub(super) fn write(path: &Path, stacks: &Stacks, measure: Measure) -> Result<usize> {
    let mut strings: Vec<String> = vec![String::new()];
    let mut string_ids: HashMap<String, i64> = HashMap::new();
    let mut intern = |s: &str, strings: &mut Vec<String>| -> i64 {
        if let Some(&id) = string_ids.get(s) {
            return id;
        }
        strings.push(s.to_string());
        let id = strings.len() as i64 - 1;
        string_ids.insert(s.to_string(), id);
        id
    };
    let measures = [Measure::Tokens, Measure::Cost, Measure::Avoidable];
    let sample_type = [("tokens", "count"), ("cost", "microdollars"), ("avoidable", "microdollars")]
        .iter()
        .map(|(t, u)| ValueType { r#type: intern(t, &mut strings), unit: intern(u, &mut strings) })
        .collect();

    let mut functions: Vec<Function> = Vec::new();
    let mut function_ids: HashMap<String, u64> = HashMap::new();
    let mut samples = Vec::new();
    for (stack, v) in stacks {
        let values: Vec<i64> = measures.iter().map(|m| m.of(v)).collect();
        if values.iter().all(|&x| x <= 0) || stack.is_empty() {
            continue;
        }
        let mut location_id = Vec::with_capacity(stack.len());
        for frame in stack.iter().rev() {
            let id = match function_ids.get(&frame.name) {
                Some(&id) => id,
                None => {
                    let id = functions.len() as u64 + 1;
                    let name = intern(&frame.name, &mut strings);
                    functions.push(Function { id, name, system_name: name });
                    function_ids.insert(frame.name.clone(), id);
                    id
                }
            };
            location_id.push(id);
        }
        samples.push(Sample { location_id, value: values });
    }
    let location = functions.iter().map(|f| Location { id: f.id, line: vec![Line { function_id: f.id }] }).collect();
    let default_sample_type = intern(
        match measure {
            Measure::Tokens => "tokens",
            Measure::Cost => "cost",
            Measure::Avoidable => "avoidable",
        },
        &mut strings,
    );
    let n = samples.len();
    let profile = Profile { sample_type, sample: samples, location, function: functions, string_table: strings, default_sample_type };

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut gz = GzEncoder::new(BufWriter::new(file), Compression::default());
    gz.write_all(&profile.encode_to_vec())?;
    gz.finish()?.flush()?;
    Ok(n)
}
