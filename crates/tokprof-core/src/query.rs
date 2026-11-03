//! Which sessions and requests a command looks at: the agents, the project, the session and the
//! time window that the command line gives.

use crate::analysis::Filter;
use crate::model::Session;
use crate::source::{AgentId, SessionRef};
use crate::xray::SessionRow;

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub agents: Vec<AgentId>,
    /// Text the project, written with `~` for the home directory, contains.
    pub project: Option<String>,
    /// The start of the session id, in any case.
    pub session: Option<String>,
    /// Requests on or after this time; for commands that pick or list sessions, the main log was
    /// written on or after it.
    pub since_ms: Option<i64>,
    /// Requests before this time; for commands that pick or list sessions, the main log was
    /// created before it.
    pub until_ms: Option<i64>,
    /// Text the client of a request contains, ignoring case; for commands that pick or list
    /// sessions, the first client the main log names.
    pub client: Option<String>,
}

impl Query {
    /// The requests that reports count: the time window and the client.
    pub fn window(&self) -> Filter {
        Filter { since_ms: self.since_ms, until_ms: self.until_ms, client: self.client.clone() }
    }

    /// Whether the times of a session's main log pass; this needs no reading.
    pub(crate) fn times(&self, f: &SessionRef) -> bool {
        let created = f.created_ms.unwrap_or(f.modified_ms());
        self.since_ms.is_none_or(|s| f.modified_ms() >= s) && self.until_ms.is_none_or(|u| created < u)
    }

    /// Whether the id, the project and the client pass, as the first lines of the log tell them.
    pub(crate) fn meta(&self, project: &str, id: &str, client: Option<&str>) -> bool {
        self.project.as_ref().is_none_or(|p| project.contains(p.as_str()))
            && self.session.as_ref().is_none_or(|s| id_starts_with(id, s))
            && self.window().keeps_client(client.unwrap_or(crate::model::UNKNOWN_CLIENT))
    }

    pub(crate) fn reads_meta(&self) -> bool {
        self.project.is_some() || self.session.is_some() || self.client.is_some()
    }

    /// Whether a listed session passes.
    pub fn row(&self, row: &SessionRow) -> bool {
        self.times(&row.file) && self.meta(&row.project, &row.session_id, row.client.as_deref())
    }

    /// Whether a log belongs to the session `session` names, for the reports: its own id, or
    /// that of the session that spawned it, starts with the text.
    pub fn holds(&self, f: &SessionRef) -> bool {
        let Some(id) = &self.session else { return true };
        id_starts_with(&f.main_id, id) || id_starts_with(&f.session_id, id)
    }

    /// Whether a parsed session passes the project and session filters, for the reports.
    pub fn accepts(&self, s: &Session, f: &SessionRef) -> bool {
        self.project.as_ref().is_none_or(|p| s.project.contains(p.as_str()))
            && self.session.as_ref().is_none_or(|id| id_starts_with(&s.id, id) || self.holds(f))
    }
}

/// Whether a session id starts with `prefix`, in any case.
pub fn id_starts_with(id: &str, prefix: &str) -> bool {
    id.get(..prefix.len()).is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Locator;

    fn log(session_id: &str, main_id: &str, modified_ms: i64, created_ms: Option<i64>) -> SessionRef {
        SessionRef {
            source: "test",
            agent: crate::testing::claude(),
            locator: Locator::File(format!("/{session_id}.jsonl").into()),
            session_id: session_id.into(),
            main_id: main_id.into(),
            subagent: session_id != main_id,
            size: 0,
            modified_ns: modified_ms as i128 * 1_000_000,
            created_ms,
        }
    }

    #[test]
    fn a_session_filter_holds_its_subagents() {
        let q = Query { session: Some("3F95".into()), ..Query::default() };
        assert!(q.holds(&log("3f951de0", "3f951de0", 0, None)));
        assert!(q.holds(&log("agent-a1", "3f951de0", 0, None)), "a subagent of the session");
        assert!(!q.holds(&log("5e5510f0", "5e5510f0", 0, None)));
        assert!(Query::default().holds(&log("x", "x", 0, None)));
    }

    #[test]
    fn times_pick_by_written_and_created() {
        let q = Query { since_ms: Some(100), until_ms: Some(200), ..Query::default() };
        assert!(q.times(&log("a", "a", 150, Some(120))));
        assert!(!q.times(&log("a", "a", 90, Some(50))), "last written before the window");
        assert!(!q.times(&log("a", "a", 250, Some(220))), "created after it");
        assert!(q.times(&log("a", "a", 150, None)), "without a birth time, the modification time counts");
        assert_eq!((q.window().since_ms, q.window().until_ms), (Some(100), Some(200)));
    }

    #[test]
    fn a_project_filter_reads_the_written_project() {
        let q = Query { project: Some("~/src/app".into()), ..Query::default() };
        assert!(q.meta("~/src/app/web", "id", None) && !q.meta("~/src/other", "id", None));
    }

    fn request(client: Option<&str>) -> crate::model::Request {
        crate::model::Request { client: client.map(Into::into), ..crate::model::Request::default() }
    }

    #[test]
    fn a_client_filter_keeps_the_requests_whose_client_contains_the_text() {
        let q = |text: &str| Query { client: Some(text.into()), ..Query::default() }.window();
        // Part of a value.
        let sdk = q("sdk");
        assert!(sdk.keeps(&request(Some("sdk-ts"))) && sdk.keeps(&request(Some("sdk-cli"))));
        assert!(!sdk.keeps(&request(Some("cli"))));
        // Case.
        assert!(q("codex desktop").keeps(&request(Some("Codex Desktop"))));
        // Requests without a client.
        let unknown = q("unknown");
        assert!(unknown.keeps(&request(None)) && !unknown.keeps(&request(Some("cli"))));
        // No filter keeps everything.
        assert!(Query::default().window().keeps(&request(None)));
    }

    #[test]
    fn the_session_lists_take_the_client_from_the_log_head() {
        let q = Query { client: Some("sdk-ts".into()), ..Query::default() };
        assert!(q.reads_meta());
        assert!(q.meta("~/src/app", "id", Some("sdk-ts")));
        assert!(!q.meta("~/src/app", "id", Some("cli")) && !q.meta("~/src/app", "id", None));
    }
}
