use crate::config::Config;
use crate::lockfile::LocalPackageLockType;
use crate::lockfile::ProjectLockfile;
use crate::lockfile::ReadOnly;
use crate::project::Project;
use crate::project::ProjectError;
use crate::project::ProjectTreeError;
use bon::Builder;
use pathdiff::diff_paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs;

#[derive(Error, Debug)]
pub enum GenLuaRcError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    ProjectTree(#[from] ProjectTreeError),
    #[error("failed to write {0}:\n{1}")]
    Write(PathBuf, io::Error),
}

#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = _build, vis = ""))]
pub(crate) struct GenLuaRc<'a> {
    config: &'a Config,
    project: &'a Project,
}

impl<State> GenLuaRcBuilder<'_, State>
where
    State: gen_lua_rc_builder::State + gen_lua_rc_builder::IsComplete,
{
    pub async fn generate_luarc(self) -> Result<(), GenLuaRcError> {
        do_generate_luarc(self._build()).await
    }
}

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
    #[serde(flatten)] // <-- capture any unknown keys here
    other: BTreeMap<String, serde_json::Value>,

    #[serde(default)]
    library: Vec<String>,
}

async fn do_generate_luarc(args: GenLuaRc<'_>) -> Result<(), GenLuaRcError> {
    let config = args.config;
    if !config.generate_luarc() {
        return Ok(());
    }
    let project = args.project;
    let lockfile = project.lockfile()?;
    let luarc_path = project.luarc_path();
    let dependency_tree = project.tree(config)?;
    let dependency_tree_root_relative_path = diff_paths(dependency_tree.root(), project.root())
        .expect("tree root should be a subpath of the project root")
        .to_path_buf();

    let test_dependency_tree = project.test_tree(config)?;
    let test_dependency_tree_root_relative_path =
        diff_paths(test_dependency_tree.root(), project.root())
            .expect("test tree root should be a subpath of the project root")
            .to_path_buf();

    // read the existing .luarc file or create a new one if it doesn't exist
    let luarc_content = fs::read_to_string(&luarc_path)
        .await
        .unwrap_or_else(|_| "{}".into());

    let dependency_dirs = find_dependency_dirs(
        &lockfile,
        dependency_tree_root_relative_path,
        &LocalPackageLockType::Regular,
    );

    let test_dependency_dirs = find_dependency_dirs(
        &lockfile,
        test_dependency_tree_root_relative_path,
        &LocalPackageLockType::Test,
    );

    let all_dependecy_dirs: Vec<PathBuf> = dependency_dirs
        .into_iter()
        .chain(test_dependency_dirs)
        .filter(|path| path.is_dir())
        .collect();

    let file = generate_luarc(luarc_content.as_str(), all_dependecy_dirs);

    fs::write(&luarc_path, file)
        .await
        .map_err(|err| GenLuaRcError::Write(luarc_path, err))?;

    Ok(())
}

fn find_dependency_dirs(
    lockfile: &ProjectLockfile<ReadOnly>,
    lux_tree_base_dir: PathBuf,
    local_package_lock_type: &LocalPackageLockType,
) -> Vec<PathBuf> {
    let rocks = lockfile.local_pkg_lock(local_package_lock_type).rocks();

    rocks
        .iter()
        .map(|t| lux_tree_base_dir.join(format!("{}-{}@{}/src", t.0, t.1.name(), t.1.version())))
        .collect()
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
            luarc.workspace.library.push(path_str);
        }
    }

    luarc.workspace.library.sort();

    serde_json::to_string_pretty(&luarc).expect("failed to serialize .luarc.json")
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
}
