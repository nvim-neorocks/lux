use crate::rockspec::LuaVersionCompatibility;
use crate::rockspec::Rockspec;
use std::{io, path::Path};

use crate::{
    config::Config,
    lua_installation::LuaInstallation,
    lua_rockspec::BuildInfo,
    luarocks::luarocks_installation::{ExecLuaRocksError, LuaRocksError, LuaRocksInstallation},
    progress::{Progress, ProgressBar},
    tree::RockLayout,
};

use tempdir::TempDir;
use thiserror::Error;

use super::utils::recursive_copy_dir;

#[derive(Error, Debug)]
pub enum LuarocksBuildError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("error instantiating luarocks compatibility layer: {0}")]
    LuaRocksError(#[from] LuaRocksError),
    #[error("error running 'luarocks make': {0}")]
    ExecLuaRocksError(#[from] ExecLuaRocksError),
}

pub(crate) async fn build<R: Rockspec>(
    rockspec: &R,
    output_paths: &RockLayout,
    lua: &LuaInstallation,
    config: &Config,
    build_dir: &Path,
    progress: &Progress<ProgressBar>,
) -> Result<BuildInfo, LuarocksBuildError> {
    progress.map(|p| {
        p.set_message(format!(
            "Building {} {} with luarocks...",
            rockspec.package(),
            rockspec.version()
        ))
    });
    let rockspec_temp_dir = TempDir::new("temp-rockspec")?.into_path();
    let rockspec_file = rockspec_temp_dir.join(format!(
        "{}-{}.rockspec",
        rockspec.package(),
        rockspec.version()
    ));
    std::fs::write(&rockspec_file, rockspec.to_rockspec_str())?;
    let luarocks = LuaRocksInstallation::new(config)?;
    let luarocks_tree = TempDir::new("luarocks-compat-tree")?;
    luarocks.make(&rockspec_file, build_dir, luarocks_tree.path(), lua)?;
    install(rockspec, &luarocks_tree.into_path(), output_paths, config)
}

fn install<R: Rockspec>(
    rockspec: &R,
    luarocks_tree: &Path,
    output_paths: &RockLayout,
    config: &Config,
) -> Result<BuildInfo, LuarocksBuildError> {
    let lua_version = rockspec
        .lua_version_matches(config)
        .expect("could not get lua version!");
    std::fs::create_dir_all(&output_paths.bin)?;
    let package_dir = luarocks_tree
        .join("lib")
        .join("lib")
        .join("luarocks")
        .join(format!(
            "rocks-{}",
            &lua_version.version_compatibility_str()
        ))
        .join(format!("{}", rockspec.package()))
        .join(format!("{}", rockspec.version()));
    recursive_copy_dir(&package_dir.join("doc"), &output_paths.doc)?;
    recursive_copy_dir(&luarocks_tree.join("bin"), &output_paths.bin)?;
    let src_dir = luarocks_tree
        .join("share")
        .join("lua")
        .join(lua_version.version_compatibility_str());
    recursive_copy_dir(&src_dir, &output_paths.src)?;
    let lib_dir = luarocks_tree
        .join("lib")
        .join("lua")
        .join(lua_version.version_compatibility_str());
    recursive_copy_dir(&lib_dir, &output_paths.lib)?;
    Ok(BuildInfo::default())
}
