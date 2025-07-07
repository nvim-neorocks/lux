use clap::CommandFactory;
use clap_complete::generate as clap_generate;
use clap_complete::Shell;
use eyre::Result;

use crate::Cli;

pub async fn generate(shell: Shell) -> Result<()> {
    clap_generate(shell, &mut Cli::command(), "lx", &mut std::io::stdout());
    Ok(())
}
