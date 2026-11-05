//! The rant command line tool, which does every start, stop and listing.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct Cli {
    pub exe: PathBuf,
    pub workspace: PathBuf,
}

impl Cli {
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(&self.exe);
        c.args(args).current_dir(&self.workspace).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
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
        self.stream(args, |_| {})
    }

    /// As run, handing each line to on_line as the command prints it.
    pub fn stream(&self, args: &[&str], mut on_line: impl FnMut(&str)) -> Result<String, String> {
        let cannot = |e: std::io::Error| format!("cannot run {}: {e}", self.exe.display());
        let mut child = self.command(args).spawn().map_err(cannot)?;
        // read apart, so a full stderr pipe never stalls the CLI while stdout is read
        let mut err = child.stderr.take().expect("stderr is piped");
        let stderr = std::thread::spawn(move || {
            let mut text = Vec::new();
            let _ = err.read_to_end(&mut text);
            text
        });
        let mut out = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let (mut stdout, mut line) = (String::new(), Vec::new());
        while let Ok(1..) = out.read_until(b'\n', &mut line) {
            let text = String::from_utf8_lossy(&line);
            on_line(text.trim_end_matches(['\r', '\n']));
            stdout.push_str(&text);
            line.clear();
        }
        let status = child.wait().map_err(cannot)?;
        let stderr = String::from_utf8_lossy(&stderr.join().unwrap_or_default()).into_owned();
        if status.success() {
            return Ok(stdout);
        }
        let why = if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() };
        Err(why.strip_prefix("error: ").unwrap_or(why).to_string())
    }
}
