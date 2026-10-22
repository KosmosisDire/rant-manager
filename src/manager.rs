//! The node: one variable and one Manage function per group and per node, the two lists, and
//! the loop that keeps them in step with the CLI and runs each Manage call.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rant::types::Empty;
use rant::{Deferred, FunctionDefinition, FunctionOptions, Level, Node, NodeOptions, Request, StatusOptions, VariableDefinition, VariableOptions};

use crate::cli::Cli;
use crate::listing::{self, Listing};
use crate::names;
use crate::types::{Action, GroupState, NodeState, RunningStatus};

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

enum Event {
    Manage { kind: Kind, name: String, action: Action, answer: Deferred<Empty> },
    Quit,
}

/// A group or node as the CLI lists it.
struct Item {
    name: String,
    description: String,
    running: bool,
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

fn group_state(i: &Item, manage_path: &str) -> Result<GroupState, rant::Error> {
    Ok(GroupState {
        name: i.name.as_str().try_into()?,
        manage_path: manage_path.try_into()?,
        description: cut(&i.description, 128).try_into()?,
        status: status(i.running),
    })
}

fn node_state(i: &Item, manage_path: &str) -> Result<NodeState, rant::Error> {
    Ok(NodeState { name: i.name.as_str().try_into()?, manage_path: manage_path.try_into()?, status: status(i.running) })
}

struct Entry<T: rant::Message> {
    state: T,
    var: VariableDefinition<T>,
    _manage: FunctionDefinition<Action, Empty>,
}

/// One kind's entities: an entry per group or node, and the list of all of them.
struct Table<T: rant::Message> {
    kind: Kind,
    root: &'static str,
    make: fn(&Item, &str) -> Result<T, rant::Error>,
    entries: BTreeMap<String, Entry<T>>,
    list: VariableDefinition<Vec<T>>,
    shown: Vec<T>,
}

impl<T: rant::Message + Clone + PartialEq> Table<T> {
    fn new(node: &Node, kind: Kind, root: &'static str, make: fn(&Item, &str) -> Result<T, rant::Error>) -> Result<Self, rant::Error> {
        let list = node.variable_definition(root, VariableOptions::new().read_only(true).initial(Vec::new()))?;
        Ok(Table { kind, root, make, entries: BTreeMap::new(), list, shown: Vec::new() })
    }

    /// Makes, updates and drops entries to match items. One that cannot be offered goes into
    /// skipped with the reason.
    fn sync(&mut self, node: &Node, events: &Sender<Event>, prefix: &str, items: &[Item], skipped: &mut Vec<String>) {
        let mut paths: BTreeMap<String, &str> = BTreeMap::new();
        let mut list = Vec::new();
        for i in items {
            let path = names::entity(self.root, &i.name);
            if let Some(first) = paths.get(&path) {
                skipped.push(format!("{} (the same path as {first})", i.name));
                continue;
            }
            paths.insert(path.clone(), &i.name);
            match self.offer(node, events, prefix, i, &path) {
                Ok(state) => list.push(state),
                Err(why) => skipped.push(format!("{} ({why})", i.name)),
            }
        }
        self.entries.retain(|name, _| paths.values().any(|n| n == name));
        if list != self.shown {
            let _ = self.list.set(&list);
            self.shown = list;
        }
    }

    /// Sets the entry's variable when its state changed, or makes its variable and function.
    fn offer(&mut self, node: &Node, events: &Sender<Event>, prefix: &str, item: &Item, path: &str) -> Result<T, String> {
        let manage = format!("{path}/Manage");
        let state = (self.make)(item, &names::full(prefix, &manage)).map_err(|e| e.to_string())?;
        if let Some(e) = self.entries.get_mut(&item.name) {
            if e.state != state {
                e.var.set(&state).map_err(|e| e.to_string())?;
                e.state = state.clone();
            }
            return Ok(state);
        }
        let var = node.variable_definition(path, VariableOptions::new().read_only(true).initial(state.clone())).map_err(|e| e.to_string())?;
        let (events, kind, name) = (events.clone(), self.kind, item.name.clone());
        let handler = move |req: Request<Action, Empty>| {
            let action = *req;
            let _ = events.send(Event::Manage { kind, name: name.clone(), action, answer: req.defer() });
        };
        let manage = node.function_definition(&manage, FunctionOptions::new(), handler).map_err(|e| e.to_string())?;
        self.entries.insert(item.name.clone(), Entry { state: state.clone(), var, _manage: manage });
        Ok(state)
    }
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
    groups: Table<GroupState>,
    nodes: Table<NodeState>,
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
        let groups = Table::new(&node, Kind::Group, "Rant/Groups", group_state)?;
        let nodes = Table::new(&node, Kind::Node, "Rant/Nodes", node_state)?;
        let (events, inbox) = mpsc::channel();
        Ok(Manager {
            node,
            cli: Cli { exe: o.rant, workspace: o.workspace },
            prefix: own.unwrap_or(placed),
            me: std::env::var("RANT_NODE_NAME").unwrap_or_else(|_| "manager".into()),
            events,
            inbox,
            listing: Listing::default(),
            failure: None,
            groups,
            nodes,
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
                    // the variables show the outcome before the caller hears it
                    self.refresh();
                    next = Instant::now() + POLL;
                    let _ = match done {
                        Ok(()) => {
                            println!("{action:?} {name}");
                            answer.complete(&Empty {})
                        }
                        Err(why) => {
                            println!("{action:?} {name} failed: {why}");
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
        let copies = copies.unwrap_or_default();
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

    /// Asks the CLI what there is and brings every entity in line with it.
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
        let groups: Vec<Item> = self
            .listing
            .groups
            .iter()
            .map(|g| Item { name: g.name.clone(), description: g.description.clone(), running: !g.copies.is_empty() })
            .collect();
        let nodes: Vec<Item> =
            self.listing.nodes.iter().map(|n| Item { name: n.name.clone(), description: String::new(), running: !n.copies.is_empty() }).collect();
        let mut skipped = Vec::new();
        self.groups.sync(&self.node, &self.events, &self.prefix, &groups, &mut skipped);
        self.nodes.sync(&self.node, &self.events, &self.prefix, &nodes, &mut skipped);
        if skipped.is_empty() {
            let _ = self.node.clear_status("skipped");
        } else {
            let text = format!("cannot offer {}", skipped.join(", "));
            let _ = self.node.set_status("skipped", Level::Warn, &text, StatusOptions::new());
        }
    }
}
