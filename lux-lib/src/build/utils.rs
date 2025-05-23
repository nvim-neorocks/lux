use crate::{
    config::Config,
    lua_installation::LuaInstallation,
    lua_rockspec::{DeploySpec, LuaModule, ModulePaths},
    tree::{RockLayout, Tree},
};
use itertools::Itertools;
use path_slash::PathExt;
use shlex::try_quote;
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::{ExitStatus, Output, Stdio},
    string::FromUtf8Error,
};
use target_lexicon::Triple;
use thiserror::Error;
use tokio::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::{
    external_dependency::ExternalDependencyInfo,
    variables::{self, VariableSubstitutionError},
};

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

#[derive(Error, Debug)]
pub enum OutputValidationError {
    #[error("compilation failed.\nstatus: {status}\nstdout: {stdout}\nstderr: {stderr}")]
    CommandFailure {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
}

fn validate_output(output: Output) -> Result<(), OutputValidationError> {
    if !output.status.success() {
        return Err(OutputValidationError::CommandFailure {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
        });
    }
    Ok(())
}

#[derive(Error, Debug)]
pub enum CompileCFilesError {
    #[error("IO operation while compiling C files: {0}")]
    Io(#[from] io::Error),
    #[error("failed to compile intermediates from C files: {0}")]
    CompileIntermediates(cc::Error),
    #[error("error compiling C files (compilation failed): {0}")]
    Compilation(#[from] cc::Error),
    #[error("error compiling C files (output validation failed): {0}")]
    OutputValidation(#[from] OutputValidationError),
    #[error("compiling C files succeeded, but the expected library {0} was not created")]
    LibOutputNotCreated(String),
}

/// Compiles a set of C files into a single dynamic library and places them under `{target_dir}/{target_file}`.
/// # Panics
/// Panics if no parent or no filename can be determined for the target path.
pub(crate) fn compile_c_files(
    files: &Vec<PathBuf>,
    target_module: &LuaModule,
    target_dir: &Path,
    lua: &LuaInstallation,
) -> Result<(), CompileCFilesError> {
    let target = target_dir.join(target_module.to_lib_path());

    let parent = target.parent().expect("Couldn't determine parent");
    let file = target
        .file_name()
        .expect("Couldn't determine filename")
        .to_string_lossy()
        .to_string();

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
        .files(files)
        .host(std::env::consts::OS)
        .include(&lua.include_dir)
        .opt_level(3)
        .out_dir(intermediate_dir)
        .target(&host.to_string());

    let compiler = build.try_get_compiler()?;
    // Suppress all warnings
    if compiler.is_like_msvc() {
        build.flag("-W0");
    } else {
        build.flag("-w");
    }
    for arg in lua.define_flags() {
        build.flag(&arg);
    }

    let objects = build
        .try_compile_intermediates()
        .map_err(CompileCFilesError::CompileIntermediates)?;

    let output_path = parent.join(&file);

    let output = if compiler.is_like_msvc() {
        let def_temp_dir = tempdir::TempDir::new("msvc-def")?.into_path().to_path_buf();
        let def_file = mk_def_file(def_temp_dir, &file, target_module)?;
        compiler
            .to_command()
            .arg("/NOLOGO")
            .args(&objects)
            .arg("/LD")
            .arg("/link")
            .arg(format!("/DEF:{}", def_file.display()))
            .arg(format!("/OUT:{}", output_path.display()))
            .args(lua.lib_link_args(&compiler))
            .output()?
    } else {
        build
            .shared_flag(true)
            .try_get_compiler()?
            .to_command()
            .args(vec!["-o".into(), output_path.to_string_lossy().to_string()])
            .args(lua.lib_link_args(&compiler))
            .args(&objects)
            .output()?
    };
    validate_output(output)?;

    if output_path.exists() {
        Ok(())
    } else {
        Err(CompileCFilesError::LibOutputNotCreated(
            output_path.to_slash_lossy().to_string(),
        ))
    }
}

/// On MSVC, we need to create Lua definitions manually
fn mk_def_file(
    dir: PathBuf,
    output_file_name: &str,
    target_module: &LuaModule,
) -> io::Result<PathBuf> {
    let mut def_file: PathBuf = dir.join(output_file_name);
    def_file.set_extension(".def");
    let exported_name = target_module.to_string().replace(".", "_");
    let exported_name = exported_name
        .split_once('-')
        .map(|(_, after_hyphen)| after_hyphen.to_string())
        .unwrap_or_else(|| exported_name.clone());
    let content = format!(
        r#"EXPORTS
luaopen_{}
"#,
        exported_name
    );
    std::fs::write(&def_file, content)?;
    Ok(def_file)
}

// TODO: (#261): special cases for mingw/cygwin?

/// the extension for Lua shared libraries.
pub(crate) fn lua_dylib_extension() -> &'static str {
    if cfg!(target_env = "msvc") {
        "dll"
    } else {
        "so"
    }
}

/// the extension for Lua static libraries.
pub(crate) fn lua_lib_extension() -> &'static str {
    if cfg!(target_env = "msvc") {
        "lib"
    } else {
        "a"
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

#[derive(Error, Debug)]
pub enum CompileCModulesError {
    #[error("IO operation while compiling C modules: {0}")]
    Io(#[from] io::Error),
    #[error("failed to compile intermediates from C modules: {0}")]
    CompileIntermediates(cc::Error),
    #[error("error compiling C modules (compilation failed): {0}")]
    Compilation(#[from] cc::Error),
    #[error("error compiling C modules (output validation failed): {0}")]
    OutputValidation(#[from] OutputValidationError),
    #[error("compiling C modules succeeded, but the expected library {0} was not created")]
    LibOutputNotCreated(String),
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
) -> Result<(), CompileCModulesError> {
    let target = target_dir.join(target_module.to_lib_path());

    let parent = target.parent().expect("Couldn't determine parent");
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
        .files(source_files)
        .host(std::env::consts::OS)
        .includes(&include_dirs)
        .include(&lua.include_dir)
        .opt_level(3)
        .out_dir(intermediate_dir)
        .target(&host.to_string());

    let compiler = build.try_get_compiler()?;
    let is_msvc = compiler.is_like_msvc();
    // Suppress all warnings
    if is_msvc {
        build.flag("-W0");
    } else {
        build.flag("-w");
    }
    for arg in lua.define_flags() {
        build.flag(&arg);
    }

    // `cc::Build` has no `defines()` function, so we manually feed in the
    // definitions in a verbose loop
    for (name, value) in &data.defines {
        build.define(name, value.as_deref());
    }

    let file = target
        .file_name()
        .expect("Couldn't determine filename")
        .to_string_lossy()
        .to_string();
    // See https://github.com/rust-lang/cc-rs/issues/594#issuecomment-2110551057
    let objects = build
        .try_compile_intermediates()
        .map_err(CompileCModulesError::CompileIntermediates)?;

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

    let output_path = parent.join(&file);
    let output = if is_msvc {
        let def_temp_dir = tempdir::TempDir::new("msvc-def")?.into_path().to_path_buf();
        let def_file = mk_def_file(def_temp_dir, &file, target_module)?;
        build
            .try_get_compiler()?
            .to_command()
            .arg("/NOLOGO")
            .args(&objects)
            .arg("/LD")
            .arg("/link")
            .arg(format!("/DEF:{}", def_file.display()))
            .arg(format!("/OUT:{}", output_path.display()))
            .args(lua.lib_link_args(&build.try_get_compiler()?))
            .args(libdir_args)
            .args(library_args)
            .output()?
    } else {
        build
            .shared_flag(true)
            .try_get_compiler()?
            .to_command()
            .args(vec!["-o".into(), output_path.to_string_lossy().to_string()])
            .args(lua.lib_link_args(&build.try_get_compiler()?))
            .args(&objects)
            .args(libdir_args)
            .args(library_args)
            .output()?
    };
    validate_output(output)?;

    if output_path.exists() {
        Ok(())
    } else {
        Err(CompileCModulesError::LibOutputNotCreated(
            output_path.to_slash_lossy().to_string(),
        ))
    }
}

#[derive(Debug, Error)]
pub enum InstallBinaryError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("error wrapping binary: {0}")]
    Wrap(#[from] WrapBinaryError),
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum WrapBinaryError {
    Io(#[from] io::Error),
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
    external_dependencies: &HashMap<String, ExternalDependencyInfo>,
    config: &Config,
) -> Result<String, VariableSubstitutionError> {
    variables::substitute(&[output_paths, lua, external_dependencies, config], input)
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
            .user_tree(Some(temp.to_path_buf()))
            .build()
            .unwrap();
        let lua_version = config.lua_version().unwrap();
        let lua = LuaInstallation::new(lua_version, &config);
        let tree = config.user_tree(lua_version.clone()).unwrap();
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
