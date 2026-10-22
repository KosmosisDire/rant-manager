//! What the manager offers on the mesh.

#[derive(rant::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RunningStatus {
    Stopped,
    Running,
}

/// What a Manage function is asked to do.
#[derive(rant::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    Run,
    Stop,
    Restart,
}

#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct GroupState {
    /// What the CLI takes: package/Group, or Group outside every package.
    pub name: rant::String<64>,
    /// The full name of the group's Manage function.
    pub manage_path: rant::String<64>,
    pub description: rant::String<128>,
    pub status: RunningStatus,
}

#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct NodeState {
    /// What the CLI takes: package/node, or node outside every package.
    pub name: rant::String<64>,
    /// The full name of the node's Manage function.
    pub manage_path: rant::String<64>,
    pub status: RunningStatus,
}
