use std::path::Path;

use nonempty::NonEmpty;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    config::Config,
    lua_installation::LuaInstallation,
    lua_rockspec::LuaVersionError,
    project::{
        project_toml::{LocalProjectTomlValidationError, RunSpec},
        Project, ProjectRoot,
    },
};

#[derive(Debug, Error)]
#[error("`{0}` should not be used as a `command` as it is not cross-platform.
You should only change the default `command` if it is a different Lua interpreter that behaves identically on all platforms.
Consider removing the `command` field and letting Lux choose the default Lua interpreter instead.")]
pub(crate) struct RunCommandError(String);

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RunCommand(String);

impl RunCommand {
    pub(crate) fn new(command: String) -> Result<Self, RunCommandError> {
        match command.as_str() {
            // Common Lua interpreters that could lead to cross-platform issues
            "lua" | "lua5.1" | "lua5.2" | "lua5.3" | "lua5.4" | "luajit" => {
                Err(RunCommandError(command))
            }
            _ => Ok(Self(command)),
        }
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum RunError {
    Toml(#[from] LocalProjectTomlValidationError),
    RunCommand(#[from] RunCommandError),
    LuaVersion(#[from] LuaVersionError),
    #[error("No `run` field found in `lux.toml`")]
    NoRunField,
}

fn run_with_local_lua(
    project: &Project,
    args: &NonEmpty<String>,
    config: &Config,
) -> Result<(), RunError> {
    let version = project.lua_version(config)?;
    let installation = LuaInstallation::new(&version, config);

    println!("Running Lua from {}", installation.lib_dir.display());

    Ok(())
}

fn run_with_command(
    root: &ProjectRoot,
    command: &RunCommand,
    args: &NonEmpty<String>,
) -> Result<(), RunError> {
    todo!()
}

pub async fn run(project: &Project, config: &Config) -> Result<(), RunError> {
    let toml = project.toml().into_local()?;

    let run_spec = toml.run().ok_or(RunError::NoRunField)?.current_platform();

    match &run_spec.command {
        Some(command) => run_with_command(project.root(), command, &run_spec.args)?,
        None => run_with_local_lua(project, &run_spec.args, config)?,
    };

    todo!()
}
