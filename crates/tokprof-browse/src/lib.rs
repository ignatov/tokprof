//! The session browser without its screen, shared by the terminal browser (`tokprof ui`) and the
//! window (`tokprof-desk`): the files tokprof keeps, the log directories it follows, the day view,
//! and the texts of the X-ray, the day and the session list.

pub mod browser;
pub mod clipboard;
pub mod day;
pub mod filters;
pub mod paths;
pub mod reading;
pub mod render_rhythm;
pub mod render_sessions;
pub mod render_tools;
pub mod render_xray;
pub mod reports;
pub mod watch;
