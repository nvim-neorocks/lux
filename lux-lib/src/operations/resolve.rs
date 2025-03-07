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
};

use super::{Download, PackageInstallSpec, RemoteRockDownload, SearchAndDownloadError};

#[derive(Clone, Debug)]
pub(crate) struct PackageInstallData {
    pub build_behaviour: BuildBehaviour,
    pub pin: PinnedState,
    pub opt: OptState,
    pub downloaded_rock: RemoteRockDownload,
    pub spec: LocalPackageSpec,
    pub is_entrypoint: bool,
}

#[async_recursion]
pub(crate) async fn get_all_dependencies<P>(
    tx: UnboundedSender<PackageInstallData>,
    packages: Vec<PackageInstallSpec>,
    package_db: Arc<RemotePackageDB>,
    lockfile: Arc<Lockfile<P>>,
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
                    build_behaviour == &BuildBehaviour::Force
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
                     is_entrypoint,
                 }| {
                    let config = config.clone();
                    let tx = tx.clone();
                    let package_db = Arc::clone(&package_db);
                    let progress = Arc::clone(&progress);
                    let lockfile = Arc::clone(&lockfile);

                    tokio::spawn(async move {
                        let bar = progress.map(|p| p.new_bar());

                        let downloaded_rock = Download::new(&package, &config, &bar)
                            .package_db(&package_db)
                            .download_remote_rock()
                            .await?;

                        let constraint = package.version_req().clone().into();

                        let dependencies = downloaded_rock
                            .rockspec()
                            .dependencies()
                            .current_platform()
                            .iter()
                            .filter(|dep| !dep.name().eq(&"lua".into()))
                            .map(|dep| {
                                PackageInstallSpec::new(
                                    dep.package_req().clone(),
                                    build_behaviour,
                                    pin,
                                    opt,
                                    false,
                                )
                            })
                            .collect_vec();

                        let dependencies = get_all_dependencies(
                            tx.clone(),
                            dependencies,
                            package_db,
                            lockfile,
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
                            is_entrypoint,
                        };

                        tx.send(install_spec).unwrap();

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
