use std::{
    env,
    fs::{self},
    path::{Path, PathBuf},
    process::Command,
};

type DynError = Box<dyn std::error::Error>;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);

    match task.as_deref() {
        Some("dist") => dist(true)?,
        _ => print_help(),
    }

    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

dist    builds the lua libraries for a given lua version (must specify via features)
"
    )
}

fn dist(release: bool) -> Result<(), DynError> {
    let _ = fs::remove_dir_all(dist_dir());
    fs::create_dir_all(dist_dir())?;

    let profile = if release { "release" } else { "debug" };

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let dest_dir = project_root().join(format!("target/{profile}"));

    let (lua_feature_flag, canonical_lua_version) = if cfg!(feature = "lua51") {
        ("lua51", "5.1")
    } else if cfg!(feature = "lua52") {
        ("lua52", "5.2")
    } else if cfg!(feature = "lua53") {
        ("lua53", "5.3")
    } else if cfg!(feature = "lua54") {
        ("lua54", "5.4")
    } else if cfg!(feature = "luajit") {
        ("luajit", "jit")
    } else {
        Err("No Lua version feature enabled")?
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
        project_root().join(format!("target/{profile}/liblux_lua.so")),
        dir.join(format!("{canonical_lua_version}/lux.so")),
    )?;

    let version = {
        let manifest_path = project_root().join("lux-lua/Cargo.toml");
        let manifest = fs::read_to_string(manifest_path)?;
        let package: toml::Value = toml::from_str(&manifest)?;
        package["package"]["version"]
            .as_str()
            .ok_or("Failed to get version")?
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
