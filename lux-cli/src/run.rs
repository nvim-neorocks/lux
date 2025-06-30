use clap::Args;
use eyre::{OptionExt, Result};
use lux_lib::{config::Config, operations, project::Project};

use crate::build::{self, Build};

#[derive(Args)]
pub struct Run {
    args: Vec<String>,

    /// Do not add `require('lux').loader()` to `LUA_INIT`.
    /// If a rock has conflicting transitive dependencies,
    /// disabling the Lux loader may result in the wrong modules being loaded.
    #[clap(default_value_t = false)]
    #[arg(long)]
    no_loader: bool,

    #[clap(flatten)]
    build: Build,
}

pub async fn run(run_args: Run, config: Config) -> Result<()> {
    let project = Project::current()?.ok_or_eyre("not in a project!")?;

    build::build(run_args.build, config.clone()).await?;

    operations::Run::new()
        .project(&project)
        .args(&run_args.args)
        .config(&config)
        .disable_loader(run_args.no_loader)
        .run()
        .await?;

    Ok(())
}
