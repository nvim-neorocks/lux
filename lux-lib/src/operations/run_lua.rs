//! Run the `lua` binary with some given arguments.
//!
//! The interfaces exposed here ensure that the correct version of Lua is being used.

use bon::Builder;

use crate::config::Config;

use std::{
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;
use tokio::process::Command;

use crate::{
    lua_installation::{LuaBinary, LuaBinaryError},
    path::{Paths, PathsError},
    tree::Tree,
    tree::TreeError,
};

#[derive(Error, Debug)]
pub enum RunLuaError {
    #[error("error running lua: {0}")]
    LuaBinary(#[from] LuaBinaryError),
    #[error("failed to run {lua_cmd}: {source}")]
    LuaCommandFailed {
        lua_cmd: String,
        #[source]
        source: io::Error,
    },
    #[error("{lua_cmd} exited with non-zero exit code: {}", exit_code.map(|code| code.to_string()).unwrap_or("unknown".into()))]
    LuaCommandNonZeroExitCode {
        lua_cmd: String,
        exit_code: Option<i32>,
    },
    #[error(transparent)]
    Paths(#[from] PathsError),

    #[error(transparent)]
    Tree(#[from] TreeError),
}

#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = run, vis = "pub"))]
pub struct RunLuaBuilder<'a> {
    root: &'a Path,
    tree: &'a Tree,
    config: &'a Config,
    lua_cmd: LuaBinary,
    args: &'a Vec<String>,
    prepend_test_paths: Option<bool>,
    prepend_build_paths: Option<bool>,
}

impl RunLuaBuilder<'_> {
    // consumes
    pub async fn run_lua(self) -> Result<(), RunLuaError> {
        let mut paths = Paths::new(self.tree)?;

        if self.prepend_test_paths.unwrap_or(false) {
            let test_tree_path = self.tree.test_tree(self.config)?;

            let test_path = Paths::new(&test_tree_path)?;

            paths.prepend(&test_path);
        }

        if self.prepend_build_paths.unwrap_or(false) {
            let build_tree_path = self.tree.build_tree(self.config)?;

            let build_path = Paths::new(&build_tree_path)?;

            paths.prepend(&build_path);
        }

        let lua_cmd: PathBuf = self.lua_cmd.try_into()?;

        let status = match Command::new(&lua_cmd)
            .current_dir(self.root)
            .args(self.args)
            .env("PATH", paths.path_prepended().joined())
            .env("LUA_PATH", paths.package_path().joined())
            .env("LUA_CPATH", paths.package_cpath().joined())
            .status()
            .await
        {
            Ok(status) => Ok(status),
            Err(err) => Err(RunLuaError::LuaCommandFailed {
                lua_cmd: lua_cmd.to_string_lossy().to_string(),
                source: err,
            }),
        }?;
        if status.success() {
            Ok(())
        } else {
            Err(RunLuaError::LuaCommandNonZeroExitCode {
                lua_cmd: lua_cmd.to_string_lossy().to_string(),
                exit_code: status.code(),
            })
        }
    }
}
