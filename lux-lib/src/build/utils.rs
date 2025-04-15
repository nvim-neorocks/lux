use crate::{
    build::BuildError,
    config::Config,
    lua_installation::LuaInstallation,
    lua_rockspec::{DeploySpec, LuaModule, ModulePaths},
    tree::{RockLayout, Tree},
};
use itertools::Itertools;
use path_slash::PathExt;
use shlex::try_quote;
use std::{
    io,
    path::{Path, PathBuf},
    process::{Output, Stdio},
    string::FromUtf8Error,
};
use target_lexicon::Triple;
use thiserror::Error;
use tokio::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::variables::{self, VariableSubstitutionError};

/// Copies a lua source file to a specific destination. The destination is described by a
/// `module.path` syntax (equivalent to the syntax provided to Lua's `require()` function).
pub(crate) fn copy_lua_to_module_path(
    source: &PathBuf,
    target_module: &LuaModule,
    target_dir: &Path,
) -> io::Result<()> {
    let target = target_dir.join(target_module.to_lua_path());

    std::fs::create_dir_all(target.parent().unwrap())?;

    std::fs::copy(source, target)?;

    Ok(())
}

/// Get the files that Lux treats as project files
/// This respects ignore files and excludes hidden files and directories.
pub(crate) fn project_files(src: &PathBuf) -> Vec<PathBuf> {
    ignore::WalkBuilder::new(src)
        .follow_links(false)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .map(|entry| entry.into_path())
        .collect_vec()
}

/// Recursively copy a directory.
/// This respects ignore files and excludes hidden files and directories.
pub(crate) fn recursive_copy_dir(src: &PathBuf, dest: &Path) -> Result<(), io::Error> {
    if src.exists() {
        for file in project_files(src) {
            let relative_src_path: PathBuf =
                pathdiff::diff_paths(src.join(&file), src).expect("failed to copy directories!");
            let target = dest.join(relative_src_path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&file, target)?;
        }
    }
    Ok(())
}
fn validate_output(output: Output) -> Result<(), BuildError> {
    if !output.status.success() {
        return Err(BuildError::CommandFailure {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
        });
    }
    Ok(())
}

/// Compiles a set of C files into a single dynamic library and places them under `{target_dir}/{target_file}`.
/// # Panics
/// Panics if no parent or no filename can be determined for the target path.
pub(crate) fn compile_c_files(
    files: &Vec<PathBuf>,
    target_module: &LuaModule,
    target_dir: &Path,
    lua: &LuaInstallation,
) -> Result<(), BuildError> {
    let target = target_dir.join(target_module.to_lib_path());

    let parent = target.parent().unwrap_or_else(|| {
        panic!(
            "Couldn't determine parent for path {}",
            target.to_str().unwrap_or("")
        )
    });
    let file = target.file_name().unwrap_or_else(|| {
        panic!(
            "Couldn't determine filename for path {}",
            target.to_str().unwrap_or("")
        )
    });

    std::fs::create_dir_all(parent)?;

    let host = Triple::host();

    // See https://github.com/rust-lang/cc-rs/issues/594#issuecomment-2110551057

    let mut build = cc::Build::new();
    let intermediate_dir = tempdir::TempDir::new(target_module.as_str())?;
    let build = build
        .cargo_output(false)
        .cargo_metadata(false)
        .cargo_warnings(false)
        .warnings(false)
        .flag("-w") // Suppress all warnings
        .files(files)
        .host(std::env::consts::OS)
        .opt_level(2)
        .out_dir(intermediate_dir)
        .target(&host.to_string());

    let is_msvc = build.try_get_compiler()?.is_like_msvc();
    if is_msvc {
        build.flag("/DLL");
    } else {
        build.shared_flag(true);
    }

    for arg in lua.compile_args() {
        build.flag(&arg);
    }

    let objects = build
        .try_compile_intermediates()
        .map_err(BuildError::CompileIntermediatesError)?;

    let output_file = format!("{}", parent.join(file).display());
    let (out_args, lib_args) = if is_msvc {
        (
            vec![
                "/link".into(),
                format!("/OUT:{}", output_file),
                "/DLL".into(),
            ],
            format!("/LIBPATH:{}", lua.lib_dir.display()),
        )
    } else {
        (
            vec!["-shared".into(), "-o".into(), output_file],
            format!("-L{}", lua.lib_dir.display()),
        )
    };

    let output = if is_msvc {
        build
            .try_get_compiler()?
            .to_command()
            .args(&objects) // MSVC wants the objects first
            .args(out_args)
            .arg(lib_args)
            .args(lua.link_args(&build.try_get_compiler()?))
            .output()?
    } else {
        build
            .try_get_compiler()?
            .to_command()
            .args(out_args)
            .arg(lib_args)
            .args(lua.link_args(&build.try_get_compiler()?)) // NOTE: In luarocks, this is behind a link_lua_explicitly config option Library directory
            .args(&objects)
            .output()?
    };
    validate_output(output)?;
    Ok(())
}

