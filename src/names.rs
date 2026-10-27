//! The prefix the manager takes.

/// Words split at anything but a letter or digit, each word's first letter raised and the
/// rest kept, so `my_tools` is `MyTools` and `IO` stays `IO`.
pub fn pascal(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .flat_map(|w| {
            let mut c = w.chars();
            c.next().into_iter().flat_map(char::to_uppercase).chain(c)
        })
        .collect()
}

/// The prefix the manager sets itself: none when placement set one, else the workspace
/// folder's name, else the hostname outside every workspace.
pub fn own_prefix(placed: &str, workspace_folder: Option<&str>, hostname: &str) -> Option<String> {
    if !placed.is_empty() {
        return None;
    }
    Some(pascal(workspace_folder.unwrap_or(hostname))).filter(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_raises_each_word() {
        assert_eq!(pascal("my_tools"), "MyTools");
        assert_eq!(pascal("lab-pc-07"), "LabPc07");
        assert_eq!(pascal("IO"), "IO");
        assert_eq!(pascal("io"), "Io");
        assert_eq!(pascal("Ft"), "Ft");
        assert_eq!(pascal("my cell.v2"), "MyCellV2");
    }

    #[test]
    fn placement_wins_over_the_workspace_and_the_host() {
        assert_eq!(own_prefix("LineA", Some("cell"), "lab-pc"), None);
        assert_eq!(own_prefix("", Some("cell"), "lab-pc").as_deref(), Some("Cell"));
        assert_eq!(own_prefix("", None, "lab-pc").as_deref(), Some("LabPc"));
    }

    #[test]
    fn a_name_nothing_spells_falls_back() {
        assert_eq!(own_prefix("", Some("--"), "lab-pc"), None);
    }
}
