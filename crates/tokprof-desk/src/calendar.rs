//! The day store as the window holds it: the calendar's worker, which reads the logs into the
//! store and computes `tokprof calendar` from it (see `tokprof_browse::calendar`), and what the
//! views that count from the store need of it. Week, Month, Quarter, Year, Sessions and the day
//! summary take the store from here.

use tokprof_browse::calendar::Calendars;
use tokprof_core::calendar::Calendar;

/// The store's worker, the window's filters, and how far the worker is.
pub struct State {
    pub data: Calendars,
    /// The window filters by client, which the store cannot.
    pub client_ignored: bool,
    /// The window's filters without `--since` and `--until`, for the cards to share.
    pub select: tokprof_core::query::Query,
    /// The worker's progress as the last poll saw it.
    pub seen: Option<(usize, usize)>,
}

impl State {
    pub fn new(data: Calendars) -> State {
        State { data, client_ignored: false, select: tokprof_core::query::Query::default(), seen: None }
    }

    /// The calendar, once computed.
    pub fn calendar(&self) -> Option<&Calendar> {
        self.data.get().and_then(|c| c.calendar.as_deref().ok())
    }
}
