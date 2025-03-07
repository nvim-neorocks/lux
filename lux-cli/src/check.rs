use eyre::{OptionExt, Result};
use lux_lib::{
    build::BuildBehaviour,
    config::Config,
    lockfile::{OptState, PinnedState},
    operations::{Install, PackageInstallSpec, Run},
    progress::MultiProgress,
    project::Project,
};

pub async fn check(config: Config) -> Result<()> {
    let project = Project::current()?.ok_or_eyre("Not in a project!")?;

    let luacheck = PackageInstallSpec::new(
        "luacheck".parse()?,
        BuildBehaviour::default(),
        PinnedState::default(),
        OptState::default(),
        true,
    );

    Install::new(&project.tree(&config)?, &config)
        .package(luacheck)
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
