//! Work in the background that a bug must not stop for good. A job that panics, on one malformed
//! log say, comes back as an error that carries the panic's text, and the worker that ran it goes
//! on with the next job.
//!
//! Contained work runs on threads marked as such: the thread that runs the job, and the threads of
//! the pools its parallel work runs on. The panic hook a screen installs (`install_hook`) says
//! nothing for a marked thread, so that a terminal stays in raw mode and nothing is printed over
//! the screen; every other panic goes to the hook that was there before.

use std::any::Any;
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::OnceLock;

use rayon::ThreadPool;

thread_local! {
    /// How many contained jobs the thread runs; a thread of a contained pool runs them all its
    /// life.
    static CONTAINED: Cell<u32> = const { Cell::new(0) };
}

fn mark(on: bool) {
    let _ = CONTAINED.try_with(|c| c.set(if on { c.get() + 1 } else { c.get().saturating_sub(1) }));
}

/// Whether the current thread runs contained work.
pub fn contained() -> bool {
    CONTAINED.try_with(|c| c.get() > 0).unwrap_or(false)
}

/// A pool of `threads` threads for contained work, one per core for 0; `None` when the threads
/// cannot be started.
pub fn pool(threads: usize) -> Option<ThreadPool> {
    rayon::ThreadPoolBuilder::new().num_threads(threads).start_handler(|_| mark(true)).build().ok()
}

/// The pool most contained work runs on, with as many threads as rayon's own pool, in its place:
/// what runs on it shares the cores as it did there.
fn shared() -> Option<&'static ThreadPool> {
    static POOL: OnceLock<Option<ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| pool(0)).as_ref()
}

/// Runs `job` on the shared pool, where the parallel work it starts runs too, and gives what it
/// gives, or the panic's text when it panicked.
pub fn run<R: Send>(job: impl FnOnce() -> R + Send) -> Result<R, String> {
    run_on(shared(), job)
}

/// Runs `job` on `pool`, or on this thread without one, as `run` does.
pub fn run_on<R: Send>(pool: Option<&ThreadPool>, job: impl FnOnce() -> R + Send) -> Result<R, String> {
    mark(true);
    let ran = panic::catch_unwind(AssertUnwindSafe(|| match pool {
        Some(pool) => pool.install(job),
        None => job(),
    }));
    mark(false);
    ran.map_err(|payload| format!("tokprof panicked: {}", text(payload.as_ref())))
}

/// The text a panic was raised with.
fn text(payload: &(dyn Any + Send)) -> &str {
    match (payload.downcast_ref::<&str>(), payload.downcast_ref::<String>()) {
        (Some(s), _) => s,
        (_, Some(s)) => s,
        _ => "a panic without a message",
    }
}

/// From now on, a panic in contained work is not reported by the panic hook, since it comes
/// back as its job's error; any other panic goes to the hook installed before. A screen calls it
/// after it set the terminal up, so that the hook that restores the terminal is the one it keeps
/// from contained work.
pub fn install_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if !contained() {
            previous(info);
        }
    }));
}

#[cfg(test)]
pub(crate) mod testing {
    //! A source whose decoder panics for one session, for the tests of the workers.

    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use anyhow::Result;
    use tokprof_core::catalog::Catalog;
    use tokprof_core::model::Session;
    use tokprof_core::source::{Activity, AgentId, Claim, Decoder, Detail, Head, LogSource, Records, SessionRef, Touched};
    use tokprof_source_claude_code::ClaudeCode;

    /// The text the decoder panics with.
    pub const BOOM: &str = "a record no decoder expected";
    /// The fixture's session, and the copy of it the decoder panics on.
    pub const HEALTHY: &str = "5e5510f0-1111-4222-8333-944445555666";
    pub const BROKEN: &str = "0badc0de-1111-4222-8333-944445555666";

    /// Claude Code's source over a folder, whose decoder panics for the session `BROKEN` while
    /// `armed` is set.
    pub struct Panicky {
        inner: ClaudeCode,
        pub armed: Arc<AtomicBool>,
    }

    struct Boom;

    impl Decoder for Boom {
        fn feed(&mut self, _: &str) {
            panic!("{BOOM}");
        }
        fn snapshot(&self) -> Session {
            panic!("{BOOM}");
        }
        fn finish(self: Box<Self>) -> Session {
            panic!("{BOOM}");
        }
    }