// TODO: (#261): special cases for mingw/cygwin?

/// the extension for Lua libraries.
pub(crate) fn lua_lib_extension() -> &'static str {
    if cfg!(target_env = "msvc") {
        "dll"
    } else {
        "so"
    }
}

/// the extension for Lua objects.
pub(crate) fn lua_obj_extension() -> &'static str {
    if cfg!(target_env = "msvc") {
        "obj"
    } else {
        "o"
    }
}

pub(crate) fn default_cflags() -> &'static str {
    if cfg!(target_env = "msvc") {
        "/NOLOGO /MD /O2"
    } else {
        "-O2"
    }
}

pub(crate) fn default_libflag() -> &'static str {
    if cfg!(target_os = "macos") {
        "-bundle -undefined dynamic_lookup -all_load"
    } else if cfg!(target_env = "msvc") {
        "/NOLOGO /DLL"
    } else {
        "-shared"
    }
}

/// Compiles a set of C files (with extra metadata) to a given destination.
/// # Panics
/// Panics if no filename for the target path can be determined.
pub(crate) fn compile_c_modules(
    data: &ModulePaths,
    source_dir: &Path,
    target_module: &LuaModule,
    target_dir: &Path,
    lua: &LuaInstallation,
) -> Result<(), BuildError> {
    let target = target_dir.join(target_module.to_lib_path());

    let parent = target.parent().unwrap_or_else(|| {
        panic!(
            "Couldn't determine parent for path {}",
            target.to_str().unwrap_or("")
        )
    });
    std::fs::create_dir_all(parent)?;

    let host = Triple::host();

    let mut build = cc::Build::new();
    let source_files = data
        .sources
        .iter()
        .map(|dir| source_dir.join(dir))
        .collect_vec();
    let include_dirs = data
        .incdirs
        .iter()
        .map(|dir| source_dir.join(dir))
        .collect_vec();

    let intermediate_dir = tempdir::TempDir::new(target_module.as_str())?;
    let build = build
        .cargo_output(false)
        .cargo_metadata(false)
        .cargo_warnings(false)
        .warnings(false)
        .flag("-w") // Suppress all warnings
        .files(source_files)
        .host(std::env::consts::OS)
        .includes(&include_dirs)
        .opt_level(2)
        .out_dir(intermediate_dir)
        .target(&host.to_string());

    let is_msvc = build.try_get_compiler()?.is_like_msvc();
    if is_msvc {
        build.flag("/DLL");
    } else {
        build.shared_flag(true);
    }

    for arg in lua.compile_args() {
        build.flag(&arg);
    }

    // `cc::Build` has no `defines()` function, so we manually feed in the
    // definitions in a verbose loop
    for (name, value) in &data.defines {
        build.define(name, value.as_deref());
    }

    let file = target.file_name().unwrap_or_else(|| {
        panic!(
            "Couldn't determine filename for path {}",
            target.to_str().unwrap_or("")
        )
    });
    // See https://github.com/rust-lang/cc-rs/issues/594#issuecomment-2110551057
    let objects = build
        .try_compile_intermediates()
        .map_err(BuildError::CompileIntermediatesError)?;

    let libdir_args = data.libdirs.iter().map(|libdir| {
        if is_msvc {
            format!("/LIBPATH:{}", source_dir.join(libdir).display())
        } else {
            format!("-L{}", source_dir.join(libdir).display())
        }
    });

    let library_args = data.libraries.iter().map(|library| {
        if is_msvc {
            format!("{}.lib", library.to_str().unwrap())
        } else {
            format!("-l{}", library.to_str().unwrap())
        }
    });

    let output_file = format!("{}", parent.join(file).display());
    let (out_args, lib_args) = if is_msvc {
        (
            vec![
                "/link".into(),
                format!("/OUT:{}", output_file),
                "/DLL".into(),
            ],
            format!("/LIBPATH:{}", lua.lib_dir.display()),
        )
    } else {
        (
            vec!["-shared".into(), "-o".into(), output_file],
            format!("-L{}", lua.lib_dir.display()),
        )
    };

    let output = if is_msvc {
        build
            .try_get_compiler()?
            .to_command()
            .args(&objects) // MSVC wants the objects first
            .args(out_args)
            .arg(lib_args)
            .args(lua.link_args(&build.try_get_compiler()?))
            .args(libdir_args)
            .args(library_args)
            .output()?
    } else {
        build
            .try_get_compiler()?
            .to_command()
            .args(out_args)
            .arg(lib_args)
            .args(lua.link_args(&build.try_get_compiler()?)) // NOTE: In luarocks, this is behind a link_lua_explicitly config option Library directory
            .args(&objects)
            .args(libdir_args)
            .args(library_args)
            .output()?
    };
    validate_output(output)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum InstallBinaryError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("error wrapping binary: {0}")]
    Wrap(#[from] WrapBinaryError),
}

