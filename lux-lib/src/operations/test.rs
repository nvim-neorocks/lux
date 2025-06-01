use std::{
    io,
    ops::Deref,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use crate::{
    build::BuildBehaviour,
    config::Config,
    lua_installation::{LuaBinary, LuaBinaryError},
    lua_rockspec::{LuaVersionError, TestSpecError},
    package::{PackageName, PackageVersionReqError},
    path::{Paths, PathsError},
    progress::{MultiProgress, Progress},
    project::{
        project_toml::LocalProjectTomlValidationError, Project, ProjectError, ProjectTreeError,
    },
    rockspec::Rockspec,
    tree::{self, Tree, TreeError},
};
use bon::Builder;
use itertools::Itertools;
use thiserror::Error;

use super::{
    BuildProject, BuildProjectError, Install, InstallError, PackageInstallSpec, Sync, SyncError,
};

#[cfg(target_family = "unix")]
const BUSTED_EXE: &str = "busted";
#[cfg(target_family = "windows")]
const BUSTED_EXE: &str = "busted.bat";

#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = _run, vis = ""))]
pub struct Test<'a> {
    #[builder(start_fn)]
    project: Project,
    #[builder(start_fn)]
    config: &'a Config,

    #[builder(field)]
    args: Vec<String>,

    no_lock: Option<bool>,

    #[builder(default)]
    env: TestEnv,
    #[builder(default = MultiProgress::new_arc())]
    progress: Arc<Progress<MultiProgress>>,
}

impl<State: test_builder::State> TestBuilder<'_, State> {
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item: Into<String>>) -> Self {
        self.args.extend(args.into_iter().map_into());
        self
    }

    pub async fn run(self) -> Result<(), RunTestsError>
    where
        State: test_builder::IsComplete,
    {
        run_tests(self._run()).await
    }
}

pub enum TestEnv {
    /// An environment that is isolated from `HOME` and `XDG` base directories (default).
    Pure,
    /// An impure environment in which `HOME` and `XDG` base directories can influence
    /// the test results.
    Impure,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::Pure
    }
}

