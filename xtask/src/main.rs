use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use cargo_packager::{
    config::{AppImageConfig, Binary, DebianConfig, PacmanConfig},
    PackageFormat,
};
use clap::{CommandFactory, ValueEnum};
use clap_complete::{generate_to, Shell};
use clap_mangen::Man;
use lux_cli::Cli;
use serde::Deserialize;
use xtask_lua::LuaFeature;

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
        // Assume that the user wants to build the release version
        // when trying to build the distributed version.
        Some("dist") => dist(true)?,
        Some("dist-man") => dist_man()?,
        Some("dist-completions") => dist_completions()?,
        Some("dist-package") => dist_package()?,
        Some("build") => build(false)?,
        Some("build-release") => build(true)?,
        _ => print_help(),
    }

    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

build               builds and links all libraries and the application
dist-man            builds man pages
dist-completions    builds shell completions
dist-package       builds an AppImage
dist                builds everything, equivalent to build + dist-man + dist-completions

LUA_LIB_DIR         when set, overrides the path to the directory containing the compiled lux-lua libraries
"
    )
}

fn dist(release: bool) -> Result<(), DynError> {
    build(release)?;
    dist_man()?;
    dist_completions()
}

fn build(release: bool) -> Result<(), DynError> {
    let profile = if release { "release" } else { "debug" };

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let dest_dir = project_root().join(format!("target/{profile}"));

    let mut args = vec!["build"];

    if release {
        args.push("--release");
    }

    // Build with luajit by default.
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(args)
        .env(
            "LUX_LIB_DIR",
            env::var("LUX_LIB_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    if release {
                        dist_dir()
                    } else {
                        dest_dir.clone()
                    }
                }),
        )
        .status()?;

    if !status.success() {
        Err("cargo build failed")?;
    }

    let dest = dest_dir.join("lx");

    if !dest.is_file() {
        Err(format!("{} not found", dest.display()))?;
    }
    if release {
        fs::create_dir_all(dist_dir())?;
        fs::copy(&dest, dist_dir().join("lx"))?;
    }

    if release
        && Command::new("strip")
            .arg("--version")
            .stdout(Stdio::null())
            .status()
            .inspect_err(|_| eprintln!("checking for `strip` utility"))
            .is_ok()
    {
        eprintln!("stripping the binary");
        let status = Command::new("strip").arg(&dest).status()?;
        if !status.success() {
            Err("strip failed")?;
        }
    }

    Ok(())
}

fn dist_man() -> Result<(), DynError> {
    fs::create_dir_all(dist_dir())?;

    let cmd = &mut Cli::command();

    Man::new(cmd.clone())
        .render(&mut File::create(dist_dir().join("lx.1")).unwrap())
        .unwrap();
    Ok(())
}

fn dist_completions() -> Result<(), DynError> {
    fs::create_dir_all(dist_dir())?;

    let cmd = &mut Cli::command();

    for shell in Shell::value_variants() {
        generate_to(*shell, cmd, "lx", dist_dir()).unwrap();
    }

    Ok(())
}

#[derive(Deserialize)]
struct LuxManifest {
    workspace: LuxWorkspace,
}

#[derive(Deserialize)]
struct LuxWorkspace {
    package: LuxPackage,
}

#[derive(Deserialize)]
struct LuxPackage {
    version: String,
}

fn dist_package() -> Result<(), DynError> {
    let _ = fs::remove_dir_all(dist_dir());
    xtask_lua::dist(
        true,
        Some(xtask_lua::DistOpts {
            lua_feature: Some(LuaFeature::Luajit),
            clean_dist_dir: false,
        }),
    )?;
    xtask_lua::dist(
        true,
        Some(xtask_lua::DistOpts {
            lua_feature: Some(LuaFeature::Lua51),
            clean_dist_dir: false,
        }),
    )?;
    xtask_lua::dist(
        true,
        Some(xtask_lua::DistOpts {
            lua_feature: Some(LuaFeature::Lua52),
            clean_dist_dir: false,
        }),
    )?;
    xtask_lua::dist(
        true,
        Some(xtask_lua::DistOpts {
            lua_feature: Some(LuaFeature::Lua53),
            clean_dist_dir: false,
        }),
    )?;
    xtask_lua::dist(
        true,
        Some(xtask_lua::DistOpts {
            lua_feature: Some(LuaFeature::Lua54),
            clean_dist_dir: false,
        }),
    )?;
    build(true)?;
    let project_root = project_root();
    let manifest_path = project_root.join("Cargo.toml");
    let manifest_content = fs::read_to_string(manifest_path)?;
    let manifest: LuxManifest = toml::from_str(&manifest_content)?;
    let lx_bin_path = dist_dir().join("lx");
    if !lx_bin_path.is_file() {
        Err(format!("{} not found", lx_bin_path.display()))?;
    }
    let svg_icon_path = project_root.join("lux-logo_256.svg");
    let png_icon_path = project_root.join("lux-logo_256.png");
    let file_mappings = [
        ("target/dist/share/lux-lua", "usr/share/lux-lua"),
        ("target/dist/lib/pkgconfig", "usr/lib/pkgconfig"),
    ];
    let config_builder = cargo_packager::Config::builder()
        .product_name("lux-cli")
        .version(manifest.workspace.package.version)
        .out_dir(dist_dir())
        .binaries(vec![Binary::new(lx_bin_path).main(true)])
        .description("A luxurious package manager for Lua")
        .homepage("https://nvim-neorocks.github.io/")
        .authors(vec!["mrcjkb", "vhyrro"])
        .publisher("nvim-neorocks")
        .identifier("org.neorocks.lux")
        .license_file(project_root.join("LICENSE"))
        .icons(vec![
            svg_icon_path.to_string_lossy(),
            png_icon_path.to_string_lossy(),
        ])
        .appimage(AppImageConfig::new().files(file_mappings))
        .pacman(
            PacmanConfig::new()
                .provides(["lx"])
                .conflicts(["lux-cli-git"])
                .files(file_mappings),
        )
        .deb(DebianConfig::new().files(file_mappings))
        .formats(vec![PackageFormat::All])
        .log_level(cargo_packager::config::LogLevel::Trace);
    // NOTE: The AppImage/linuxdeploy-<target>.AppImage will fail on NixOS.
    cargo_packager::package(config_builder.config())?; // TODO(mrcjkb): use package_and_sign
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
