use anyhow::Result;
use clap::ArgMatches;

pub fn handle(args: &ArgMatches) -> Result<()> {
    let sub_args = match args.subcommand_matches("receive") {
        Some(matches) => matches,
        None => {
            tracing::error!("Failed to get subcommand matches");
            anyhow::bail!("Failed to get subcommand matches");
        }
    };
    match sub_args.get_one::<String>("ticket") {
        Some(ticket) => {
            tracing::info!("Ticket: {}", ticket);
        }
        None => {
            tracing::error!("Failed to get ticket");
            anyhow::bail!("Failed to get ticket");
        }
    }
    Ok(())
}
