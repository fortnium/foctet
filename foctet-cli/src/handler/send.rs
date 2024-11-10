use anyhow::Result;
use clap::ArgMatches;

pub fn handle(args: &ArgMatches) -> Result<()> {
    let sub_args = match args.subcommand_matches("send") {
        Some(matches) => matches,
        None => {
            tracing::error!("Failed to get subcommand matches");
            anyhow::bail!("Failed to get subcommand matches");
        }
    };
    if sub_args.contains_id("file") {
    } else if sub_args.contains_id("dir") {
    } else if sub_args.contains_id("text") {
    } else {
        tracing::error!("No file, directory, or text specified");
        anyhow::bail!("No file, directory, or text specified");
    }
    Ok(())
}
