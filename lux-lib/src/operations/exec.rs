use std::{io, process::ExitStatus};
use tokio::process::Command;

use crate::{
    config::{Config, LuaVersion, LuaVersionUnset},
    lua_rockspec::LuaVersionError,
    operations::Install,
    package::{PackageReq, PackageVersionReqError},
    path::{Paths, PathsError},
    project::{Project, ProjectTreeError},
    remote_package_db::RemotePackageDBError,
    tree::{self, TreeError},
};
use bon::Builder;
use itertools::Itertools;
use thiserror::Error;

use super::{InstallError, PackageInstallSpec};

/// Rocks package runner, providing fine-grained control
/// over how a package should be run.
#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = _exec, vis = ""))]
pub struct Exec<'a> {
    #[builder(start_fn)]
    command: &'a str,
    #[builder(start_fn)]
    project: Option<&'a Project>,
    #[builder(start_fn)]
    config: &'a Config,

    #[builder(field)]
    args: Vec<String>,
}

impl<State: exec_builder::State> ExecBuilder<'_, State> {
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item: Into<String>>) -> Self {
        self.args.extend(args.into_iter().map_into());
        self
    }

    pub async fn exec(self) -> Result<(), ExecError>
    where
        State: exec_builder::IsComplete,
    {
        exec(self._exec()).await
    }
}

#[derive(Error, Debug)]
pub enum ExecError {
    #[error("failed to execute `{name}`.\n\n{status}\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}")]
    RunCommandFailure {
        name: String,
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error(transparent)]
    LuaVersionUnset(#[from] LuaVersionUnset),
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error(transparent)]
    Paths(#[from] PathsError),
    #[error(transparent)]
    LuaVersionError(#[from] LuaVersionError),
    #[error(transparent)]
    ProjectTreeError(#[from] ProjectTreeError),
    #[error("failed to execute `{0}`:\n{1}")]
    Io(String, io::Error),
}

async fn exec(run: Exec<'_>) -> Result<(), ExecError> {
    let lua_version = run
        .project
        .map(|project| project.lua_version(run.config))
        .transpose()?
        .unwrap_or(LuaVersion::from(run.config)?.clone());

    let user_tree = run.config.user_tree(lua_version)?;
    let mut paths = Paths::new(&user_tree)?;

    if let Some(project) = run.project {
        paths.prepend(&Paths::new(&project.tree(run.config)?)?);
    }

    match Command::new(run.command)
        .args(run.args)
        .env("PATH", paths.path_prepended().joined())
        .env("LUA_PATH", paths.package_path().joined())
        .env("LUA_CPATH", paths.package_cpath().joined())
        .output()
        .await
    {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(ExecError::RunCommandFailure {
            name: run.command.to_string(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
        }),
        Err(err) => Err(ExecError::Io(run.command.to_string(), err)),
    }
}

#[derive(Error, Debug)]
#[error(transparent)]
pub enum InstallCmdError {
    InstallError(#[from] InstallError),
    PackageVersionReqError(#[from] PackageVersionReqError),
    RemotePackageDBError(#[from] RemotePackageDBError),
    Tree(#[from] TreeError),
    LuaVersionUnset(#[from] LuaVersionUnset),
}

/// Ensure that a command is installed.
/// This defaults to the local project tree if cwd is a project root.
pub async fn install_command(command: &str, config: &Config) -> Result<(), InstallCmdError> {
    let install_spec = PackageInstallSpec::new(
        PackageReq::new(command.into(), None)?,
        tree::EntryType::Entrypoint,
    )
    .build();
    let tree = config.user_tree(LuaVersion::from(config)?.clone())?;
    Install::new(config)
        .package(install_spec)
        .tree(tree)
        .install()
        .await?;
    Ok(())
}
