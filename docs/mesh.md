# On the mesh

`rant run Manager` in a workspace offers its groups and nodes, and the installed packages'
ones, as two lists with a task beside each that runs, stops or restarts one of them, with
progress on each node, through the rant CLI.

## Names

Every name starts with a prefix:

- the placement's prefix (`prefix` in workspace.rant, the group or an include), when set
- else the workspace folder's name in PascalCase: `cell` is `Cell`
- else, outside every workspace, the host name in PascalCase

| entity | kind | type |
|---|---|---|
| `Cell/Rant/Groups` | variable | `GroupState[]`, every group |
| `Cell/Rant/Groups/Manage` | task | `ManageRequest` to `Empty`, `NodeProgress[]` progress, for a group |
| `Cell/Rant/Nodes` | variable | `NodeState[]`, every node |
| `Cell/Rant/Nodes/Manage` | task | `ManageRequest` to `Empty`, `NodeProgress[]` progress, for a node |

## Types

```
RunningStatus  = enum<u8> { Stopped, Running, Starting, Stopping }
Action         = enum<u8> { Run, Stop, Restart }
ProgressStatus = enum<u8> { Starting, Running, Stopped, Failed }
ManageRequest  { name: string, action: Action }
NodeProgress   { name: string, status: ProgressStatus, note: string }
GroupState     { name: string, description: string, status: RunningStatus,
                 running: string[], nodes: string[], groups: string[] }
NodeState      { name: string, status: RunningStatus, groups: string[] }
```

- `name` is what the CLI takes: `io/Ft` or `io/my_node`, or `Ft` outside every package.
- A group's `nodes` are the nodes its own file declares, and its `groups` the groups it
  includes. A node's `groups` are the groups whose own files declare it.
- A group's `running` holds the mesh names of the nodes its run holds that run, those of
  the groups it includes too.
- A group with a param that has no default lists no nodes.
- A group is Running only while a run of it started with `rant run` holds a node. A group
  that a running group includes, or that declares a running node, stays Stopped.

## Manage

- Pass a `name` from the list beside the task. A name not in it answers an error.
- Run is `rant run`, Stop `rant stop`, Restart `rant restart`, none of them building.
- Each progress update lists every node the call touched so far by its mesh name, as the
  CLI reports it: Starting until it comes up, then Running, Stopped, or Failed when it exited
  or never came up. `note` holds the CLI's words, such as `already running` or
  `exited after 0.3 s (logs/brief.log)`.
- A node's Stop and Restart act on each running copy. Run, or Restart of a stopped node,
  runs it alone, outside its groups.
- A failed call answers an error naming each failed node, or with the CLI's text.
- A cancel is refused, since a CLI cut short could leave a node it started untracked.
- The manager refuses to stop or restart a group or node that holds itself.
- While a call runs, its group or node shows Starting for Run, Stopping for Stop, and for
  Restart Stopping until its first node starts again, then Starting. A group's `running`
  follows each node as it comes up or goes down. Runs and stops made elsewhere show only
  Running and Stopped.
- The lists show the outcome before the call answers. Changes made elsewhere show within 2 s.
