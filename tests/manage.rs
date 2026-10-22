//! End to end: a scratch workspace whose package `manager` runs this build through the real
//! Manager.run, started with rant run and driven over the mesh on loopback only.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use rant::types::Empty;
use rant::{CallOptions, FunctionOptions, Node, NodeOptions, RemoteFunction, VariableOptions};
use rant_manager::{Action, GroupState, NodeState, RunningStatus};

/// Set on every rant command the test runs, so the nodes those start inherit it.
const SLEEPER: &str = "MANAGER_TEST_SLEEPER";

/// Not a test: the program the scratch workspace's nodes run, which sleeps until stopped.
#[test]
#[ignore]
fn sleeper() {
    if std::env::var_os(SLEEPER).is_some() {
        // a failed test cannot leave it running long
        std::thread::sleep(Duration::from_secs(120));
    }
}

fn rant_cli() -> PathBuf {
    std::env::var_os("RANT_CLI").map_or_else(|| "rant".into(), PathBuf::from)
}

/// HCL takes a backslash as an escape, and rant takes forward slashes on every OS.
fn slashed(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

struct Workspace {
    root: PathBuf,
    home: PathBuf,
}

impl Workspace {
    /// A workspace folder `cell` on domain with two sleepers in group io/Ft, and the manager.
    fn new(test: &str, domain: u16) -> Workspace {
        let scratch = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let (root, home) = (scratch.join("cell"), scratch.join("home"));
        let sleeper = format!("'{}' --exact sleeper --ignored --nocapture", slashed(&std::env::current_exe().unwrap()));
        let manager = format!("'{}' --interface 127.0.0.1 --rant '{}'", slashed(Path::new(env!("CARGO_BIN_EXE_rant-manager"))), slashed(&rant_cli()));
        let files = [
            ("workspace.rant", format!("domain = {domain}\n")),
            (
                "io/package.rant",
                format!("node \"sleeper\" {{\n  run = \"{sleeper}\"\n  expect_mesh = false\n}}\nnode \"napper\" {{\n  run = \"{sleeper}\"\n  expect_mesh = false\n}}\n"),
            ),
            ("io/Ft.run", "description = \"Two sleepers\"\nnode \"io/sleeper\" {}\nnode \"io/napper\" {}\n".into()),
            // the manager joins on loopback, where rant run would never see it come up
            ("manager/package.rant", format!("node \"manager\" {{\n  run = \"{manager}\"\n  expect_mesh = false\n}}\n")),
            ("manager/Manager.run", std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Manager.run")).unwrap()),
        ];
        for (file, text) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        std::fs::create_dir_all(&home).unwrap();
        Workspace { root, home }
    }

    fn rant(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new(rant_cli())
            .args(args)
            .current_dir(&self.root)
            .env("RANT_HOME", &self.home)
            .env(SLEEPER, "1")
            .output()
            .expect("rant runs, set RANT_CLI to its path when it is not on PATH");
        let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
        if out.status.success() { Ok(text) } else { Err(text) }
    }

    /// The mesh names of the nodes this workspace's rant started that run now.
    fn running(&self) -> Vec<String> {
        let json: serde_json::Value = serde_json::from_str(&self.rant(&["ls", "nodes", "--no-mesh", "--json"]).unwrap()).unwrap();
        let runs = json["groups"].as_array().unwrap().iter().flat_map(|r| r["nodes"].as_array().unwrap());
        let mut names: Vec<String> = runs.chain(json["nodes"].as_array().unwrap()).map(|n| n["name"].as_str().unwrap().to_string()).collect();
        names.sort();
        names.dedup();
        names
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = self.rant(&["stop", "-y"]);
    }
}

fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    while !ok() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn tester(domain: u16) -> Node {
    Node::new("tester", NodeOptions::new().domain(domain).multicast_interface("127.0.0.1")).unwrap()
}

fn manage(node: &Node, path: &str) -> RemoteFunction<Action, Empty> {
    // a call ends at its handle's timeout, whatever wait the call itself asks for
    let f = node.remote_function::<Action, Empty>(path, FunctionOptions::new().timeout(Duration::from_secs(30))).unwrap();
    wait_for(path, || f.match_count() > 0);
    f
}

fn call(f: &RemoteFunction<Action, Empty>, action: Action) -> Result<(), String> {
    let r = f.call(&action, CallOptions::new()).unwrap();
    if r.is_ok() { Ok(()) } else { Err(r.message().to_string()) }
}

#[test]
fn manage_runs_stops_and_restarts_groups_and_nodes() {
    let ws = Workspace::new("manage", 180);
    ws.rant(&["run", "Manager", "--no-build", "-y"]).unwrap();
    let node = tester(180);

    // the prefix is the workspace folder's name, as no placement sets one
    let groups = node.remote_variable::<Vec<GroupState>>("Cell/Rant/Groups", VariableOptions::new()).unwrap();
    wait_for("the group list", || groups.get().unwrap().is_some_and(|l| l.len() == 2));
    let ft = groups.get().unwrap().unwrap().into_iter().find(|g| g.name == "io/Ft").unwrap();
    assert_eq!(ft.manage_path, "Cell/Rant/Groups/Io/Ft/Manage");
    assert_eq!(ft.description, "Two sleepers");
    assert_eq!(ft.status, RunningStatus::Stopped);

    let group = node.remote_variable::<GroupState>("Cell/Rant/Groups/Io/Ft", VariableOptions::new()).unwrap();
    let sleeper = node.remote_variable::<NodeState>("Cell/Rant/Nodes/Io/Sleeper", VariableOptions::new()).unwrap();
    let group_is = |s| group.get().unwrap().is_some_and(|g| g.status == s);
    let sleeper_is = |s| sleeper.get().unwrap().is_some_and(|n| n.status == s);

    let ft = manage(&node, "Cell/Rant/Groups/Io/Ft/Manage");
    call(&ft, Action::Run).unwrap();
    wait_for("io/Ft running", || group_is(RunningStatus::Running) && sleeper_is(RunningStatus::Running));
    assert_eq!(ws.running(), ["manager", "napper", "sleeper"]);

    let one = manage(&node, "Cell/Rant/Nodes/Io/Sleeper/Manage");
    call(&one, Action::Restart).unwrap();
    let log = ws.root.join("logs/sleeper.log.1");
    assert!(log.is_file(), "a restart starts a new log and keeps the last");
    assert!(sleeper_is(RunningStatus::Running));

    call(&one, Action::Stop).unwrap();
    wait_for("io/sleeper stopped", || sleeper_is(RunningStatus::Stopped));
    assert!(group_is(RunningStatus::Running), "io/Ft still holds napper");
    assert_eq!(ws.running(), ["manager", "napper"]);

    call(&ft, Action::Stop).unwrap();
    wait_for("io/Ft stopped", || group_is(RunningStatus::Stopped));
    let again = call(&ft, Action::Stop).unwrap_err();
    assert!(again.contains("not running"), "{again}");

    // a node run alone stays outside the group
    call(&one, Action::Run).unwrap();
    wait_for("io/sleeper alone", || sleeper_is(RunningStatus::Running));
    assert!(group_is(RunningStatus::Stopped));
    call(&one, Action::Stop).unwrap();

    let me = manage(&node, "Cell/Rant/Groups/Manager/Manager/Manage");
    let refused = call(&me, Action::Restart).unwrap_err();
    assert!(refused.contains("holds the manager itself"), "{refused}");
    assert_eq!(ws.running(), ["manager"]);
}

#[test]
fn stopping_the_manager_leaves_the_nodes_it_started() {
    let ws = Workspace::new("stop", 181);
    ws.rant(&["run", "Manager", "--no-build", "-y"]).unwrap();
    let node = tester(181);
    call(&manage(&node, "Cell/Rant/Groups/Io/Ft/Manage"), Action::Run).unwrap();
    assert_eq!(ws.running(), ["manager", "napper", "sleeper"]);
    ws.rant(&["stop", "Manager", "-y"]).unwrap();
    assert_eq!(ws.running(), ["napper", "sleeper"]);
}
