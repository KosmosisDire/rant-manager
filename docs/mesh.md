# On the mesh

`rant run Manager` in a workspace offers its groups and nodes, and the installed packages'
ones, with a function each that runs, stops or restarts them through the rant CLI.

## Names

Every name starts with a prefix:

- the placement's prefix (`prefix` in workspace.rant, the group or an include), when set
- else the workspace folder's name in PascalCase: `cell` is `Cell`
- else, outside every workspace, the host name in PascalCase

Groups and nodes keep their package in the path, both in PascalCase:

| entity | kind | type |
|---|---|---|
| `Cell/Rant/Groups` | variable | `GroupState[]`, every group |
| `Cell/Rant/Groups/Io/Ft` | variable | `GroupState`, the group `io/Ft` |
| `Cell/Rant/Groups/Io/Ft/Manage` | function | `Action` to `Empty` |
| `Cell/Rant/Nodes` | variable | `NodeState[]`, every node |
| `Cell/Rant/Nodes/Io/MyNode` | variable | `NodeState`, the node `io/my_node` |
| `Cell/Rant/Nodes/Io/MyNode/Manage` | function | `Action` to `Empty` |

## Types

```
RunningStatus = enum<u8> { Stopped, Running }
Action        = enum<u8> { Run, Stop, Restart }
GroupState    { name: string<64>, manage_path: string<64>, description: string<128>, status: RunningStatus }
NodeState     { name: string<64>, manage_path: string<64>, status: RunningStatus }
```

- `name` is what the CLI takes: `io/Ft`, or `Ft` outside every package.
- `manage_path` is the full name of the Manage function.

## Manage

- Run is `rant run`, Stop `rant stop`, Restart `rant restart`, none of them building.
- A node's Stop and Restart act on each running copy. Run, or Restart of a stopped node,
  runs it alone, outside its groups.
- A failed call answers an error with the CLI's text.
- Run and Restart wait up to 12 s for the nodes to start, so give the remote function handle
  a timeout over that, such as 30 s. Its 5 s default ends the call first, whatever wait the
  call itself asks for.
- The manager refuses to stop or restart a group or node that holds itself.
- The variables show the outcome before the call answers. Changes made elsewhere show within 2 s.
