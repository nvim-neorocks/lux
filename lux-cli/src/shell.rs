use clap::Args;
use eyre::Result;
use lux_lib::config::Config;

use std::env;
use tokio::process::Command;

use lux_lib::{path::Paths, project::Project};

use std::process::Stdio;

#[derive(Args)]
pub struct Shell {
    #[arg(long)]
    test: bool,
    #[arg(long)]
    build: bool,
}

pub async fn shell(data: Shell, config: Config) -> Result<()> {
    let project =
        Project::current()?.ok_or_else(|| eyre::eyre!("Not in a Lux project directory"))?;
    let project_tree = project.tree(&config)?;

    let mut path = Paths::new(&project_tree)?;

    let shell = env::var("SHELL").unwrap_or_else(|_| {
        #[cfg(target_os = "linux")]
        return "/bin/bash".to_string();
        #[cfg(target_os = "windows")]
        return "cmd.exe".to_string();
        #[cfg(target_os = "macos")]
        return "/bin/zsh".to_string();
    });

    if data.test {
        let test_tree_path = project_tree.test_tree(&config)?;
        let test_path = Paths::new(&test_tree_path)?;
        path.prepend(&test_path);
    }

    if data.build {
        let build_tree_path = project_tree.build_tree(&config)?;
        let build_path = Paths::new(&build_tree_path)?;
        path.prepend(&build_path);
    }

    let loader_init = if project_tree.version().lux_lib_dir().is_none() {
        eprintln!(
            "⚠️ WARNING: lux-lua library not found.
Cannot use the `lux.loader`.
            "
        );
        "".to_string()
    } else {
        path.init()
    };
    let lua_init = format!(
        r#"print([==[{}]==])
        exit = os.exit
        print([==[
{}
To exit type 'exit()' or <C-d>.
]==])
    "#,
        "hi", loader_init
    );

    let _ = Command::new(&shell)
        .env(
            "PATH",
            format!(
                "{}:{}",
                path.path().joined(),
                env::var("PATH").unwrap_or_default()
            ),
        )
        .env("LUA_PATH", path.package_path().joined())
        .env("LUA_CPATH", path.package_cpath().joined())
        .env("LUA_INIT", lua_init)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?
        .wait()
        .await?;

    Ok(())
}
