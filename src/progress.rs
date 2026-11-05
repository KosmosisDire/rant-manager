//! What rant run, stop and restart report as they go, read from the row `--json --lines`
//! prints each time a node comes or changes.

use serde::Deserialize;

use crate::types::{NodeProgress, ProgressStatus};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum StateJson {
    Starting,
    Running,
    Stopped,
    Failed,
}

#[derive(Deserialize)]
struct RowJson {
    name: String,
    state: StateJson,
    status: String,
    #[serde(default)]
    detail: String,
}

/// The nodes one Manage task touched, each as its newest row says.
#[derive(Default)]
pub struct Progress {
    pub nodes: Vec<NodeProgress>,
}

impl Progress {
    /// Takes one printed row, in place of the earlier one of its node.
    pub fn take(&mut self, line: &str) -> Result<(), String> {
        let row: RowJson = serde_json::from_str(line).map_err(|e| format!("a row the CLI printed did not parse: {e}: {line}"))?;
        let status = match row.state {
            StateJson::Starting => ProgressStatus::Starting,
            StateJson::Running => ProgressStatus::Running,
            StateJson::Stopped => ProgressStatus::Stopped,
            StateJson::Failed => ProgressStatus::Failed,
        };
        let note = if row.detail.is_empty() { row.status } else { format!("{} ({})", row.status, row.detail) };
        let node = NodeProgress { name: row.name, status, note };
        match self.nodes.iter_mut().find(|n| n.name == node.name) {
            Some(old) => *old = node,
            None => self.nodes.push(node),
        }
        Ok(())
    }

    /// Each failed node and what the CLI says of it, or none when none failed.
    pub fn failed(&self) -> Option<String> {
        let failed: Vec<String> = self.nodes.iter().filter(|n| n.status == ProgressStatus::Failed).map(|n| format!("{} {}", n.name, n.note)).collect();
        (!failed.is_empty()).then(|| failed.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_takes_the_place_of_its_nodes_earlier_one() {
        let mut p = Progress::default();
        p.take(r#"{"name":"sleeper","state":"starting","status":"starting","pid":12,"log":"logs/sleeper.log"}"#).unwrap();
        p.take(r#"{"name":"waiter","state":"starting","status":"starting","pid":13,"log":"logs/waiter.log"}"#).unwrap();
        p.take(r#"{"name":"sleeper","state":"running","status":"started","pid":12,"log":"logs/sleeper.log"}"#).unwrap();
        let shown: Vec<(&str, ProgressStatus)> = p.nodes.iter().map(|n| (n.name.as_str(), n.status)).collect();
        assert_eq!(shown, [("sleeper", ProgressStatus::Running), ("waiter", ProgressStatus::Starting)]);
        assert_eq!(p.nodes[0].note, "started");
        assert_eq!(p.failed(), None);
    }

    #[test]
    fn failed_nodes_say_why() {
        let mut p = Progress::default();
        p.take(r#"{"name":"sleeper","state":"running","status":"started"}"#).unwrap();
        p.take(r#"{"name":"brief","state":"failed","status":"exited after 0.3 s","detail":"logs/brief.log","running":false}"#).unwrap();
        assert_eq!(p.failed().as_deref(), Some("brief exited after 0.3 s (logs/brief.log)"));
        assert!(p.take("not json").is_err());
    }
}
