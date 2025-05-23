use path_slash::PathBufExt;
use pkg_config::{Config as PkgConfig, Library};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::{
    config::external_deps::ExternalDependencySearchConfig, lua_rockspec::ExternalDependencySpec,
};

use super::{utils::format_path, variables::HasVariables};

#[derive(Error, Debug)]
pub enum ExternalDependencyError {
    #[error("{}", not_found_error_msg(.0))]
    NotFound(String),
    #[error("IO error while trying to detect external dependencies: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0} was probed successfully, but the header {1} could not be found")]
    SuccessfulProbeHeaderNotFound(String, String),
    #[error("error probing external dependency {0}: the header {1} could not be found")]
    HeaderNotFound(String, String),
    #[error("error probing external dependency {0}: the library {1} could not be found")]
    LibraryNotFound(String, String),
}

#[derive(Debug)]
pub struct ExternalDependencyInfo {
    include_dir: Option<PathBuf>,
    lib_dir: Option<PathBuf>,
    bin_dir: Option<PathBuf>,
}

fn pkg_config_probe(name: &str) -> Option<Library> {
    PkgConfig::new()
        .print_system_libs(false)
        .cargo_metadata(false)
        .env_metadata(false)
        .probe(&name.to_lowercase())
        .ok()
}

impl ExternalDependencyInfo {
    pub fn probe(
        name: &str,
        dependency: &ExternalDependencySpec,
        config: &ExternalDependencySearchConfig,
    ) -> Result<Self, ExternalDependencyError> {
        let lib_info = pkg_config_probe(name)
            .or(pkg_config_probe(&format!("lib{}", name.to_lowercase())))
            .or(dependency.library.as_ref().and_then(|lib_name| {
                let lib_name = lib_name.to_string_lossy().to_string();
                let lib_name_without_ext = lib_name.split('.').next().unwrap_or(&lib_name);
                pkg_config_probe(lib_name_without_ext)
                    .or(pkg_config_probe(&format!("lib{}", lib_name_without_ext)))
            }));
        if let Some(info) = lib_info {
            let include_dir = if let Some(header) = &dependency.header {
                Some(
                    info.include_paths
                        .iter()
                        .find(|path| path.join(header).exists())
                        .ok_or(ExternalDependencyError::SuccessfulProbeHeaderNotFound(
                            name.to_string(),
                            header.to_slash_lossy().to_string(),
                        ))?
                        .clone(),
                )
            } else {
                info.include_paths.first().cloned()
            };
            let lib_dir = if let Some(lib) = &dependency.library {
                info.link_paths
                    .iter()
                    .find(|path| library_exists(path, lib, &config.lib_patterns))
                    .cloned()
                    .or(info.link_paths.first().cloned())
            } else {
                info.link_paths.first().cloned()
            };
            let bin_dir = lib_dir.as_ref().and_then(|lib_dir| {
                lib_dir
                    .parent()
                    .map(|parent| parent.join("bin"))
                    .filter(|dir| dir.is_dir())
            });
            return Ok(ExternalDependencyInfo {
                include_dir,
                lib_dir,
                bin_dir,
            });
        }
        Self::fallback_probe(name, dependency, config)
    }

    fn fallback_probe(
        name: &str,
        dependency: &ExternalDependencySpec,
        config: &ExternalDependencySearchConfig,
    ) -> Result<Self, ExternalDependencyError> {
        let env_prefix = std::env::var(format!("{}_DIR", name.to_uppercase())).ok();

        let mut search_prefixes = Vec::new();
        if let Some(dir) = env_prefix {
            search_prefixes.push(PathBuf::from(dir));
        }
        if let Some(prefix) = config.prefixes.get(&format!("{}_DIR", name.to_uppercase())) {
            search_prefixes.push(prefix.clone());
        }
        search_prefixes.extend(config.search_prefixes.iter().cloned());

        let mut include_dir = get_incdir(name, config);

        if let Some(header) = &dependency.header {
            if !&include_dir
                .as_ref()
                .is_some_and(|inc_dir| inc_dir.join(header).exists())
            {
                // Search prefixes
                let inc_dir = search_prefixes
                    .iter()
                    .find_map(|prefix| {
                        let inc_dir = prefix.join(&config.include_subdir);
                        if inc_dir.join(header).exists() {
                            Some(inc_dir)
                        } else {
                            None
                        }
                    })
                    .ok_or(ExternalDependencyError::HeaderNotFound(
                        name.to_string(),
                        header.to_slash_lossy().to_string(),
                    ))?;
                include_dir = Some(inc_dir);
            }
        }

        let mut lib_dir = get_libdir(name, config);

        if let Some(lib) = &dependency.library {
            if !lib_dir
                .as_ref()
                .is_some_and(|lib_dir| library_exists(lib_dir, lib, &config.lib_patterns))
            {
                let probed_lib_dir = search_prefixes
                    .iter()
                    .find_map(|prefix| {
                        for lib_subdir in &config.lib_subdirs {
                            let lib_dir_candidate = prefix.join(lib_subdir);
                            if library_exists(&lib_dir_candidate, lib, &config.lib_patterns) {
                                return Some(lib_dir_candidate);
                            }
                        }
                        None
                    })
                    .ok_or(ExternalDependencyError::LibraryNotFound(
                        name.to_string(),
                        lib.to_slash_lossy().to_string(),
                    ))?;
                lib_dir = Some(probed_lib_dir);
            }
        }

        if let (None, None) = (&include_dir, &lib_dir) {
            return Err(ExternalDependencyError::NotFound(name.into()));
        }
        let bin_dir = lib_dir.as_ref().and_then(|lib_dir| {
            lib_dir
                .parent()
                .map(|parent| parent.join("bin"))
                .filter(|dir| dir.is_dir())
        });
        Ok(ExternalDependencyInfo {
            include_dir,
            lib_dir,
            bin_dir,
        })
    }
}

