use std::{
    io::{self, Cursor},
    path::Path,
    process::{ExitStatus, Stdio},
};

use crate::{
    build::{external_dependency::ExternalDependencyInfo, utils},
    config::{external_deps::ExternalDependencySearchConfig, Config, LuaVersion},
    hash::HasIntegrity,
    lua_rockspec::ExternalDependencySpec,
    operations::{self, UnpackError},
    package::PackageVersion,
    progress::{Progress, ProgressBar},
};
use bon::Builder;
use ssri::Integrity;
use tempdir::TempDir;
use thiserror::Error;
use tokio::process::Command;
use url::Url;

#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = _build, vis = ""))]
pub(crate) struct BuildLua<'a> {
    lua_version: &'a LuaVersion,
    install_dir: &'a Path,
    config: &'a Config,
    progress: &'a Progress<ProgressBar>,
}

#[derive(Debug, Error)]
pub enum BuildLuaError {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Unpack(#[from] UnpackError),
    #[error("source integrity mismatch.\nExpected: {expected},\nbut got: {actual}")]
    SourceIntegrityMismatch {
        expected: Integrity,
        actual: Integrity,
    },
    #[error("{name} failed.\n\n{status}\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}")]
    CommandFailure {
        name: String,
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
}

impl<State: build_lua_builder::State + build_lua_builder::IsComplete> BuildLuaBuilder<'_, State> {
    pub async fn build(self) -> Result<(), BuildLuaError> {
        let args = self._build();
        let lua_version = args.lua_version;
        match lua_version {
            LuaVersion::Lua51 | LuaVersion::Lua52 | LuaVersion::Lua53 | LuaVersion::Lua54 => {
                do_build_lua(args).await
            }
            LuaVersion::LuaJIT | LuaVersion::LuaJIT52 => do_build_luajit(args).await,
        }
    }
}

async fn do_build_luajit(args: BuildLua<'_>) -> Result<(), BuildLuaError> {
    unimplemented!()
}

async fn do_build_lua(args: BuildLua<'_>) -> Result<(), BuildLuaError> {
    let lua_version = args.lua_version;
    let progress = args.progress;

    let build_dir = TempDir::new("lux_lua_build_dir")
        .expect("failed to create lua_installation temp directory")
        .into_path();

    let (file_name, source_integrity, pkg_version): (String, Integrity, PackageVersion) =
        match lua_version {
            LuaVersion::Lua51 => (
                "lua-5.1.5.tar.gz".into(),
                "sha256-JkD8VqeV8p0o7xXhPDSkfiI5YLAkDoywqC2bBzhpUzM="
                    .parse()
                    .unwrap(),
                "5.1.5".parse().unwrap(),
            ),
            LuaVersion::Lua52 => (
                "lua-5.2.4.tar.gz".into(),
                "sha256-ueLkqtZ4mztjoFbUQveznw7Pyjrg8fwK5OlhRAG2n0s="
                    .parse()
                    .unwrap(),
                "5.2.4".parse().unwrap(),
            ),
            LuaVersion::Lua53 => (
                "lua-5.3.6.tar.gz".into(),
                "sha256-/F/Wm7hzYyPwJmcrG3I12mE9cXfnJViJOgvc0yBGbWA="
                    .parse()
                    .unwrap(),
                "5.3.6".parse().unwrap(),
            ),
            LuaVersion::Lua54 => (
                "lua-5.4.8.tar.gz".into(),
                "sha256-TxjdrhVOeT5G7qtyfFnvHAwMK3ROe5QhlxDXb1MGKa4="
                    .parse()
                    .unwrap(),
                "5.4.8".parse().unwrap(),
            ),
            LuaVersion::LuaJIT | LuaVersion::LuaJIT52 => unreachable!(),
        };

    let source_url: Url = format!("https://www.lua.org/ftp/{file_name}")
        .parse()
        .unwrap();

    progress.map(|p| p.set_message(format!("📥 Downloading {}", &source_url)));

    let response = reqwest::get(source_url)
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let hash = response.hash()?;

    if hash.matches(&source_integrity).is_none() {
        return Err(BuildLuaError::SourceIntegrityMismatch {
            expected: source_integrity,
            actual: hash,
        });
    }

    let cursor = Cursor::new(response);
    let mime_type = infer::get(cursor.get_ref()).map(|file_type| file_type.mime_type());
    operations::unpack::unpack(mime_type, cursor, true, file_name, &build_dir, progress).await?;

    if cfg!(target_env = "msvc") {
        do_build_lua_msvc(args, &build_dir, lua_version, pkg_version).await
    } else {
        do_build_lua_unix(args, &build_dir, lua_version, pkg_version).await
    }
}

async fn do_build_lua_unix(
    args: BuildLua<'_>,
    build_dir: &Path,
    lua_version: &LuaVersion,
    pkg_version: PackageVersion,
) -> Result<(), BuildLuaError> {
    let config = args.config;
    let progress = args.progress;
    let install_dir = args.install_dir;

    progress.map(|p| p.set_message(format!("🛠️ Building Lua {}", &pkg_version)));

    let readline_spec = ExternalDependencySpec {
        header: Some("readline/readline.h".into()),
        library: None,
    };
    let build_target = match ExternalDependencyInfo::probe(
        "readline",
        &readline_spec,
        &ExternalDependencySearchConfig::default(),
    ) {
        Ok(_) => {
            // NOTE: The Lua < 5.4 linux targets depend on readline
            if cfg!(target_os = "linux") {
                if matches!(&lua_version, LuaVersion::Lua54) {
                    "linux-readline"
                } else {
                    "linux"
                }
            } else if cfg!(target_os = "macos") {
                "macosx"
            } else if matches!(&lua_version, LuaVersion::Lua54) {
                "linux"
            } else {
                "generic"
            }
        }
        _ => "generic",
    };

    match Command::new(config.make_cmd())
        .current_dir(build_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg(build_target)
        .output()
        .await
    {
        Ok(output) if output.status.success() => utils::log_command_output(&output, config),
        Ok(output) => {
            return Err(BuildLuaError::CommandFailure {
                name: "build".into(),
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into(),
                stderr: String::from_utf8_lossy(&output.stderr).into(),
            });
        }
        Err(err) => return Err(BuildLuaError::Io(err)),
    };

    progress.map(|p| p.set_message(format!("💻 Installing Lua {}", &pkg_version)));

    match Command::new(config.make_cmd())
        .current_dir(build_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("install")
        .arg(format!("INSTALL_TOP={}", install_dir.display()))
        .output()
        .await
    {
        Ok(output) if output.status.success() => utils::log_command_output(&output, config),
        Ok(output) => {
            return Err(BuildLuaError::CommandFailure {
                name: "install".into(),
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into(),
                stderr: String::from_utf8_lossy(&output.stderr).into(),
            });
        }
        Err(err) => return Err(BuildLuaError::Io(err)),
    };

    Ok(())
}

async fn do_build_lua_msvc(
    args: BuildLua<'_>,
    build_dir: &Path,
    lua_version: &LuaVersion,
    pkg_version: PackageVersion,
) -> Result<(), BuildLuaError> {
    unimplemented!();
}

#[cfg(test)]
mod test {
    use assert_fs::{assert::PathAssert, prelude::PathChild};
    use predicates::prelude::predicate;

    use super::*;
    use crate::{
        config::{ConfigBuilder, LuaVersion},
        progress::MultiProgress,
    };

    #[tokio::test]
    async fn test_build_lua() {
        if std::env::var("LUX_SKIP_IMPURE_TESTS").unwrap_or("0".into()) == "1" {
            println!("Skipping impure test");
            return;
        }
        let progress = MultiProgress::new();
        for lua_version in [
            LuaVersion::Lua51,
            LuaVersion::Lua52,
            LuaVersion::Lua53,
            LuaVersion::Lua54,
        ] {
            let target_dir = assert_fs::TempDir::new().unwrap();
            let target_path = target_dir.to_path_buf();
            let user_tree = assert_fs::TempDir::new().unwrap();
            let config = ConfigBuilder::new()
                .unwrap()
                .user_tree(Some(user_tree.to_path_buf()))
                .lua_version(Some(lua_version))
                .build()
                .unwrap();
            let bar = Progress::Progress(progress.new_bar());
            BuildLua::new()
                .lua_version(config.lua_version().unwrap())
                .progress(&bar)
                .install_dir(&target_path)
                .config(&config)
                .build()
                .await
                .unwrap();
            let lua_bin = target_dir.child("bin").child("lua");
            lua_bin.assert(predicate::path::is_file());
            let lua_include_dir = target_dir.child("include");
            lua_include_dir.assert(predicate::path::is_dir());
            let lua_lib_dir = target_dir.child("lib");
            lua_lib_dir.assert(predicate::path::is_dir());
            let lua_lib = lua_lib_dir.child("liblua.a");
            lua_lib.assert(predicate::path::is_file());
        }
    }
}
