use anyhow::Result;
use logseq_srs::Card;

pub fn print_card_raw(card: &Card) -> Result<()> {
    println!("{}", card.body.prompt);
    println!("{}", card.body.response);
    Ok(())
}

pub fn print_card_clean(card: &Card) -> Result<()> {
    let clean_prompt = strip_prompt_metadata(&card.body.prompt);
    println!("{}", clean_prompt);
    println!("{}", card.body.response);
    Ok(())
}

fn strip_prompt_metadata(prompt: &str) -> String {
    prompt.split("\n").filter(|l| !is_metadata_line(l)).collect::<Vec<_>>().join("\n")
}

fn is_metadata_line(l: &str) -> bool {
    l.trim_start().starts_with("card-")
}