impl HasVariables for HashMap<String, ExternalDependencyInfo> {
    fn get_variable(&self, input: &str) -> Option<String> {
        input.split_once('_').and_then(|(dep_key, dep_dir_type)| {
            self.get(dep_key)
                .and_then(|dep| match dep_dir_type {
                    "DIR" => dep
                        .include_dir
                        .as_ref()
                        .and_then(|dir| dir.parent().map(|parent| parent.to_path_buf())),
                    "INCDIR" => dep.include_dir.clone(),
                    "LIBDIR" => dep.lib_dir.clone(),
                    "BINDIR" => dep.bin_dir.clone(),
                    _ => None,
                })
                .as_deref()
                .map(format_path)
        })
    }
}

fn library_exists(lib_dir: &Path, lib: &Path, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        let file_name = pattern.replace('?', &format!("{}", lib.display()));
        lib_dir.join(&file_name).exists()
    })
}

fn get_incdir(name: &str, config: &ExternalDependencySearchConfig) -> Option<PathBuf> {
    let var_name = format!("{}_INCDIR", name.to_uppercase());
    if let Ok(env_incdir) = std::env::var(&var_name) {
        Some(env_incdir.into())
    } else {
        config.prefixes.get(&var_name).cloned()
    }
    .filter(|dir| dir.is_dir())
}

fn get_libdir(name: &str, config: &ExternalDependencySearchConfig) -> Option<PathBuf> {
    let var_name = format!("{}_LIBDIR", name.to_uppercase());
    if let Ok(env_incdir) = std::env::var(&var_name) {
        Some(env_incdir.into())
    } else {
        config.prefixes.get(&var_name).cloned()
    }
    .filter(|dir| dir.is_dir())
}

