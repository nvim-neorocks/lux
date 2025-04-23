use crate::{
    build::BuildBehaviour,
    lockfile::{LockConstraint, OptState, PinnedState},
    lua_rockspec::RockSourceSpec,
    package::PackageReq,
    tree,
};

/// Specifies how to install a package
#[derive(Debug)]
pub struct PackageInstallSpec {
    pub(crate) package: PackageReq,
    pub(crate) build_behaviour: BuildBehaviour,
    pub(crate) pin: PinnedState,
    pub(crate) opt: OptState,
    pub(crate) entry_type: tree::EntryType,
    /// Optional constraint, carried over from a previous install,
    /// e.g. defined in a lockfile.
    pub(crate) constraint: Option<LockConstraint>,
    pub(crate) source: Option<RockSourceSpec>,
}

impl PackageInstallSpec {
    pub fn new(
        package: PackageReq,
        build_behaviour: BuildBehaviour,
        pin: PinnedState,
        opt: OptState,
        entry_type: tree::EntryType,
        constraint: Option<LockConstraint>,
        source: Option<RockSourceSpec>,
    ) -> Self {
        Self {
            package,
            build_behaviour,
            pin,
            opt,
            entry_type,
            constraint,
            source,
        }
    }
}
