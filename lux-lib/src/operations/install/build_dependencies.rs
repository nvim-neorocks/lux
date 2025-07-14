use itertools::Itertools;
use std::{collections::HashMap, io, sync::Arc};

use bon::Builder;
use futures::future::join_all;
use thiserror::Error;

use crate::{
    build::{Build, BuildError},
    config::Config,
    lockfile::{LocalPackage, LocalPackageId},
    operations::{get_all_dependencies, install::PackageInstallSpec, SearchAndDownloadError},
    progress::{MultiProgress, Progress, ProgressBar},
    remote_package_db::{RemotePackageDB, RemotePackageDBError},
    rockspec::Rockspec,
    tree::{self, Tree, TreeError},
};

#[derive(Error, Debug)]
pub enum InstallBuildDependenciesError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error(transparent)]
    RemotePackageDBError(#[from] RemotePackageDBError),
    #[error(transparent)]
    SearchAndDownloadError(#[from] SearchAndDownloadError),
    #[error(transparent)]
    BuildError(#[from] BuildError),
}

#[derive(Builder)]
#[builder(start_fn = new, finish_fn(name = _build, vis = ""))]
pub(crate) struct InstallBuildDependencies<'a> {
    config: &'a Config,
    packages: Vec<PackageInstallSpec>,
    tree: &'a Tree,
    package_db: Option<&'a RemotePackageDB>,
    progress: Option<Arc<Progress<MultiProgress>>>,
}

impl<State> InstallBuildDependenciesBuilder<'_, State>
where
    State:
        install_build_dependencies_builder::State + install_build_dependencies_builder::IsComplete,
{
    pub async fn install(self) -> Result<(), InstallBuildDependenciesError> {
        do_install(self._build()).await
    }
}

async fn do_install(
    args: InstallBuildDependencies<'_>,
) -> Result<(), InstallBuildDependenciesError> {
    let progress = match args.progress {
        Some(p) => p,
        None => MultiProgress::new_arc(),
    };
    let package_db = match args.package_db {
        Some(db) => db.clone(),
        None => {
            let bar = progress.map(|p| p.new_bar());
            RemotePackageDB::from_config(args.config, &bar).await?
        }
    };
    let mut lockfile = args.tree.lockfile()?.write_guard();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    get_all_dependencies(
        tx,
        args.packages,
        Arc::new(package_db),
        Arc::new(lockfile.clone()),
        args.config,
        progress.clone(),
    )
    .await?;

    let mut all_packages = HashMap::with_capacity(rx.len());
    while let Some(dep) = rx.recv().await {
        all_packages.insert(dep.spec.id(), dep);
    }

    let installed_packages = join_all(all_packages.clone().into_values().map(|install_spec| {
        let bar = progress.map(|p| {
            p.add(ProgressBar::from(format!(
                "💻 Installing build dependency: {}",
                install_spec.downloaded_rock.rockspec().package(),
            )))
        });
        let config = args.config.clone();
        let tree = args.tree.clone();
        tokio::spawn(async move {
            let rockspec = install_spec.downloaded_rock.rockspec();
            let pkg = Build::new(rockspec, &tree, tree::EntryType::Entrypoint, &config, &bar)
                .constraint(install_spec.spec.constraint())
                .behaviour(install_spec.build_behaviour)
                .build()
                .await?;

            bar.map(|b| b.finish_and_clear());

            Ok::<_, InstallBuildDependenciesError>((pkg.id(), (pkg, install_spec.entry_type)))
        })
    }))
    .await
    .into_iter()
    .flatten()
    .try_collect::<_, HashMap<LocalPackageId, (LocalPackage, tree::EntryType)>, _>()?;

    installed_packages
        .iter()
        .for_each(|(id, (pkg, entry_type))| {
            if *entry_type == tree::EntryType::Entrypoint {
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
                        &installed_packages
                            .get(dependency_id)
                            // NOTE: This can happen if an install thread panics
                            .expect("required dependency not found [This is a bug!]")
                            .0,
                    );
                });
        });

    Ok(())
}
