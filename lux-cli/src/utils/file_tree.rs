use itertools::Itertools;
use pathdiff::diff_paths;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use termtree::Tree;

pub(crate) fn term_tree_from_paths(paths: &[PathBuf]) -> Tree<String> {
    let root_dir = find_common_root(paths).unwrap_or_else(|| PathBuf::from("."));

    let mut path_children: HashMap<PathBuf, Vec<(PathBuf, String)>> = HashMap::new();

    let sorted_paths = paths
        .iter()
        .sorted_by_key(|p| p.components().count())
        .collect::<Vec<_>>();

    for path in sorted_paths {
        let rel_path = diff_paths(path, &root_dir).unwrap_or_else(|| path.clone());

        let mut component_path = root_dir.clone();
        let mut components_with_paths = Vec::new();

        for component in rel_path.components() {
            component_path = component_path.join(component);
            let name = component.as_os_str().to_string_lossy().to_string();
            components_with_paths.push((component_path.clone(), name));
        }

        if !components_with_paths.is_empty() {
            path_children
                .entry(root_dir.clone())
                .or_default()
                .push(components_with_paths[0].clone());

            for pair in components_with_paths.windows(2) {
                let (parent_path, _) = &pair[0];
                path_children
                    .entry(parent_path.clone())
                    .or_default()
                    .push(pair[1].clone());
            }
        }
    }

    for children in path_children.values_mut() {
        children.sort_by(|a, b| a.1.cmp(&b.1));
        children.dedup_by(|a, b| a.0 == b.0);
    }

    let root_name = root_dir.file_name().map_or_else(
        || root_dir.to_string_lossy().to_string(),
        |name| name.to_string_lossy().to_string(),
    );

    build_tree_node(root_dir, root_name, &path_children)
}

fn build_tree_node(
    path: PathBuf,
    name: String,
    path_children: &HashMap<PathBuf, Vec<(PathBuf, String)>>,
) -> Tree<String> {
    let mut node = Tree::new(name);
    if let Some(children) = path_children.get(&path) {
        for (child_path, child_name) in children {
            let child_node = build_tree_node(child_path.clone(), child_name.clone(), path_children);
            node.push(child_node);
        }
    }
    node
}

fn find_common_root(paths: &[PathBuf]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    if paths.len() == 1 {
        return Some(PathBuf::from("."));
    }
    let get_parent = |path: &PathBuf| -> PathBuf {
        path.parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    };
    let first_parent = get_parent(&paths[0]);

    let common_prefix = |a: &Path, b: &Path| -> Option<PathBuf> {
        let a_components = a.components().collect::<Vec<_>>();
        let b_components = b.components().collect::<Vec<_>>();
        let common = a_components
            .iter()
            .zip(b_components.iter())
            .take_while(|(x, y)| x == y)
            .map(|(x, _)| *x)
            .collect::<Vec<_>>();
        if common.is_empty() {
            None
        } else {
            Some(common.into_iter().collect())
        }
    };

    paths
        .iter()
        .skip(1)
        .map(get_parent)
        .try_fold(first_parent, |acc, path| common_prefix(&acc, &path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tree_structure() {
        let paths: Vec<PathBuf> = vec![
            PathBuf::from("src/main.lua"),
            PathBuf::from("src/lib.lua"),
            PathBuf::from("src/utils/helpers.lua"),
            PathBuf::from("src/utils/config.lua"),
            PathBuf::from("tests/integration.lua"),
            PathBuf::from("README.md"),
        ];

        let tree = term_tree_from_paths(&paths);
        let tree_string = tree.to_string();

        assert!(tree_string.contains("─ src"));
        assert!(tree_string.contains("─ tests"));
        assert!(tree_string.contains("─ utils"));
        assert!(tree_string.contains("─ main.lua"));
        assert!(tree_string.contains("─ lib.lua"));
        assert!(tree_string.contains("─ helpers.lua"));
        assert!(tree_string.contains("─ config.lua"));
        assert!(tree_string.contains("─ integration.lua"));
        assert!(tree_string.contains("─ README.md"));
    }

    #[test]
    fn test_empty_paths() {
        let paths: Vec<PathBuf> = vec![];
        let tree = term_tree_from_paths(&paths);
        assert_eq!(tree.to_string(), ".\n");
    }

    #[test]
    fn test_single_file() {
        let paths: Vec<PathBuf> = vec![PathBuf::from("file.txt")];
        let tree = term_tree_from_paths(&paths);
        assert!(tree.to_string().contains("file.txt"));
    }

    #[test]
    fn test_nested_single_path() {
        let paths: Vec<PathBuf> = vec![PathBuf::from("a/b/c/d/file.txt")];
        let tree = term_tree_from_paths(&paths);
        let tree_string = tree.to_string();

        assert!(tree_string.contains("a"));
        assert!(tree_string.contains("b"));
        assert!(tree_string.contains("c"));
        assert!(tree_string.contains("d"));
        assert!(tree_string.contains("file.txt"));
    }

    #[test]
    fn test_find_common_root() {
        let paths = vec![
            PathBuf::from("/home/user/project/src/main.rs"),
            PathBuf::from("/home/user/project/src/lib.rs"),
            PathBuf::from("/home/user/project/tests/test.rs"),
        ];

        let common_root = find_common_root(&paths);
        assert!(common_root.is_some());
        assert_eq!(common_root.unwrap(), PathBuf::from("/home/user/project"));
    }

    #[test]
    fn test_no_common_root() {
        let paths = vec![
            PathBuf::from("/home/user1/file.txt"),
            PathBuf::from("/home/user2/file.txt"),
        ];

        let common_root = find_common_root(&paths);
        assert!(common_root.is_some());
        assert_eq!(common_root.unwrap(), PathBuf::from("/home"));
    }

    #[test]
    fn test_duplicate_paths() {
        let paths: Vec<PathBuf> = vec![
            PathBuf::from("src/file.txt"),
            PathBuf::from("src/file.txt"),
            PathBuf::from("src/other.txt"),
        ];

        let tree = term_tree_from_paths(&paths);
        let tree_string = tree.to_string();

        assert_eq!(
            tree_string.matches("file.txt").count(),
            1,
            "Duplicate paths should be deduplicated"
        );
    }

    #[test]
    fn test_maintain_depth() {
        let paths: Vec<PathBuf> = vec![
            PathBuf::from("a/b/c/deep.txt"),
            PathBuf::from("a/shallow.txt"),
        ];

        let tree = term_tree_from_paths(&paths);
        let tree_string = tree.to_string();

        assert!(tree_string.contains("deep.txt"));
        assert!(tree_string.contains("shallow.txt"));
        assert!(tree_string.contains("a"));
        assert!(tree_string.contains("b"));
        assert!(tree_string.contains("c"));
    }
}
