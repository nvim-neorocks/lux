use clap::Args;
use eyre::Result;
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
    let tree = current_project_or_user_tree(&config).unwrap();

    let mut path = Paths::new(&tree)?;

    let shell: PathBuf = match env::var("SHELL") {
        Ok(val) => PathBuf::from(val),
        Err(_) => {
            #[cfg(target_os = "linux")]
            let fallback = which("bash")
                .map_err(|_| eyre::eyre!("Cannot find shell `bash` on your system!"))?;

            #[cfg(target_os = "windows")]
            let fallback = which("cmd.exe")
                .map_err(|_| eyre::eyre!("Cannot find shell `cmd.exe` on your system!"))?;

            #[cfg(target_os = "macos")]
            let fallback =
                which("zsh").map_err(|_| eyre::eyre!("Cannot find shell `zsh` on your system!"))?;

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
    if !data.no_loader {
        if tree.version().lux_lib_dir().is_none() {
            eprintln!("⚠️ WARNING: lux-lua library not found.\nCannot use the `lux.loader`.");
            eprintln!("To suppress this warning, set the --no-loader option.");
            std::process::exit(1);
        }
    }

    let _ = Command::new(&shell)
        .env("PATH", path.path_prepended().joined())
        .env("LUA_PATH", path.package_path().joined())
        .env("LUA_CPATH", path.package_cpath().joined())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?
        .wait()
        .await?;

    Ok(())
}
