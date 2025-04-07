use std::process::Command;

use clap::Args;
use eyre::{eyre, Result};
use itertools::Itertools;
use lux_lib::{
    config::{Config, LuaVersion},
    operations::{self, LuaBinary},
    project::Project,
    rockspec::LuaVersionCompatibility,
};

use crate::build::{self, Build};

#[derive(Args, Default)]
#[clap(disable_help_flag = true)]
pub struct RunLua {
    /// Arguments to pass to Lua. See `lua -h`.
    args: Option<Vec<String>>,

    /// Path to the Lua interpreter to use
    #[arg(long)]
    lua: Option<String>,

    /// Print help
    #[arg(long)]
    help: bool,

    #[clap(flatten)]
    build: Build,
}

pub async fn run_lua(run_lua: RunLua, config: Config) -> Result<()> {
    let lua_cmd = run_lua.lua.map(LuaBinary::Custom).unwrap_or(LuaBinary::Lua);

    if run_lua.help {
        return print_lua_help(&lua_cmd);
    }

    let project = Project::current()?;
    let (lua_version, root, tree) = match &project {
        Some(prj) => (
            prj.toml().lua_version_matches(&config)?,
            prj.root().to_path_buf(),
            prj.tree(&config)?,
        ),
        None => {
            let version = LuaVersion::from(&config)?;
            (
                version.clone(),
                std::env::current_dir()?,
                config.tree(version)?,
            )
        }
    };

    if project.is_some() {
        build::build(run_lua.build, config.clone()).await?;
    }

    operations::run_lua(
        &root,
        &tree,
        lua_version,
        lua_cmd,
        &run_lua.args.unwrap_or_default(),
    )
    .await?;

    Ok(())
}

fn print_lua_help(lua_cmd: &LuaBinary) -> Result<()> {
    let output = match Command::new(lua_cmd.to_string())
        // HACK: This fails with exit 1, because lua doesn't actually have a help flag (╯°□°)╯︵ ┻━┻
        .arg("-h")
        .output()
    {
        Ok(output) => Ok(output),
        Err(err) => Err(eyre!("Failed to run {}: {}", lua_cmd, err)),
    }?;
    let lua_help = String::from_utf8_lossy(&output.stderr)
        .lines()
        .skip(2)
        .map(|line| format!("  {}", line))
        .collect_vec()
        .join("\n");
    print!(
        "
Usage: lx lua -- [LUA_OPTIONS] [SCRIPT [ARGS]]...

Arguments:
  [LUA_OPTIONS]...
{}

Options:
  --lua       Path to the Lua interpreter to use
  --no-lock   When building a project, ignore the project's lockfile and don't create one
  -h, --help  Print help
",
        lua_help,
    );
    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use lux_lib::config::ConfigBuilder;
    use serial_test::serial;

    use super::*;

    // FIXME: This can fail locally if there's a lux project in the lux root
    #[serial]
    #[tokio::test]
    async fn test_run_lua() {
        let args = RunLua {
            args: Some(vec!["-v".into()]),
            ..RunLua::default()
        };
        let temp: PathBuf = assert_fs::TempDir::new().unwrap().path().into();
        let config = ConfigBuilder::new()
            .unwrap()
            .tree(Some(temp.clone()))
            .luarocks_tree(Some(temp))
            .build()
            .unwrap();
        run_lua(args, config).await.unwrap()
    }
}
