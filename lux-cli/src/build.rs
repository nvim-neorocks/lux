use eyre::Context;
use itertools::Itertools;
use std::sync::Arc;

use clap::Args;
use eyre::Result;
use lux_lib::{
    build::{self, BuildBehaviour},
    config::Config,
    luarocks::luarocks_installation::LuaRocksInstallation,
    operations::{Install, PackageInstallSpec, Sync},
    progress::MultiProgress,
    project::Project,
    rockspec::Rockspec,
    tree,
};

#[derive(Args, Default)]
pub struct Build {
    /// Ignore the project's lockfile and don't create one.
    #[arg(long)]
    no_lock: bool,

    /// Build only the dependencies
    #[arg(long)]
    only_deps: bool,
}

pub async fn build(data: Build, config: Config) -> Result<()> {
    let project = Project::current_or_err()?;
    let progress_arc = MultiProgress::new_arc();
    let progress = Arc::clone(&progress_arc);

    let project_toml = project.toml().into_local()?;
    let project_tree = project.tree(&config)?;

    let dependencies = project_toml
        .dependencies()
        .current_platform()
        .iter()
        .cloned()
        .collect_vec();

    let build_dependencies = project_toml
        .build_dependencies()
        .current_platform()
        .iter()
        .cloned()
        .collect_vec();

    let build_tree = project.build_tree(&config)?;
    let luarocks = LuaRocksInstallation::new(&config, build_tree.clone())?;

    if data.no_lock {
        let dependencies_to_install = dependencies
            .into_iter()
            .filter(|dep| {
                project_tree
                    .match_rocks(dep.package_req())
                    .is_ok_and(|rock_match| !rock_match.is_found())
            })
            .map(|dep| {
                PackageInstallSpec::new(dep.clone().into_package_req(), tree::EntryType::Entrypoint)
                    .pin(*dep.pin())
                    .opt(*dep.opt())
                    .maybe_source(dep.source().clone())
                    .build()
            })
            .collect();

        Install::new(&config)
            .packages(dependencies_to_install)
            .project(&project)?
            .progress(progress.clone())
            .install()
            .await?;

        let build_dependencies_to_install = build_dependencies
            .into_iter()
            .filter(|dep| {
                project_tree
                    .match_rocks(dep.package_req())
                    .is_ok_and(|rock_match| !rock_match.is_found())
            })
            .map(|dep| {
                PackageInstallSpec::new(dep.clone().into_package_req(), tree::EntryType::Entrypoint)
                    .pin(*dep.pin())
                    .opt(*dep.opt())
                    .maybe_source(dep.source().clone())
                    .build()
            })
            .collect_vec();

        if !build_dependencies_to_install.is_empty() {
            let bar = progress.map(|p| p.new_bar());
            luarocks.ensure_installed(&bar).await?;
            Install::new(&config)
                .packages(build_dependencies_to_install)
                .tree(build_tree)
                .progress(progress.clone())
                .install()
                .await?;
        }
    } else {
        Sync::new(&project, &config)
            .progress(progress.clone())
            .sync_dependencies()
            .await
            .wrap_err(
                "
syncing dependencies with the project lockfile failed.
Use --no-lock to force a new build.
",
            )?;

        Sync::new(&project, &config)
            .progress(progress.clone())
            .sync_build_dependencies()
            .await
            .wrap_err(
                "
syncing build dependencies with the project lockfile failed.
Use --no-lock to force a new build.
",
            )?;
    }

    if !data.only_deps {
        let package = build::Build::new(
            &project_toml,
            &project_tree,
            tree::EntryType::Entrypoint,
            &config,
            &progress.map(|p| p.new_bar()),
        )
        .behaviour(BuildBehaviour::Force)
        .build()
        .await?;

        let lockfile = project_tree.lockfile()?;
        let dependencies = lockfile
            .rocks()
            .iter()
            .filter_map(|(pkg_id, value)| {
                if lockfile.is_entrypoint(pkg_id) {
                    Some(value)
                } else {
                    None
                }
            })
            .cloned()
            .collect_vec();
        let mut lockfile = lockfile.write_guard();
        lockfile.add_entrypoint(&package);
        for dep in dependencies {
            lockfile.add_dependency(&package, &dep);
            lockfile.remove_entrypoint(&dep);
        }
    }

    Ok(())
}