fn not_found_error_msg(name: &String) -> String {
    let env_dir = format!("{}_DIR", &name.to_uppercase());
    let env_inc = format!("{}_INCDIR", &name.to_uppercase());
    let env_lib = format!("{}_LIBDIR", &name.to_uppercase());

    format!(
        r#"External dependency not found: {}.
Consider one of the following:
1. Set environment variables:
   - {} for the installation prefix, or
   - {} and {} for specific directories
2. Add the installation prefix to the configuration:
   {} = "/path/to/installation""#,
        name, env_dir, env_inc, env_lib, env_dir,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::{prelude::*, TempDir};

    #[tokio::test]
    async fn test_detect_zlib_pkg_config_header() {
        // requires zlib to be in the nativeCheckInputs or dev environment
        let config = ExternalDependencySearchConfig::default();
        ExternalDependencyInfo::probe(
            "zlib",
            &ExternalDependencySpec {
                header: Some("zlib.h".into()),
                library: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_detect_zlib_pkg_config_library_libz() {
        // requires zlib to be in the nativeCheckInputs or dev environment
        let config = ExternalDependencySearchConfig::default();
        ExternalDependencyInfo::probe(
            "zlib",
            &ExternalDependencySpec {
                library: Some("libz".into()),
                header: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_detect_zlib_pkg_config_library_z() {
        // requires zlib to be in the nativeCheckInputs or dev environment
        let config = ExternalDependencySearchConfig::default();
        ExternalDependencyInfo::probe(
            "zlib",
            &ExternalDependencySpec {
                library: Some("z".into()),
                header: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_detect_zlib_pkg_config_library_zlib() {
        // requires zlib to be in the nativeCheckInputs or dev environment
        let config = ExternalDependencySearchConfig::default();
        ExternalDependencyInfo::probe(
            "zlib",
            &ExternalDependencySpec {
                library: Some("zlib".into()),
                header: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_fallback_detect_header_prefix() {
        let temp = TempDir::new().unwrap();
        let prefix_dir = temp.child("usr");
        let include_dir = prefix_dir.child("include");
        include_dir.create_dir_all().unwrap();

        let header = include_dir.child("foo.h");
        header.touch().unwrap();

        let mut config = ExternalDependencySearchConfig::default();
        config
            .prefixes
            .insert("FOO_DIR".into(), prefix_dir.path().to_path_buf());

        ExternalDependencyInfo::fallback_probe(
            "foo",
            &ExternalDependencySpec {
                header: Some("foo.h".into()),
                library: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_fallback_detect_header_prefix_incdir() {
        let temp = TempDir::new().unwrap();
        let include_dir = temp.child("include");
        include_dir.create_dir_all().unwrap();

        let header = include_dir.child("foo.h");
        header.touch().unwrap();

        let mut config = ExternalDependencySearchConfig::default();
        config
            .prefixes
            .insert("FOO_INCDIR".into(), include_dir.path().to_path_buf());

        ExternalDependencyInfo::fallback_probe(
            "foo",
            &ExternalDependencySpec {
                header: Some("foo.h".into()),
                library: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_fallback_detect_library_prefix() {
        let temp = TempDir::new().unwrap();
        let prefix_dir = temp.child("usr");
        let include_dir = prefix_dir.child("include");
        let lib_dir = prefix_dir.child("lib");
        include_dir.create_dir_all().unwrap();
        lib_dir.create_dir_all().unwrap();

        #[cfg(target_os = "linux")]
        let lib = lib_dir.child("libfoo.so");
        #[cfg(target_os = "macos")]
        let lib = lib_dir.child("libfoo.dylib");
        #[cfg(target_family = "windows")]
        let lib = lib_dir.child("foo.dll");

        lib.touch().unwrap();

        let mut config = ExternalDependencySearchConfig::default();
        config
            .prefixes
            .insert("FOO_DIR".to_string(), prefix_dir.path().to_path_buf());

        ExternalDependencyInfo::fallback_probe(
            "foo",
            &ExternalDependencySpec {
                library: Some("foo".into()),
                header: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_fallback_detect_library_dirs() {
        let temp = TempDir::new().unwrap();

        let include_dir = temp.child("include");
        include_dir.create_dir_all().unwrap();

        let lib_dir = temp.child("lib");
        lib_dir.create_dir_all().unwrap();

        #[cfg(target_os = "linux")]
        let lib = lib_dir.child("libfoo.so");
        #[cfg(target_os = "macos")]
        let lib = lib_dir.child("libfoo.dylib");
        #[cfg(target_family = "windows")]
        let lib = lib_dir.child("foo.dll");

        lib.touch().unwrap();

        let mut config = ExternalDependencySearchConfig::default();
        config
            .prefixes
            .insert("FOO_INCDIR".into(), include_dir.path().to_path_buf());
        config
            .prefixes
            .insert("FOO_LIBDIR".into(), lib_dir.path().to_path_buf());

        ExternalDependencyInfo::fallback_probe(
            "foo",
            &ExternalDependencySpec {
                library: Some("foo".into()),
                header: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_fallback_detect_search_prefixes() {
        let temp = TempDir::new().unwrap();
        let prefix_dir = temp.child("usr");
        let include_dir = prefix_dir.child("include");
        let lib_dir = prefix_dir.child("lib");
        include_dir.create_dir_all().unwrap();
        lib_dir.create_dir_all().unwrap();

        #[cfg(target_os = "linux")]
        let lib = lib_dir.child("libfoo.so");
        #[cfg(target_os = "macos")]
        let lib = lib_dir.child("libfoo.dylib");
        #[cfg(target_family = "windows")]
        let lib = lib_dir.child("foo.dll");

        lib.touch().unwrap();

        let mut config = ExternalDependencySearchConfig::default();
        config.search_prefixes.push(prefix_dir.path().to_path_buf());

        ExternalDependencyInfo::fallback_probe(
            "foo",
            &ExternalDependencySpec {
                library: Some("foo".into()),
                header: None,
            },
            &config,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_fallback_detect_not_found() {
        let config = ExternalDependencySearchConfig::default();

        let result = ExternalDependencyInfo::fallback_probe(
            "foo",
            &ExternalDependencySpec {
                header: Some("foo.h".into()),
                library: None,
            },
            &config,
        );

        assert!(matches!(
            result,
            Err(ExternalDependencyError::HeaderNotFound { .. })
        ));
    }
}