#[derive(Debug, Error)]
pub enum WrapBinaryError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Utf8(#[from] FromUtf8Error),
}

/// Returns the file path of the installed binary
pub(crate) async fn install_binary(
    source: &Path,
    target: &str,
    tree: &Tree,
    lua: &LuaInstallation,
    deploy: &DeploySpec,
    config: &Config,
) -> Result<PathBuf, InstallBinaryError> {
    tokio::fs::create_dir_all(&tree.bin()).await?;
    let script = if deploy.wrap_bin_scripts
        && needs_wrapper(source).await?
        && is_compatible_lua_script(source, lua, config).await
    {
        install_wrapped_binary(source, target, tree, lua, config).await?
    } else {
        let target = tree.bin().join(target);
        tokio::fs::copy(source, &target).await?;
        target
    };

    #[cfg(unix)]
    set_executable_permissions(&script).await?;

    Ok(script)
}

async fn install_wrapped_binary(
    source: &Path,
    target: &str,
    tree: &Tree,
    lua: &LuaInstallation,
    config: &Config,
) -> Result<PathBuf, WrapBinaryError> {
    let unwrapped_bin_dir = tree.unwrapped_bin();
    tokio::fs::create_dir_all(&unwrapped_bin_dir).await?;
    let unwrapped_bin = unwrapped_bin_dir.join(target);
    tokio::fs::copy(source, &unwrapped_bin).await?;

    #[cfg(target_family = "unix")]
    let target = tree.bin().join(target);
    #[cfg(target_family = "windows")]
    let target = tree.bin().join(format!("{}.bat", target));

    let lua_bin = lua.lua_binary(config).unwrap_or("lua".into());

    #[cfg(target_family = "unix")]
    let content = format!(
        r#"#!/bin/sh

exec {0} "{1}" "$@"
"#,
        lua_bin,
        unwrapped_bin.display(),
    );

    #[cfg(target_family = "windows")]
    let content = format!(
        r#"@echo off
setlocal

{0} "{1}" %*

exit /b %ERRORLEVEL%
"#,
        lua_bin,
        unwrapped_bin.display(),
    );

    tokio::fs::write(&target, content).await?;
    Ok(target)
}