#[derive(Error, Debug)]
pub enum RunTestsError {
    #[error(transparent)]
    InstallTestDependencies(#[from] InstallTestDependenciesError),
    #[error("error building project:\n{0}")]
    BuildProject(#[from] BuildProjectError),
    #[error("tests failed!")]
    TestFailure,
    #[error("failed to execute `{0}`: {1}")]
    RunCommandFailure(String, io::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Paths(#[from] PathsError),
    #[error(transparent)]
    Tree(#[from] ProjectTreeError),
    #[error(transparent)]
    ProjectTomlValidation(#[from] LocalProjectTomlValidationError),
    #[error("failed to sync dependencies: {0}")]
    Sync(#[from] SyncError),
    #[error(transparent)]
    TestSpec(#[from] TestSpecError),
    #[error(transparent)]
    LuaVersion(#[from] LuaVersionError),
    #[error(transparent)]
    LuaBinary(#[from] LuaBinaryError),
}

async fn run_tests(test: Test<'_>) -> Result<(), RunTestsError> {
    let rocks = test.project.toml().into_local()?;
    let no_lock = test.no_lock.unwrap_or(false);

    // TODO(#204): Only ensure busted if running with busted (e.g. a .busted directory exists)
    if no_lock {
        ensure_dependencies(&test.project, &rocks, test.config, test.progress).await?;
    } else {
        Sync::new(&test.project, test.config)
            .progress(test.progress.clone())
            .sync_test_dependencies()
            .await?;
    }

    BuildProject::new(&test.project, test.config)
        .no_lock(no_lock)
        .only_deps(false)
        .build()
        .await?;

    let project_tree = test.project.tree(test.config)?;
    let test_tree = test.project.test_tree(test.config)?;

    let test_tree_root = &test_tree.root().clone();
    let mut paths = Paths::new(&project_tree)?;
    let test_tree_paths = Paths::new(&test_tree)?;
    paths.prepend(&test_tree_paths);

    let test_spec = rocks
        .test()
        .current_platform()
        .to_validated(test.project.root())?;
    let mut command = match &test_spec {
        crate::lua_rockspec::ValidatedTestSpec::Busted(_) => Command::new(BUSTED_EXE),
        crate::lua_rockspec::ValidatedTestSpec::Command(spec) => Command::new(spec.command.clone()),
        crate::lua_rockspec::ValidatedTestSpec::LuaScript(_) => {
            let lua_version = test.project.lua_version(test.config)?;
            let lua_binary = LuaBinary::new(lua_version, test.config);
            let lua_bin_path: PathBuf = lua_binary.try_into()?;
            Command::new(lua_bin_path)
        }
    };
    let mut command = command
        .current_dir(test.project.root().deref())
        .args(test_spec.args())
        .args(test.args)
        .env("PATH", paths.path_prepended().joined())
        .env("LUA_PATH", paths.package_path().joined())
        .env("LUA_CPATH", paths.package_cpath().joined());
    if let TestEnv::Pure = test.env {
        // isolate the test runner from the user's own config/data files
        // by initialising empty HOME and XDG base directory paths
        let home = test_tree_root.join("home");
        let xdg = home.join("xdg");
        let _ = std::fs::remove_dir_all(&home);
        let xdg_config_home = xdg.join("config");
        std::fs::create_dir_all(&xdg_config_home)?;
        let xdg_state_home = xdg.join("local").join("state");
        std::fs::create_dir_all(&xdg_state_home)?;
        let xdg_data_home = xdg.join("local").join("share");
        std::fs::create_dir_all(&xdg_data_home)?;
        command = command
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", xdg_config_home)
            .env("XDG_STATE_HOME", xdg_state_home)
            .env("XDG_DATA_HOME", xdg_data_home);
    }
    let status = match command.status() {
        Ok(status) => Ok(status),
        Err(err) => Err(RunTestsError::RunCommandFailure("busted".into(), err)),
    }?;
    if status.success() {
        Ok(())
    } else {
        Err(RunTestsError::TestFailure)
    }
}

#[derive(Error, Debug)]
#[error("error installing test dependencies: {0}")]
pub enum InstallTestDependenciesError {
    ProjectTree(#[from] ProjectTreeError),
    Tree(#[from] TreeError),
    Install(#[from] InstallError),
    PackageVersionReq(#[from] PackageVersionReqError),
}

/// Ensure that the test runner is installed.
async fn ensure_test_runner(
    project_root: &Path,
    rockspec: &impl Rockspec,
    tree: Tree,
    config: &Config,
    progress: Arc<Progress<MultiProgress>>,
) -> Result<(), InstallTestDependenciesError> {
    if let Some(test_runner) = rockspec.test().current_platform().runner(project_root) {
        if !tree.match_rocks(&test_runner)?.is_found() {
            let install_spec =
                PackageInstallSpec::new(test_runner, tree::EntryType::Entrypoint).build();
            Install::new(config)
                .package(install_spec)
                .tree(tree)
                .progress(progress)
                .install()
                .await?;
        }
    }
    Ok(())
}

/// Ensure dependencies and test dependencies are installed
/// This defaults to the local project tree if cwd is a project root.
async fn ensure_dependencies(
    project: &Project,
    rockspec: &impl Rockspec,
    config: &Config,
    progress: Arc<Progress<MultiProgress>>,
) -> Result<(), InstallTestDependenciesError> {
    let test_tree = project.test_tree(config)?;
    ensure_test_runner(
        project.root(),
        rockspec,
        test_tree.clone(),
        config,
        progress.clone(),
    )
    .await?;
    let test_dependencies = rockspec
        .test_dependencies()
        .current_platform()
        .iter()
        .filter(|req| !req.name().eq(&PackageName::new("lua".into())))
        .filter_map(|dep| {
            let build_behaviour = if test_tree
                .match_rocks(dep.package_req())
                .is_ok_and(|matches| !matches.is_found())
            {
                Some(BuildBehaviour::Force)
            } else {
                None
            };
            build_behaviour.map(|build_behaviour| {
                PackageInstallSpec::new(dep.package_req().clone(), tree::EntryType::Entrypoint)
                    .build_behaviour(build_behaviour)
                    .pin(*dep.pin())
                    .opt(*dep.opt())
                    .maybe_source(dep.source.clone())
                    .build()
            })
        })
        .collect();

    Install::new(config)
        .packages(test_dependencies)
        .tree(test_tree)
        .progress(progress.clone())
        .install()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{ConfigBuilder, LuaVersion},
        lua_installation::detect_installed_lua_version,
    };

    use super::*;
    use assert_fs::{prelude::PathCopy, TempDir};

    #[tokio::test]
    async fn test_command_spec() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test/sample-project-command-test");
        run_test(&project_root).await
    }

    #[tokio::test]
    async fn test_lua_script_spec() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test/sample-project-lua-script-test");
        run_test(&project_root).await
    }

    async fn run_test(project_root: &Path) {
        let temp_dir = TempDir::new().unwrap();
        temp_dir.copy_from(project_root, &["**"]).unwrap();
        let project_root = temp_dir.path();
        let project: Project = Project::from(project_root).unwrap().unwrap();
        let tree_root = project.root().to_path_buf().join(".lux");
        let _ = std::fs::remove_dir_all(&tree_root);

        let lua_version = detect_installed_lua_version().or(Some(LuaVersion::Lua51));

        let config = ConfigBuilder::new()
            .unwrap()
            .user_tree(Some(tree_root))
            .lua_version(lua_version)
            .build()
            .unwrap();

        Test::new(project, &config).run().await.unwrap();
    }
}
