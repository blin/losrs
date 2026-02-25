use std::time::Duration;

use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::FixedOffset;
use chrono::Utc;
use rs_fsrs::FSRS;
use rs_fsrs::Rating;

use crate::output::show_card;
use crate::output::show_card_prompt;
use crate::settings::OutputSettings;
use crate::storage::StorageManager;
use crate::terminal::PreReviewResponse;
use crate::terminal::ReviewResponse;
use crate::terminal::clear_screen;
use crate::terminal::wait_for_prereview;
use crate::terminal::wait_for_review;
use crate::types::Card;
use crate::types::CardMetadata;
use crate::types::FSRSMeta;
use crate::types::SRSMeta;

impl From<&ReviewResponse> for Rating {
    fn from(value: &ReviewResponse) -> Self {
        match *value {
            ReviewResponse::LittleEffort => Rating::Easy,
            ReviewResponse::SomeEffort => Rating::Good,
            ReviewResponse::MuchEffort => Rating::Hard,
            ReviewResponse::NoRecall => Rating::Again,
        }
    }
}

struct ReviewableFSRSMeta<'a> {
    inner: &'a FSRSMeta,
    reviewed_at: DateTime<FixedOffset>,
}

impl<'a> ReviewableFSRSMeta<'a> {
    fn new(fsrs_meta: &'a FSRSMeta, reviewed_at: DateTime<FixedOffset>) -> Result<Self> {
        if reviewed_at < fsrs_meta.last_review.fixed_offset() {
            return Err(anyhow!(
                "reviewing a card that was last reviewed on {:?} at {:?}, before it was last reviewed!",
                fsrs_meta.last_review,
                reviewed_at
            ));
        };
        Ok(Self { inner: fsrs_meta, reviewed_at })
    }
}

fn truncate_to_millis(dt: &DateTime<Utc>) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(dt.timestamp_millis()).unwrap()
}

fn clean_up_fsrs_meta(value: &FSRSMeta) -> FSRSMeta {
    FSRSMeta {
        due: truncate_to_millis(&value.due),
        stability: (value.stability * 1000.0).round() / 1000.0,
        difficulty: (value.difficulty * 1000.0).round() / 1000.0,
        elapsed_days: value.elapsed_days,
        scheduled_days: value.scheduled_days,
        reps: value.reps,
        lapses: value.lapses,
        state: value.state,
        last_review: truncate_to_millis(&value.last_review),
    }
}

fn compute_next_fsrs_meta(fsrs_meta: &ReviewableFSRSMeta, resp: &ReviewResponse) -> FSRSMeta {
    let reviewed_at = fsrs_meta.reviewed_at;
    let fsrs_params = rs_fsrs::Parameters { enable_short_term: false, ..Default::default() };
    let fsrs = FSRS::new(fsrs_params);

    let next = fsrs.next(fsrs_meta.inner.clone(), reviewed_at.into(), resp.into());
    clean_up_fsrs_meta(&next.card)
}

fn compute_next_srs_meta(fsrs_meta: &ReviewableFSRSMeta, resp: &ReviewResponse) -> SRSMeta {
    let next_fsrs_meta = compute_next_fsrs_meta(fsrs_meta, resp);
    let next_logseq_srs_meta = (&next_fsrs_meta).into();

    SRSMeta { logseq_srs_meta: next_logseq_srs_meta, fsrs_meta: next_fsrs_meta }
}

fn compute_delayed_srs_meta(fsrs_meta: &ReviewableFSRSMeta, delay: Duration) -> SRSMeta {
    let mut delayed_fsrs_meta = fsrs_meta.inner.clone();
    // Delay is relative to review time, not card due time,
    // as we might be reviewing a card long past original due date.
    delayed_fsrs_meta.due = truncate_to_millis(&(fsrs_meta.reviewed_at + delay).into());
    let delayed_logseq_srs_meta = (&delayed_fsrs_meta).into();

    SRSMeta { logseq_srs_meta: delayed_logseq_srs_meta, fsrs_meta: delayed_fsrs_meta }
}

fn format_reviewing_phrase(cm: &CardMetadata) -> String {
    match cm.card_ref.serial_num {
        Some(serial_num) => format!(
            "Reviewing card with serial number {} from {}",
            serial_num,
            cm.card_ref.source_path.display()
        ),
        None => format!(
            "Reviewing card with prompt fingerprint {} from {}",
            cm.card_ref.prompt_fingerprint,
            cm.card_ref.source_path.display()
        ),
    }
}

// TODO: supply only card_ref and fsrs_meta
pub fn review_card(
    cm: &CardMetadata,
    reviewed_at: DateTime<FixedOffset>,
    output_settings: &OutputSettings,
    storage_manager: &mut StorageManager,
) -> Result<()> {
    // We construct ReviewableFSRSMeta early so as to not require user action
    // if card is unreviewable.
    let reviewable_fsrs_meta = ReviewableFSRSMeta::new(&cm.srs_meta.fsrs_meta, reviewed_at)?;

    let card_body = storage_manager.load_card_body_by_ref(&cm.card_ref).with_context(|| {
        format!(
            "When extracting card with fingerprint {} from {}",
            cm.card_ref.prompt_fingerprint,
            cm.card_ref.source_path.display(),
        )
    })?;
    let card = Card { metadata: cm.clone(), body: card_body };

    clear_screen()?;
    let review_phrase = format_reviewing_phrase(cm);
    println!("{}", review_phrase);

    // TODO: make show_card returns bytes,
    // so that we can print everything together, without delay.
    // 1. Show progressbar
    // 2. Format card into buffer
    // 3. Complete progressbar
    // 4. Show the whole thing
    show_card_prompt(&card, output_settings)?;

    let prereview_response = wait_for_prereview()?;

    let new_srs_meta = match prereview_response {
        PreReviewResponse::ShowResponse => {
            clear_screen()?;
            println!("{}", review_phrase);

            show_card(&card, output_settings)?;

            let review_response = wait_for_review()?;
            compute_next_srs_meta(&reviewable_fsrs_meta, &review_response)
        }
        PreReviewResponse::DelayReview => {
            compute_delayed_srs_meta(&reviewable_fsrs_meta, Duration::from_hours(24))
        }
    };

    storage_manager.rewrite_card_meta(&card.metadata.card_ref, &new_srs_meta)?;

    Ok(())
}
