//! Entity names: the prefix the manager takes and the path of each group and node.

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

/// The entity of a CLI name under root: `io/Ft` under `Rant/Groups` is `Rant/Groups/Io/Ft`.
pub fn entity(root: &str, cli_name: &str) -> String {
    cli_name.split('/').fold(root.to_string(), |path, part| path + "/" + &pascal(part))
}

/// The prefix the manager sets itself: none when placement set one, else the workspace
/// folder's name, else the hostname outside every workspace.
pub fn own_prefix(placed: &str, workspace_folder: Option<&str>, hostname: &str) -> Option<String> {
    if !placed.is_empty() {
        return None;
    }
    Some(pascal(workspace_folder.unwrap_or(hostname))).filter(|p| !p.is_empty())
}

/// A name as the mesh knows it, under the prefix the node's names carry.
pub fn full(prefix: &str, name: &str) -> String {
    if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") }
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
    fn entity_paths_follow_the_cli_name() {
        assert_eq!(entity("Rant/Groups", "io/Ft"), "Rant/Groups/Io/Ft");
        assert_eq!(entity("Rant/Groups", "Top"), "Rant/Groups/Top");
        assert_eq!(entity("Rant/Nodes", "io/my_node2"), "Rant/Nodes/Io/MyNode2");
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
        assert_eq!(full("", "Rant/Groups"), "Rant/Groups");
        assert_eq!(full("Cell", "Rant/Groups"), "Cell/Rant/Groups");
    }
}
