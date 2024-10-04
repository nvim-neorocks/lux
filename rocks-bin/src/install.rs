use eyre::Result;
use indicatif::MultiProgress;
use rocks_lib::{config::Config, lua_package::LuaPackageReq};

#[derive(clap::Args)]
pub struct Install {
    #[clap(flatten)]
    package_req: LuaPackageReq,
}

pub async fn install(install_data: Install, config: Config) -> Result<()> {
    // TODO(vhyrro): If the tree doesn't exist then error out.
    rocks_lib::operations::install(&MultiProgress::new(), install_data.package_req, &config)
        .await?;

    Ok(())
}
