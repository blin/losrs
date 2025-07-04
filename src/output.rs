use std::fmt::Display;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

use anyhow::{Context, Result, anyhow};
use tempfile::NamedTempFile;

use crate::parse::{Card, CardMetadata};

#[derive(Clone)]
pub enum OutputFormat {
    Clean,
    Typst,
    Sixel,
}

pub fn show_card(card: &Card, format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Clean => print_card_clean(card)?,
        OutputFormat::Typst => print_card_typst(card)?,
        OutputFormat::Sixel => print_card_sixel(card)?,
    };
    Ok(())
}

pub fn show_metadata(cm: &CardMetadata) -> Result<()> {
    println!("{:?}", cm);
    Ok(())
}

pub fn print_card_clean(card: &Card) -> Result<()> {
    println!("{}", card.body.prompt);
    println!("{}", card.body.response);
    Ok(())
}

pub fn print_card_typst(card: &Card) -> Result<()> {
    let markdown = format!("{}\n{}", card.body.prompt, card.body.response);
    let typst = markdown_to_typst(markdown)
        .with_context(|| "failed to convert markdown to typst using pandoc".to_owned())?;
    print!("{}", typst);
    Ok(())
}

pub fn print_card_sixel(card: &Card) -> Result<()> {
    let markdown = format!("{}\n{}", card.body.prompt, card.body.response);

    let typst = markdown_to_typst(markdown)
        .with_context(|| "failed to convert markdown to typst using pandoc".to_owned())?;

    // standard logseq layout is
    //
    // graph_root
    // ├── assets
    // │   ├── image_1666695381725_0.png
    // │   ├── ...
    // ├── journals
    // │   ├── 2022_02_13.md
    // │   ├── ...
    // ├── logseq
    // │   ├── config.edn
    // │   ├── custom.css
    // │   ├── metadata.edn
    // │   └── srs-of-matrix.edn
    // └── pages
    //     ├── Sphere.md
    //     ├── ...
    //
    // So to get graph_root we need to go up twice
    let page_path = card.metadata.card_ref.source_path;
    let graph_root = page_path.parent().and_then(Path::parent).ok_or(anyhow!(
        "page file does not have a grandparent. The page is {}",
        page_path.display()
    ))?;

    let png_buf = typst_to_png(typst, graph_root)
        .with_context(|| "failed to convert typst to png via typst cli".to_owned())?;

    let sixel_buf = png_to_sixel(png_buf)
        .with_context(|| "failed to convert png to sixel via img2sixel cli".to_owned())?;
    io::stdout().write_all(&sixel_buf)?;

    Ok(())
}

// TODO: add https://crates.io/crates/rsille as an option

struct Typst(String);

impl Display for Typst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn markdown_to_typst(markdown: String) -> Result<Typst> {
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

    Ok(Typst(stdout))
}

const TYPST_FRONTMATTER: &str = "#set page(width: 13cm, height: auto, margin: 10pt)\n";

fn typst_to_png(typst: Typst, graph_root: &Path) -> Result<Vec<u8>> {
    // typst_file needs to be in graph_root to support root relative references to assets,
    // like `![](assets/image_1666695381725_0.png)`
    //
    // typst does not support page relative references to assets,
    // like `![](../assets/image_1666695381725_0.png)`
    //
    // TODO: find or file an issue
    let mut typst_file = NamedTempFile::new_in(graph_root)?;
    typst_file.write_all(TYPST_FRONTMATTER.as_bytes())?;
    typst_file.write_all(typst.to_string().as_bytes())?;

    let mut png_file = NamedTempFile::new()?;
    let output = process::Command::new("typst")
        .arg("compile")
        .arg("--ppi=300")
        .arg("--format=png")
        .arg(typst_file.path())
        .arg(png_file.path())
        .stderr(process::Stdio::piped())
        .output()?;

    let status = output.status;
    let stderr = String::from_utf8(output.stderr).with_context(|| "processing stderr failed")?;

    if !status.success() {
        return Err(anyhow!("{}", stderr));
    }

    let mut png_buf: Vec<u8> = Vec::new();
    png_file.read_to_end(&mut png_buf)?;

    Ok(png_buf)
}

fn png_to_sixel(png_buf: Vec<u8>) -> Result<Vec<u8>> {
    // TODO: use libsixel instead of shelling out
    let mut child = process::Command::new("img2sixel")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()?;
    {
        let mut stdin =
            child.stdin.take().expect("Stdin via pipe requested but ChildStdin is not present");
        stdin.write_all(&png_buf).with_context(|| "could not pass png on stdin")?;
    }

    let output = child.wait_with_output()?;
    let status = output.status;
    let stderr = String::from_utf8(output.stderr).with_context(|| "processing stderr failed")?;

    if !status.success() {
        return Err(anyhow!("{}", stderr));
    }

    Ok(output.stdout)
}
