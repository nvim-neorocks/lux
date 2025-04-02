//! Run the `lua` binary with some given arguments.
//!
//! The interfaces exposed here ensure that the correct version of Lua is being used.

use std::{fmt, io, path::Path};

use thiserror::Error;
use tokio::process::Command;

use crate::{config::LuaVersion, lua_installation, path::Paths, tree::Tree};

pub enum LuaBinary {
    /// The regular Lua interpreter.
    Lua,
    /// Custom Lua interpreter.
    Custom(String),
}

impl fmt::Display for LuaBinary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LuaBinary::Lua => write!(f, "lua"),
            LuaBinary::Custom(cmd) => write!(f, "{cmd}"),
        }
    }
}

#[derive(Error, Debug)]
pub enum RunLuaError {
    #[error(
        "{} -v (= {}) does not match expected Lua version {}",
        lua_cmd,
        installed_version,
        lua_version
    )]
    LuaVersionMismatch {
        lua_cmd: String,
        installed_version: LuaVersion,
        lua_version: LuaVersion,
    },
    #[error("could not parse Lua version from '{0} -v' output")]
    ParseLuaVersionError(String),
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
    Io(#[from] io::Error),
}

pub async fn run_lua(
    root: &Path,
    tree: &Tree,
    expected_version: LuaVersion,
    binary_name: LuaBinary,
    args: &Vec<String>,
) -> Result<(), RunLuaError> {
    let lua_cmd = binary_name.to_string();
    let paths = Paths::new(tree)?;

    match lua_installation::get_installed_lua_version(&lua_cmd)
        .and_then(|ver| Ok(LuaVersion::from_version(ver)?))
    {
        Ok(installed_version) if installed_version != expected_version => {
            Err(RunLuaError::LuaVersionMismatch {
                lua_cmd: lua_cmd.clone(),
                installed_version,
                lua_version: expected_version,
            })?
        }
        Ok(_) => {}
        Err(_) => Err(RunLuaError::ParseLuaVersionError(lua_cmd.clone()))?,
    }

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
            lua_cmd: lua_cmd.clone(),
            source: err,
        }),
    }?;
    if status.success() {
        Ok(())
    } else {
        Err(RunLuaError::LuaCommandNonZeroExitCode {
            lua_cmd,
            exit_code: status.code(),
        })
    }
}
