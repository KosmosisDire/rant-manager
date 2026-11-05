---
name: manager
description: The Rant manager, which offers a workspace's groups and nodes on the mesh with a Manage task to run, stop or restart them and follow each node as it starts. Use when a program or an HMI should see which Rant groups and nodes run, or run, stop or restart them.
---

# Rant manager

The package `manager` offers a workspace's groups and nodes on the mesh, so a program or an
HMI can see which run and run, stop or restart them.

| goal | command |
|---|---|
| install | `rant install manager` |
| run it | `rant run Manager` in the workspace, `rant stop Manager` |
| output | `rant log Manager`, `-f` to follow |

- Names start with the workspace's prefix, else its folder name in PascalCase (`cell` is
  `Cell`). `Cell/Rant/Groups` lists every group as `GroupState[]`, `Cell/Rant/Nodes` every
  node as `NodeState[]`, each row with its `name`, as the CLI takes it, and its `status`
  (Stopped, Running).
- A group's row lists the nodes its own file declares and the groups it includes, a node's
  the groups whose files declare it, as `string[]`.
- `Cell/Rant/Groups/Manage` and `Cell/Rant/Nodes/Manage` are tasks that take a
  `ManageRequest { name, action }`, the name from the list beside it and the action Run, Stop
  or Restart.
- Each progress update is a `NodeProgress[]`: every node the call touched so far by its mesh
  name, its `status` (Starting, Running, Stopped, Failed) and a `note` in the CLI's words.
- It never builds and passes no params. A cancel is refused. Stopping the manager leaves
  running what it started.
