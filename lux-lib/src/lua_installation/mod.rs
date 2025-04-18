use is_executable::IsExecutable;
use itertools::Itertools;
use path_slash::PathBufExt;
use pkg_config::{Config as PkgConfig, Library};
use std::io;
use std::path::Path;
use std::{path::PathBuf, process::Command};
use target_lexicon::Triple;
use thiserror::Error;

use crate::build::utils::escape_path;
use crate::{
    build::variables::HasVariables,
    config::{Config, LuaVersion},
    package::PackageVersion,
};

pub struct LuaInstallation {
    pub include_dir: PathBuf,
    pub lib_dir: PathBuf,
    pub version: LuaVersion,
    /// Binary to the Lua executable, if present
    bin: Option<PathBuf>,
    /// pkg-config library information if available
    lib_info: Option<Library>,
}

impl LuaInstallation {
    pub fn new(version: &LuaVersion, config: &Config) -> Self {
        let pkg_name = match version {
            LuaVersion::Lua51 => "lua5.1",
            LuaVersion::Lua52 => "lua5.2",
            LuaVersion::Lua53 => "lua5.3",
            LuaVersion::Lua54 => "lua5.4",
            LuaVersion::LuaJIT | LuaVersion::LuaJIT52 => "luajit",
        };
        let lib_info = PkgConfig::new()
            .print_system_libs(false)
            .cargo_metadata(false)
            .env_metadata(false)
            .probe(pkg_name)
            .ok();

        if let Some(info) = lib_info {
            if !&info.include_paths.is_empty() && !&info.link_paths.is_empty() {
                let lib_dir = PathBuf::from(&info.link_paths[0]);
                let bin = lib_dir
                    .parent()
                    .map(|parent| parent.join("bin"))
                    .filter(|dir| dir.is_dir())
                    .and_then(|bin_path| find_lua_executable(&bin_path));
                return Self {
                    include_dir: PathBuf::from(&info.include_paths[0]),
                    lib_dir,
                    version: version.clone(),
                    lib_info: Some(info),
                    bin,
                };
            }
        }

        let output = Self::path(version, config);
        if output.exists() {
            let bin_path = output.join("bin");
            let bin = if bin_path.is_dir() {
                find_lua_executable(&bin_path)
            } else {
                None
            };
            LuaInstallation {
                include_dir: output.join("include"),
                lib_dir: output.join("lib"),
                version: version.clone(),
                lib_info: None,
                bin,
            }
        } else {
            let host = Triple::host();
            let target = &host.to_string();
            let host_operating_system = &host.operating_system.to_string();

            let (include_dir, lib_dir) = match version {
                LuaVersion::LuaJIT | LuaVersion::LuaJIT52 => {
                    // XXX: luajit_src panics if this is not set.
                    let target_pointer_width =
                        std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap_or("64".into());
                    std::env::set_var("CARGO_CFG_TARGET_POINTER_WIDTH", target_pointer_width);
                    let build = luajit_src::Build::new()
                        .target(target)
                        .host(host_operating_system)
                        .out_dir(&output)
                        .lua52compat(matches!(version, LuaVersion::LuaJIT52))
                        .build();

                    (
                        build.include_dir().to_path_buf(),
                        build.lib_dir().to_path_buf(),
                    )
                }
                _ => {
                    let build = lua_src::Build::new()
                        .target(target)
                        .host(host_operating_system)
                        .out_dir(&output)
                        .build(match version {
                            LuaVersion::Lua51 => lua_src::Version::Lua51,
                            LuaVersion::Lua52 => lua_src::Version::Lua52,
                            LuaVersion::Lua53 => lua_src::Version::Lua53,
                            LuaVersion::Lua54 => lua_src::Version::Lua54,
                            _ => unreachable!(),
                        });

                    (
                        build.include_dir().to_path_buf(),
                        build.lib_dir().to_path_buf(),
                    )
                }
            };

            let bin_path = output.join("bin");
            let bin = if bin_path.is_dir() {
                find_lua_executable(&bin_path)
            } else {
                None
            };
            LuaInstallation {
                include_dir,
                lib_dir,
                version: version.clone(),
                lib_info: None,
                bin,
            }
        }
    }

    pub fn path(version: &LuaVersion, config: &Config) -> PathBuf {
        config.lua_dir().join(version.to_string())
    }

    pub(crate) fn lua_lib_name(&self) -> String {
        match self.version {
            LuaVersion::LuaJIT => format!("luajit-5.1.{}", std::env::consts::DLL_EXTENSION),
            LuaVersion::LuaJIT52 => format!("luajit-5.2.{}", std::env::consts::DLL_EXTENSION),
            _ => format!("lua.{}", std::env::consts::DLL_EXTENSION),
        }
    }

    pub(crate) fn compile_args(&self) -> Vec<String> {
        if let Some(info) = &self.lib_info {
            info.include_paths
                .iter()
                .map(|p| format!("-I{}", p.display()))
                .chain(info.defines.iter().map(|(k, v)| match v {
                    Some(val) => format!("-D{}={}", k, val),
                    None => format!("-D{}", k),
                }))
                .collect_vec()
        } else {
            vec![format!("-I{}", self.include_dir.display())]
        }
    }

