use std::path::PathBuf;

/// Template configuration for a rock's tree layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RockLayoutConfig {
    /// The root of a packages `etc` directory.
    /// If unset (the default), the root is the package root.
    /// If set, it is a directory relative to the given Lua version's install tree root.
    /// With the `--nvim` preset, this is `site/pack/lux`.
    pub(crate) etc_root: Option<PathBuf>,
    /// The `etc` directory for non-optional packages
    /// Default: `etc` With the `--nvim` preset, this is `start`
    /// Note: If `etc_root` is set, the package ID is appended.
    pub(crate) etc: PathBuf,
    /// The `etc` directory template for optional packages
    /// Default: `etc`
    /// With the `--nvim` preset, this is `opt`
    /// Note: If `etc_root` is set, the package ID is appended.
    pub(crate) opt_etc: String,
}

impl RockLayoutConfig {
    /// Creates a `RockLayoutConfig` for use with Neovim
    /// - `etc_root`: `site/pack/lux`
    /// - `etc`: `start`
    /// - `opt_etc`: `opt`
    pub fn new_nvim_layout() -> Self {
        Self {
            etc_root: Some("site/pack/lux".into()),
            etc: "start".into(),
            opt_etc: "opt".into(),
        }
    }
}

impl Default for RockLayoutConfig {
    fn default() -> Self {
        Self {
            etc_root: None,
            etc: "etc".into(),
            opt_etc: "etc".into(),
        }
    }
}
