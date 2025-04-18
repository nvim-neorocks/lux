use eyre::{Context, OptionExt, Result};
use itertools::Itertools;
use lux_lib::{
    config::Config,
    lockfile::{OptState, PinnedState},
    luarocks::luarocks_installation::LuaRocksInstallation,
    operations::Sync,
    package::PackageReq,
    progress::{MultiProgress, Progress, ProgressBar},
    project::Project,
    remote_package_db::RemotePackageDB,
    rockspec::lua_dependency::{self, LuaDependencySpec},
};

#[derive(clap::Args)]
pub struct Add {
    /// Package or list of packages to install.
    package_req: Vec<PackageReq>,

    /// Pin the packages so that they doesn't get updated.
    #[arg(long)]
    pin: bool,

    /// Mark the packages as optional.
    #[arg(long)]
    opt: bool,

    /// Reinstall without prompt if a package is already installed.
    #[arg(long)]
    force: bool,

    /// Install the package as a development dependency.
    /// Also called `dev`.
    #[arg(short, long, alias = "dev", visible_short_aliases = ['d', 'b'])]
    build: Option<Vec<PackageReq>>,

    /// Install the package as a test dependency.
    #[arg(short, long)]
    test: Option<Vec<PackageReq>>,
}

pub async fn add(data: Add, config: Config) -> Result<()> {
    let mut project = Project::current()?.ok_or_eyre("No project found")?;

    let pin = PinnedState::from(data.pin);
    let opt = OptState::from(data.opt);
    let tree = project.tree(&config)?;
    let db = RemotePackageDB::from_config(&config, &Progress::Progress(ProgressBar::new())).await?;

    let progress = MultiProgress::new_arc();

    if !data.package_req.is_empty() {
        // NOTE: We only update the lockfile if one exists.
        // Otherwise, the next `lx build` will install the packages.
        if let Some(lockfile) = project.try_lockfile()? {
            let mut lockfile = lockfile.write_guard();
            Sync::new(&tree, &mut lockfile, &config)
                .progress(progress.clone())
                .packages(
                    data.package_req
                        .iter()
                        .cloned()
                        .map(|pkg| LuaDependencySpec::new(pkg, pin, opt))
                        .collect_vec(),
                )
                .sync_dependencies()
                .await
                .wrap_err("syncing dependencies with the project lockfile failed.")?;
        }

        project
            .add(
                lua_dependency::DependencyType::Regular(data.package_req),
                &db,
            )
            .await?;
    }

    let build_packages = data.build.unwrap_or_default();
    if !build_packages.is_empty() {
        if let Some(lockfile) = project.try_lockfile()? {
            let luarocks = LuaRocksInstallation::new(&config)?;
            let mut lockfile = lockfile.write_guard();
            Sync::new(luarocks.tree(), &mut lockfile, luarocks.config())
                .progress(progress.clone())
                .packages(
                    build_packages
                        .iter()
                        .cloned()
                        .map(|pkg| LuaDependencySpec::new(pkg, pin, opt))
                        .collect_vec(),
                )
                .sync_build_dependencies()
                .await
                .wrap_err("syncing build dependencies with the project lockfile failed.")?;
        }

        project
            .add(lua_dependency::DependencyType::Build(build_packages), &db)
            .await?;
    }

    let test_packages = data.test.unwrap_or_default();
    if !test_packages.is_empty() {
        if let Some(lockfile) = project.try_lockfile()? {
            let tree = project.test_tree(&config)?;
            let mut lockfile = lockfile.write_guard();
            Sync::new(&tree, &mut lockfile, &config)
                .progress(progress.clone())
                .packages(
                    test_packages
                        .iter()
                        .cloned()
                        .map(|pkg| LuaDependencySpec::new(pkg, pin, opt))
                        .collect_vec(),
                )
                .sync_test_dependencies()
                .await
                .wrap_err("syncing test dependencies with the project lockfile failed.")?;

            project
                .add(lua_dependency::DependencyType::Test(test_packages), &db)
                .await?;
        }
    }

    Ok(())
}
