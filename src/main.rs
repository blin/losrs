use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::FixedOffset;
use clap::Args;
use clap::Parser;
use clap::Subcommand;

use crate::output::show_card;
use crate::settings::Settings;
use crate::storage::CardSerialNumAllocator;
use crate::storage::extract_card_by_ref;
use crate::storage::extract_card_metadatas;
use crate::storage::find_graph_root;
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

fn act_on_card_ref<F>(path: &Path, card_id: Option<CardId>, mut f: F) -> Result<()>
where
    F: FnMut(&mut Vec<CardMetadata>) -> Result<()>,
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

        if let Some(card_id) = card_id.clone() {
            match card_id {
                CardId::Fingerprint(fingerprint) => {
                    card_metadatas.retain(|cm| cm.card_ref.prompt_fingerprint == fingerprint)
                }
                CardId::SerialNum(serial_num) => {
                    card_metadatas.retain(|cm| cm.serial_num == Some(serial_num))
                }
            }
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

struct NoOpSerialNumAllocator {}

impl CardSerialNumAllocator for NoOpSerialNumAllocator {
    fn allocate_and_get(&self) -> Option<Result<u64>> {
        // Not attempted
        None
    }
}

struct GraphRootSerialNumAllocator {
    graph_root: PathBuf,
}

impl GraphRootSerialNumAllocator {
    fn allocate_and_get_inner(&self) -> Result<u64> {
        let card_serial_num_path = self.graph_root.join(".card-serial");

        if !card_serial_num_path.exists() {
            let mut card_serial_num_file = File::create(card_serial_num_path)?;
            card_serial_num_file.write_all(b"0")?;
            return Ok(0);
        }

        let mut card_serial_num_file =
            OpenOptions::new().read(true).write(true).open(&card_serial_num_path)?;

        let mut card_serial_num_raw = String::new();
        card_serial_num_file.read_to_string(&mut card_serial_num_raw)?;

        let card_serial: u64 = card_serial_num_raw.parse()?;

        card_serial_num_file.seek(SeekFrom::Start(0))?;
        card_serial_num_file.set_len(0)?;
        card_serial_num_file.write_all((card_serial + 1).to_string().as_bytes())?;

        Ok(card_serial)
    }
}

impl CardSerialNumAllocator for GraphRootSerialNumAllocator {
    fn allocate_and_get(&self) -> Option<Result<u64>> {
        Some(
            self.allocate_and_get_inner()
                .with_context(|| anyhow!("failed to read/write card serial number")),
        )
    }
}

fn choose_serial_num_allocator(path: &Path) -> Result<Box<dyn CardSerialNumAllocator>> {
    let Some(graph_root) = find_graph_root(path)? else {
        return Ok(Box::new(NoOpSerialNumAllocator {}));
    };
    Ok(Box::new(GraphRootSerialNumAllocator { graph_root }))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let settings = Settings::new(cli.config)?;

    match cli.command {
        Commands::Show { card_ref: CardRefArgs { path, card_id } } => {
            let output_settings = settings.output;
            act_on_card_ref(&path, card_id, |card_metas| {
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

            match act_on_card_ref(&path, card_id, |card_metas| {
                card_metas.retain(|cm| cm.srs_meta.logseq_srs_meta.next_schedule <= up_to);
                shuffle_slice(card_metas, seed.unwrap_or_default());
                for cm in card_metas {
                    review::review_card(cm, at, &output_settings, serial_num_allocator.as_mut())?
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
        Commands::Metadata { card_ref: CardRefArgs { path, card_id } } => {
            act_on_card_ref(&path, card_id, |card_metas| {
                for cm in card_metas {
                    output::show_metadata(cm)?;
                }
                Ok(())
            })?;
        }
        Commands::FixMetadata { card_ref: CardRefArgs { path, card_id } } => {
            let mut serial_num_allocator = choose_serial_num_allocator(&path)?;
            act_on_card_ref(&path, card_id, |card_metas| {
                for cm in card_metas {
                    // TODO: detect cards that are in the same file with the same fingerprint and nope out
                    storage::rewrite_card_meta(
                        &cm.card_ref,
                        &cm.srs_meta,
                        serial_num_allocator.as_mut(),
                    )?;
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
