use itertools::Itertools;
use path_slash::PathBufExt;
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
};
use thiserror::Error;
use tokio::process::Command;

use crate::{
    build::{
        backend::{BuildBackend, BuildInfo},
        utils,
    },
    config::Config,
    lua_installation::LuaInstallation,
    lua_rockspec::MakeBuildSpec,
    progress::{Progress, ProgressBar},
    tree::{RockLayout, Tree},
    variables::VariableSubstitutionError,
};

use super::external_dependency::ExternalDependencyInfo;

#[derive(Error, Debug)]
pub enum MakeError {
    #[error("{name} step failed.\n\n{status}\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}")]
    CommandFailure {
        name: String,
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error("failed to run `make` step: {0}")]
    Io(io::Error),
    #[error("failed to run `make` step: `{0}` command not found!")]
    CommandNotFound(String),
    #[error(transparent)]
    VariableSubstitutionError(#[from] VariableSubstitutionError),
    #[error("{0} not found")]
    MakefileNotFound(PathBuf),
}

impl BuildBackend for MakeBuildSpec {
    type Err = MakeError;

    async fn run(
        self,
        output_paths: &RockLayout,
        no_install: bool,
        lua: &LuaInstallation,
        external_dependencies: &HashMap<String, ExternalDependencyInfo>,
        config: &Config,
        _tree: &Tree,
        build_dir: &Path,
        _progress: &Progress<ProgressBar>,
    ) -> Result<BuildInfo, Self::Err> {
        // Build step
        if self.build_pass {
            let build_args = self
                .variables
                .iter()
                .chain(&self.build_variables)
                .filter(|(_, value)| !value.is_empty())
                .map(|(key, value)| {
                    let substituted_value = utils::substitute_variables(
                        value,
                        output_paths,
                        lua,
                        external_dependencies,
                        config,
                    )?;
                    Ok(format!("{key}={substituted_value}").trim().to_string())
                })
                .try_collect::<_, Vec<_>, Self::Err>()?;
            let mut cmd = Command::new(config.make_cmd());
            if let Some(build_target) = &self.build_target {
                cmd.arg(build_target);
            }
            match cmd
                .current_dir(build_dir)
                .args(["-f", &self.makefile.to_slash_lossy()])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .args(build_args)
                .spawn()
            {
                Ok(child) => match child.wait_with_output().await {
                    Ok(output) if output.status.success() => {}
                    Ok(output) => {
                        return Err(MakeError::CommandFailure {
                            name: match self.build_target {
                                Some(build_target) => {
                                    format!("{} {}", config.make_cmd(), build_target)
                                }
                                None => config.make_cmd(),
                            },

                            status: output.status,
                            stdout: String::from_utf8_lossy(&output.stdout).into(),
                            stderr: String::from_utf8_lossy(&output.stderr).into(),
                        });
                    }
                    Err(err) => return Err(MakeError::Io(err)),
                },
                Err(_) => return Err(MakeError::CommandNotFound(config.make_cmd().clone())),
            }
        };

        // Install step
        if self.install_pass && !no_install {
            let install_args = self
                .variables
                .iter()
                .chain(&self.install_variables)
                .filter(|(_, value)| !value.is_empty())
                .map(|(key, value)| {
                    let substituted_value = utils::substitute_variables(
                        value,
                        output_paths,
                        lua,
                        external_dependencies,
                        config,
                    )?;
                    Ok(format!("{key}={substituted_value}").trim().to_string())
                })
                .try_collect::<_, Vec<_>, Self::Err>()?;
            match Command::new(config.make_cmd())
                .current_dir(build_dir)
                .arg(&self.install_target)
                .args(["-f", &self.makefile.to_slash_lossy()])
                .args(install_args)
                .output()
                .await
            {
                Ok(output) if output.status.success() => {}
                Ok(output) => {
                    return Err(MakeError::CommandFailure {
                        name: format!("{} {}", config.make_cmd(), self.install_target),
                        status: output.status,
                        stdout: String::from_utf8_lossy(&output.stdout).into(),
                        stderr: String::from_utf8_lossy(&output.stderr).into(),
                    })
                }
                Err(err) => return Err(MakeError::Io(err)),
            }
        };

        Ok(BuildInfo::default())
    }
}