#[cfg(unix)]
async fn set_executable_permissions(script: &Path) -> std::io::Result<()> {
    let mut perms = tokio::fs::metadata(&script).await?.permissions();
    perms.set_mode(0o744);
    tokio::fs::set_permissions(&script, perms).await?;
    Ok(())
}

/// Tries to load the file with Lua. If the file can be loaded,
/// we treat it as a valid Lua script.
async fn is_compatible_lua_script(file: &Path, lua: &LuaInstallation, config: &Config) -> bool {
    let lua_bin = lua.lua_binary(config).unwrap_or("lua".into());
    Command::new(lua_bin)
        .arg("-e")
        .arg(format!(
            "if loadfile('{}') then os.exit(0) else os.exit(1) end",
            // On Windows, Lua escapes path separators, so we ensure forward slashes
            file.to_slash_lossy()
        ))
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

#[cfg(target_family = "unix")]
async fn needs_wrapper(script: &Path) -> Result<bool, WrapBinaryError> {
    let content = String::from_utf8(tokio::fs::read(script).await?)?;
    Ok(!content.starts_with("#!/usr/bin/env "))
}

#[cfg(target_family = "windows")]
async fn needs_wrapper(_script: &Path) -> io::Result<bool> {
    Ok(true)
}

pub(crate) fn substitute_variables(
    input: &str,
    output_paths: &RockLayout,
    lua: &LuaInstallation,
    config: &Config,
) -> Result<String, VariableSubstitutionError> {
    variables::substitute(&[output_paths, lua, config], input)
}

pub(crate) fn format_path(path: &Path) -> String {
    let path_str = path.to_slash_lossy();
    if cfg!(windows) {
        path_str.to_string()
    } else {
        try_quote(&path_str)
            .map(|str| str.to_string())
            .unwrap_or(format!("'{}'", path_str))
    }
}

#[cfg(test)]
mod tests {
    use crate::config::ConfigBuilder;

    use super::*;

    #[tokio::test]
    async fn test_is_compatible_lua_script() {
        let config = ConfigBuilder::new().unwrap().build().unwrap();
        let lua_version = config.lua_version().unwrap();
        let lua = LuaInstallation::new(lua_version, &config);
        let valid_script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test/sample_lua_bin_script_valid");
        assert!(is_compatible_lua_script(&valid_script, &lua, &config).await);
        let invalid_script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test/sample_lua_bin_script_invalid");
        assert!(!is_compatible_lua_script(&invalid_script, &lua, &config).await);
    }

    #[tokio::test]
    async fn test_install_wrapped_binary() {
        let temp = assert_fs::TempDir::new().unwrap();
        let config = ConfigBuilder::new()
            .unwrap()
            .tree(Some(temp.to_path_buf()))
            .build()
            .unwrap();
        let lua_version = config.lua_version().unwrap();
        let lua = LuaInstallation::new(lua_version, &config);
        let tree = config.tree(lua_version.clone()).unwrap();
        let valid_script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test/sample_lua_bin_script_valid");
        let script_name = "test_script";
        let script_path = install_wrapped_binary(&valid_script, script_name, &tree, &lua, &config)
            .await
            .unwrap();

        #[cfg(unix)]
        set_executable_permissions(&script_path).await.unwrap();

        assert!(Command::new(script_path.to_string_lossy().to_string())
            .status()
            .await
            .is_ok_and(|status| status.success()));
    }
}
