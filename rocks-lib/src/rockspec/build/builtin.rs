use std::{collections::HashMap, path::PathBuf};

use eyre::{OptionExt, Result};
use itertools::Itertools;
use serde::{de, Deserialize, Deserializer};
use walkdir::WalkDir;

use crate::{rockspec::Rockspec, tree::TreeLayout};

use super::Build;

#[derive(Debug, PartialEq, Deserialize, Default, Clone)]
pub struct BuiltinBuildSpec {
    /// Keys are module names in the format normally used by the `require()` function
    pub modules: HashMap<String, ModuleType>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ModuleType {
    /// Pathnames of Lua files or C sources, for modules based on a single source file.
    SourcePath(PathBuf),
    /// Pathnames of C sources of a simple module written in C composed of multiple files.
    SourcePaths(Vec<PathBuf>),
    ModulePaths(ModulePaths),
}

impl<'de> Deserialize<'de> for ModuleType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_string() {
            let src_path = serde_json::from_value(value).map_err(de::Error::custom)?;
            Ok(Self::SourcePath(src_path))
        } else if value.is_array() {
            let src_paths = serde_json::from_value(value).map_err(de::Error::custom)?;
            Ok(Self::SourcePaths(src_paths))
        } else {
            let module_paths = serde_json::from_value(value).map_err(de::Error::custom)?;
            Ok(Self::ModulePaths(module_paths))
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct ModulePaths {
    /// Path names of C sources, mandatory field
    pub sources: Vec<PathBuf>,
    /// External libraries to be linked
    #[serde(default)]
    pub libraries: Vec<PathBuf>,
    /// C defines, e.g. { "FOO=bar", "USE_BLA" }
    #[serde(default)]
    pub defines: Vec<String>,
    /// Directories to be added to the compiler's headers lookup directory list.
    #[serde(default)]
    pub incdirs: Vec<PathBuf>,
    /// Directories to be added to the linker's library lookup directory list.
    #[serde(default)]
    pub libdirs: Vec<PathBuf>,
}

impl Build for BuiltinBuildSpec {
    fn run(self, _rockspec: Rockspec, output_paths: TreeLayout, _no_install: bool) -> Result<()> {
        // Detect all Lua modules
        let modules = autodetect_modules()?
            .into_iter()
            .chain(self.modules)
            .collect::<HashMap<_, _>>();

        for (destination_path, module_type) in &modules {
            match module_type {
                ModuleType::SourcePath(source) => {
                    let target = output_paths.src.join(PathBuf::from(
                        destination_path.replace('.', std::path::MAIN_SEPARATOR_STR) + ".lua",
                    ));

                    std::fs::create_dir_all(target.parent().unwrap())?;

                    std::fs::copy(source, target)?;
                }
                ModuleType::SourcePaths(files) => {
                    let path = PathBuf::from(destination_path);
                    std::fs::create_dir_all(path.parent().unwrap())?;

                    cc::Build::new()
                        .shared_flag(true)
                        .files(files)
                        .try_compile(destination_path)?;
                }
                ModuleType::ModulePaths(data) => {
                    let path = PathBuf::from(destination_path);
                    std::fs::create_dir_all(path.parent().unwrap())?;

                    // TODO: Defines, libraries
                    cc::Build::new()
                        .shared_flag(true)
                        .files(&data.sources)
                        .includes(&data.incdirs)
                        .try_compile(destination_path)?;
                }
            }
        }

        Ok(())
    }
}

fn autodetect_modules() -> Result<HashMap<String, ModuleType>> {
    WalkDir::new("src")
        .into_iter()
        .chain(WalkDir::new("lua"))
        .chain(WalkDir::new("lib"))
        .filter_map(|file| {
            file.ok().and_then(|file| {
                if PathBuf::from(file.file_name())
                    .extension()
                    .map(|ext| ext == "lua")
                    .unwrap_or(false)
                    && !matches!(
                        file.file_name().to_string_lossy().as_bytes(),
                        b"spec" | b".luarocks" | b"lua_modules" | b"test.lua" | b"tests.lua"
                    )
                {
                    Some(file)
                } else {
                    None
                }
            })
        })
        .map(|file| {
            let cwd = std::env::current_dir().unwrap();
            let diff: PathBuf = pathdiff::diff_paths(cwd.join(file.into_path()), cwd)
                .ok_or_eyre("unable to autodetect modules")?;

            // NOTE(vhyrro): You may ask why we convert all paths to Lua module paths
            // just to convert them back later in the `run()` stage.
            //
            // The rockspec requires the format to be like this, and representing our
            // data in this form allows us to respect any overrides made by the user (which follow
            // the `module.name` format, not our internal one).
            let lua_module_path = diff
                .components()
                .skip(1)
                .collect::<PathBuf>()
                .to_string_lossy()
                .trim_end_matches(".lua")
                .replace(std::path::MAIN_SEPARATOR_STR, ".");

            Ok((lua_module_path, ModuleType::SourcePath(diff)))
        })
        .try_collect()
}

#[cfg(test)]
mod tests {
    use mlua::{Lua, LuaSerdeExt};

    use super::*;

    #[tokio::test]
    pub async fn modules_spec_from_lua() {
        let lua_content = "
        build = {\n
            modules = {\n
                foo = 'lua/foo/init.lua',\n
                bar = {\n
                  'lua/bar.lua',\n
                  'lua/bar/internal.lua',\n
                },\n
                baz = {\n
                    sources = {\n
                        'lua/baz.lua',\n
                    },\n
                    defines = { 'USE_BAZ' },\n
                },\n
            },\n
        }\n
        ";
        let lua = Lua::new();
        lua.load(lua_content).exec().unwrap();
        let build_spec: BuiltinBuildSpec =
            lua.from_value(lua.globals().get("build").unwrap()).unwrap();
        let foo = build_spec.modules.get("foo").unwrap();
        assert_eq!(*foo, ModuleType::SourcePath("lua/foo/init.lua".into()));
        let bar = build_spec.modules.get("bar").unwrap();
        assert_eq!(
            *bar,
            ModuleType::SourcePaths(vec!["lua/bar.lua".into(), "lua/bar/internal.lua".into()])
        );
        let baz = build_spec.modules.get("baz").unwrap();
        assert!(matches!(baz, ModuleType::ModulePaths { .. }));
        let lua_content_no_sources = "
        build = {\n
            modules = {\n
                baz = {\n
                    defines = { 'USE_BAZ' },\n
                },\n
            },\n
        }\n
        ";
        lua.load(lua_content_no_sources).exec().unwrap();
        let result: mlua::Result<BuiltinBuildSpec> =
            lua.from_value(lua.globals().get("build").unwrap());
        let _err = result.unwrap_err();
    }
}
