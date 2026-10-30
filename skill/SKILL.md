---
name: manager
description: The Rant manager, which offers a workspace's groups and nodes on the mesh with a Manage function to run, stop or restart them. Use when a program or an HMI should see which Rant groups and nodes run, or run, stop or restart them.
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
  the groups whose files declare it, up to 32 names each in fixed `Name[32]` lists, unused
  slots empty.
- `Cell/Rant/Groups/Manage` and `Cell/Rant/Nodes/Manage` take a `ManageRequest { name,
  action }`, the name from the list beside it and the action Run, Stop or Restart.
- Run and Restart wait up to 12 s for the nodes, so give the remote function handle a timeout
  over that, such as 30 s. Its 5 s default ends the call first.
- It never builds and passes no params. Stopping the manager leaves running what it started.
