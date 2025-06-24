use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

use logseq_srs::{act_on_card_ref, extract_card_by_ref};

pub mod output;

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
enum OutputFormat {
    Raw,
    Clean,
    Typst,
    Sixel,
}

#[derive(Subcommand)]
enum Commands {
    /// prints cards in a file
    Show {
        #[command(flatten)]
        card_ref: CardRefArgs,

        #[arg(
            long,
            default_value_t = OutputFormat::Raw,
            value_enum
        )]
        format: OutputFormat,
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
        Commands::Show { card_ref: CardRefArgs { path, prompt_fingerprint }, format } => {
            act_on_card_ref(&path, prompt_fingerprint, |cm| {
                let card = extract_card_by_ref(&cm.card_ref)
                            .with_context(|| format!(
                                "When extract card with fingerprint {:016x} from {}, card with prompt prefix: {}",
                                cm.card_ref.prompt_fingerprint, cm.card_ref.source_path.display(), cm.prompt_prefix
                            ))?;
                match format {
                    OutputFormat::Raw => output::print_card_raw(&card)?,
                    OutputFormat::Clean => output::print_card_clean(&card)?,
                    OutputFormat::Typst => output::print_card_typst(&card)?,
                    OutputFormat::Sixel => output::print_card_sixel(&card)?,
                };
                Ok(())
            })?;
        }
        Commands::Metadata { card_ref: CardRefArgs { path, prompt_fingerprint } } => {
            act_on_card_ref(&path, prompt_fingerprint, |cm| {
                println!("{:?}", cm);
                Ok(())
            })?;
        }
    }

    Ok(())
}
