use std::{collections::HashMap, io, sync::Arc};

use crate::{
    build::{Build, BuildBehaviour, BuildError},
    config::{Config, LuaVersionUnset},
    lockfile::{
        LocalPackage, LocalPackageId, LockConstraint, Lockfile, OptState, PinnedState, ReadOnly,
        ReadWrite,
    },
    lua_rockspec::BuildBackendSpec,
    luarocks::{
        install_binary_rock::{BinaryRockInstall, InstallBinaryRockError},
        luarocks_installation::{
            InstallBuildDependenciesError, LuaRocksError, LuaRocksInstallError,
            LuaRocksInstallation,
        },
    },
    package::{PackageName, PackageNameList},
    progress::{MultiProgress, Progress, ProgressBar},
    project::{Project, ProjectTreeError},
    remote_package_db::{RemotePackageDB, RemotePackageDBError, RemotePackageDbIntegrityError},
    rockspec::Rockspec,
    tree::{self, Tree, TreeError},
};

use bon::Builder;
use bytes::Bytes;
use futures::future::join_all;
use itertools::Itertools;
use thiserror::Error;

use super::{
    install_spec::PackageInstallSpec, resolve::get_all_dependencies, DownloadedRockspec,
    RemoteRockDownload, SearchAndDownloadError,
};

/// A rocks package installer, providing fine-grained control
/// over how packages should be installed.
/// Can install multiple packages in parallel.
#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = _install, vis = ""))]
pub struct Install<'a> {
    #[builder(start_fn)]
    config: &'a Config,
    #[builder(field)]
    packages: Vec<PackageInstallSpec>,
    #[builder(setters(name = "_tree", vis = ""))]
    tree: Tree,
    package_db: Option<RemotePackageDB>,
    progress: Option<Arc<Progress<MultiProgress>>>,
}

impl<'a, State> InstallBuilder<'a, State>
where
    State: install_builder::State,
{
    pub fn tree(self, tree: Tree) -> InstallBuilder<'a, install_builder::SetTree<State>>
    where
        State::Tree: install_builder::IsUnset,
    {
        self._tree(tree)
    }

    pub fn project(
        self,
        project: &'a Project,
    ) -> Result<InstallBuilder<'a, install_builder::SetTree<State>>, ProjectTreeError>
    where
        State::Tree: install_builder::IsUnset,
    {
        let config = self.config;
        Ok(self._tree(project.tree(config)?))
    }

    pub fn packages(self, packages: Vec<PackageInstallSpec>) -> Self {
        Self { packages, ..self }
    }

    pub fn package(self, package: PackageInstallSpec) -> Self {
        Self {
            packages: self
                .packages
                .into_iter()
                .chain(std::iter::once(package))
                .collect(),
            ..self
        }
    }
}

impl<State> InstallBuilder<'_, State>
where
    State: install_builder::State + install_builder::IsComplete,
{
    /// Install the packages.
    pub async fn install(self) -> Result<Vec<LocalPackage>, InstallError> {
        let install_built = self._install();
        let progress = match install_built.progress {
            Some(p) => p,
            None => MultiProgress::new_arc(),
        };
        let package_db = match install_built.package_db {
            Some(db) => db,
            None => {
                let bar = progress.map(|p| p.new_bar());
                RemotePackageDB::from_config(install_built.config, &bar).await?
            }
        };

        let duplicate_entrypoints = install_built
            .packages
            .iter()
            .filter(|pkg| pkg.entry_type == tree::EntryType::Entrypoint)
            .map(|pkg| pkg.package.name())
            .duplicates()
            .cloned()
            .collect_vec();

        if !duplicate_entrypoints.is_empty() {
            return Err(InstallError::DuplicateEntrypoints(PackageNameList::new(
                duplicate_entrypoints,
            )));
        }

        install_impl(
            install_built.packages,
            Arc::new(package_db),
            install_built.config,
            &install_built.tree,
            install_built.tree.lockfile()?,
            progress,
        )
        .await
    }
}

