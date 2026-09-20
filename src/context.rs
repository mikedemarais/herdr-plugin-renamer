//! Parse the Herdr status event and admit only transitions to `working`.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EventEnvelope {
    data: EventData,
}

#[derive(Debug, Deserialize)]
struct EventData {
    pane_id: String,
    agent_status: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Eligible {
    pub pane_id: String,
}

pub fn evaluate(event_json: &str) -> Option<Eligible> {
    let event: EventEnvelope = serde_json::from_str(event_json).ok()?;
    (event.data.agent_status == "working").then_some(Eligible {
        pane_id: event.data.pane_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(status: &str) -> String {
        format!(
            r#"{{"event":"pane_agent_status_changed","data":{{"pane_id":"w4B:p1","agent_status":"{status}","agent":"codex"}}}}"#
        )
    }

    #[test]
    fn working_is_eligible() {
        assert_eq!(
            evaluate(&event("working")),
            Some(Eligible {
                pane_id: "w4B:p1".to_string()
            })
        );
    }

    #[test]
    fn other_statuses_and_garbage_bail() {
        assert!(evaluate(&event("idle")).is_none());
        assert!(evaluate(&event("blocked")).is_none());
        assert!(evaluate("not json").is_none());
    }
}
