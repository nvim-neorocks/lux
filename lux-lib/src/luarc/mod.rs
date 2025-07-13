use crate::config::Config;
use crate::lockfile::LocalPackageLockType;
use crate::lockfile::ProjectLockfile;
use crate::lockfile::ReadOnly;
use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

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

// TODO: improve error handling
pub fn update_luarc(config: &Config) -> Result<(), ()> {
    if !config.generate_luarc() {
        return Ok(());
    }
    let project = Project::current_or_err().expect("failed to get current project");
    let tree = project.tree(&config).expect("failed to get project tree");
    let lockfile = project.lockfile().expect("should have a lockfile");
    let luarc_path = project.luarc_path();
    let relative_tree_root_path = tree
        .root()
        .strip_prefix(&project.root())
        .expect("tree root should be a subpath of project root")
        .to_path_buf();

    // read the existing .luarc file or create a new one if it doesn't exist
    let luarc_content = fs::read_to_string(&luarc_path).unwrap_or_else(|_| String::from("{}"));

    let dependency_dirs = find_dependency_dirs(&lockfile, relative_tree_root_path)
        .into_iter()
        // make sure the paths actually exist
        .filter(|path| fs::exists(path).is_ok_and(|exists| exists))
        .collect();

    let file = generate_luarc(luarc_content.as_str(), dependency_dirs);

    fs::write(&luarc_path, file)
        .expect(format!("failed to write {} file", luarc_path.display()).as_str());

    Ok(())
}

fn find_dependency_dirs(
    lockfile: &ProjectLockfile<ReadOnly>,
    lux_tree_base_dir: PathBuf,
) -> Vec<PathBuf> {
    let rocks = lockfile
        .local_pkg_lock(&LocalPackageLockType::Regular)
        .rocks();

    let directories: Vec<PathBuf> = rocks
        .iter()
        .map(|t| lux_tree_base_dir.join(format!("{}-{}@{}/src", t.0, t.1.name(), t.1.version())))
        .collect();

    let test_rocks = lockfile.local_pkg_lock(&LocalPackageLockType::Test).rocks();

    let test_directories: Vec<PathBuf> = test_rocks
        .iter()
        .map(|t| {
            lux_tree_base_dir.join(format!(
                "test-dependencies/{}-{}@{}/src",
                t.0,
                t.1.name(),
                t.1.version()
            ))
        })
        .collect();

    return directories
        .into_iter()
        .chain(test_directories.into_iter())
        .collect();
}

fn generate_luarc(prev_contents: &str, extra_paths: Vec<PathBuf>) -> String {
    let mut luarc: LuaRC = serde_json::from_str(prev_contents).unwrap();

    // remove any preexisting lux library paths
    luarc
        .workspace
        .library
        .retain(|path| !path.starts_with(".lux/"));

    for p in extra_paths {
        let path = p.clone().into_os_string().into_string();
        if let Ok(path_str) = path {
            if !luarc.workspace.library.contains(&path_str) {
                luarc.workspace.library.push(path_str);
            }
        }
    }

    luarc.workspace.library.sort();

    serde_json::to_string_pretty(&luarc).expect("failed to serialize luarc")
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_generate_luarc_with_previous_libraries_parametrized() {
        let cases = vec![
            (
                "Empty existing libraries, adding single lib", // 📝 Description
                r#"{
                    "workspace": {
                        "library": []
                    }
                }"#,
                vec![".lux/5.1/my-lib".into()],
                r#"{
                    "workspace": {
                        "library": [".lux/5.1/my-lib"]
                    }
                }"#,
            ),
            (
                "Other fields present, adding libs", // 📝 Description
                r#"{
                    "any-other-field": true,
                    "workspace": {
                        "library": []
                    }
                }"#,
                vec![".lux/5.1/lib-A".into(), ".lux/5.1/lib-B".into()],
                r#"{
                    "any-other-field": true,
                    "workspace": {
                        "library": [".lux/5.1/lib-A", ".lux/5.1/lib-B"]
                    }
                }"#,
            ),
            (
                "Removes not present libs, without removing others", // 📝 Description
                r#"{
                    "workspace": {
                        "library": [".lux/5.1/lib-A", ".lux/5.4/lib-B"]
                    }
                }"#,
                vec![".lux/5.1/lib-C".into()],
                r#"{
                    "workspace": {
                        "library": [".lux/5.1/lib-C"]
                    }
                }"#,
            ),
        ];

        for (description, initial, new_libs, expected) in cases {
            let content = super::generate_luarc(initial, new_libs.clone());

            assert_eq!(
                serde_json::from_str::<LuaRC>(&content).unwrap(),
                serde_json::from_str::<LuaRC>(expected).unwrap(),
                "Case failed: {}\nInitial input:\n{}\nNew libs: {:?}",
                description,
                initial,
                &new_libs
            );
        }
    }

    #[test]
    fn test_find_deps() {
        let lockfile_path = std::env::current_dir()
            .unwrap()
            .join("resources/test/lux.lock");
        let result = find_dependency_dirs(
            &ProjectLockfile::new(lockfile_path).unwrap(),
            "resources/test".into(),
        );

        result.iter().for_each(|name| {
            println!("Found dependency folder: {}", name.display());
        });
    }
}
