use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use chrono::FixedOffset;
use clap::Args;
use clap::Parser;
use clap::Subcommand;

use crate::output::show_card;
use crate::settings::Settings;
use crate::storage::extract_card_by_ref;
use crate::storage::extract_card_metadatas;
use crate::storage::find_page_files;
use crate::types::CardMetadata;
use crate::types::Fingerprint;

pub mod output;
pub mod review;
pub mod settings;
pub mod storage;
pub mod terminal;
pub mod types;

/// Work with Spaced Repetition System (SRS) cards embedded in Logseq pages
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Override path to the config file.
    /// Use `config path` command to find the default path.
    #[arg(long)]
    config: Option<PathBuf>,
}

fn parse_hex(src: &str) -> Result<Fingerprint> {
    let s = src.trim_start_matches("0x");
    Ok(u64::from_str_radix(s, 16)?.into())
}

fn parse_datetime(src: &str) -> Result<DateTime<FixedOffset>> {
    Ok(DateTime::parse_from_rfc3339(src)?)
}

#[derive(Args)]
struct CardRefArgs {
    /// The path to the page file or graph root directory
    path: PathBuf,

    /// Fingerprint of the card's prompt.
    /// Use `metadata` command to find one.
    #[arg(value_parser = parse_hex)]
    prompt_fingerprint: Option<Fingerprint>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print cards
    Show {
        #[command(flatten)]
        card_ref: CardRefArgs,
    },
    /// Review cards
    Review {
        #[command(flatten)]
        card_ref: CardRefArgs,

        /// RFC3999 timestamp to use as the time of the review.
        /// Affects both selection and updating.
        #[arg(long, value_parser = parse_datetime, value_name = "TIMESTAMP")]
        at: Option<DateTime<FixedOffset>>,

        /// Seed used for shuffling cards ready to be reviewed
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Print metadata for cards
    Metadata {
        #[command(flatten)]
        card_ref: CardRefArgs,
    },
    /// Fix metadata for cards
    FixMetadata {
        #[command(flatten)]
        card_ref: CardRefArgs,
    },
    /// Manage configuration
    #[command(after_help = include_str!("../docs/configuration.md"))]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show the merged configuration
    Show,
    /// Show the path to the default configuration file
    Path,
}

fn act_on_card_ref<F>(path: &Path, prompt_fingerprint: Option<Fingerprint>, f: F) -> Result<()>
where
    F: Fn(&mut Vec<CardMetadata>) -> Result<()>,
{
    // The complexity in act_on_card_ref is introduced so that PathBufs from page_files
    // outlive CardMetadatas from all_card_metadatas, which refer to these PathBufs.
    // There is probably a better way to do this.
    let page_files: Vec<PathBuf> = find_page_files(path)?;
    let mut all_card_metadatas: Vec<CardMetadata> = Vec::new();
    for page_file in page_files.iter() {
        let mut card_metadatas = extract_card_metadatas(page_file).with_context(|| {
            format!("when extracting card metadatas from {}", page_file.display())
        })?;

        if let Some(prompt_fingerprint) = prompt_fingerprint.clone() {
            card_metadatas.retain(|cm| cm.card_ref.prompt_fingerprint == prompt_fingerprint);
        }
        all_card_metadatas.extend(card_metadatas);
    }
    f(&mut all_card_metadatas)?;
    Ok(())
}

fn shuffle_slice<T>(s: &mut [T], seed: u64) {
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use rand::seq::SliceRandom;
    let mut rng = SmallRng::seed_from_u64(seed);
    s.shuffle(&mut rng);
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let settings = Settings::new(cli.config)?;

    match cli.command {
        Commands::Show { card_ref: CardRefArgs { path, prompt_fingerprint } } => {
            let output_settings = settings.output;
            act_on_card_ref(&path, prompt_fingerprint, |card_metas| {
                card_metas.sort_by(|a, b| a.card_ref.source_path.cmp(b.card_ref.source_path));
                for cm in card_metas {
                    let card = extract_card_by_ref(&cm.card_ref).with_context(|| {
                        format!(
                            "When extracting card with fingerprint {} from {}, card with prompt prefix: {}",
                            cm.card_ref.prompt_fingerprint,
                            cm.card_ref.source_path.display(),
                            cm.prompt_prefix
                        )
                    })?;
                    show_card(&card, &output_settings)?
                }
                Ok(())
            })?;
        }
        Commands::Review { card_ref: CardRefArgs { path, prompt_fingerprint }, at, seed } => {
            let output_settings = settings.output;
            let at = match at {
                Some(at) => at,
                None => chrono::offset::Utc::now().fixed_offset(),
            };

            match act_on_card_ref(&path, prompt_fingerprint, |card_metas| {
                card_metas.retain(|cm| cm.srs_meta.logseq_srs_meta.next_schedule <= at);
                shuffle_slice(card_metas, seed.unwrap_or_default());
                for cm in card_metas {
                    review::review_card(cm, at, &output_settings)?
                }
                Ok(())
            }) {
                Ok(_) => println!("Reviewed all cards, huzzah!"),
                Err(err) => match err.downcast_ref::<terminal::NopeOutError>() {
                    Some(e) => println!("{}", e),
                    None => Err(err)?,
                },
            }
        }
        Commands::Metadata { card_ref: CardRefArgs { path, prompt_fingerprint } } => {
            act_on_card_ref(&path, prompt_fingerprint, |card_metas| {
                for cm in card_metas {
                    output::show_metadata(cm)?;
                }
                Ok(())
            })?;
        }
        Commands::FixMetadata { card_ref: CardRefArgs { path, prompt_fingerprint } } => {
            act_on_card_ref(&path, prompt_fingerprint, |card_metas| {
                for cm in card_metas {
                    // TODO: detect cards that are in the same file with the same fingerprint and nope out
                    storage::rewrite_card_srs_meta(&cm.card_ref, &cm.srs_meta)?;
                }
                Ok(())
            })?;
        }
        Commands::Config { command } => match command {
            ConfigCommands::Show => {
                println!("{}", serde_json::to_string_pretty(&settings)?)
            }
            ConfigCommands::Path => {
                println!("{}", Settings::get_config_path()?.display());
            }
        },
    }

    Ok(())
}
