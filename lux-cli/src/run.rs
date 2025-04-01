use clap::Args;
use eyre::{OptionExt, Result};
use lux_lib::{config::Config, operations, project::Project};

use crate::build::{self, Build};

#[derive(Args)]
pub struct Run {
    args: Vec<String>,

    #[clap(flatten)]
    build: Build,
}

pub async fn run(run_args: Run, config: Config) -> Result<()> {
    let project = Project::current()?.ok_or_eyre("not in a project!")?;

    build::build(run_args.build, config.clone()).await?;

    operations::run(&project, &run_args.args, &config).await?;

    Ok(())
}
