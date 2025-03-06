use eyre::{Context, OptionExt};
use itertools::Itertools;
use std::sync::Arc;

use clap::Args;
use eyre::Result;
use lux_lib::{
    build::{self, BuildBehaviour},
    config::Config,
    luarocks::luarocks_installation::LuaRocksInstallation,
    operations::{Install, PackageInstallSpec, Sync},
    package::PackageName,
    progress::MultiProgress,
    project::Project,
    rockspec::Rockspec,
};

#[derive(Args, Default)]
pub struct Build {
    /// Ignore the project's lockfile and don't create one.
    #[arg(long)]
    no_lock: bool,
}

pub async fn build(data: Build, config: Config) -> Result<()> {
    let project = Project::current()?.ok_or_eyre("Not in a project!")?;
    let progress_arc = MultiProgress::new_arc();
    let progress = Arc::clone(&progress_arc);

    let tree = project.tree(&config)?;
    let rocks = project.toml().into_local()?;

    let dependencies = rocks
        .dependencies()
        .current_platform()
        .iter()
        .filter(|package| !package.name().eq(&PackageName::new("lua".into())))
        .cloned()
        .collect_vec();

    let build_dependencies = rocks
        .build_dependencies()
        .current_platform()
        .iter()
        .filter(|package| !package.name().eq(&PackageName::new("lua".into())))
        .cloned()
        .collect_vec();

    let luarocks = LuaRocksInstallation::new(&config)?;

    if data.no_lock {
        let dependencies_to_install = dependencies
            .into_iter()
            .filter(|dep| {
                tree.match_rocks(dep.package_req())
                    .is_ok_and(|rock_match| !rock_match.is_found())
            })
            .map(|dep| {
                PackageInstallSpec::new(
                    dep.clone().into_package_req(),
                    BuildBehaviour::default(),
                    *dep.pin(),
                    *dep.opt(),
                )
            });

        Install::new(&tree, &config)
            .packages(dependencies_to_install)
            .project(&project)
            .progress(progress.clone())
            .install()
            .await?;

        let build_dependencies_to_install = build_dependencies
            .into_iter()
            .filter(|dep| {
                tree.match_rocks(dep.package_req())
                    .is_ok_and(|rock_match| !rock_match.is_found())
            })
            .map(PackageInstallSpec::from)
            .collect_vec();

        if !build_dependencies_to_install.is_empty() {
            let bar = progress.map(|p| p.new_bar());
            luarocks.ensure_installed(&bar).await?;
            Install::new(luarocks.tree(), luarocks.config())
                .packages(build_dependencies_to_install)
                .project(&project)
                .progress(progress.clone())
                .install()
                .await?;
        }
    } else {
        let mut project_lockfile = project.lockfile()?.write_guard();

        Sync::new(&tree, &mut project_lockfile, &config)
            .progress(progress.clone())
            .packages(dependencies)
            .sync_dependencies()
            .await
            .wrap_err(
                "
syncing dependencies with the project lockfile failed.
Use --ignore-lockfile to force a new build.
",
            )?;

        Sync::new(luarocks.tree(), &mut project_lockfile, luarocks.config())
            .progress(progress.clone())
            .packages(build_dependencies)
            .sync_build_dependencies()
            .await
            .wrap_err(
                "
syncing build dependencies with the project lockfile failed.
Use --ignore-lockfile to force a new build.
",
            )?;
    }

    build::Build::new(&rocks, &tree, &config, &progress.map(|p| p.new_bar()))
        .behaviour(BuildBehaviour::Force)
        .build()
        .await?;

    Ok(())
}
