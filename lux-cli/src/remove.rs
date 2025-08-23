use clap::Args;
use eyre::{OptionExt, Result};
use lux_lib::{
    config::Config, package::PackageName, progress::MultiProgress, project::Project,
    rockspec::lua_dependency,
};

use crate::utils::project::{
    sync_build_dependencies_if_locked, sync_dependencies_if_locked,
    sync_test_dependencies_if_locked,
};

#[derive(Args)]
pub struct Remove {
    /// Package or list of packages to remove from the dependencies.
    package: Vec<PackageName>,

    /// Remove a development dependency.
    /// Also called `dev`.
    #[arg(short, long, alias = "dev", visible_short_aliases = ['d', 'b'])]
    build: Option<Vec<PackageName>>,

    /// Remove a test dependency.
    #[arg(short, long)]
    test: Option<Vec<PackageName>>,
}

pub async fn remove(data: Remove, config: Config) -> Result<()> {
    let mut project = Project::current()?.ok_or_eyre("No project found")?;
    let progress = MultiProgress::new_arc(&config);

    if !data.package.is_empty() {
        project
            .remove(lua_dependency::DependencyType::Regular(data.package))
            .await?;
        sync_dependencies_if_locked(&project, progress.clone(), &config).await?;
    }

    let build_packages = data.build.unwrap_or_default();
    if !build_packages.is_empty() {
        project
            .remove(lua_dependency::DependencyType::Build(build_packages))
            .await?;
        sync_build_dependencies_if_locked(&project, progress.clone(), &config).await?;
    }

    let test_packages = data.test.unwrap_or_default();
    if !test_packages.is_empty() {
        project
            .remove(lua_dependency::DependencyType::Test(test_packages))
            .await?;
        sync_test_dependencies_if_locked(&project, progress.clone(), &config).await?;
    }

    Ok(())
}
