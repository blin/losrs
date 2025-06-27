use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use logseq_srs::act_on_card_ref;

pub mod output;
pub mod review;
pub mod terminal;

/// Work with Spaced Repetition Cards (SRS) embedded in Logseq pages
#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    #[command(subcommand)]
    command: Commands,
}

fn parse_hex(src: &str) -> Result<u64> {
    let s = src.trim_start_matches("0x");
    Ok(u64::from_str_radix(s, 16)?)
}

#[derive(Args)]
struct CardRefArgs {
    /// The path to the file to read
    path: PathBuf,

    /// Fingerprint of the card's prompt.
    /// Use metadata command to find one.
    #[arg(value_parser = parse_hex)]
    prompt_fingerprint: Option<u64>,
}

#[derive(Clone, ValueEnum)]
enum OutputFormatArg {
    Raw,
    Clean,
    Typst,
    Sixel,
}

#[derive(Subcommand)]
enum Commands {
    /// print cards
    Show {
        #[command(flatten)]
        card_ref: CardRefArgs,

        #[arg(
            long,
            default_value_t = OutputFormatArg::Raw,
            value_enum
        )]
        format: OutputFormatArg,
    },
    /// review cards
    Review {
        #[command(flatten)]
        card_ref: CardRefArgs,

        #[arg(
            long,
            default_value_t = OutputFormatArg::Raw,
            value_enum
        )]
        format: OutputFormatArg,
    },
    /// prints metadata for cards
    Metadata {
        #[command(flatten)]
        card_ref: CardRefArgs,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new().filter_level(cli.verbosity.into()).init();

    match cli.command {
        Commands::Show { card_ref: CardRefArgs { path, prompt_fingerprint }, format } => {
            act_on_card_ref(&path, prompt_fingerprint, |cm| {
                output::show_card(cm, (&format).into())
            })?;
        }
        Commands::Review { card_ref: CardRefArgs { path, prompt_fingerprint }, format } => {
            act_on_card_ref(&path, prompt_fingerprint, |cm| {
                review::review_card(cm, (&format).into())
            })?;
        }
        Commands::Metadata { card_ref: CardRefArgs { path, prompt_fingerprint } } => {
            act_on_card_ref(&path, prompt_fingerprint, output::show_metadata)?;
        }
    }

    Ok(())
}
