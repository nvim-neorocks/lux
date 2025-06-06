use std::ops::Deref;

use itertools::Itertools;
use nonempty::NonEmpty;
use serde::Deserialize;
use thiserror::Error;
use tokio::process::Command;

use crate::{
    config::Config,
    lua_installation::LuaBinary,
    lua_rockspec::LuaVersionError,
    operations,
    path::{Paths, PathsError},
    project::{project_toml::LocalProjectTomlValidationError, Project, ProjectTreeError},
};

use super::RunLuaError;

#[derive(Debug, Error)]
#[error("`{0}` should not be used as a `command` as it is not cross-platform.
You should only change the default `command` if it is a different Lua interpreter that behaves identically on all platforms.
Consider removing the `command` field and letting Lux choose the default Lua interpreter instead.")]
pub struct RunCommandError(String);

#[derive(Debug, Clone)]
pub struct RunCommand(String);

impl RunCommand {
    pub fn from(command: String) -> Result<Self, RunCommandError> {
        match command.as_str() {
            // Common Lua interpreters that could lead to cross-platform issues
            // Luajit is also included because it may or may not have lua52 syntax support compiled in.
            "lua" | "lua5.1" | "lua5.2" | "lua5.3" | "lua5.4" | "luajit" => {
                Err(RunCommandError(command))
            }
            _ => Ok(Self(command)),
        }
    }
}

impl<'de> Deserialize<'de> for RunCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let command = String::deserialize(deserializer)?;

        RunCommand::from(command).map_err(serde::de::Error::custom)
    }
}

impl Deref for RunCommand {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum RunError {
    Toml(#[from] LocalProjectTomlValidationError),
    RunCommand(#[from] RunCommandError),
    LuaVersion(#[from] LuaVersionError),
    RunLua(#[from] RunLuaError),
    ProjectTree(#[from] ProjectTreeError),
    Io(#[from] std::io::Error),
    Paths(#[from] PathsError),
    #[error("No `run` field found in `lux.toml`")]
    NoRunField,
}

async fn run_with_local_lua(
    project: &Project,
    args: &NonEmpty<String>,
    config: &Config,
) -> Result<(), RunError> {
    let version = project.lua_version(config)?;

    let tree = project.tree(config)?;
    let args = &args.into_iter().cloned().collect();

    let runluabuilder = operations::RunLuaBuilder::builder()
        .root(project.root())
        .tree(&tree)
        .config(config)
        .lua_cmd(LuaBinary::new(version, config))
        .args(args)
        .build();

    runluabuilder.run_lua().await?;

    Ok(())
}

async fn run_with_command(
    project: &Project,
    command: &RunCommand,
    args: &NonEmpty<String>,
    config: &Config,
) -> Result<(), RunError> {
    let tree = project.tree(config)?;
    let paths = Paths::new(&tree)?;

    match Command::new(command.deref())
        .args(args.into_iter().cloned().collect_vec())
        .current_dir(project.root().deref())
        .env("PATH", paths.path_prepended().joined())
        .env("LUA_PATH", paths.package_path().joined())
        .env("LUA_CPATH", paths.package_cpath().joined())
        .status()
        .await?
        .code()
    {
        Some(0) => Ok(()),
        code => Err(RunLuaError::LuaCommandNonZeroExitCode {
            lua_cmd: command.to_string(),
            exit_code: code,
        }
        .into()),
    }
}

pub async fn run(
    project: &Project,
    extra_args: &[String],
    config: &Config,
) -> Result<(), RunError> {
    let toml = project.toml().into_local()?;

    let run_spec = toml
        .run()
        .ok_or(RunError::NoRunField)?
        .current_platform()
        .clone();

    let mut args = run_spec.args.unwrap_or_default();

    if !extra_args.is_empty() {
        args.extend(extra_args.iter().cloned());
    }

    match &run_spec.command {
        Some(command) => run_with_command(project, command, &args, config).await,
        None => run_with_local_lua(project, &args, config).await,
    }
}
