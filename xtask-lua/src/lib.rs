use std::{
    env::{self, consts::DLL_EXTENSION},
    fs::{self},
    path::{Path, PathBuf},
    process::Command,
};

pub type DynError = Box<dyn std::error::Error>;

pub enum LuaFeature {
    Lua51,
    Lua52,
    Lua53,
    Lua54,
    Luajit,
}

#[derive(Default)]
pub struct DistOpts {
    pub lua_feature: Option<LuaFeature>,
    /// Whether to delete the `target/dist` directory
    pub clean_dist_dir: bool,
}

pub fn dist(release: bool, opts: Option<DistOpts>) -> Result<(), DynError> {
    let opts = opts.unwrap_or_default();
    if opts.clean_dist_dir {
        let _ = fs::remove_dir_all(dist_dir());
    }
    fs::create_dir_all(dist_dir())?;

    let profile = if release { "release" } else { "debug" };

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let dest_dir = project_root().join(format!("target/{profile}"));

    let lua_feature = match opts.lua_feature {
        Some(lua_feature) => lua_feature,
        None => {
            if cfg!(feature = "lua51") {
                LuaFeature::Lua51
            } else if cfg!(feature = "lua52") {
                LuaFeature::Lua52
            } else if cfg!(feature = "lua53") {
                LuaFeature::Lua53
            } else if cfg!(feature = "lua54") {
                LuaFeature::Lua54
            } else if cfg!(feature = "luajit") {
                LuaFeature::Luajit
            } else {
                Err("No Lua version feature enabled")?
            }
        }
    };

    let (lua_feature_flag, canonical_lua_version) = match lua_feature {
        LuaFeature::Lua51 => ("lua51", "5.1"),
        LuaFeature::Lua52 => ("lua52", "5.2"),
        LuaFeature::Lua53 => ("lua53", "5.3"),
        LuaFeature::Lua54 => ("lua54", "5.4"),
        LuaFeature::Luajit => ("luajit", "jit"),
    };

    let mut args = vec![
        "build",
        "--no-default-features",
        "--features",
        lua_feature_flag,
    ];

    if release {
        args.push("--release");
    }

    let status = Command::new(&cargo)
        .current_dir(project_root().join("lux-lua"))
        .args(args)
        .status()?;

    if !status.success() {
        Err("cargo build failed")?;
    }

    let dir = if release {
        dist_dir()
    } else {
        dest_dir.clone()
    };

    let _ = fs::remove_dir_all(dir.join(canonical_lua_version));
    fs::create_dir_all(dir.join(canonical_lua_version))?;

    fs::copy(
        project_root().join(format!("target/{profile}/liblux_lua.{DLL_EXTENSION}")),
        dir.join(format!("{canonical_lua_version}/lux.so")),
    )?;

    let version = {
        let manifest_path = project_root().join("Cargo.toml");
        let manifest = fs::read_to_string(manifest_path)?;
        let package: toml::Value = toml::from_str(&manifest)?;
        package["workspace"]["package"]["version"]
            .as_str()
            .ok_or("lux-lua: Failed to get version from Cargo.toml")?
            .to_string()
    };

    // Create and write the pkg-config file
    let pkg_config_dir = dir.join("lib").join("pkgconfig");
    fs::create_dir_all(&pkg_config_dir)?;

    let lua_full_name = if canonical_lua_version == "jit" {
        "luajit".to_string()
    } else {
        format!("Lua {}", canonical_lua_version)
    };

    let pc_content = format!(
        r#"prefix=${{pcfiledir}}/../..
exec_prefix=${{prefix}}
libdir=${{prefix}}
luaversion={}

Name: lux-lua{}
Description: Lux API for {}
Version: {}
Cflags:
Libs: -L${{libdir}} -llux-lua"#,
        canonical_lua_version, canonical_lua_version, lua_full_name, version,
    );

    fs::write(
        pkg_config_dir.join(format!("lux-lua{canonical_lua_version}.pc")),
        pc_content,
    )?;

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn dist_dir() -> PathBuf {
    project_root().join("target/dist")
}