    pub(crate) fn link_args(&self) -> Vec<String> {
        if let Some(info) = &self.lib_info {
            info.link_paths
                .iter()
                .map(|p| format!("-L{}", p.display()))
                .chain(info.libs.iter().map(|lib| format!("-l{}", lib)))
                .chain(info.ld_args.iter().map(|ld_arg_group| {
                    ld_arg_group
                        .iter()
                        .map(|arg| format!("-Wl,{}", arg))
                        .collect::<Vec<_>>()
                        .join(" ")
                }))
                .collect_vec()
        } else {
            let link_lua_arg = match self.version {
                LuaVersion::LuaJIT => "-lluajit-5.1",
                LuaVersion::LuaJIT52 => "-lluajit-5.2",
                _ => "-llua",
            };
            vec![
                format!("-L{}", self.lib_dir.display()),
                link_lua_arg.to_string(),
            ]
        }
    }

    /// Get the Lua binary (if present), prioritising
    /// a potentially overridden value in the config.
    pub(crate) fn lua_binary(&self, config: &Config) -> Option<String> {
        config.variables().get("LUA").cloned().or(self
            .bin
            .as_ref()
            .map(|bin| bin.clone().to_slash_lossy().to_string()))
    }
}

impl HasVariables for LuaInstallation {
    fn get_variable(&self, input: &str) -> Option<String> {
        let result = match input {
            "LUA_INCDIR" => Some(escape_path(&self.include_dir)),
            "LUA_LIBDIR" => Some(escape_path(&self.lib_dir)),
            "LUA" => Some(escape_path(&self.bin.clone().unwrap_or("lua".into()))),
            "LUALIB" => Some(self.lua_lib_name()),
            _ => None,
        }?;
        Some(result)
    }
}

#[derive(Error, Debug)]
pub enum GetLuaVersionError {
    #[error("failed to run {0}: {1}")]
    RunLuaCommandError(String, io::Error),
    #[error("failed to parse Lua version from output: {0}")]
    ParseLuaVersionError(String),
    #[error(transparent)]
    PackageVersionParseError(#[from] crate::package::PackageVersionParseError),
    #[error(transparent)]
    LuaVersionError(#[from] crate::config::LuaVersionError),
}

pub fn get_installed_lua_version(lua_cmd: &str) -> Result<PackageVersion, GetLuaVersionError> {
    let output = match Command::new(lua_cmd).arg("-v").output() {
        Ok(output) => Ok(output),
        Err(err) => Err(GetLuaVersionError::RunLuaCommandError(lua_cmd.into(), err)),
    }?;
    let output_vec = if output.stderr.is_empty() {
        output.stdout
    } else {
        // Yes, Lua 5.1 prints to stderr (-‸ლ)
        output.stderr
    };
    let lua_output = String::from_utf8_lossy(&output_vec).to_string();
    parse_lua_version_from_output(&lua_output)
}

fn parse_lua_version_from_output(lua_output: &str) -> Result<PackageVersion, GetLuaVersionError> {
    let lua_version_str = lua_output
        .trim_start_matches("Lua")
        .trim_start_matches("JIT")
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
        .ok_or(GetLuaVersionError::ParseLuaVersionError(
            lua_output.to_string(),
        ))?;
    Ok(PackageVersion::parse(&lua_version_str)?)
}

fn find_lua_executable(bin_path: &Path) -> Option<PathBuf> {
    std::fs::read_dir(bin_path).ok().and_then(|entries| {
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().to_path_buf())
            .filter(|file| {
                file.is_executable()
                    && file.file_name().is_some_and(|name| {
                        matches!(
                            name.to_string_lossy().to_string().as_str(),
                            "lua" | "luajit"
                        )
                    })
            })
            .collect_vec()
            .first()
            .cloned()
    })
}

#[cfg(test)]
mod test {
    use crate::config::ConfigBuilder;

    use super::*;

    #[tokio::test]
    async fn parse_luajit_version() {
        let luajit_output =
            "LuaJIT 2.1.1713773202 -- Copyright (C) 2005-2023 Mike Pall. https://luajit.org/";
        parse_lua_version_from_output(luajit_output).unwrap();
    }

    #[tokio::test]
    async fn parse_lua_51_version() {
        let lua_output = "Lua 5.1.5  Copyright (C) 1994-2012 Lua.org, PUC-Rio";
        parse_lua_version_from_output(lua_output).unwrap();
    }

    #[tokio::test]
    async fn lua_installation_bin() {
        if std::env::var("LUX_SKIP_IMPURE_TESTS").unwrap_or("0".into()) == "1" {
            println!("Skipping impure test");
            return;
        }
        let config = ConfigBuilder::new().unwrap().build().unwrap();
        let lua_version = config.lua_version().unwrap();
        let lua_installation = LuaInstallation::new(lua_version, &config);
        // FIXME: This fails when run in the nix checkPhase
        assert!(lua_installation.bin.is_some());
        let pkg_version =
            get_installed_lua_version(&lua_installation.bin.unwrap().to_string_lossy()).unwrap();
        assert_eq!(&LuaVersion::from_version(pkg_version).unwrap(), lua_version);
    }
}
