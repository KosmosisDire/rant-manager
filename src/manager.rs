//! The node: the group and node lists, a Manage task beside each, and the loop that keeps the
//! lists in step with the CLI and runs each Manage call.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rant::types::Empty;
use rant::{Level, Node, NodeOptions, PendingTask, StatusOptions, TaskDefinition, TaskOptions, TaskRequest, VariableDefinition, VariableOptions};

use crate::cli::Cli;
use crate::listing::{self, Listing};
use crate::names;
use crate::progress::Progress;
use crate::types::{Action, GroupState, ManageRequest, NodeProgress, NodeState, RunningStatus};

/// How often the CLI is asked what runs, so changes made elsewhere show.
const POLL: Duration = Duration::from_secs(2);

pub struct Options {
    pub workspace: PathBuf,
    pub rant: PathBuf,
    pub interface: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Group,
    Node,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Group => "group",
            Kind::Node => "node",
        }
    }
}

type Manage = TaskDefinition<ManageRequest, Vec<NodeProgress>, Empty>;
type Pending = PendingTask<Vec<NodeProgress>, Empty>;

enum Event {
    Manage { kind: Kind, name: String, action: Action, task: Pending },
    Quit,
}

fn status(running: bool) -> RunningStatus {
    if running { RunningStatus::Running } else { RunningStatus::Stopped }
}

fn group_state(g: &listing::Group) -> GroupState {
    GroupState {
        name: g.name.clone(),
        description: g.description.clone(),
        status: status(!g.copies.is_empty()),
        nodes: g.nodes.clone(),
        groups: g.groups.clone(),
    }
}

fn node_state(n: &listing::Node) -> NodeState {
    NodeState { name: n.name.clone(), status: status(!n.copies.is_empty()), groups: n.groups.clone() }
}

/// A list variable, set only when its rows change.
struct List<T: rant::Message> {
    var: VariableDefinition<Vec<T>>,
    shown: Vec<T>,
}

impl<T: rant::Message + PartialEq> List<T> {
    fn new(node: &Node, path: &str) -> Result<Self, rant::Error> {
        let var = node.variable_definition(path, VariableOptions::new().read_only(true).initial(Vec::new()))?;
        Ok(List { var, shown: Vec::new() })
    }

    fn show(&mut self, rows: Vec<T>) {
        if rows != self.shown {
            let _ = self.var.set(&rows);
            self.shown = rows;
        }
    }
}

fn manage_task(node: &Node, path: &str, kind: Kind, events: &Sender<Event>) -> Result<Manage, rant::Error> {
    let events = events.clone();
    let handler = move |req: TaskRequest<ManageRequest, Vec<NodeProgress>, Empty>| {
        let (name, action) = (req.name.clone(), req.action);
        let _ = events.send(Event::Manage { kind, name, action, task: req.defer() });
    };
    // a CLI cut short could leave a node it started untracked
    node.task_definition(path, TaskOptions::new().no_cancel(true), handler)
}

pub struct Manager {
    node: Node,
    cli: Cli,
    /// What every name the node makes starts with on the mesh.
    prefix: String,
    /// The manager's own name on the mesh, as the CLI lists the copy it started.
    me: String,
    events: Sender<Event>,
    inbox: Receiver<Event>,
    listing: Listing,
    /// Why the CLI last failed to list, so a lasting failure is logged once.
    failure: Option<String>,
    groups: List<GroupState>,
    nodes: List<NodeState>,
    _manage: [Manage; 2],
}

impl Manager {
    pub fn new(o: Options) -> Result<Manager, rant::Error> {
        let placed = std::env::var("RANT_PREFIX").unwrap_or_default();
        let folder = o.workspace.join("workspace.rant").is_file().then(|| o.workspace.file_name()).flatten();
        let host = gethostname::gethostname();
        let own = names::own_prefix(&placed, folder.map(|f| f.to_string_lossy()).as_deref(), &host.to_string_lossy());

        let mut opts = NodeOptions::new();
        if let Some(p) = &own {
            opts = opts.prefix(p.as_str());
        }
        if let Some(ip) = &o.interface {
            opts = opts.multicast_interface(ip.as_str());
        }
        let node = Node::new("manager", opts)?;
        let (events, inbox) = mpsc::channel();
        Ok(Manager {
            groups: List::new(&node, "Rant/Groups")?,
            nodes: List::new(&node, "Rant/Nodes")?,
            _manage: [manage_task(&node, "Rant/Groups/Manage", Kind::Group, &events)?, manage_task(&node, "Rant/Nodes/Manage", Kind::Node, &events)?],
            node,
            cli: Cli { exe: o.rant, workspace: o.workspace },
            prefix: own.unwrap_or(placed),
            me: std::env::var("RANT_NODE_NAME").unwrap_or_else(|_| "manager".into()),
            events,
            inbox,
            listing: Listing::default(),
            failure: None,
        })
    }