    impl LogSource for Panicky {
        fn name(&self) -> &'static str {
            self.inner.name()
        }
        fn agents(&self) -> Vec<AgentId> {
            self.inner.agents()
        }
        fn detail(&self) -> Detail {
            self.inner.detail()
        }
        fn discover(&self) -> Vec<SessionRef> {
            self.inner.discover()
        }
        fn session_at(&self, path: &Path) -> Option<SessionRef> {
            self.inner.session_at(path)
        }
        fn claim(&self, path: &Path) -> Claim {
            self.inner.claim(path)
        }
        fn watched_paths(&self) -> Vec<PathBuf> {
            self.inner.watched_paths()
        }
        fn touched(&self, changed: &Path, arrived: bool) -> Touched {
            self.inner.touched(changed, arrived)
        }
        fn head(&self, s: &SessionRef) -> Head {
            self.inner.head(s)
        }
        fn last_active(&self, s: &SessionRef) -> Option<i64> {
            self.inner.last_active(s)
        }
        fn first_active(&self, s: &SessionRef) -> Option<i64> {
            self.inner.first_active(s)
        }
        fn subagents(&self, s: &SessionRef) -> Vec<SessionRef> {
            self.inner.subagents(s)
        }
        fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>> {
            self.inner.open(s)
        }
        fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
            match s.session_id == BROKEN && self.armed.load(Ordering::SeqCst) {
                true => Box::new(Boom),
                false => self.inner.decoder(s),
            }
        }
        fn activity(&self, s: &SessionRef) -> Result<Activity> {
            self.inner.activity(s)
        }
    }

    /// Keeps the panics of contained work out of the tests' output, as a screen does.
    pub fn quiet() {
        static HOOK: std::sync::Once = std::sync::Once::new();
        HOOK.call_once(super::install_hook);
    }

    /// A folder of `name` with the fixture's session and a copy of its main log as `BROKEN`, and
    /// a catalog over it whose decoder panics for the copy; with the switch that arms it, set.
    pub fn catalog(name: &str) -> (Arc<Catalog>, Arc<AtomicBool>, PathBuf) {
        quiet();
        let dir = std::env::temp_dir().join(format!("tokprof-contain-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sources/claude-code/tests/fixtures/projects/-w-app");
        let app = dir.join("projects/-w-app");
        let subagents = app.join(HEALTHY).join("subagents");
        std::fs::create_dir_all(&subagents).unwrap();
        for e in std::fs::read_dir(fixtures.join(HEALTHY).join("subagents")).unwrap().flatten() {
            std::fs::copy(e.path(), subagents.join(e.file_name())).unwrap();
        }
        let main = std::fs::read_to_string(fixtures.join(format!("{HEALTHY}.jsonl"))).unwrap();
        std::fs::write(app.join(format!("{HEALTHY}.jsonl")), &main).unwrap();
        std::fs::write(app.join(format!("{BROKEN}.jsonl")), main.replace(HEALTHY, BROKEN)).unwrap();
        let armed = Arc::new(AtomicBool::new(true));
        let source = Panicky { inner: ClaudeCode::at(dir.join("projects"), Path::new("/w")), armed: armed.clone() };
        (Arc::new(Catalog::new(vec![Box::new(source)], PathBuf::from("/w"))), armed, dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_comes_back_as_the_jobs_error_and_the_pool_goes_on() {
        testing::quiet();
        assert_eq!(run(|| 2 + 2), Ok(4));
        let failed: Result<(), String> = run(|| panic!("{}", testing::BOOM));
        assert_eq!(failed, Err(format!("tokprof panicked: {}", testing::BOOM)));
        // A panic in parallel work fails its job; the pool is still there for the next one.
        let failed = run(|| {
            use rayon::prelude::*;
            (0..64).into_par_iter().map(|i| if i == 63 { panic!("one item") } else { i }).sum::<i32>()
        });
        assert_eq!(failed, Err("tokprof panicked: one item".to_string()));
        assert_eq!(run(contained), Ok(true), "the job's thread is marked");
        assert!(!contained(), "the caller is not marked after the job");
        assert_eq!(run_on(None, || 5), Ok(5));
    }
}
