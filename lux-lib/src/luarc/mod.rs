use crate::lockfile::LocalPackageLockType;
use crate::lockfile::ProjectLockfile;
use crate::lockfile::ReadOnly;
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

    let luarc_content = fs::read_to_string(&luarc_path).unwrap_or_else(|_| String::from("{}"));

    let dependency_folders =
        find_dependency_folders(&project.lockfile().expect("should have a lockfile"));
    let file = generate_luarc(luarc_content.as_str(), dependency_folders);

    std::fs::write(&luarc_path, file)
        .expect(format!("failed to write {} file", luarc_path.display()).as_str());
    Ok(())
}

fn find_dependency_folders(lockfile: &ProjectLockfile<ReadOnly>) -> Vec<String> {
    let rocks = lockfile
        .local_pkg_lock(&LocalPackageLockType::Regular)
        .rocks();

    rocks
        .iter()
        .map(|t| format!(".lux/5.1/{}-{}@{}/src", t.0, t.1.name(), t.1.version()))
        .collect()
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

    #[test]
    fn test_find_deps() {
        let lockfile_path = std::env::current_dir()
            .unwrap()
            .join("resources/test/lux.lock");
        let result = find_dependency_folders(&ProjectLockfile::new(lockfile_path).unwrap());

        result.iter().for_each(|name| {
            println!("Found dependency folder: {}", name);
        });
    }
}
