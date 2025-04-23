//! Utilities for converting a list of packages into a list with the correct build behaviour.

use eyre::Result;
use inquire::Confirm;
use lux_lib::{
    build::BuildBehaviour,
    lockfile::{LocalPackageId, OptState, PinnedState},
    operations::PackageInstallSpec,
    package::PackageReq,
    tree::{self, RockMatches, Tree},
};

pub fn apply_build_behaviour(
    package_reqs: Vec<PackageReq>,
    pin: PinnedState,
    force: bool,
    tree: &Tree,
) -> Result<Vec<PackageInstallSpec>> {
    let lockfile = tree.lockfile()?;
    Ok(package_reqs
        .into_iter()
        .filter_map(|req| {
            let existing_packages: Vec<LocalPackageId> = match tree
                .match_rocks_and(&req, |rock| pin == rock.pinned())
                .expect("unable to get tree data")
            {
                RockMatches::Single(id) => vec![id],
                RockMatches::Many(ids) => ids,
                _ => Vec::new(),
            };
            // NOTE: Because the rock layout may change, we must force a rebuild
            // if a package is installed, but it is not an entrypoint.
            let force = force
                || existing_packages
                    .iter()
                    .all(|pkg_id| !lockfile.is_entrypoint(pkg_id));
            let build_behaviour: Option<BuildBehaviour> = if force || existing_packages.is_empty() {
                Some(BuildBehaviour::from(force))
            } else if Confirm::new(&format!("Package {} already exists. Overwrite?", req))
                .with_default(false)
                .prompt()
                .expect("Error prompting for reinstall")
            {
                Some(BuildBehaviour::Force)
            } else {
                None
            };
            build_behaviour.map(|it| {
                PackageInstallSpec::new(
                    req,
                    it,
                    pin,
                    OptState::Required,
                    tree::EntryType::Entrypoint,
                    None,
                    None,
                )
            })
        })
        .collect())
}
