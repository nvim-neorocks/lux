use crate::{
    build::BuildBehaviour,
    lockfile::{OptState, PinnedState},
    package::PackageReq,
};

/// Specifies how to install a package
pub struct PackageInstallSpec {
    pub(crate) package: PackageReq,
    pub(crate) build_behaviour: BuildBehaviour,
    pub(crate) pin: PinnedState,
    pub(crate) opt: OptState,
}

impl PackageInstallSpec {
    pub fn new(
        package: PackageReq,
        build_behaviour: BuildBehaviour,
        pin: PinnedState,
        opt: OptState,
    ) -> Self {
        Self {
            package,
            build_behaviour,
            pin,
            opt,
        }
    }
    pub fn default_for(package: PackageReq) -> Self {
        Self {
            package,
            build_behaviour: BuildBehaviour::default(),
            pin: PinnedState::default(),
            opt: OptState::default(),
        }
    }
}
