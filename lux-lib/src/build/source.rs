use std::{io, path::Path, string::FromUtf8Error};

use thiserror::Error;

use crate::{
    config::Config,
    lua_installation::LuaInstallation,
    lua_rockspec::{
        Build, BuildBackendSpec, BuildInfo, BuildSpec, LocalLuaRockspec, LuaRockspecError,
    },
    progress::{Progress, ProgressBar},
    project::{
        project_toml::{LocalProjectTomlValidationError, PartialProjectToml},
        ProjectRoot, PROJECT_TOML,
    },
    rockspec::Rockspec,
    tree::RockLayout,
};

use super::{
    builtin::BuiltinBuildError, cmake::CMakeError, command::CommandError, make::MakeError,
    rust_mlua::RustError, treesitter_parser::TreesitterBuildError, utils::recursive_copy_dir,
};

#[derive(Error, Debug)]
pub enum SourceBuildError {
    #[error("IO operation failed: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    FromUtf8(#[from] FromUtf8Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    LocalProjectTomlValidation(#[from] LocalProjectTomlValidationError),
    #[error(transparent)]
    LuaRockspec(#[from] LuaRockspecError),
    #[error("builtin build failed: {0}")]
    Builtin(#[from] BuiltinBuildError),
    #[error("cmake build failed: {0}")]
    CMake(#[from] CMakeError),
    #[error("make build failed: {0}")]
    Make(#[from] MakeError),
    #[error("command build failed: {0}")]
    Command(#[from] CommandError),
    #[error("rust-mlua build failed: {0}")]
    Rust(#[from] RustError),
    #[error("treesitter-parser build failed: {0}")]
    TreesitterBuild(#[from] TreesitterBuildError),
    #[error("cannot build from a project source that requires a luarocks build backend: {0}")]
    UnsupporedLuarocksBuildBackend(String),
}

pub(crate) async fn build(
    output_paths: &RockLayout,
    lua: &LuaInstallation,
    config: &Config,
    build_dir: &Path,
    progress: &Progress<ProgressBar>,
) -> Result<BuildInfo, SourceBuildError> {
    let mut build_spec = BuildSpec::default();
    let mut copy_directories = None;
    for path in std::fs::read_dir(build_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
    {
        if path.file_name().is_some_and(|name| name == PROJECT_TOML) {
            let toml_content = String::from_utf8(tokio::fs::read(path).await?)?;
            let project_toml =
                PartialProjectToml::new(&toml_content, ProjectRoot::new())?.into_local()?;
            build_spec = project_toml.build().current_platform().clone();
            copy_directories = Some(build_spec.copy_directories);
            break;
        } else if path.extension().is_some_and(|ext| ext == "rockspec") {
            let rockspec_content = String::from_utf8(tokio::fs::read(path).await?)?;
            let rockspec = LocalLuaRockspec::new(&rockspec_content, ProjectRoot::new())?;
            build_spec = rockspec.build().current_platform().clone();
            copy_directories = Some(build_spec.copy_directories);
            break;
        }
    }
    let build_info = match build_spec.build_backend {
        Some(BuildBackendSpec::Builtin(build_spec)) => {
            build_spec
                .run(output_paths, false, lua, config, build_dir, progress)
                .await?
        }
        Some(BuildBackendSpec::Make(make_spec)) => {
            make_spec
                .run(output_paths, false, lua, config, build_dir, progress)
                .await?
        }
        Some(BuildBackendSpec::CMake(cmake_spec)) => {
            cmake_spec
                .run(output_paths, false, lua, config, build_dir, progress)
                .await?
        }
        Some(BuildBackendSpec::Command(command_spec)) => {
            command_spec
                .run(output_paths, false, lua, config, build_dir, progress)
                .await?
        }
        Some(BuildBackendSpec::RustMlua(rust_mlua_spec)) => {
            rust_mlua_spec
                .run(output_paths, false, lua, config, build_dir, progress)
                .await?
        }
        Some(BuildBackendSpec::TreesitterParser(treesitter_parser_spec)) => {
            treesitter_parser_spec
                .run(output_paths, false, lua, config, build_dir, progress)
                .await?
        }
        Some(BuildBackendSpec::LuaRock(build_backend)) => return Err(SourceBuildError::UnsupporedLuarocksBuildBackend(build_backend)),
        Some(BuildBackendSpec::Source) | // This should not be possible. Let's ignore it.
        None => BuildInfo::default(),
    };
    match copy_directories {
        Some(copy_directories) => {
            for directory in copy_directories.iter().filter(|dir| {
                dir.file_name()
                    .is_some_and(|name| name != "doc" && name != "docs")
            }) {
                recursive_copy_dir(&build_dir.join(directory), &output_paths.etc)?;
            }
        }
        None => {
            // We copy all directories if there is no rockspec
            for subdirectory in std::fs::read_dir(build_dir)?
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let path = entry.path();
                    if path.is_dir()
                        && path.file_name().is_some_and(|name| {
                            !matches!(
                                name.to_string_lossy().to_string().as_str(),
                                "lua" | "src" | "doc" | "docs"
                            )
                        })
                    {
                        path.file_name()
                            .map(|name| name.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            {
                recursive_copy_dir(
                    &build_dir.join(&subdirectory),
                    &output_paths.etc.join(&subdirectory),
                )?;
            }
        }
    }
    Ok(build_info)
}
