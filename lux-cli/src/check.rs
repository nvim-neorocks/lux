use clap::Args;
use eyre::Result;
use lux_lib::{
    config::Config,
    operations::{Exec, Install, PackageInstallSpec},
    progress::MultiProgress,
    project::Project,
    tree,
};

#[derive(Args)]
pub struct Check {
    /// Arguments to pass to the luacheck command.
    check_args: Option<Vec<String>>,
}

pub async fn check(check: Check, config: Config) -> Result<()> {
    let project = Project::current_or_err()?;

    let luacheck =
        PackageInstallSpec::new("luacheck".parse()?, tree::EntryType::Entrypoint).build();

    let check_args: Vec<String> = check.check_args.unwrap_or_default();

    Install::new(&config)
        .package(luacheck)
        .project(&project)?
        .progress(MultiProgress::new_arc())
        .install()
        .await?;

    Exec::new("luacheck", Some(&project), &config)
        .arg(project.root().to_string_lossy())
        .args(check_args)
        .arg("--exclude-files")
        .arg(".lux")
        .arg(".direnv")
        .arg(project.tree(&config)?.root().to_string_lossy())
        .exec()
        .await?;

    Ok(())
}
