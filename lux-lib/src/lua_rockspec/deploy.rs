use std::convert::Infallible;

use serde::Deserialize;

use super::{PartialOverride, PerPlatform, PlatformOverridable};

/// An undocumented part of the rockspec format.
///
/// Specifies additional install options
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DeploySpec {
    /// Whether to wrap installed Lua bin scripts to be executed with
    /// the detected or configured Lua installation.
    /// Defaults to `true`.
    /// This only affects lua bin scripts that are declared in the rockspec.
    #[serde(default = "default_wrap_bin_scripts")]
    pub wrap_bin_scripts: bool,
}

impl Default for DeploySpec {
    fn default() -> Self {
        Self {
            wrap_bin_scripts: true,
        }
    }
}

impl PartialOverride for DeploySpec {
    type Err = Infallible;

    fn apply_overrides(&self, override_spec: &Self) -> Result<Self, Self::Err> {
        Ok(Self {
            wrap_bin_scripts: override_spec.wrap_bin_scripts,
        })
    }
}

impl PlatformOverridable for DeploySpec {
    type Err = Infallible;

    fn on_nil<T>() -> Result<PerPlatform<T>, <Self as PlatformOverridable>::Err>
    where
        T: PlatformOverridable,
        T: Default,
    {
        Ok(PerPlatform::default())
    }
}

fn default_wrap_bin_scripts() -> bool {
    true
}
