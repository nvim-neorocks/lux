use std::sync::Arc;

use bon::Builder;
use itertools::Itertools;
use thiserror::Error;

use crate::{
    build::{Build, BuildBehaviour, BuildError},
    config::Config,
    lockfile::LocalPackage,
    luarocks::luarocks_installation::{LuaRocksError, LuaRocksInstallError, LuaRocksInstallation},
    progress::{MultiProgress, Progress},
    project::{project_toml::LocalProjectTomlValidationError, Project, ProjectTreeError},
    rockspec::Rockspec,
    tree::{self, TreeError},
};

use super::{Install, InstallError, PackageInstallSpec, Sync, SyncError};

#[derive(Debug, Error)]
pub enum BuildProjectError {
    #[error(transparent)]
    LocalProjectTomlValidation(#[from] LocalProjectTomlValidationError),
    #[error(transparent)]
    ProjectTree(#[from] ProjectTreeError),
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error(transparent)]
    LuaRocks(#[from] LuaRocksError),
    #[error(transparent)]
    LuaRocksInstall(#[from] LuaRocksInstallError),
    #[error("error installind dependencies:\n{0}")]
    InstallDependencies(InstallError),
    #[error("error installind build dependencies:\n{0}")]
    InstallBuildDependencies(InstallError),
    #[error("syncing dependencies with the project lockfile failed.\nUse --no-lock to force a new build.\n\n{0}")]
    SyncDependencies(SyncError),
    #[error("syncing build dependencies with the project lockfile failed.\nUse --no-lock to force a new build.\n\n{0}")]
    SyncBuildDependencies(SyncError),
    #[error("error building project:\n{0}")]
    Build(#[from] BuildError),
}

#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = _build, vis = ""))]
pub struct BuildProject<'a> {
    #[builder(start_fn)]
    project: &'a Project,

    #[builder(start_fn)]
    config: &'a Config,

    /// Ignore the project's lockfile and don't create one
    no_lock: bool,

    /// Build only the dependencies
    only_deps: bool,

    #[builder(default = MultiProgress::new_arc())]
    progress: Arc<Progress<MultiProgress>>,
}

impl<State: build_project_builder::State + build_project_builder::IsComplete>
    BuildProjectBuilder<'_, State>
{
    /// Returns `Some` if the `only_deps` option is set to `false`.
    pub async fn build(self) -> Result<Option<LocalPackage>, BuildProjectError> {
        let args = self._build();
        let project = args.project;
        let config = args.config;
        let progress_arc = args.progress;
        let progress = Arc::clone(&progress_arc);

        let project_toml = project.toml().into_local()?;
        let project_tree = project.tree(config)?;

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

        let build_tree = project.build_tree(config)?;
        let luarocks = LuaRocksInstallation::new(config, build_tree.clone())?;

        if args.no_lock {
            let dependencies_to_install = dependencies
                .into_iter()
                .filter(|dep| {
                    project_tree
                        .match_rocks(dep.package_req())
                        .is_ok_and(|rock_match| !rock_match.is_found())
                })
                .map(|dep| {
                    PackageInstallSpec::new(
                        dep.clone().into_package_req(),
                        tree::EntryType::Entrypoint,
                    )
                    .pin(*dep.pin())
                    .opt(*dep.opt())
                    .maybe_source(dep.source().clone())
                    .build()
                })
                .collect();

            Install::new(config)
                .packages(dependencies_to_install)
                .project(project)?
                .progress(progress.clone())
                .install()
                .await
                .map_err(BuildProjectError::InstallDependencies)?;

            let build_dependencies_to_install = build_dependencies
                .into_iter()
                .filter(|dep| {
                    project_tree
                        .match_rocks(dep.package_req())
                        .is_ok_and(|rock_match| !rock_match.is_found())
                })
                .map(|dep| {
                    PackageInstallSpec::new(
                        dep.clone().into_package_req(),
                        tree::EntryType::Entrypoint,
                    )
                    .pin(*dep.pin())
                    .opt(*dep.opt())
                    .maybe_source(dep.source().clone())
                    .build()
                })
                .collect_vec();

            if !build_dependencies_to_install.is_empty() {
                let bar = progress.map(|p| p.new_bar());
                luarocks.ensure_installed(&bar).await?;
                Install::new(config)
                    .packages(build_dependencies_to_install)
                    .tree(build_tree)
                    .progress(progress.clone())
                    .install()
                    .await
                    .map_err(BuildProjectError::InstallBuildDependencies)?;
            }
        } else {
            Sync::new(project, config)
                .progress(progress.clone())
                .sync_dependencies()
                .await
                .map_err(BuildProjectError::SyncDependencies)?;

            Sync::new(project, config)
                .progress(progress.clone())
                .sync_build_dependencies()
                .await
                .map_err(BuildProjectError::SyncBuildDependencies)?;
        }

        if !args.only_deps {
            let package = Build::new(
                &project_toml,
                &project_tree,
                tree::EntryType::Entrypoint,
                config,
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
            Ok(Some(package))
        } else {
            Ok(None)
        }
    }
}
