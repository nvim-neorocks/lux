use clap::Args;
use eyre::Result;
use lux_lib::{config::Config, project::Project, upload::ProjectUpload};

#[cfg(not(target_env = "msvc"))]
use lux_lib::upload::SignatureProtocol;

#[derive(Args)]
pub struct Upload {
    /// The protocol to use when signing upload artefacts
    #[cfg(not(target_env = "msvc"))]
    #[arg(long, default_value_t)]
    sign_protocol: SignatureProtocol,
}

#[cfg(not(target_env = "msvc"))]
pub async fn upload(data: Upload, config: Config) -> Result<()> {
    let project = Project::current()?.unwrap();

    ProjectUpload::new(project, &config)
        .sign_protocol(data.sign_protocol)
        .upload_to_luarocks()
        .await?;

    Ok(())
}

#[cfg(target_env = "msvc")]
pub async fn upload(_data: Upload, config: Config) -> Result<()> {
    let project = Project::current()?.unwrap();

    ProjectUpload::new(project, &config)
        .upload_to_luarocks()
        .await?;

    Ok(())
}
