use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use chrono::FixedOffset;
use clap::Args;
use clap::Parser;
use clap::Subcommand;

use crate::output::show_card;
use crate::settings::Settings;
use crate::storage::choose_serial_num_allocator;
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

fn parse_fingerprint_or_id(src: &str) -> Result<CardId> {
    if src.starts_with("0x") {
        Ok(CardId::Fingerprint(parse_hex(src)?))
    } else {
        Ok(CardId::SerialNum(src.parse::<u64>()?))
    }
}

fn parse_datetime(src: &str) -> Result<DateTime<FixedOffset>> {
    Ok(DateTime::parse_from_rfc3339(src)?)
}

#[derive(Clone)]
enum CardId {
    Fingerprint(Fingerprint),
    SerialNum(u64),
}

#[derive(Args)]
struct CardRefArgs {
    /// The path to the page file or graph root directory
    path: PathBuf,

    /// Card's serial number or fingerprint of the card's prompt.
    /// Use `metadata` command to find either.
    #[arg(value_parser = parse_fingerprint_or_id)]
    card_id: Option<CardId>,
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
        /// Affects updating.
        #[arg(long, value_parser = parse_datetime, value_name = "TIMESTAMP")]
        at: Option<DateTime<FixedOffset>>,

        /// RFC3999 timestamp to use as an upper bound on due time.
        /// Affects selection.
        #[arg(long, value_parser = parse_datetime, value_name = "TIMESTAMP")]
        up_to: Option<DateTime<FixedOffset>>,

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

fn select_card_metadata(path: &Path, card_id: Option<CardId>) -> Result<Vec<CardMetadata>> {
    let page_files: Vec<PathBuf> = find_page_files(path)?;
    let mut all_card_metadatas: Vec<CardMetadata> = Vec::new();
    for page_file in page_files.into_iter() {
        // avoid copying page_file just so we can print it later
        let context = format!("when extracting card metadatas from {}", &page_file.display());
        let mut card_metadatas =
            extract_card_metadatas(Rc::new(page_file)).with_context(|| context)?;

        if let Some(card_id) = card_id.clone() {
            let p: Box<dyn Fn(&CardMetadata) -> bool> = match &card_id {
                CardId::Fingerprint(fingerprint) => {
                    Box::new(|cm: &CardMetadata| cm.card_ref.prompt_fingerprint == *fingerprint)
                }
                CardId::SerialNum(serial_num) => {
                    Box::new(|cm: &CardMetadata| cm.card_ref.serial_num == Some(*serial_num))
                }
            };
            card_metadatas.retain(p);
        }
        all_card_metadatas.extend(card_metadatas);
    }
    Ok(all_card_metadatas)
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
        Commands::Show { card_ref: CardRefArgs { path, card_id } } => {
            let output_settings = settings.output;
            let mut card_metas = select_card_metadata(&path, card_id)?;
            card_metas.sort_by(|a, b| a.card_ref.source_path.cmp(&b.card_ref.source_path));
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
        }
        Commands::Review { card_ref: CardRefArgs { path, card_id }, at, up_to, seed } => {
            let mut serial_num_allocator = choose_serial_num_allocator(&path)?;
            let output_settings = settings.output;
            let now = chrono::offset::Utc::now().fixed_offset();
            let (at, up_to) = match (at, up_to) {
                (None, None) => (now, now),
                (None, Some(up_to)) => (now, up_to),
                (Some(at), None) => (at, at),
                (Some(at), Some(up_to)) => (at, up_to),
            };

            let mut card_metas = select_card_metadata(&path, card_id)?;
            match (|| -> Result<()> {
                card_metas.retain(|cm| cm.srs_meta.logseq_srs_meta.next_schedule <= up_to);
                shuffle_slice(&mut card_metas, seed.unwrap_or_default());
                for cm in card_metas {
                    review::review_card(&cm, at, &output_settings, serial_num_allocator.as_mut())?
                }
                Ok(())
            })() {
                Ok(_) => println!("Reviewed all cards, huzzah!"),
                Err(err) => match err.downcast_ref::<terminal::NopeOutError>() {
                    Some(e) => println!("{}", e),
                    None => Err(err)?,
                },
            }
        }
        Commands::Metadata { card_ref: CardRefArgs { path, card_id } } => {
            let card_metas = select_card_metadata(&path, card_id)?;
            for cm in card_metas {
                output::show_metadata(&cm)?;
            }
        }
        Commands::FixMetadata { card_ref: CardRefArgs { path, card_id } } => {
            let mut serial_num_allocator = choose_serial_num_allocator(&path)?;
            let card_metas = select_card_metadata(&path, card_id)?;
            for cm in card_metas {
                // TODO: detect cards that are in the same file with the same fingerprint and nope out
                storage::rewrite_card_meta(
                    &cm.card_ref,
                    &cm.srs_meta,
                    serial_num_allocator.as_mut(),
                )?;
            }
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
