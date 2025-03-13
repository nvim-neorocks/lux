use std::{collections::HashMap, fmt::Display, io, sync::Arc};

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
    package::PackageName,
    progress::{MultiProgress, Progress, ProgressBar},
    project::{Project, ProjectTreeError},
    remote_package_db::{RemotePackageDB, RemotePackageDBError, RemotePackageDbIntegrityError},
    rockspec::Rockspec,
    tree::Tree,
};

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
pub struct Install<'a> {
    config: &'a Config,
    tree: &'a Tree,
    package_db: Option<RemotePackageDB>,
    packages: Vec<PackageInstallSpec>,
    progress: Option<Arc<Progress<MultiProgress>>>,
    project: Option<&'a Project>,
}

impl<'a> Install<'a> {
    /// Construct a new installer.
    pub fn new(tree: &'a Tree, config: &'a Config) -> Self {
        Self {
            config,
            tree,
            package_db: None,
            packages: Vec::new(),
            progress: None,
            project: None,
        }
    }

    /// Sets the package database to use for searching for packages.
    /// Instantiated from the config if not set.
    pub fn package_db(self, package_db: RemotePackageDB) -> Self {
        Self {
            package_db: Some(package_db),
            ..self
        }
    }

    /// Specify packages to install, along with each package's build behaviour.
    pub fn packages<I>(self, packages: I) -> Self
    where
        I: IntoIterator<Item = PackageInstallSpec>,
    {
        Self {
            packages: self.packages.into_iter().chain(packages).collect_vec(),
            ..self
        }
    }

    /// Add a package to the set of packages to install.
    pub fn package(self, package: PackageInstallSpec) -> Self {
        self.packages(std::iter::once(package))
    }

    /// Pass a `MultiProgress` to this installer.
    /// By default, a new one will be created.
    pub fn progress(self, progress: Arc<Progress<MultiProgress>>) -> Self {
        Self {
            progress: Some(progress),
            ..self
        }
    }

    /// Attach a project to the install phase (also affects the project's `lux.lock`).
    pub fn project(self, project: &'a Project) -> Self {
        Self {
            project: Some(project),
            ..self
        }
    }

    /// Install the packages.
    pub async fn install(self) -> Result<Vec<LocalPackage>, InstallError> {
        let progress = match self.progress {
            Some(p) => p,
            None => MultiProgress::new_arc(),
        };
        let package_db = match self.package_db {
            Some(db) => db,
            None => {
                let bar = progress.map(|p| p.new_bar());
                RemotePackageDB::from_config(self.config, &bar).await?
            }
        };

        let duplicate_entrypoints = self
            .packages
            .iter()
            .filter(|pkg| pkg.is_entrypoint)
            .duplicates_by(|pkg| pkg.package.name.clone())
            .map(|pkg| pkg.package.name.clone())
            .collect_vec();

        if !duplicate_entrypoints.is_empty() {
            return Err(InstallError::DuplicateEntrypoints(PackageNameList(
                duplicate_entrypoints,
            )));
        }

        install(
            self.packages,
            package_db,
            self.project,
            self.tree,
            self.config,
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

#[derive(Debug)]
pub struct PackageNameList(Vec<PackageName>);

impl Display for PackageNameList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.iter().map(|pkg| pkg.to_string()).join(", ").as_str())
    }
}

async fn install(
    packages: Vec<PackageInstallSpec>,
    package_db: RemotePackageDB,
    project: Option<&Project>,
    tree: &Tree,
    config: &Config,
    progress: Arc<Progress<MultiProgress>>,
) -> Result<Vec<LocalPackage>, InstallError>
where
{
    let lockfile = tree.lockfile()?;
    let tree = project.map_or(Ok(tree.clone()), |p| p.tree(config))?;

    install_impl(
        packages,
        Arc::new(package_db),
        config,
        &tree,
        lockfile,
        progress,
    )
    .await
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
                let luarocks = LuaRocksInstallation::new(&config)?;
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
                        &config,
                        progress_arc,
                    )
                    .await?
                }
                RemoteRockDownload::SrcRock { .. } => todo!(
                    "lux does not yet support installing .src.rock packages without a rockspec"
                ),
            };

            Ok::<_, InstallError>((pkg.id(), (pkg, install_spec.is_entrypoint)))
        })
    }))
    .await
    .into_iter()
    .flatten()
    .try_collect::<_, HashMap<LocalPackageId, (LocalPackage, bool)>, _>()?;

    let write_dependency = |lockfile: &mut Lockfile<ReadWrite>,
                            id: &LocalPackageId,
                            pkg: &LocalPackage,
                            is_entrypoint: bool| {
        if is_entrypoint {
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
                        .expect("required dependency not found"),
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
        let luarocks = LuaRocksInstallation::new(config)?;
        luarocks.ensure_installed(&bar).await?;
        luarocks
            .install_build_dependencies(build_backend, &rockspec, progress_arc)
            .await?;
    }

    let pkg = Build::new(&rockspec, tree, config, &bar)
        .pin(pin)
        .opt(opt)
        .constraint(constraint)
        .behaviour(behaviour)
        .source(source)
        .source_url(rockspec_download.source_url)
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
    config: &Config,
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
        config,
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
