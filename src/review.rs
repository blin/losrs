use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use chrono::DateTime;
use chrono::Duration;
use chrono::FixedOffset;

use crate::output::OutputFormat;
use crate::output::show_card;
use crate::output::show_card_prompt;
use crate::parse::extract_card_by_ref;
use crate::parse::rewrite_card_srs_meta;
use crate::terminal::ReviewResponse;
use crate::terminal::clear_screen;
use crate::terminal::wait_for_anykey;
use crate::terminal::wait_for_review;
use crate::types::CardMetadata;
use crate::types::SRSMeta;

fn compute_next_srs_meta(
    srs_meta: &SRSMeta,
    resp: &ReviewResponse,
    reviewed_at: DateTime<FixedOffset>,
) -> SRSMeta {
    let repeats = match resp {
        ReviewResponse::Remembered => srs_meta.repeats + 1,
        ReviewResponse::Forgot => 0,
    };
    let offset_days = (2.5_f64).powi(repeats.into());
    let day_seconds = Duration::days(1).as_seconds_f64();
    let offset = Duration::seconds((day_seconds * offset_days) as i64);
    let next_schedule = reviewed_at + offset;

    SRSMeta {
        repeats,
        next_schedule,
        last_reviewed: reviewed_at,
        last_interval: offset_days,
        ease_factor: 2.5,
        last_score: 5,
    }
}

pub fn review_card(
    cm: &CardMetadata,
    format: OutputFormat,
    reviewed_at: DateTime<FixedOffset>,
) -> Result<()> {
    let card = extract_card_by_ref(&cm.card_ref).with_context(|| {
        format!(
            "When extracting card with fingerprint {} from {}, card with prompt prefix: {}",
            cm.card_ref.prompt_fingerprint,
            cm.card_ref.source_path.display(),
            cm.prompt_prefix
        )
    })?;

    clear_screen()?;
    println!(
        "Reviewing {} from {}",
        cm.card_ref.prompt_fingerprint,
        cm.card_ref.source_path.display()
    );

    // TODO: make show_card returns bytes,
    // so that we can print everything together, without delay.
    // 1. Show progressbar
    // 2. Format card into buffer
    // 3. Complete progressbar
    // 4. Show the whole thing
    show_card_prompt(&card, &format)?;

    wait_for_anykey("show the answer")?;

    clear_screen()?;
    println!(
        "Reviewing {} from {}",
        cm.card_ref.prompt_fingerprint,
        cm.card_ref.source_path.display()
    );

    show_card(&card, &format)?;

    let review_response = wait_for_review()?;
    let next_srs_meta =
        compute_next_srs_meta(&card.metadata.srs_meta, &review_response, reviewed_at);

    rewrite_card_srs_meta(&card.metadata.card_ref, next_srs_meta)?;

    Ok(())
}