#[derive(Error, Debug)]
pub enum InstallError {
    #[error(transparent)]
    SearchAndDownloadError(#[from] SearchAndDownloadError),
    #[error(transparent)]
    LuaVersionUnset(#[from] LuaVersionUnset),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error("error instantiating LuaRocks compatibility layer: {0}")]
    LuaRocksError(#[from] LuaRocksError),
    #[error("error installing LuaRocks compatibility layer: {0}")]
    LuaRocksInstallError(#[from] LuaRocksInstallError),
    #[error("error installing LuaRocks build dependencies: {0}")]
    InstallBuildDependenciesError(#[from] InstallBuildDependenciesError),
    #[error("failed to build {0}: {1}")]
    BuildError(PackageName, BuildError),
    #[error("error initialising remote package DB: {0}")]
    RemotePackageDB(#[from] RemotePackageDBError),
    #[error("failed to install pre-built rock {0}: {1}")]
    InstallBinaryRockError(PackageName, InstallBinaryRockError),
    #[error("integrity error for package {0}: {1}\n")]
    Integrity(PackageName, RemotePackageDbIntegrityError),
    #[error(transparent)]
    ProjectTreeError(#[from] ProjectTreeError),
    #[error("cannot install duplicate entrypoints: {0}")]
    DuplicateEntrypoints(PackageNameList),
}

// TODO(vhyrro): This function has too many arguments. Refactor it.
#[allow(clippy::too_many_arguments)]
async fn install_impl(
    packages: Vec<PackageInstallSpec>,
    package_db: Arc<RemotePackageDB>,
    config: &Config,
    tree: &Tree,
    lockfile: Lockfile<ReadOnly>,
    progress_arc: Arc<Progress<MultiProgress>>,
) -> Result<Vec<LocalPackage>, InstallError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    get_all_dependencies(
        tx,
        packages,
        package_db.clone(),
        Arc::new(lockfile.clone()),
        config,
        progress_arc.clone(),
    )
    .await?;

    let mut all_packages = HashMap::with_capacity(rx.len());

    while let Some(dep) = rx.recv().await {
        all_packages.insert(dep.spec.id(), dep);
    }

    let installed_packages = join_all(all_packages.clone().into_values().map(|install_spec| {
        let progress_arc = progress_arc.clone();
        let downloaded_rock = install_spec.downloaded_rock;
        let config = config.clone();
        let tree = tree.clone();

        tokio::spawn(async move {
            let rockspec = downloaded_rock.rockspec();
            if let Some(BuildBackendSpec::LuaRock(build_backend)) =
                &rockspec.build().current_platform().build_backend
            {
                let luarocks_tree = tree.build_tree(&config)?;
                let luarocks = LuaRocksInstallation::new(&config, luarocks_tree)?;
                luarocks
                    .install_build_dependencies(build_backend, rockspec, progress_arc.clone())
                    .await?;
            }

            let pkg = match downloaded_rock {
                RemoteRockDownload::RockspecOnly { rockspec_download } => {
                    install_rockspec(
                        rockspec_download,
                        install_spec.spec.constraint(),
                        install_spec.build_behaviour,
                        install_spec.pin,
                        install_spec.opt,
                        install_spec.entry_type,
                        &tree,
                        &config,
                        progress_arc,
                    )
                    .await?
                }
                RemoteRockDownload::BinaryRock {
                    rockspec_download,
                    packed_rock,
                } => {
                    install_binary_rock(
                        rockspec_download,
                        packed_rock,
                        install_spec.spec.constraint(),
                        install_spec.build_behaviour,
                        install_spec.pin,
                        install_spec.opt,
                        install_spec.entry_type,
                        &config,
                        &tree,
                        progress_arc,
                    )
                    .await?
                }
                RemoteRockDownload::SrcRock { .. } => todo!(
                    "lux does not yet support installing .src.rock packages without a rockspec"
                ),
            };

            Ok::<_, InstallError>((pkg.id(), (pkg, install_spec.entry_type)))
        })
    }))
    .await
    .into_iter()
    .flatten()
    .try_collect::<_, HashMap<LocalPackageId, (LocalPackage, tree::EntryType)>, _>()?;

    let write_dependency = |lockfile: &mut Lockfile<ReadWrite>,
                            id: &LocalPackageId,
                            pkg: &LocalPackage,
                            entry_type: tree::EntryType| {
        if entry_type == tree::EntryType::Entrypoint {
            lockfile.add_entrypoint(pkg);
        }

        all_packages
            .get(id)
            .map(|pkg| pkg.spec.dependencies())
            .unwrap_or_default()
            .into_iter()
            .for_each(|dependency_id| {
                lockfile.add_dependency(
                    pkg,
                    installed_packages
                        .get(dependency_id)
                        .map(|(pkg, _)| pkg)
                        // NOTE: This can happen if an install thread panics
                        .expect("required dependency not found [This is a bug!]"),
                );
            });
    };

    lockfile.map_then_flush(|lockfile| {
        installed_packages
            .iter()
            .for_each(|(id, (pkg, is_entrypoint))| {
                write_dependency(lockfile, id, pkg, *is_entrypoint)
            });

        Ok::<_, io::Error>(())
    })?;

    Ok(installed_packages
        .into_values()
        .map(|(pkg, _)| pkg)
        .collect_vec())
}

#[allow(clippy::too_many_arguments)]
async fn install_rockspec(
    rockspec_download: DownloadedRockspec,
    constraint: LockConstraint,
    behaviour: BuildBehaviour,
    pin: PinnedState,
    opt: OptState,
    entry_type: tree::EntryType,
    tree: &Tree,
    config: &Config,
    progress_arc: Arc<Progress<MultiProgress>>,
) -> Result<LocalPackage, InstallError> {
    let progress = Arc::clone(&progress_arc);
    let rockspec = rockspec_download.rockspec;
    let source = rockspec_download.source;
    let package = rockspec.package().clone();
    let bar = progress.map(|p| p.add(ProgressBar::from(format!("💻 Installing {}", &package,))));

    if let Some(BuildBackendSpec::LuaRock(build_backend)) =
        &rockspec.build().current_platform().build_backend
    {
        let luarocks_tree = tree.build_tree(config)?;
        let luarocks = LuaRocksInstallation::new(config, luarocks_tree)?;
        luarocks.ensure_installed(&bar).await?;
        luarocks
            .install_build_dependencies(build_backend, &rockspec, progress_arc)
            .await?;
    }

    let pkg = Build::new(&rockspec, tree, entry_type, config, &bar)
        .pin(pin)
        .opt(opt)
        .constraint(constraint)
        .behaviour(behaviour)
        .source(source)
        .maybe_source_url(rockspec_download.source_url)
        .build()
        .await
        .map_err(|err| InstallError::BuildError(package, err))?;

    bar.map(|b| b.finish_and_clear());

    Ok(pkg)
}

#[allow(clippy::too_many_arguments)]
async fn install_binary_rock(
    rockspec_download: DownloadedRockspec,
    packed_rock: Bytes,
    constraint: LockConstraint,
    behaviour: BuildBehaviour,
    pin: PinnedState,
    opt: OptState,
    entry_type: tree::EntryType,
    config: &Config,
    tree: &Tree,
    progress_arc: Arc<Progress<MultiProgress>>,
) -> Result<LocalPackage, InstallError> {
    let progress = Arc::clone(&progress_arc);
    let rockspec = rockspec_download.rockspec;
    let package = rockspec.package().clone();
    let bar = progress.map(|p| {
        p.add(ProgressBar::from(format!(
            "💻 Installing {} (pre-built)",
            &package,
        )))
    });
    let pkg = BinaryRockInstall::new(
        &rockspec,
        rockspec_download.source,
        packed_rock,
        entry_type,
        config,
        tree,
        &bar,
    )
    .pin(pin)
    .opt(opt)
    .constraint(constraint)
    .behaviour(behaviour)
    .install()
    .await
    .map_err(|err| InstallError::InstallBinaryRockError(package, err))?;

    bar.map(|b| b.finish_and_clear());

    Ok(pkg)
}
