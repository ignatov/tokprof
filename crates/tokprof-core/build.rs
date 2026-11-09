//! Names the git commit the library is built from, for the day store's fingerprint: numbers
//! made by another build may follow other rules, so its logs are read again.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(env!("CARGO_MANIFEST_DIR")).output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

fn main() {
    let commit = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_default();
    println!("cargo:rustc-env=TOKPROF_COMMIT={commit}");
    // Built again when HEAD moves: a checkout, a commit or a reset.
    for (flag, file) in [("--git-dir", "HEAD"), ("--git-common-dir", "packed-refs")] {
        if let Some(dir) = git(&["rev-parse", flag]) {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
            println!("cargo:rerun-if-changed={}", dir.join(file).display());
        }
    }
    if let (Some(dir), Some(head)) = (git(&["rev-parse", "--git-common-dir"]), git(&["symbolic-ref", "-q", "HEAD"])) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
        println!("cargo:rerun-if-changed={}", dir.join(head).display());
    }
    println!("cargo:rerun-if-changed=build.rs");
}
