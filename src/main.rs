use std::path::PathBuf;

use clap::Parser;
use rant_manager::{Manager, Options};

/// Offers each group and node of a workspace on the mesh, with a Manage function that runs,
/// stops or restarts it through the rant command line tool.
#[derive(Parser)]
#[command(name = "rant-manager", version)]
struct Args {
    /// The workspace to manage, rant's home for the installed packages alone. This folder by default.
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// The rant command line tool.
    #[arg(long, default_value = "rant")]
    rant: PathBuf,
    /// Join the mesh on one interface only, 127.0.0.1 for this host alone.
    #[arg(long)]
    interface: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = Args::parse();
    let workspace = match a.workspace {
        Some(w) => w,
        None => std::env::current_dir()?,
    };
    let manager = Manager::new(Options { workspace, rant: a.rant, interface: a.interface })?;
    ctrlc::set_handler(manager.quitter())?;
    manager.run();
    Ok(())
}
