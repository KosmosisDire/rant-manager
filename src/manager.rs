//! The node: the group and node lists, a Manage function beside each, and the loop that keeps
//! the lists in step with the CLI and runs each Manage call.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rant::types::Empty;
use rant::{Deferred, FunctionDefinition, FunctionOptions, Level, Node, NodeOptions, Request, StatusOptions, VariableDefinition, VariableOptions};

use crate::cli::Cli;
use crate::listing::{self, Listing};
use crate::names;
use crate::types::{Action, GroupState, MEMBERS, ManageRequest, Name, NodeState, RunningStatus};

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

enum Event {
    Manage { kind: Kind, name: String, action: Action, answer: Deferred<Empty> },
    Quit,
}

fn status(running: bool) -> RunningStatus {
    if running { RunningStatus::Running } else { RunningStatus::Stopped }
}

/// The longest start of s that fits in max bytes, cut between characters.
fn cut(s: &str, max: usize) -> &str {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// A row's name, or none with the name in skipped when it does not fit.
fn row_name(name: &str, skipped: &mut Vec<String>) -> Option<rant::String<64>> {
    let fits = name.try_into().ok();
    if fits.is_none() {
        skipped.push(format!("{name} (over 64 bytes)"));
    }
    fits
}

/// A row's fixed list of names. Those past its end are named in skipped, and a name too long
/// for a row of its own is left out, as its row is.
fn members(owner: &str, what: &str, names: &[String], skipped: &mut Vec<String>) -> [Name; MEMBERS] {
    let fit: Vec<Name> = names.iter().filter_map(|n| Some(Name { name: n.as_str().try_into().ok()? })).collect();
    if fit.len() > MEMBERS {
        skipped.push(format!("the {what} of {owner} past the first {MEMBERS}"));
    }
    let mut out = [Name::default(); MEMBERS];
    out.iter_mut().zip(fit).for_each(|(slot, n)| *slot = n);
    out
}

fn group_state(g: &listing::Group, skipped: &mut Vec<String>) -> Option<GroupState> {
    Some(GroupState {
        name: row_name(&g.name, skipped)?,
        description: cut(&g.description, 128).try_into().ok()?,
        status: status(!g.copies.is_empty()),
        nodes: members(&g.name, "nodes", &g.nodes, skipped),
        groups: members(&g.name, "groups", &g.groups, skipped),
    })
}

fn node_state(n: &listing::Node, skipped: &mut Vec<String>) -> Option<NodeState> {
    Some(NodeState { name: row_name(&n.name, skipped)?, status: status(!n.copies.is_empty()), groups: members(&n.name, "groups", &n.groups, skipped) })
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

fn manage_function(node: &Node, path: &str, kind: Kind, events: &Sender<Event>) -> Result<FunctionDefinition<ManageRequest, Empty>, rant::Error> {
    let events = events.clone();
    let handler = move |req: Request<ManageRequest, Empty>| {
        let (name, action) = (req.name.to_string(), req.action);
        let _ = events.send(Event::Manage { kind, name, action, answer: req.defer() });
    };
    node.function_definition(path, FunctionOptions::new(), handler)
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
    _manage: [FunctionDefinition<ManageRequest, Empty>; 2],
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
            _manage: [manage_function(&node, "Rant/Groups/Manage", Kind::Group, &events)?, manage_function(&node, "Rant/Nodes/Manage", Kind::Node, &events)?],
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
                Ok(Event::Manage { kind, name, action, answer }) => {
                    let done = self.manage(kind, &name, action);
                    // the lists show the outcome before the caller hears it
                    self.refresh();
                    next = Instant::now() + POLL;
                    let _ = match done {
                        Ok(()) => {
                            println!("{action:?} {} {name}", kind.word());
                            answer.complete(&Empty {})
                        }
                        Err(why) => {
                            println!("{action:?} {} {name} failed: {why}", kind.word());
                            answer.fail(&why)
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

    fn manage(&mut self, kind: Kind, name: &str, action: Action) -> Result<(), String> {
        let act = |target: &str| match action {
            Action::Run if kind == Kind::Node => self.cli.run(&["run", "node", target, "--no-build", "-y"]),
            Action::Run => self.cli.run(&["run", target, "--no-build", "-y"]),
            Action::Stop => self.cli.run(&["stop", target, "-y"]),
            Action::Restart => self.cli.run(&["restart", target, "--no-build", "-y"]),
        };
        let copies = match kind {
            Kind::Group => self.listing.groups.iter().find(|g| g.name == name).map(|g| g.copies.clone()),
            Kind::Node => self.listing.nodes.iter().find(|n| n.name == name).map(|n| n.copies.clone()),
        };
        let Some(copies) = copies else {
            return Err(format!("no {} `{name}`, see `rant ls {}s`", kind.word(), kind.word()));
        };
        // its answer would never arrive, and a stop would end it for good
        if action != Action::Run && copies.contains(&self.me) {
            return Err(format!("the manager cannot stop or restart `{name}`, which holds the manager itself"));
        }
        if kind == Kind::Group || action == Action::Run {
            return act(name).map(|_| ());
        }
        // stop and restart take a running copy by its mesh name, and a node may run as many
        match action {
            Action::Stop if copies.is_empty() => Err(format!("node `{name}` is not running, see `rant ls`")),
            // as rant restart starts a group that is not running
            Action::Restart if copies.is_empty() => self.cli.run(&["run", "node", name, "--no-build", "-y"]).map(|_| ()),
            _ => copies.iter().try_for_each(|c| act(c).map(|_| ())),
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
        let mut skipped = Vec::new();
        let groups = self.listing.groups.iter().filter_map(|g| group_state(g, &mut skipped)).collect();
        let nodes = self.listing.nodes.iter().filter_map(|n| node_state(n, &mut skipped)).collect();
        self.groups.show(groups);
        self.nodes.show(nodes);
        if skipped.is_empty() {
            let _ = self.node.clear_status("skipped");
        } else {
            let text = format!("cannot offer {}", skipped.join(", "));
            let _ = self.node.set_status("skipped", Level::Warn, &text, StatusOptions::new());
        }
    }
}
