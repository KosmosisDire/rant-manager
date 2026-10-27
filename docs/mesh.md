# On the mesh

`rant run Manager` in a workspace offers its groups and nodes, and the installed packages'
ones, as two lists with a function beside each that runs, stops or restarts one of them
through the rant CLI.

## Names

Every name starts with a prefix:

- the placement's prefix (`prefix` in workspace.rant, the group or an include), when set
- else the workspace folder's name in PascalCase: `cell` is `Cell`
- else, outside every workspace, the host name in PascalCase

| entity | kind | type |
|---|---|---|
| `Cell/Rant/Groups` | variable | `GroupState[]`, every group |
| `Cell/Rant/Groups/Manage` | function | `ManageRequest` to `Empty`, for a group |
| `Cell/Rant/Nodes` | variable | `NodeState[]`, every node |
| `Cell/Rant/Nodes/Manage` | function | `ManageRequest` to `Empty`, for a node |

## Types

```
RunningStatus = enum<u8> { Stopped, Running }
Action        = enum<u8> { Run, Stop, Restart }
ManageRequest { name: string<64>, action: Action }
Name          { name: string<64> }
GroupState    { name: string<64>, description: string<128>, status: RunningStatus,
                nodes: Name[32], groups: Name[32] }
NodeState     { name: string<64>, status: RunningStatus, groups: Name[32] }
```

- `name` is what the CLI takes: `io/Ft` or `io/my_node`, or `Ft` outside every package.
- A group's `nodes` are the nodes its own file declares, and its `groups` the groups it
  includes. A node's `groups` are the groups whose own files declare it.
- Those lists hold 32 names, unused slots empty. A group with a param that has no default
  lists no nodes.

## Manage

- Pass a `name` from the list beside the function. A name not in it answers an error.
- Run is `rant run`, Stop `rant stop`, Restart `rant restart`, none of them building.
- A node's Stop and Restart act on each running copy. Run, or Restart of a stopped node,
  runs it alone, outside its groups.
- A failed call answers an error with the CLI's text.
- Run and Restart wait up to 12 s for the nodes to start, so give the remote function handle
  a timeout over that, such as 30 s. Its 5 s default ends the call first, whatever wait the
  call itself asks for.
- The manager refuses to stop or restart a group or node that holds itself.
- The lists show the outcome before the call answers. Changes made elsewhere show within 2 s.
