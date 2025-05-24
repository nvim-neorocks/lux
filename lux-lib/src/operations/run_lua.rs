//! Run the `lua` binary with some given arguments.
//!
//! The interfaces exposed here ensure that the correct version of Lua is being used.

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use thiserror::Error;
use tokio::process::Command;
use which::which;

use crate::{
    config::LuaVersion,
    lua_installation,
    path::{Paths, PathsError},
    tree::Tree,
};

#[derive(Clone, Default)]
pub enum LuaBinary {
    /// The regular Lua interpreter.
    #[default]
    Lua,
    /// Custom Lua interpreter.
    Custom(String),
}

#[derive(Debug, Error)]
pub enum LuaBinaryError {
    #[error("neither `lua` nor `luajit` found on the PATH")]
    LuaBinaryNotFound,
    #[error("{0} not found on the PATH")]
    CustomBinaryNotFound(String),
}

impl fmt::Display for LuaBinary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LuaBinary::Lua => write!(f, "lua"),
            LuaBinary::Custom(cmd) => write!(f, "{cmd}"),
        }
    }
}

impl From<PathBuf> for LuaBinary {
    fn from(value: PathBuf) -> Self {
        Self::Custom(value.to_string_lossy().to_string())
    }
}

impl TryFrom<LuaBinary> for PathBuf {
    type Error = LuaBinaryError;

    fn try_from(value: LuaBinary) -> Result<Self, Self::Error> {
        match value {
            LuaBinary::Lua => match which("lua") {
                Ok(path) => Ok(path),
                Err(_) => match which("luajit") {
                    Ok(path) => Ok(path),
                    Err(_) => Err(LuaBinaryError::LuaBinaryNotFound),
                },
            },
            LuaBinary::Custom(bin) => match which(&bin) {
                Ok(path) => Ok(path),
                Err(_) => Err(LuaBinaryError::CustomBinaryNotFound(bin)),
            },
        }
    }
}

#[derive(Error, Debug)]
pub enum RunLuaError {
    #[error("error running lua: {0}")]
    LuaBinary(#[from] LuaBinaryError),
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
    expected_version: LuaVersion,
    lua_cmd: LuaBinary,
    args: &Vec<String>,
) -> Result<(), RunLuaError> {
    let paths = Paths::new(tree)?;

    match lua_installation::detect_installed_lua_version(lua_cmd.clone())
        .await
        .and_then(|ver| Ok(LuaVersion::from_version(ver)?))
    {
        Ok(installed_version) if installed_version != expected_version => {
            Err(RunLuaError::LuaVersionMismatch {
                lua_cmd: lua_cmd.to_string(),
                installed_version,
                lua_version: expected_version,
            })?
        }
        Ok(_) => {}
        Err(_) => {
            eprintln!(
                "⚠️ WARNING: could not parse lua version from '{} -v' output.
Cannot verify that the expected version is being used.
                ",
                &lua_cmd
            );
        }
    }

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
