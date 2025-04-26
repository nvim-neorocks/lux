use eyre::{OptionExt, Result};
use lux_lib::{
    config::Config,
    operations::{Exec, Install, PackageInstallSpec},
    progress::MultiProgress,
    project::Project,
    tree,
};

pub async fn check(config: Config) -> Result<()> {
    let project = Project::current()?.ok_or_eyre("Not in a project!")?;

    let luacheck =
        PackageInstallSpec::new("luacheck".parse()?, tree::EntryType::Entrypoint).build();

    Install::new(&project.tree(&config)?, &config)
        .package(luacheck)
        .progress(MultiProgress::new_arc())
        .install()
        .await?;

    Exec::new("luacheck", Some(&project), &config)
        .arg(project.root().to_string_lossy())
        .arg("--exclude-files")
        .arg(project.tree(&config)?.root().to_string_lossy())
        .exec()
        .await?;

    Ok(())
}
