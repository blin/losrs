use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};

use logseq_srs::{extract_card_by_ref, extract_card_metadatas};

/// Work with Spaced Repetition Cards (SRS) embedded in Logseq pages
#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct CardRefArgs {
    /// The path to the file to read
    path: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// prints cards in a file
    Show {
        #[command(flatten)]
        card_ref: CardRefArgs,
    },
    /// prints metadata for cards in a file
    Metadata {
        #[command(flatten)]
        card_ref: CardRefArgs,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new().filter_level(cli.verbosity.into()).init();

    match cli.command {
        Commands::Show { card_ref: CardRefArgs { path } } => {
            if !path.exists() {
                return Err(anyhow!("{} does not exist", path.display()));
            }
            let card_metadatas = extract_card_metadatas(&path)
                .with_context(|| format!("when processing {}", path.display()))?;

            for cm in card_metadatas {
                let card = extract_card_by_ref(&cm.card_ref)
                            .with_context(|| format!(
                                "When extract card with fingerprint {:016x} from {}, card with prompt prefix: {}",
                                cm.card_ref.prompt_fingerprint, cm.card_ref.source_path.display(), cm.prompt_prefix
                            ))?;
                println!("{}", card.body.prompt);
                println!("{}", card.body.response);
            }
        }
        Commands::Metadata { card_ref: CardRefArgs { path } } => {
            if !path.exists() {
                return Err(anyhow!("{} does not exist", path.display()));
            }
            let card_metadatas = extract_card_metadatas(&path)
                .with_context(|| format!("when processing {}", path.display()))?;

            for card_metadata in card_metadatas {
                println!("{:?}", card_metadata);
            }
        }
    }

    Ok(())
}
