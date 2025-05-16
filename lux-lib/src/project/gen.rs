use std::path::PathBuf;

use ::serde::Deserialize;
use thiserror::Error;

use crate::{lua_rockspec::RockSourceInternal, package::{PackageName, PackageVersion}};

use super::ProjectRoot;

#[derive(Debug, PartialEq, Deserialize, Clone, Default)]
/// Template for generating a remote rockspec source
///
/// Variables that can be substituted in each of the fields:
/// - `$(PACKAGE)`: Package name
/// - `$(VERSION)`: Package version
/// - `$(REF)`: Git revision or tag
///
/// Fields can also be substituted with environment variables.
pub(crate) struct RockSourceTemplate {
    /// URL template for `SemVer` releases
    url: Option<String>,

    /// URL template for `DevVer` releases
    dev: Option<String>,

    /// File name of the source archive.
    /// Can be omitted if it can be inferred from the generated URL.
    file: Option<String>,

    /// Name of the directory created when the source archive is unpacked.
    /// Can be omitted if it can be inferred from the `file` field.
    dir: Option<PathBuf>,

    /// The tag or revision to be checked out if the source URL is a git source.
    /// If unset, Lux will try to auto-detect it.
    tag: Option<String>,
}

#[derive(Debug, Error, Clone)]
pub enum GenerateSourceError {}

impl RockSourceTemplate {
    pub(crate) fn try_generate(
        &self,
        _project_root: &ProjectRoot,
        _package: &PackageName,
        _version: &PackageVersion,
    ) -> Result<RockSourceInternal, GenerateSourceError> {
        todo!()
    }
}

#[derive(Debug, PartialEq, Deserialize, Clone, Default)]
pub(crate) struct PackageVersionTemplate(Option<PackageVersion>);

#[derive(Debug, Error, Clone)]
pub enum GenerateVersionError {}

impl PackageVersionTemplate {
    pub(crate) fn try_generate(
        &self,
        _project_root: &ProjectRoot,
    ) -> Result<PackageVersion, GenerateSourceError> {
        if let Some(version) = &self.0 {
            Ok(version.clone())
        } else {
            todo!("use libgit to detect SemVer version");
        }
    }
}

// TODO:
// - Set source.tag automatically if url is a git url
//   and there exists a tag.
