use clap::Args;
use eyre::{eyre, Result, WrapErr};
use lux_lib::{config::Config, path::Paths};
use which::which;

use std::{env, path::PathBuf};
use tokio::process::Command;

use std::process::Stdio;

use super::utils::project::current_project_or_user_tree;

#[derive(Args)]
pub struct Shell {
    /// Whether to load test dependencies into the new shell
    #[arg(long)]
    test: bool,

    /// Whether to load build dependencies into the new shell
    #[arg(long)]
    build: bool,

    /// Suppresses the warning for checking if the lux-lua lib exists
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
        .env("LUA_PATH", path.package_path().joined())
        .env("LUA_CPATH", path.package_cpath().joined())
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
