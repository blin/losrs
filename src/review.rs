use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;

use crate::output::OutputFormat;
use crate::output::show_card;
use crate::parse::extract_card_by_ref;
use crate::terminal::clear_screen;
use crate::terminal::wait_for_anykey;
use crate::terminal::wait_for_review;
use crate::types::CardMetadata;

pub fn review_card(cm: &CardMetadata, format: OutputFormat) -> Result<()> {
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
    // so that we can print everything together, without delay
    show_card(&card, &format)?;

    wait_for_anykey("show the answer")?;

    show_card(&card, &format)?;

    println!("review response: {:?}", wait_for_review()?);

    wait_for_anykey("continue")?;

    Ok(())
}
