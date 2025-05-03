pub mod cache;
pub mod clusterer;
pub mod diff;
pub mod files;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct CLI {
    #[command(subcommand)]
    mode: Mode
}

#[derive(Subcommand)]
enum Mode {
    /// Find and report duplicate and similar files in the current folder
    Diff(DiffArgs),
    /// Removes entries in the cache that do not reference a file of the current
    /// folder
    Clean,
    /// Review the reported results interactively
    Interactive
}

#[derive(Args)]
struct DiffArgs {
    /// Number of bits difference to have similar perceptual hash
    bits: Option<usize>,
    ///// Number of parallel executions
    //parallel: Option<usize>
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct CLIDefault {
    #[command(flatten)]
    diffargs: DiffArgs
}

fn main()
{
    // based on https://stackoverflow.com/a/79564853/5765334
    let cli = CLI::try_parse()
        // Result<CLI, Error>
        .or_else(|err| {
            match err.kind() {
                clap::error::ErrorKind::InvalidSubcommand
                | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                    CLIDefault::try_parse()
                        // Result<CLIDefault, Error>
                        .map_or_else(
                            |_| Err(err), // if default fails, return CLI error
                            |cli_default| Ok(
                                CLI { mode: Mode::Diff(cli_default.diffargs) }
                            )
                        )
                        // Result<CLI, Error>
                }
                _ => Err(err)
            }
        })
        // Result<CLI, Error>
        .unwrap_or_else(|err| {
            err.exit();
        }); // CLI
    match cli.mode {
        Mode::Diff(args) => diff::diff(args.bits/*, args.parallel*/),
        Mode::Clean => todo!(),
        Mode::Interactive => todo!()
    }
}
