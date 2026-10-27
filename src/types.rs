//! What the manager offers on the mesh.

/// How many names a row's lists hold. A row's size is fixed, as it is an array element.
pub const MEMBERS: usize = 32;

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

/// A call to Manage: a name from its list and what to do with it.
#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct ManageRequest {
    pub name: rant::String<64>,
    pub action: Action,
}

/// A group or node in another row's list, empty in an unused slot.
#[derive(rant::Message, Clone, Copy, Debug, Default, PartialEq)]
pub struct Name {
    pub name: rant::String<64>,
}

#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct GroupState {
    /// What the CLI takes: package/Group, or Group outside every package.
    pub name: rant::String<64>,
    pub description: rant::String<128>,
    pub status: RunningStatus,
    /// The nodes its own file declares.
    pub nodes: [Name; MEMBERS],
    /// The groups it includes.
    pub groups: [Name; MEMBERS],
}

#[derive(rant::Message, Clone, Debug, PartialEq)]
pub struct NodeState {
    /// What the CLI takes: package/node, or node outside every package.
    pub name: rant::String<64>,
    pub status: RunningStatus,
    /// The groups whose own files declare it.
    pub groups: [Name; MEMBERS],
}
