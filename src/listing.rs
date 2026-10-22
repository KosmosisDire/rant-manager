//! The groups and nodes of a workspace, read from `rant ls groups --json` and
//! `rant ls nodes --no-mesh --json`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    pub description: String,
    /// The mesh names of the running nodes it holds, its own runs' and those its blocks made.
    pub copies: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    /// The mesh names of its running copies, which rant stop and restart take.
    pub copies: Vec<String>,
}

fn add(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|n| n == name) {
        names.push(name.to_string());
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listing {
    pub groups: Vec<Group>,
    pub nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct GroupJson {
    name: String,
    description: String,
}

#[derive(Deserialize)]
struct NodesJson {
    groups: Vec<RunJson>,
    nodes: Vec<CopyJson>,
    not_running: Vec<IdleJson>,
}

#[derive(Deserialize)]
struct RunJson {
    group: String,
    nodes: Vec<CopyJson>,
}

#[derive(Deserialize)]
struct CopyJson {
    name: String,
    #[serde(default, rename = "type")]
    node_type: String,
    #[serde(default)]
    group: String,
}

#[derive(Deserialize)]
struct IdleJson {
    name: String,
}

/// The CLI calls a node outside every package by its file, but takes its file name too.
fn node_name(node_type: &str) -> String {
    let p = Path::new(node_type);
    match p.file_stem() {
        Some(stem) if p.is_absolute() => stem.to_string_lossy().into_owned(),
        _ => node_type.to_string(),
    }
}

pub fn parse(groups_json: &str, nodes_json: &str) -> Result<Listing, String> {
    let groups: Vec<GroupJson> = serde_json::from_str(groups_json).map_err(|e| format!("rant ls groups: {e}"))?;
    let nodes: NodesJson = serde_json::from_str(nodes_json).map_err(|e| format!("rant ls nodes: {e}"))?;

    // a run shows "Nav speed=2" when the group runs twice, and an included group runs
    // inside the run that includes it, named on each node its own blocks made
    let mut by_group: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for run in &nodes.groups {
        let root = run.group.split(' ').next().unwrap_or_default();
        for c in &run.nodes {
            add(by_group.entry(root).or_default(), &c.name);
        }
    }
    let copies: Vec<&CopyJson> = nodes.groups.iter().flat_map(|r| &r.nodes).chain(&nodes.nodes).collect();
    for c in copies.iter().filter(|c| !c.group.is_empty()) {
        add(by_group.entry(&c.group).or_default(), &c.name);
    }

    // a node two runs share shows under both
    let mut by_node: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in copies.iter().filter(|c| !c.node_type.is_empty()) {
        add(by_node.entry(node_name(&c.node_type)).or_default(), &c.name);
    }
    for idle in &nodes.not_running {
        by_node.entry(idle.name.clone()).or_default();
    }

    Ok(Listing {
        groups: groups
            .into_iter()
            .map(|g| Group { copies: by_group.get(g.name.as_str()).cloned().unwrap_or_default(), name: g.name, description: g.description })
            .collect(),
        nodes: by_node.into_iter().map(|(name, copies)| Node { name, copies }).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUPS: &str = r#"[
        { "name": "Top", "file": "/w/Top.run", "description": "Top level", "params": "", "includes": ["io/Ft"], "nodes": [] },
        { "name": "io/Ft", "file": "/w/io/Ft.run", "description": "Two sleepers", "params": "", "includes": [], "nodes": [] },
        { "name": "io/Idle", "file": "/w/io/Idle.run", "description": "", "params": "", "includes": [], "nodes": [] }
    ]"#;

    #[test]
    fn a_run_marks_its_group_and_the_groups_it_includes() {
        let nodes = r#"{
            "domain": 151,
            "groups": [ { "group": "Top", "nodes": [
                { "name": "sleeper", "type": "io/sleeper", "domain": 151, "group": "io/Ft" },
                { "name": "other_waiter", "type": "io/waiter", "domain": 151, "group": "io/Ft" }
            ] } ],
            "nodes": [],
            "not_running": [ { "name": "io/napper", "kind": "declared", "path": null } ]
        }"#;
        let l = parse(GROUPS, nodes).unwrap();
        let held: Vec<(&str, Vec<String>)> = l.groups.iter().map(|g| (g.name.as_str(), g.copies.clone())).collect();
        let both = vec!["sleeper".to_string(), "other_waiter".to_string()];
        assert_eq!(held, [("Top", both.clone()), ("io/Ft", both), ("io/Idle", vec![])]);
        assert_eq!(l.groups[1].description, "Two sleepers");
        assert_eq!(
            l.nodes,
            [
                Node { name: "io/napper".into(), copies: vec![] },
                Node { name: "io/sleeper".into(), copies: vec!["sleeper".into()] },
                Node { name: "io/waiter".into(), copies: vec!["other_waiter".into()] },
            ]
        );
    }

    #[test]
    fn copies_gather_under_their_node_once() {
        let nodes = r#"{
            "domain": 0,
            "groups": [
                { "group": "Nav speed=1", "nodes": [ { "name": "left_planner", "type": "arm/planner", "group": "Nav" } ] },
                { "group": "Nav speed=2", "nodes": [ { "name": "right_planner", "type": "arm/planner", "group": "Nav" } ] },
                { "group": "Both", "nodes": [ { "name": "left_planner", "type": "arm/planner", "group": "Nav" } ] }
            ],
            "nodes": [ { "name": "solo", "type": "arm/solo" } ],
            "not_running": []
        }"#;
        let l = parse("[]", nodes).unwrap();
        assert_eq!(
            l.nodes,
            [
                Node { name: "arm/planner".into(), copies: vec!["left_planner".into(), "right_planner".into()] },
                Node { name: "arm/solo".into(), copies: vec!["solo".into()] },
            ]
        );
    }

    #[test]
    fn a_node_outside_every_package_goes_by_its_file_name() {
        let file = if cfg!(windows) { "C:/w/tools/probe.py" } else { "/w/tools/probe.py" };
        let nodes = format!(r#"{{ "domain": 0, "groups": [], "nodes": [ {{ "name": "probe", "type": "{file}" }} ], "not_running": [] }}"#);
        let l = parse("[]", &nodes).unwrap();
        assert_eq!(l.nodes, [Node { name: "probe".into(), copies: vec!["probe".into()] }]);
    }
}
