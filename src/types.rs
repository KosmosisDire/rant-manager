//! What the manager offers on the mesh.

#[derive(rant::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RunningStatus {
    Stopped,
    Running,
}

/// What a Manage task is asked to do.
#[derive(rant::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    Run,
    Stop,
    Restart,
}

/// A call to Manage: a name from its list and what to do with it.
#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct ManageRequest {
    pub name: String,
    pub action: Action,
}

#[derive(rant::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProgressStatus {
    Starting,
    Running,
    Stopped,
    Failed,
}

/// A node a Manage task touched, as the CLI reports it.
#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct NodeProgress {
    /// Its name on the mesh.
    pub name: String,
    pub status: ProgressStatus,
    /// The CLI's words, such as started, already running or exited after 0.3 s.
    pub note: String,
}

#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct GroupState {
    /// What the CLI takes: package/Group, or Group outside every package.
    pub name: String,
    pub description: String,
    pub status: RunningStatus,
    /// The nodes its own file declares.
    pub nodes: Vec<String>,
    /// The groups it includes.
    pub groups: Vec<String>,
}

#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct NodeState {
    /// What the CLI takes: package/node, or node outside every package.
    pub name: String,
    pub status: RunningStatus,
    /// The groups whose own files declare it.
    pub groups: Vec<String>,
}
