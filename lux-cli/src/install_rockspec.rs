use eyre::{eyre, Context};
use itertools::Itertools;
use std::{path::PathBuf, sync::Arc};

use clap::Args;
use eyre::Result;
use lux_lib::{
    build::{self, BuildBehaviour},
    config::Config,
    lockfile::{OptState, PinnedState},
    lua_rockspec::{BuildBackendSpec, RemoteLuaRockspec},
    luarocks::luarocks_installation::LuaRocksInstallation,
    operations::{Install, PackageInstallSpec},
    package::PackageName,
    progress::MultiProgress,
    project::Project,
    rockspec::{LuaVersionCompatibility, Rockspec},
    tree,
};

#[derive(Args, Default)]
pub struct InstallRockspec {
    /// The path to the RockSpec file to install
    rockspec_path: PathBuf,

    /// Whether to pin the installed package and dependencies.
    #[arg(long)]
    pin: bool,
}

pub async fn install_rockspec(data: InstallRockspec, config: Config) -> Result<()> {
    let pin = PinnedState::from(data.pin);
    let project_opt = Project::current()?;
    let path = data.rockspec_path;

    if path
        .extension()
        .map(|ext| ext != "rockspec")
        .unwrap_or(true)
    {
        return Err(eyre!("Provided path is not a valid rockspec!"));
    }
    let content = std::fs::read_to_string(path)?;
    let rockspec = RemoteLuaRockspec::new(&content)?;
    let lua_version = rockspec.lua_version_matches(&config)?;
    let tree = config.tree(lua_version)?;

    // Ensure all dependencies are installed first
    let dependencies = rockspec
        .dependencies()
        .current_platform()
        .iter()
        .filter(|package| !package.name().eq(&PackageName::new("lua".into())))
        .collect_vec();

    let progress_arc = MultiProgress::new_arc();
    let progress = Arc::clone(&progress_arc);

    let dependencies_to_install = dependencies
        .into_iter()
        .filter(|dep| {
            tree.match_rocks(dep.package_req())
                .is_ok_and(|rock_match| rock_match.is_found())
        })
        .map(|dep| {
            PackageInstallSpec::new(dep.package_req().clone(), tree::EntryType::DependencyOnly)
                .build_behaviour(BuildBehaviour::NoForce)
                .pin(pin)
                .opt(OptState::Required)
                .build()
        });

    Install::new(&tree, &config)
        .packages(dependencies_to_install)
        .progress(progress_arc.clone())
        .install()
        .await?;

    if let Some(project) = project_opt {
        std::fs::copy(tree.lockfile_path(), project.lockfile_path())
            .wrap_err("error creating project lockfile.")?;
    }

    if let Some(BuildBackendSpec::LuaRock(build_backend)) =
        &rockspec.build().current_platform().build_backend
    {
        let luarocks = LuaRocksInstallation::new(&config)?;
        let bar = progress.map(|p| p.new_bar());
        luarocks.ensure_installed(&bar).await?;
        luarocks
            .install_build_dependencies(build_backend, &rockspec, progress_arc.clone())
            .await?;
    }

    build::Build::new(
        &rockspec,
        &tree,
        tree::EntryType::Entrypoint,
        &config,
        &progress.map(|p| p.new_bar()),
    )
    .pin(pin)
    .behaviour(BuildBehaviour::Force)
    .build()
    .await?;

    Ok(())
}
