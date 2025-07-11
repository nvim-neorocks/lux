use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

#[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
#[serde(default)]
struct LuaRC {
    #[serde(flatten)] // <-- capture any unknown keys here
    other: BTreeMap<String, serde_json::Value>,

    #[serde(default)]
    workspace: Workspace,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
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
            if a.is_ok_and(|read_dir| read_dir.count() > 0) {
                // Add the folder path to the list
                let folder_path = source_dir
                    .strip_prefix(project.root())
                    .expect("could not strip prefix")
                    .to_str()
                    .unwrap()
                    .to_string();
                directories.push(folder_path);
            }
        }
    }

    directories
}

fn generate_luarc(prev_contents: &str, extra_paths: Vec<String>) -> String {
    let mut luarc: LuaRC = serde_json::from_str(prev_contents).unwrap();

    for p in extra_paths {
        if !luarc.workspace.library.contains(&p) {
            luarc.workspace.library.push(p);
        }
    }

    luarc.workspace.library.sort();

    serde_json::to_string_pretty(&luarc).expect("failed to serialize luarc")
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_generate_luarc_from_previous_file_content() {
        let content = super::generate_luarc(
            r#"
        {"any-other-field": true}
        "#,
            vec![String::from("some-lib-A"), String::from("some-lib-B")],
        );
        let expected = r#"{
  "any-other-field": true,
  "workspace": {
    "library": [
      "some-lib-A",
      "some-lib-B"
    ]
  }
}"#;

        assert_eq!(
            serde_json::from_str::<LuaRC>(content.as_str()).unwrap(),
            serde_json::from_str::<LuaRC>(expected.into()).unwrap(),
        );
    }

    #[test]
    fn test_generate_luarc_with_previous_libraries() {
        let content = super::generate_luarc(
            r#"{
  "workspace": {
    "library": [
      "2-preexisting-lib"
    ]
  }
}"#,
            vec![String::from("1-some-lib-A"), String::from("3-some-lib-B")],
        );

        let expected = r#"{
  "workspace": {
    "library": [
      "1-some-lib-A",
      "2-preexisting-lib",
      "3-some-lib-B"
    ]
  }
}"#;
        assert_eq!(
            serde_json::from_str::<LuaRC>(content.as_str()).unwrap(),
            serde_json::from_str::<LuaRC>(expected.into()).unwrap(),
        );
    }
}
