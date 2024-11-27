use std::{collections::HashMap, io, sync::Arc};

use crate::{
    build::{BuildBehaviour, BuildError},
    config::{Config, LuaVersion, LuaVersionUnset},
    lockfile::{
        LocalPackage, LocalPackageId, LocalPackageSpec, LockConstraint, Lockfile, PinnedState,
    },
    manifest::ManifestMetadata,
    operations::download_rockspec,
    package::{PackageReq, PackageVersionReq},
    progress::{MultiProgress, ProgressBar},
    rockspec::{LuaVersionError, Rockspec},
    tree::Tree,
};

use async_recursion::async_recursion;
use futures::future::join_all;
use itertools::Itertools;
use semver::VersionReq;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

use super::SearchAndDownloadError;

#[derive(Error, Debug)]
#[error(transparent)]
pub enum InstallError {
    SearchAndDownloadError(#[from] SearchAndDownloadError),
    LuaVersionError(#[from] LuaVersionError),
    LuaVersionUnset(#[from] LuaVersionUnset),
    Io(#[from] io::Error),
    BuildError(#[from] BuildError),
}

#[derive(Clone, Debug)]
struct PackageInstallSpec {
    build_behaviour: BuildBehaviour,
    rockspec: Rockspec,
    spec: LocalPackageSpec,
}

pub async fn install(
    progress: &MultiProgress,
    packages: Vec<(BuildBehaviour, PackageReq)>,
    pin: PinnedState,
    manifest: &ManifestMetadata,
    config: &Config,
) -> Result<Vec<LocalPackage>, InstallError>
where
{
    let lua_version = LuaVersion::from(config)?;
    let tree = Tree::new(config.tree().clone(), lua_version)?;
    let mut lockfile = tree.lockfile()?;
    let result = install_impl(
        progress,
        packages,
        pin,
        manifest.clone(),
        config,
        &mut lockfile,
    )
    .await;
    lockfile.flush()?;
    result
}

async fn install_impl(
    progress: &MultiProgress,
    packages: Vec<(BuildBehaviour, PackageReq)>,
    pin: PinnedState,
    manifest: ManifestMetadata,
    config: &Config,
    lockfile: &mut Lockfile,
) -> Result<Vec<LocalPackage>, InstallError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    get_all_dependencies(
        tx,
        progress.clone(),
        packages,
        pin,
        Arc::new(manifest),
        config,
    )
    .await?;

    let mut all_packages = HashMap::with_capacity(rx.len());

    while let Some(dep) = rx.recv().await {
        all_packages.insert(dep.spec.id(), dep);
    }

    let installed_packages = join_all(all_packages.clone().into_values().map(|install_spec| {
        let bar = progress.add(ProgressBar::from(format!(
            "💻 Installing {}",
            install_spec.rockspec.package,
        )));
        let config = config.clone();

        tokio::spawn(async move {
            let pkg = crate::build::build(
                &bar,
                install_spec.rockspec,
                pin,
                install_spec.spec.constraint(),
                install_spec.build_behaviour,
                &config,
            )
            .await?;

            bar.finish_and_clear();

            Ok::<_, BuildError>((pkg.id(), pkg))
        })
    }))
    .await
    .into_iter()
    .flatten()
    .try_collect::<_, HashMap<LocalPackageId, LocalPackage>, _>()?;

    installed_packages.iter().for_each(|(id, pkg)| {
        lockfile.add(pkg);

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
                        .expect("required dependency not found"),
                );
            });
    });

    Ok(installed_packages.into_values().collect_vec())
}

#[async_recursion]
async fn get_all_dependencies(
    tx: UnboundedSender<PackageInstallSpec>,
    progress: MultiProgress,
    packages: Vec<(BuildBehaviour, PackageReq)>,
    pin: PinnedState,
    manifest: Arc<ManifestMetadata>,
    config: &Config,
) -> Result<Vec<LocalPackageId>, SearchAndDownloadError> {
    join_all(packages.into_iter().map(|(build_behaviour, package)| {
        let config = config.clone();
        let tx = tx.clone();
        let progress = progress.clone();
        let manifest = Arc::clone(&manifest);

        tokio::spawn(async move {
            let bar = progress.new_bar();

            let rockspec = download_rockspec(&bar, &package, &manifest, &config)
                .await
                .unwrap();

            let constraint =
                if *package.version_req() == PackageVersionReq::SemVer(VersionReq::STAR) {
                    LockConstraint::Unconstrained
                } else {
                    LockConstraint::Constrained(package.version_req().clone())
                };

            let dependencies = rockspec
                .dependencies
                .current_platform()
                .iter()
                .filter(|dep| !dep.name().eq(&"lua".into()))
                .map(|dep| (build_behaviour, dep.clone()))
                .collect_vec();

            let dependencies =
                get_all_dependencies(tx.clone(), progress, dependencies, pin, manifest, &config)
                    .await?;

            let local_spec = LocalPackageSpec::new(
                &rockspec.package,
                &rockspec.version,
                constraint,
                dependencies,
                &pin,
            );

            let install_spec = PackageInstallSpec {
                build_behaviour,
                spec: local_spec.clone(),
                rockspec,
            };

            tx.send(install_spec).unwrap();

            Ok::<_, SearchAndDownloadError>(local_spec.id())
        })
    }))
    .await
    .into_iter()
    .flatten()
    .try_collect()
}
