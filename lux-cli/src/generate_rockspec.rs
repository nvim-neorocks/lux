use clap::Args;
use eyre::Result;
use lux_lib::{project::Project, rockspec::Rockspec};

#[derive(Args)]
pub struct GenerateRockspec {}

pub fn generate_rockspec(_data: GenerateRockspec) -> Result<()> {
    let project = Project::current_or_err()?;

    let toml = project.toml().into_remote(None)?;
    let rockspec = toml.to_lua_remote_rockspec_string()?;

    let path = project
        .root()
        .join(format!("{}-{}.rockspec", toml.package(), toml.version()));

    std::fs::write(&path, rockspec)?;

    println!("Wrote rockspec to {}", path.display());

    Ok(())
}
