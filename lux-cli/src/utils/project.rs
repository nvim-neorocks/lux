use std::sync::Arc;

use eyre::{Context, Result};
use lux_lib::{
    config::Config,
    luarocks::luarocks_installation::LuaRocksInstallation,
    operations::Sync,
    progress::{MultiProgress, Progress},
    project::Project,
    rockspec::Rockspec,
};

pub async fn sync_dependencies_if_locked(
    project: &Project,
    progress: Arc<Progress<MultiProgress>>,
    config: &Config,
) -> Result<()> {
    // NOTE: We only update the lockfile if one exists.
    // Otherwise, the next `lx build` will remove the packages.
    if let Some(lockfile) = project.try_lockfile()? {
        let mut lockfile = lockfile.write_guard();
        let tree = project.tree(config)?;
        let packages = project
            .toml()
            .into_local()?
            .dependencies()
            .current_platform()
            .clone();
        Sync::new(&tree, &mut lockfile, config)
            .packages(packages)
            .progress(progress)
            .sync_dependencies()
            .await
            .wrap_err("syncing dependencies with the project lockfile failed.")?;
    }
    Ok(())
}

pub async fn sync_build_dependencies_if_locked(
    project: &Project,
    progress: Arc<Progress<MultiProgress>>,
    config: &Config,
) -> Result<()> {
    if let Some(lockfile) = project.try_lockfile()? {
        let luarocks = LuaRocksInstallation::new(config)?;
        let mut lockfile = lockfile.write_guard();
        let packages = project
            .toml()
            .into_local()?
            .build_dependencies()
            .current_platform()
            .clone();
        Sync::new(luarocks.tree(), &mut lockfile, luarocks.config())
            .packages(packages)
            .progress(progress.clone())
            .sync_build_dependencies()
            .await
            .wrap_err("syncing build dependencies with the project lockfile failed.")?;
    }
    Ok(())
}

pub async fn sync_test_dependencies_if_locked(
    project: &Project,
    progress: Arc<Progress<MultiProgress>>,
    config: &Config,
) -> Result<()> {
    if let Some(lockfile) = project.try_lockfile()? {
        let mut lockfile = lockfile.write_guard();
        let packages = project
            .toml()
            .into_local()?
            .test_dependencies()
            .current_platform()
            .clone();
        let tree = project.test_tree(config)?;
        Sync::new(&tree, &mut lockfile, config)
            .packages(packages)
            .progress(progress.clone())
            .sync_test_dependencies()
            .await
            .wrap_err("syncing test dependencies with the project lockfile failed.")?;
    }
    Ok(())
}
