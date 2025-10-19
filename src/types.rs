use std::fmt::Debug;
use std::path::Path;

use chrono::DateTime;
use chrono::FixedOffset;

use rs_fsrs;

pub type FSRSMeta = rs_fsrs::Card;

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
// * If a card has just been added it will not have a serial number assigned, so we need to use something else when writing back
// * source_path is potentially used in lots of cards, avoid copying it
pub struct CardRef<'a> {
    pub source_path: &'a Path,
    // prompt_fingerprint is XXH3 64 and will remain valid within the version of the crate,
    // but not necessarily accross.
    // The intended use is to list a set of cards, then immediately act on them one by one.
    pub prompt_fingerprint: Fingerprint,
}

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
pub struct LogseqSRSMeta {
    pub last_interval: f64,
    // TODO: change repeats type to i32 to match fsrs meta
    pub repeats: u8,
    pub ease_factor: f64,
    // TODO: change next_schedule and last_reviewied types to DateTime<Utc> to match fsrs meta
    pub next_schedule: DateTime<FixedOffset>,
    pub last_reviewed: DateTime<FixedOffset>,
    pub last_score: u8,
}

impl Default for LogseqSRSMeta {
    fn default() -> Self {
        // [tag:card-last-interval-default]
        // Logseq defaults are defined in default-card-properties-map
        // card-last-interval-property by deault is -1
        // but we use 0 so that
        // LogseqSRSMeta::default() and LogseqSRSMeta::from(&FSRSMeta::default())
        // are roughly compatible.
        // TODO: actually test this
        Self {
            last_interval: 0.0,
            repeats: 0,
            ease_factor: 2.5,
            next_schedule: DateTime::UNIX_EPOCH.fixed_offset(),
            last_reviewed: DateTime::UNIX_EPOCH.fixed_offset(),
            last_score: 5,
        }
    }
}

impl From<&LogseqSRSMeta> for FSRSMeta {
    fn from(logseq_srs_meta: &LogseqSRSMeta) -> Self {
        // We use [ref:card-last-interval-default]
        // to detect new cards.
        if logseq_srs_meta.last_interval <= 0.0f64 {
            FSRSMeta::default()
        } else {
            FSRSMeta {
                due: logseq_srs_meta.next_schedule.into(),
                stability: logseq_srs_meta.last_interval,
                difficulty: 5.0,
                elapsed_days: logseq_srs_meta.last_interval as i64,
                scheduled_days: logseq_srs_meta.last_interval as i64,
                reps: logseq_srs_meta.repeats as i32,
                lapses: 0,
                state: rs_fsrs::State::Review,
                last_review: logseq_srs_meta.last_reviewed.into(),
            }
        }
    }
}

// TODO: running fix-metadata the second time produces a different result, fix.
impl From<&FSRSMeta> for LogseqSRSMeta {
    fn from(fsrs_meta: &FSRSMeta) -> Self {
        LogseqSRSMeta {
            last_interval: fsrs_meta.scheduled_days as f64,
            repeats: fsrs_meta.reps as u8,
            ease_factor: 2.5,
            next_schedule: fsrs_meta.due.into(),
            last_reviewed: fsrs_meta.last_review.into(),
            last_score: 5,
        }
    }
}

// Spaced Repetition System (SRS) Metadata
#[derive(Debug, Clone)]
pub struct SRSMeta {
    pub logseq_srs_meta: LogseqSRSMeta,
    // fsrs_meta is optional on read, but we will always write it out
    pub fsrs_meta: FSRSMeta,
}

pub struct CardMetadata<'a> {
    // serial_num is potentilaly unset at read time,
    // we populate only before writing to avoid wasting serial numbers.
    pub serial_num: Option<u64>,
    pub card_ref: CardRef<'a>,
    pub prompt_prefix: String,
    pub srs_meta: SRSMeta,
}

impl Debug for CardMetadata<'_> {
    // Skip formatting to preserve visual alignment
    #[rustfmt::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CardMetadata {{")?;
        writeln!(f, "  serial_num         : {}", (self.serial_num.map(|serial_num| serial_num.to_string()).unwrap_or("N/A".to_string())))?;
        writeln!(f, "  source_path        : {}", self.card_ref.source_path.display())?;
        writeln!(f, "  prompt_fingerprint : {}", self.card_ref.prompt_fingerprint)?;
        writeln!(f, "  prompt_prefix      : {}", self.prompt_prefix)?;
        writeln!(f, "  srs_meta           : SRSMeta {{")?;
        writeln!(f, "    repeats       : {}", self.srs_meta.logseq_srs_meta.repeats)?;
        writeln!(f, "    next_schedule : {:?}", self.srs_meta.logseq_srs_meta.next_schedule)?;
        writeln!(f, "    last_reviewed : {:?}", self.srs_meta.logseq_srs_meta.last_reviewed)?;
        writeln!(f, "    fsrs_meta     : {:?}", self.srs_meta.fsrs_meta)?;
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
