use crate::{
    build::BuildBehaviour,
    lockfile::{OptState, PinnedState},
    package::PackageReq,
    tree,
};

/// Specifies how to install a package
pub struct PackageInstallSpec {
    pub(crate) package: PackageReq,
    pub(crate) build_behaviour: BuildBehaviour,
    pub(crate) pin: PinnedState,
    pub(crate) opt: OptState,
    pub(crate) entry_type: tree::EntryType,
}

impl PackageInstallSpec {
    pub fn new(
        package: PackageReq,
        build_behaviour: BuildBehaviour,
        pin: PinnedState,
        opt: OptState,
        entry_type: tree::EntryType,
    ) -> Self {
        Self {
            package,
            build_behaviour,
            pin,
            opt,
            entry_type,
        }
    }
}
