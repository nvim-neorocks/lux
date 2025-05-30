//! Run the `lua` binary with some given arguments.
//!
//! The interfaces exposed here ensure that the correct version of Lua is being used.

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
}

pub async fn run_lua(
    root: &Path,
    tree: &Tree,
    lua_cmd: LuaBinary,
    args: &Vec<String>,
) -> Result<(), RunLuaError> {
    let paths = Paths::new(tree)?;

    let lua_cmd: PathBuf = lua_cmd.try_into()?;

    let status = match Command::new(&lua_cmd)
        .current_dir(root)
        .args(args)
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
