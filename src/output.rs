use std::io::Write;
use std::process;

use anyhow::{Context, Result, anyhow};
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

pub fn print_card_typst(card: &Card) -> Result<()> {
    let clean_prompt = strip_prompt_metadata(&card.body.prompt);
    let markdown = format!("{}\n{}", clean_prompt, card.body.response);
    let typst: String = markdown_to_typst(markdown)
        .with_context(|| "failed to convert markdown to typst using pandoc".to_owned())?;
    print!("{}", typst);
    Ok(())
}

fn markdown_to_typst(markdown: String) -> Result<String> {
    // TODO: check pandoc is sufficiently advanced

    let mut child = process::Command::new("pandoc")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .args(vec!["--from", "markdown", "--to", "typst"])
        .spawn()?;
    {
        let mut stdin =
            child.stdin.take().expect("Stdin via pipe requested but ChildStdin is not present");
        stdin.write_all(markdown.as_bytes()).with_context(|| "could not pass markdown on stdin")?;
    }

    let output = child.wait_with_output()?;

    let status = output.status;
    let stdout = String::from_utf8(output.stdout).with_context(|| "processing stdout failed")?;
    let stderr = String::from_utf8(output.stderr).with_context(|| "processing stderr failed")?;

    if status.success() && !stderr.is_empty() {
        return Err(anyhow!("the output is invalid, got warnings:\n{}", stderr));
    } else if !status.success() {
        return Err(anyhow!("{}", stderr));
    }

    Ok(stdout)
}
