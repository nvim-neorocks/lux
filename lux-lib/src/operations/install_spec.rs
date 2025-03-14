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
    pub(crate) is_entrypoint: bool,
}

impl PackageInstallSpec {
    pub fn new(
        package: PackageReq,
        build_behaviour: BuildBehaviour,
        pin: PinnedState,
        opt: OptState,
        is_entrypoint: bool,
    ) -> Self {
        Self {
            package,
            build_behaviour,
            pin,
            opt,
            is_entrypoint,
        }
    }
}
