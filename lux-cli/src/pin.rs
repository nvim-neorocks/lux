use clap::Args;
use eyre::eyre;
use eyre::Result;
use lux_lib::config::{Config, LuaVersion};
use lux_lib::lockfile::PinnedState;
use lux_lib::operations;
use lux_lib::package::PackageSpec;
use lux_lib::tree::RockMatches;

#[derive(Args)]
pub struct ChangePin {
    package: PackageSpec,
}

pub fn set_pinned_state(data: ChangePin, config: Config, pin: PinnedState) -> Result<()> {
    let tree = config.tree(LuaVersion::from(&config)?)?;

    match tree.match_rocks_and(&data.package.clone().into_package_req(), |package| {
        pin != package.pinned()
    })? {
        RockMatches::Single(rock) => Ok(operations::set_pinned_state(&rock, &tree, pin)?),
        RockMatches::Many(_) => {
            todo!("Add an error here about many conflicting types and to use `all:`")
        }
        RockMatches::NotFound(_) => Err(eyre!("Rock {} not found!", data.package)),
    }
}
