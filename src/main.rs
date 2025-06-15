use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};

use logseq_srs::cards_in_file;

/// Work with Spaced Repetition Cards (SRS) embedded in Logseq pages
#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// prints cards in a file
    CardsInFile {
        /// The path to the file to read
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbosity.into())
        .init();

    match cli.command {
        Commands::CardsInFile { path } => {
            if !path.exists() {
                return Err(anyhow!("{} does not exist", path.display()));
            }
            cards_in_file(&path).with_context(|| format!("when processing {}", path.display()))?;
        }
    }

    Ok(())
}
