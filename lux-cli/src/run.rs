use clap::Args;
use eyre::{OptionExt, Result};
use lux_lib::{config::Config, operations, project::Project};

#[derive(Args)]
pub struct Run {
    args: Vec<String>,
}

pub async fn run(run_args: Run, config: Config) -> Result<()> {
    let project = Project::current()?.ok_or_eyre("not in a project!")?;

    operations::run(&project, &run_args.args, &config).await?;

    Ok(())
}
