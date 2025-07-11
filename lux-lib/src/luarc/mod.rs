use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Luarc {
    #[serde(flatten)] // <-- capture any unknown keys here
    other: BTreeMap<String, serde_json::Value>,

    #[serde(default)]
    workspace: Workspace,
}

#[derive(Serialize, Deserialize, Default)]
struct Workspace {
    #[serde(default)]
    library: Vec<String>,
}

pub fn update_luarc() -> Result<(), ()> {
    let project = Project::current_or_err().expect("failed to get current project");
    let luarc_path = project.luarc_path();

    let luarc_content = fs::read_to_string(&luarc_path).expect("failed to read luarc file");

    let dependency_folders = find_dependency_folders();
    let file = generate_luarc(luarc_content.as_str(), dependency_folders);

    std::fs::write(&luarc_path, file)
        .expect(format!("failed to write {} file", luarc_path.display()).as_str());
    Ok(())
}

fn find_dependency_folders() -> Vec<String> {
    let project = Project::current_or_err().expect("failed to get current project");
    // TODO: use version to find the correct folder
    // dependency dirs look like ./lux/<lua_version>/<dependency_hash_name_and_version>/src
    let base_folder = project.root().as_path().join(".lux/5.1/");
    let mut directories = Vec::new();

    for entry in std::fs::read_dir(&base_folder).expect("failed to read lux directory") {
        let entry = entry.expect("failed to read entry");
        if entry.path().is_dir() {
            let source_dir = entry.path().join("src");
            let a = std::fs::read_dir(&source_dir);
            // Check if the folder contains a /src directory
            // if folder_name.starts_with("dependency_") {
            if a.is_ok() && a.unwrap().count() > 0 {
                // Add the folder path to the list
                let folder_path = source_dir.to_str().unwrap().to_string();
                directories.push(folder_path);
            }
        }
    }

    directories
}

fn generate_luarc(prev_contents: &str, extra_paths: Vec<String>) -> String {
    // 1. Parse what we already have, or fall back to an empty struct
    let mut luarc: Luarc = serde_json::from_str(prev_contents).unwrap_or_default();

    // 2. Push the new paths, avoiding duplicates
    for p in extra_paths {
        if !luarc.workspace.library.contains(&p) {
            luarc.workspace.library.push(p);
        }
    }

    // 3. Serialise back, pretty-printed
    serde_json::to_string_pretty(&luarc).expect("failed to serialize luarc")
}

#[cfg(test)]
mod test {

    #[test]
    fn test_generate_luarc() {
        let content = super::generate_luarc(
            r#"
        {
  "any-other-field": true
        }
        "#,
            vec![String::from("hola"), String::from("mundo")],
        );
        assert_eq!(
            content,
            r#"{
  "any-other-field": true,
  "workspace": {
    "library": [
      "hola",
      "mundo"
    ]
  }
}"#
        );
    }
}
