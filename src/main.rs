use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::output::{OutputFormat, show_card};
use crate::parse::{CardMetadata, Fingerprint, extract_card_by_ref, extract_card_metadatas};

pub mod output;
pub mod parse;
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

fn parse_hex(src: &str) -> Result<Fingerprint> {
    let s = src.trim_start_matches("0x");
    Ok(u64::from_str_radix(s, 16)?.into())
}

#[derive(Args)]
struct CardRefArgs {
    /// The path to the file to read
    path: PathBuf,

    /// Fingerprint of the card's prompt.
    /// Use metadata command to find one.
    #[arg(value_parser = parse_hex)]
    prompt_fingerprint: Option<Fingerprint>,
}

#[derive(Clone, ValueEnum)]
enum OutputFormatArg {
    Raw,
    Clean,
    Typst,
    Sixel,
}

impl From<&OutputFormatArg> for OutputFormat {
    fn from(value: &OutputFormatArg) -> Self {
        match value {
            OutputFormatArg::Raw => OutputFormat::Raw,
            OutputFormatArg::Clean => OutputFormat::Clean,
            OutputFormatArg::Typst => OutputFormat::Typst,
            OutputFormatArg::Sixel => OutputFormat::Sixel,
        }
    }
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

pub fn act_on_card_ref<F>(path: &Path, prompt_fingerprint: Option<Fingerprint>, f: F) -> Result<()>
where
    F: Fn(&CardMetadata) -> Result<()>,
{
    if !path.exists() {
        return Err(anyhow!("{} does not exist", path.display()));
    }
    let mut card_metadatas = extract_card_metadatas(path)
        .with_context(|| format!("when processing {}", path.display()))?;

    if let Some(prompt_fingerprint) = prompt_fingerprint {
        card_metadatas.retain(|cm| cm.card_ref.prompt_fingerprint == prompt_fingerprint);
    }
    for cm in card_metadatas {
        f(&cm)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new().filter_level(cli.verbosity.into()).init();

    match cli.command {
        Commands::Show { card_ref: CardRefArgs { path, prompt_fingerprint }, format } => {
            act_on_card_ref(&path, prompt_fingerprint, |cm| {
                let format = (&format).into();
                let card = extract_card_by_ref(&cm.card_ref).with_context(|| {
                    format!(
                        "When extracting card with fingerprint {} from {}, card with prompt prefix: {}",
                        cm.card_ref.prompt_fingerprint,
                        cm.card_ref.source_path.display(),
                        cm.prompt_prefix
                    )
                })?;
                show_card(&card, &format)
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
