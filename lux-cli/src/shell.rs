use clap::Args;
use eyre::{eyre, Result, WrapErr};
use lux_lib::{config::Config, path::PackagePath, path::Paths};
use which::which;

use std::{env, path::PathBuf};
use tokio::process::Command;

use std::process::Stdio;
use std::str::FromStr;

use super::utils::project::current_project_or_user_tree;

#[derive(Args)]
pub struct Shell {
    /// Add test dependencies to the shell's paths
    #[arg(long)]
    test: bool,

    /// Add build dependencies to the shell's paths
    #[arg(long)]
    build: bool,

    /// Disable the Lux loader.
    /// If a rock has conflicting transitive dependencies,
    /// disabling the Lux loader may result in the wrong modules being loaded.
    #[arg(long)]
    no_loader: bool,
}

pub async fn shell(data: Shell, config: Config) -> Result<()> {
    if env::var("LUX_SHELL").is_ok() {
        return Err(eyre!("Already in a Lux shell."));
    }

    let tree = current_project_or_user_tree(&config).unwrap();

    let mut path = Paths::new(&tree)?;

    let shell: PathBuf = match env::var("SHELL") {
        Ok(val) => PathBuf::from(val),
        Err(_) => {
            #[cfg(target_os = "linux")]
            let fallback = which("bash").wrap_err("Cannot find `bash` on your system!")?;

            #[cfg(target_os = "windows")]
            let fallback = which("cmd.exe").wrap_err("Cannot find `cmd.exe` on your system!")?;

            #[cfg(target_os = "macos")]
            let fallback = which("zsh").wrap_err("Cannot find `zsh` on your system!")?;

            fallback
        }
    };

    if data.test {
        let test_tree_path = tree.test_tree(&config)?;
        let test_path = Paths::new(&test_tree_path)?;
        path.prepend(&test_path);
    }

    if data.build {
        let build_tree_path = tree.build_tree(&config)?;
        let build_path = Paths::new(&build_tree_path)?;
        path.prepend(&build_path);
    }

    let mut lua_path = PackagePath::from_str(env::var("LUA_PATH").unwrap_or_default().as_str())
        .unwrap_or_default();
    lua_path.prepend(path.package_path());

    let lua_cpath = PackagePath::from_str(env::var("LUA_CPATH").unwrap_or_default().as_str())
        .unwrap_or_default();
    lua_path.prepend(path.package_cpath());

    let lua_init = if data.no_loader {
        None
    } else if tree.version().lux_lib_dir().is_none() {
        eprintln!(
            "⚠️ WARNING: lux-lua library not found. 
    Cannot use the `lux.loader`. 
    To suppress this warning, set the `--no-loader` option. 
                    "
        );
        None
    } else {
        Some(path.init())
    };

    let _ = Command::new(&shell)
        .env("PATH", path.path_prepended().joined())
        .env("LUA_PATH", lua_path.joined())
        .env("LUA_CPATH", lua_cpath.joined())
        .env("LUA_INIT", lua_init.unwrap_or_default())
        .env("LUX_SHELL", "")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?
        .wait()
        .await?;

    Ok(())
}
