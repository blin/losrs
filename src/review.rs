use anyhow::Result;

use crate::parse::CardMetadata;

use crate::output::{OutputFormat, show_card};
use crate::terminal::{clear_screen, wait_for_anykey, wait_for_review};

pub fn review_card(cm: &CardMetadata, format: OutputFormat) -> Result<()> {
    clear_screen()?;
    println!(
        "Reviewing {} from {}",
        cm.card_ref.prompt_fingerprint,
        cm.card_ref.source_path.display()
    );
    // TODO: make show_card returns bytes,
    // so that we can print everything together, without delay
    show_card(cm, format.clone())?;

    wait_for_anykey("show the answer")?;

    show_card(cm, format.clone())?;

    println!("review response: {:?}", wait_for_review()?);

    wait_for_anykey("continue")?;

    Ok(())
}
