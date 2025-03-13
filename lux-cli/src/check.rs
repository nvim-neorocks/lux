use eyre::{OptionExt, Result};
use lux_lib::{
    config::Config,
    operations::{Install, Run},
    package::PackageReq,
    progress::MultiProgress,
    project::Project,
};

pub async fn check(config: Config) -> Result<()> {
    let project = Project::current()?.ok_or_eyre("Not in a project!")?;

    let luacheck: PackageReq = "luacheck".parse()?;
    Install::new(&project.tree(&config)?, &config)
        .package(luacheck.into())
        .progress(MultiProgress::new_arc())
        .install()
        .await?;

    Run::new("luacheck", Some(&project), &config)
        .arg(project.root().to_string_lossy())
        .arg("--exclude-files")
        .arg(project.tree(&config)?.root().to_string_lossy())
        .run()
        .await?;

    Ok(())
}