    /// Ends run from any thread, such as a signal handler's.
    pub fn quitter(&self) -> impl FnMut() + Send + 'static {
        let events = self.events.clone();
        move || {
            let _ = events.send(Event::Quit);
        }
    }

    pub fn run(mut self) {
        println!("managing {} as {}/Rant", self.cli.workspace.display(), self.prefix);
        self.refresh();
        let mut next = Instant::now() + POLL;
        loop {
            match self.inbox.recv_timeout(next.saturating_duration_since(Instant::now())) {
                Ok(Event::Manage { kind, name, action, task }) => {
                    let done = self.manage(kind, &name, action, &task);
                    // the lists show the outcome before the caller hears it
                    self.refresh();
                    next = Instant::now() + POLL;
                    let _ = match done {
                        Ok(()) => {
                            println!("{action:?} {} {name}", kind.word());
                            task.complete(&Empty {})
                        }
                        Err(why) => {
                            println!("{action:?} {} {name} failed: {why}", kind.word());
                            task.fail(&why)
                        }
                    };
                }
                Ok(Event::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {
                    self.refresh();
                    next = Instant::now() + POLL;
                }
            }
        }
    }

    fn manage(&self, kind: Kind, name: &str, action: Action, task: &Pending) -> Result<(), String> {
        let copies = match kind {
            Kind::Group => self.listing.groups.iter().find(|g| g.name == name).map(|g| &g.copies),
            Kind::Node => self.listing.nodes.iter().find(|n| n.name == name).map(|n| &n.copies),
        };
        let Some(copies) = copies else {
            return Err(format!("no {} `{name}`, see `rant ls {}s`", kind.word(), kind.word()));
        };
        // its answer would never arrive, and a stop would end it for good
        if action != Action::Run && copies.contains(&self.me) {
            return Err(format!("the manager cannot stop or restart `{name}`, which holds the manager itself"));
        }
        // each node's row goes to the caller as the CLI prints it, and a failed node says why
        let mut progress = Progress::default();
        let mut rant = |args: &[&str]| {
            let args = [args, &["-y", "--json", "--lines"][..]].concat();
            let done = self.cli.stream(&args, |line| match progress.take(line) {
                Ok(()) => {
                    let _ = task.progress(&progress.nodes);
                }
                Err(why) => eprintln!("{why}"),
            });
            done.map(|_| ()).map_err(|why| progress.failed().unwrap_or(why))
        };
        let mut act = |action: Action, target: &str| match action {
            Action::Run if kind == Kind::Node => rant(&["run", "node", target, "--no-build"]),
            Action::Run => rant(&["run", target, "--no-build"]),
            Action::Stop => rant(&["stop", target]),
            Action::Restart => rant(&["restart", target, "--no-build"]),
        };
        if kind == Kind::Group || action == Action::Run {
            return act(action, name);
        }
        // stop and restart take a running copy by its mesh name, and a node may run as many
        match action {
            Action::Stop if copies.is_empty() => Err(format!("node `{name}` is not running, see `rant ls`")),
            // as rant restart starts a group that is not running
            Action::Restart if copies.is_empty() => act(Action::Run, name),
            _ => copies.iter().try_for_each(|c| act(action, c)),
        }
    }

    /// Asks the CLI what there is and brings both lists in line with it.
    fn refresh(&mut self) {
        let read = self
            .cli
            .run(&["ls", "groups", "--json"])
            .and_then(|g| self.cli.run(&["ls", "nodes", "--no-mesh", "--json"]).map(|n| (g, n)))
            .and_then(|(g, n)| listing::parse(&g, &n));
        match read {
            Ok(l) => {
                if self.failure.take().is_some() {
                    let _ = self.node.clear_status("cli");
                }
                self.listing = l;
            }
            Err(why) => {
                if self.failure.as_ref() != Some(&why) {
                    eprintln!("{why}");
                    let _ = self.node.set_status("cli", Level::Error, &why, StatusOptions::new());
                    self.failure = Some(why);
                }
                return;
            }
        }
        self.groups.show(self.listing.groups.iter().map(group_state).collect());
        self.nodes.show(self.listing.nodes.iter().map(node_state).collect());
    }
}
