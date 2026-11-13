//! The session browser without its screen, shared by the terminal browser (`tokprof ui`) and the
//! window (`tokprof-desk`): the files tokprof keeps, the log directories it follows, the day view,
//! and the texts of the X-ray, the day and the session list.

pub mod browser;
pub mod calendar;
pub mod clipboard;
pub mod contain;
pub mod day;
pub mod filters;
pub mod paths;
pub mod period;
pub mod reading;
pub mod render_calendar;
pub mod render_month;
pub mod render_requests;
pub mod render_rhythm;
pub mod render_sessions;
pub mod render_tools;
pub mod render_xray;
pub mod render_year;
pub mod reports;
pub mod watch;
pub mod week;
