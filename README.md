# rant-manager

Runs, stops and restarts a workspace's groups and nodes from the mesh.

```
rant install manager
rant run Manager
```

Each group and node gets a state variable and a Manage function under `<Prefix>/Rant`, as
[docs/mesh.md](docs/mesh.md) lays out.

## Build

```
cargo build --release
```
