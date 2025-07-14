use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use chrono::DateTime;
use chrono::FixedOffset;
use rs_fsrs::FSRS;
use rs_fsrs::Rating;

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
use crate::types::FSRSMeta;
use crate::types::SRSMeta;

fn compute_next_fsrs_meta(
    srs_meta: &SRSMeta,
    resp: &ReviewResponse,
    reviewed_at: DateTime<FixedOffset>,
) -> FSRSMeta {
    let fsrs_params = rs_fsrs::Parameters { enable_short_term: false, ..Default::default() };
    let fsrs = FSRS::new(fsrs_params);

    let rating = if *resp == ReviewResponse::Forgot { Rating::Again } else { Rating::Good };
    let next = fsrs.next(srs_meta.fsrs_meta.clone(), reviewed_at.into(), rating);
    next.card
}

fn compute_next_srs_meta(
    srs_meta: &SRSMeta,
    resp: &ReviewResponse,
    reviewed_at: DateTime<FixedOffset>,
) -> SRSMeta {
    let next_fsrs_meta = compute_next_fsrs_meta(srs_meta, resp, reviewed_at);
    let next_logseq_srs_meta = (&next_fsrs_meta).into();

    SRSMeta { logseq_srs_meta: next_logseq_srs_meta, fsrs_meta: next_fsrs_meta }
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
