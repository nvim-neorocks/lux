use std::sync::Arc;

use eyre::{Context, Result};
use lux_lib::{
    config::Config,
    luarocks::luarocks_installation::LuaRocksInstallation,
    operations::Sync,
    progress::{MultiProgress, Progress},
    project::Project,
};

pub async fn sync_dependencies_if_locked(
    project: &Project,
    progress: Arc<Progress<MultiProgress>>,
    config: &Config,
) -> Result<()> {
    // NOTE: We only update the lockfile if one exists.
    // Otherwise, the next `lx build` will remove the packages.
    Sync::new(project, config)
        .progress(progress)
        .sync_dependencies()
        .await
        .wrap_err("syncing dependencies with the project lockfile failed.")?;
    Ok(())
}

pub async fn sync_build_dependencies_if_locked(
    project: &Project,
    progress: Arc<Progress<MultiProgress>>,
    config: &Config,
) -> Result<()> {
    let luarocks = LuaRocksInstallation::new(config)?;
    Sync::new(project, luarocks.config())
        .progress(progress.clone())
        .custom_tree(luarocks.tree())
        .sync_build_dependencies()
        .await
        .wrap_err("syncing build dependencies with the project lockfile failed.")?;
    Ok(())
}

pub async fn sync_test_dependencies_if_locked(
    project: &Project,
    progress: Arc<Progress<MultiProgress>>,
    config: &Config,
) -> Result<()> {
    Sync::new(project, config)
        .progress(progress.clone())
        .sync_test_dependencies()
        .await
        .wrap_err("syncing test dependencies with the project lockfile failed.")?;
    Ok(())
}
