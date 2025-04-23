use clap::Args;
use eyre::{eyre, Result};
use inquire::Confirm;
use itertools::Itertools;
use lux_lib::{
    build::BuildBehaviour,
    config::{Config, LuaVersion},
    lockfile::LocalPackageId,
    operations::{self, PackageInstallSpec},
    package::PackageReq,
    progress::MultiProgress,
    tree::{self, RockMatches, TreeError},
};

#[derive(Args)]
pub struct Uninstall {
    /// The package or packages to uninstall from the system.
    packages: Vec<PackageReq>,
}

pub async fn uninstall(uninstall_args: Uninstall, config: Config) -> Result<()> {
    let tree = config.tree(LuaVersion::from(&config)?)?;

    let package_matches = uninstall_args
        .packages
        .iter()
        .map(|package_req| tree.match_rocks(package_req))
        .try_collect::<_, Vec<_>, TreeError>()?;

    let (packages, nonexistent_packages, duplicate_packages) = package_matches.into_iter().fold(
        (Vec::new(), Vec::new(), Vec::new()),
        |(mut p, mut n, mut d), rock_match| {
            match rock_match {
                RockMatches::NotFound(req) => n.push(req),
                RockMatches::Single(package) => p.push(package),
                RockMatches::Many(packages) => d.extend(packages),
            };

            (p, n, d)
        },
    );

    if !nonexistent_packages.is_empty() {
        // TODO(vhyrro): Render this in the form of a tree.
        return Err(eyre!(
            "The following packages were not found: {:#?}",
            nonexistent_packages
        ));
    }

    if !duplicate_packages.is_empty() {
        return Err(eyre!(
            "
Multiple packages satisfying your version requirements were found:
{:#?}

Please specify the exact package to uninstall:
> lux uninstall '<name>@<version>'
",
            duplicate_packages,
        ));
    }

    let lockfile = tree.lockfile()?;
    let non_entrypoints = packages
        .iter()
        .filter_map(|pkg_id| {
            if lockfile.is_entrypoint(pkg_id) {
                None
            } else {
                Some(unsafe { lockfile.get_unchecked(pkg_id) }.name().to_string())
            }
        })
        .collect_vec();
    if !non_entrypoints.is_empty() {
        return Err(eyre!(
            "
Cannot uninstall dependencies:
{:#?}
",
            non_entrypoints,
        ));
    }

    let (dependencies, entrypoints): (Vec<LocalPackageId>, Vec<LocalPackageId>) = packages
        .iter()
        .cloned()
        .partition(|pkg_id| lockfile.is_dependency(pkg_id));

    operations::Remove::new(&config)
        .packages(entrypoints)
        .remove()
        .await?;

    if !dependencies.is_empty() {
        let package_names = dependencies
            .iter()
            .map(|pkg_id| unsafe { lockfile.get_unchecked(pkg_id) }.name().to_string())
            .collect_vec();
        let prompt = if package_names.len() == 1 {
            format!(
                "
            Package {} can be removed from the entrypoints, but it is also a dependency, so it will have to be reinstalled.
Reinstall?
            ",
                package_names[0]
            )
        } else {
            format!(
                "
            The following packages can be removed from the entrypoints, but are also dependencies:
{:#?}

They will have to be reinstalled.
Reinstall?
            ",
                package_names
            )
        };
        if Confirm::new(&prompt)
            .with_default(false)
            .prompt()
            .expect("Error prompting for reinstall")
        {
            let reinstall_specs = dependencies
                .iter()
                .map(|pkg_id| {
                    let package = unsafe { lockfile.get_unchecked(pkg_id) };
                    PackageInstallSpec::new(
                        package.clone().into_package_req(),
                        BuildBehaviour::Force,
                        package.pinned(),
                        package.opt(),
                        tree::EntryType::DependencyOnly,
                        Some(package.constraint()),
                        None,
                    )
                })
                .collect_vec();
            let progress = MultiProgress::new_arc();
            operations::Remove::new(&config)
                .packages(dependencies)
                .progress(progress.clone())
                .remove()
                .await?;
            operations::Install::new(&tree, &config)
                .packages(reinstall_specs)
                .progress(progress)
                .install()
                .await?;
        } else {
            return Err(eyre!("Operation cancelled."));
        }
    };

    Ok(())
}
