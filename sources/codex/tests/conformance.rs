//! The source passes the checks every log source must pass, over its fixtures.

use std::path::{Path, PathBuf};

#[test]
fn the_fixtures_conform() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    tokprof_core::testkit::conform(&tokprof_source_codex::Codex::at(fixtures, Path::new("/h"))).unwrap();
}
