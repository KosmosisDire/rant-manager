//! The rant command line tool, which does every start, stop and listing.

use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct Cli {
    pub exe: PathBuf,
    pub workspace: PathBuf,
}

impl Cli {
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(&self.exe);
        c.args(args).current_dir(&self.workspace).stdin(Stdio::null());
        // the CLI joins the shell's prefixes in front of what the run files place, and these
        // are the manager's own
        c.env_remove("RANT_PREFIX").env_remove("RANT_NODE_NAME_PREFIX");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        c
    }

    /// What the command printed, or why it failed in the CLI's own words.
    pub fn run(&self, args: &[&str]) -> Result<String, String> {
        let out = self.command(args).output().map_err(|e| format!("cannot run {}: {e}", self.exe.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() {
            return Ok(stdout);
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        let why = if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() };
        Err(why.strip_prefix("error: ").unwrap_or(why).to_string())
    }
}
