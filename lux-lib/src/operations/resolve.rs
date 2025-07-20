use std::sync::Arc;

use async_recursion::async_recursion;
use futures::future::join_all;
use itertools::Itertools;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    build::BuildBehaviour,
    config::Config,
    lockfile::{
        LocalPackageId, LocalPackageSpec, Lockfile, LockfilePermissions, OptState, PinnedState,
    },
    progress::{MultiProgress, Progress},
    remote_package_db::RemotePackageDB,
    rockspec::Rockspec,
    tree,
};

use super::{Download, PackageInstallSpec, RemoteRockDownload, SearchAndDownloadError};

#[derive(Clone, Debug)]
pub(crate) struct PackageInstallData {
    pub build_behaviour: BuildBehaviour,
    pub pin: PinnedState,
    pub opt: OptState,
    pub downloaded_rock: RemoteRockDownload,
    pub spec: LocalPackageSpec,
    pub entry_type: tree::EntryType,
}

#[async_recursion]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn get_all_dependencies<P>(
    dependencies_tx: UnboundedSender<PackageInstallData>,
    build_dependencies_tx: UnboundedSender<PackageInstallData>,
    packages: Vec<PackageInstallSpec>,
    package_db: Arc<RemotePackageDB>,
    lockfile: Arc<Lockfile<P>>,
    build_lockfile: Arc<Lockfile<P>>,
    config: &Config,
    progress: Arc<Progress<MultiProgress>>,
) -> Result<Vec<LocalPackageId>, SearchAndDownloadError>
where
    P: LockfilePermissions + Send + Sync + 'static,
{
    join_all(
        packages
            .into_iter()
            // Exclude packages that are already installed
            .filter(
                |PackageInstallSpec {
                     package,
                     build_behaviour,
                     ..
                 }| {
                    *build_behaviour == BuildBehaviour::Force
                        || lockfile.has_rock(package, None).is_none()
                },
            )
            .map(
                // NOTE: we propagate build_behaviour, pin and opt to all dependencies
                |PackageInstallSpec {
                     package,
                     build_behaviour,
                     pin,
                     opt,
                     entry_type,
                     constraint,
                     source,
                 }| {
                    let config = config.clone();
                    let dependencies_tx = dependencies_tx.clone();
                    let build_dependencies_tx = build_dependencies_tx.clone();
                    let package_db = Arc::clone(&package_db);
                    let progress = Arc::clone(&progress);
                    let build_dep_progress = Arc::clone(&progress);
                    let lockfile = Arc::clone(&lockfile);
                    let build_lockfile = Arc::clone(&build_lockfile);

                    tokio::spawn(async move {
                        let bar = progress.map(|p| p.new_bar());

                        let downloaded_rock = if let Some(source) = source {
                            RemoteRockDownload::from_package_req_and_source_spec(
                                package.clone(),
                                source,
                            )?
                        } else {
                            Download::new(&package, &config, &bar)
                                .package_db(&package_db)
                                .download_remote_rock()
                                .await?
                        };

                        let constraint = constraint.unwrap_or(package.version_req().clone().into());

                        let rockspec = downloaded_rock.rockspec();
                        let build_dependencies = rockspec
                            .build_dependencies()
                            .current_platform()
                            .iter()
                            .map(|dep| {
                                // We always install build dependencies as entrypoints
                                // with regard to the build tree
                                let entry_type = tree::EntryType::Entrypoint;
                                PackageInstallSpec::new(dep.package_req().clone(), entry_type)
                                    .build_behaviour(build_behaviour)
                                    .pin(pin)
                                    .opt(opt)
                                    .maybe_source(dep.source().clone())
                                    .build()
                            })
                            .collect_vec();

                        // NOTE: We treat transitive regular dependencies of build dependencies
                        // as build dependencies
                        get_all_dependencies(
                            build_dependencies_tx.clone(),
                            build_dependencies_tx.clone(),
                            build_dependencies,
                            package_db.clone(),
                            build_lockfile.clone(),
                            build_lockfile.clone(),
                            &config,
                            build_dep_progress,
                        )
                        .await?;

                        let dependencies = rockspec
                            .dependencies()
                            .current_platform()
                            .iter()
                            .map(|dep| {
                                // If we're forcing a rebuild, retain the `EntryType`
                                // of existing dependencies
                                let entry_type = if build_behaviour == BuildBehaviour::Force
                                    && lockfile.has_rock(dep.package_req(), None).is_some_and(
                                        |installed_rock| {
                                            lockfile.is_entrypoint(&installed_rock.id())
                                        },
                                    ) {
                                    tree::EntryType::Entrypoint
                                } else {
                                    tree::EntryType::DependencyOnly
                                };

                                PackageInstallSpec::new(dep.package_req().clone(), entry_type)
                                    .build_behaviour(build_behaviour)
                                    .pin(pin)
                                    .opt(opt)
                                    .maybe_source(dep.source().clone())
                                    .build()
                            })
                            .collect_vec();

                        let dependencies = get_all_dependencies(
                            dependencies_tx.clone(),
                            build_dependencies_tx,
                            dependencies,
                            package_db,
                            lockfile,
                            build_lockfile,
                            &config,
                            progress,
                        )
                        .await?;

                        let rockspec = downloaded_rock.rockspec();
                        let local_spec = LocalPackageSpec::new(
                            rockspec.package(),
                            rockspec.version(),
                            constraint,
                            dependencies,
                            &pin,
                            &opt,
                            rockspec.binaries(),
                        );

                        let install_spec = PackageInstallData {
                            build_behaviour,
                            pin,
                            opt,
                            spec: local_spec.clone(),
                            downloaded_rock,
                            entry_type,
                        };

                        dependencies_tx.send(install_spec).unwrap();

                        Ok::<_, SearchAndDownloadError>(local_spec.id())
                    })
                },
            ),
    )
    .await
    .into_iter()
    .flatten()
    .try_collect()
}
