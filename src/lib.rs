//! Runs, stops and restarts a workspace's groups and nodes from the mesh, through the rant
//! command line tool.

mod cli;
mod listing;
mod manager;
mod names;
mod types;

pub use manager::{Manager, Options};
pub use types::{Action, GroupState, NodeState, RunningStatus};
