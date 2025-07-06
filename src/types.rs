use std::fmt::Debug;
use std::path::Path;

use chrono::{DateTime, FixedOffset};

#[derive(PartialEq, Clone)]
pub struct Fingerprint(pub u64);

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

impl From<u64> for Fingerprint {
    fn from(value: u64) -> Self {
        Fingerprint(value)
    }
}

impl From<&str> for Fingerprint {
    fn from(value: &str) -> Self {
        xxhash_rust::xxh3::xxh3_64(value.as_bytes()).into()
    }
}

// Some considerations
// * I want to be able to hold all card metadata in memory, without holding all card data in memory
// * I want to be able to load one card at a time and immediately store it back modified
// * source_path is potentially used in lots of cards, avoid copying it
pub struct CardRef<'a> {
    pub source_path: &'a Path,
    // prompt_fingerprint is XXH3 64 and will remain valid within the version of the crate,
    // but not necessarily accross.
    // The intended use is to list a set of cards, then immediately act on them one by one.
    pub prompt_fingerprint: Fingerprint,
}

// Spaced Repetition System (SRS) Metadata
//
// Logseq standard format:
//   card-last-interval:: 39.06
//   card-repeats:: 4
//   card-ease-factor:: 1.0
//   card-next-schedule:: 2025-07-15T00:00:00.000Z
//   card-last-reviewed:: 2025-06-06T16:24:48.795Z
//   card-last-score:: 1
//
// We don't use most of these,
// but we preserve them anyway to enable simultaneous use with Logseq.
// The order of properties is from `operation-score!` function in Logseq.
#[derive(Debug, Clone)]
pub struct SRSMeta {
    pub last_interval: f64,
    pub repeats: u8,
    pub ease_factor: f64,
    pub next_schedule: DateTime<FixedOffset>,
    pub last_reviewed: DateTime<FixedOffset>,
    pub last_score: u8,
}

impl Default for SRSMeta {
    fn default() -> Self {
        Self {
            last_interval: -1.0,
            repeats: 0,
            ease_factor: 2.5,
            next_schedule: DateTime::UNIX_EPOCH.fixed_offset(),
            last_reviewed: DateTime::UNIX_EPOCH.fixed_offset(),
            last_score: 5,
        }
    }
}

pub struct CardMetadata<'a> {
    pub card_ref: CardRef<'a>,
    pub prompt_prefix: String,
    pub srs_meta: SRSMeta,
}

impl Debug for CardMetadata<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CardMetadata {{")?;
        writeln!(f, "  source_path        : {}", self.card_ref.source_path.display())?;
        writeln!(f, "  prompt_fingerprint : {}", self.card_ref.prompt_fingerprint)?;
        writeln!(f, "  prompt_prefix      : {}", self.prompt_prefix)?;
        writeln!(f, "  srs_meta           : SRSMeta {{")?;
        writeln!(f, "    repeats       : {}", self.srs_meta.repeats)?;
        writeln!(f, "    next_schedule : {:?}", self.srs_meta.next_schedule)?;
        writeln!(f, "    last_reviewed : {:?}", self.srs_meta.last_reviewed)?;
        writeln!(f, "  }}")?;
        write!(f, "}}")
    }
}

pub struct CardBody {
    // Both prompt and response are stored as read from file
    pub prompt: String,
    pub prompt_indent: usize,
    pub response: String,
}

pub struct Card<'a> {
    pub metadata: CardMetadata<'a>,
    pub body: CardBody,
}
