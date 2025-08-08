use std::fmt::Display;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use serde::Serialize;
use tempfile::NamedTempFile;

use crate::terminal::grab_term_size;
use crate::types::Card;
use crate::types::CardMetadata;
use crate::types::FSRSMeta;
use crate::types::SRSMeta;

#[derive(Clone)]
pub enum OutputFormat {
    Clean,
    Typst,
    Sixel,
    Storage,
}

pub struct OutputSettings {
    pub format: OutputFormat,

    pub ppi: f32,

    pub base_font_size_pt: i32,
}

pub enum CardBodyParts {
    Prompt,
    All,
}

fn show_card_inner(
    card: &Card,
    card_body_parts: &CardBodyParts,
    output_settings: &OutputSettings,
) -> Result<()> {
    let mut result = Vec::new();
    match output_settings.format {
        OutputFormat::Clean => format_card_clean(card, &mut result, card_body_parts)?,
        OutputFormat::Typst => format_card_typst(card, &mut result, card_body_parts)?,
        OutputFormat::Sixel => {
            format_card_sixel(card, &mut result, card_body_parts, output_settings)?
        }
        OutputFormat::Storage => format_card_storage(card, &mut result, card_body_parts)?,
    };
    std::io::stdout().write_all(&result)?;
    Ok(())
}

pub fn show_card(card: &Card, output_settings: &OutputSettings) -> Result<()> {
    show_card_inner(card, &CardBodyParts::All, output_settings)
}

pub fn show_card_prompt(card: &Card, output_settings: &OutputSettings) -> Result<()> {
    show_card_inner(card, &CardBodyParts::Prompt, output_settings)
}

pub fn show_metadata(cm: &CardMetadata) -> Result<()> {
    println!("{:?}", cm);
    Ok(())
}

pub fn format_card_clean(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: &CardBodyParts,
) -> Result<()> {
    match card_body_parts {
        CardBodyParts::Prompt => writeln!(writer, "{}", card.body.prompt)?,
        CardBodyParts::All => {
            writeln!(writer, "{}", card.body.prompt)?;
            writeln!(writer, "{}", card.body.response)?;
        }
    }
    Ok(())
}

pub fn format_card_typst(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: &CardBodyParts,
) -> Result<()> {
    let markdown = match card_body_parts {
        CardBodyParts::Prompt => card.body.prompt.clone(),
        CardBodyParts::All => format!("{}\n{}", card.body.prompt, card.body.response),
    };
    let typst = markdown_to_typst(markdown)
        .with_context(|| "failed to convert markdown to typst using pandoc".to_owned())?;
    write!(writer, "{}", typst)?;
    Ok(())
}

pub fn format_card_sixel(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: &CardBodyParts,
    output_settings: &OutputSettings,
) -> Result<()> {
    let markdown = match card_body_parts {
        CardBodyParts::Prompt => card.body.prompt.clone(),
        CardBodyParts::All => format!("{}\n{}", card.body.prompt, card.body.response),
    };

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

    let png_buf = typst_to_png(typst, graph_root, output_settings)
        .with_context(|| "failed to convert typst to png via typst cli".to_owned())?;

    let sixel_buf = png_to_sixel(png_buf)
        .with_context(|| "failed to convert png to sixel via img2sixel cli".to_owned())?;
    writer.write_all(&sixel_buf)?;

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

// The 0.625 ratio is better suited for monospace fonts, but we use it regardless.
const FONT_HEIGHT_TO_WIDTH_SCALING: f32 = 0.625;
// Fun fact: "desktop publishing point" is exactly 1/72 of an inch.
const POINTS_PER_INCH: f32 = 72.0;
// Microsoft terminal configures line height as a multiple of font size.
// TODO: make this configurable or auto-detect
const LINE_HEIGHT_SCALING: f32 = 1.2;

fn build_typst_frontmatter(
    output_settings: &OutputSettings,
    (lines, columns): (u16, u16),
) -> String {
    // Convert early to avoid sprinkling conversions later
    let base_font_size_pt = output_settings.base_font_size_pt as f32;
    let columns = (columns) as f32;
    // The output during aborted review is:
    //
    // Reviewing [...]
    // <card-image>
    // How much effort [...]
    // [...] Esc to nope out
    // Error: Immediate nope out requested
    // <terminal-prompt>
    //
    // For a total of 5 lines
    let lines = (lines - 5) as f32;
    let base_ppi = 96.0;
    let ppi = output_settings.ppi;
    let ui_scaling = ppi / base_ppi;

    let width_pt = columns * (base_font_size_pt * FONT_HEIGHT_TO_WIDTH_SCALING);
    let width_in = width_pt / POINTS_PER_INCH;
    let width_scaled_in = width_in * ui_scaling;

    let height_pt = lines * base_font_size_pt * LINE_HEIGHT_SCALING;
    let height_in = height_pt / POINTS_PER_INCH;
    let height_scaled_in = height_in * ui_scaling;

    let font_size_pt = base_font_size_pt * 2.0;
    let font_size_scaled_pt = font_size_pt * ui_scaling;

    // TODO: if there is no image, use height: auto
    let mut rv = String::new();
    rv.push_str(&format!(
        "#set page(width: {}in, height: {}in, margin: {}pt)\n",
        width_scaled_in, height_scaled_in, font_size_scaled_pt,
    ));
    rv.push_str(&format!("#set text(size: {}pt)\n", font_size_scaled_pt));
    rv.push_str(&format!(
        "#show quote: it => {{ rect( inset: (left: {}pt, rest: {}pt), stroke: (left: {}pt + gray, rest: none), it.body) }}\n",
        font_size_scaled_pt, (font_size_scaled_pt/2.0), (font_size_scaled_pt/4.0)
    ));

    rv
}

fn typst_to_png(
    typst: Typst,
    graph_root: &Path,
    output_settings: &OutputSettings,
) -> Result<Vec<u8>> {
    // typst_file needs to be in graph_root to support root relative references to assets,
    // like `![](assets/image_1666695381725_0.png)`
    //
    // typst does not support page relative references to assets,
    // like `![](../assets/image_1666695381725_0.png)`
    //
    // TODO: find or file an issue

    let typst_frontmatter = build_typst_frontmatter(output_settings, grab_term_size());
    let mut typst_file = NamedTempFile::new_in(graph_root)?;
    typst_file.write_all(typst_frontmatter.as_bytes())?;
    typst_file.write_all(typst.to_string().as_bytes())?;

    let mut png_file = NamedTempFile::new()?;
    let output = process::Command::new("typst")
        .arg("compile")
        .arg(format!("--ppi={}", output_settings.ppi))
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

fn format_card_storage_text(
    mut writer: impl std::io::Write,
    text: &str,
    indent: &str,
) -> Result<()> {
    for line in text.lines() {
        writeln!(writer, "{indent}{line}")?
    }
    Ok(())
}

#[derive(Serialize)]
struct FSRSMetaForStorage {
    pub due: DateTime<Utc>,
    pub stability: f64,
    pub difficulty: f64,
    pub elapsed_days: i64,
    pub scheduled_days: i64,
    pub reps: i32,
    pub lapses: i32,
    pub state: rs_fsrs::State,
    pub last_review: DateTime<Utc>,
}

fn truncate_to_millis(dt: &DateTime<Utc>) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(dt.timestamp_millis()).unwrap()
}

impl From<&FSRSMeta> for FSRSMetaForStorage {
    fn from(value: &FSRSMeta) -> Self {
        FSRSMetaForStorage {
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
}

fn format_card_storage_srs_meta(
    mut writer: impl std::io::Write,
    srs_meta: &SRSMeta,
    indent: &str,
) -> Result<()> {
    let logseq_srs_meta = &srs_meta.logseq_srs_meta;
    writeln!(writer, "{indent}card-last-interval:: {}", logseq_srs_meta.last_interval)?;
    writeln!(writer, "{indent}card-repeats:: {}", logseq_srs_meta.repeats)?;
    writeln!(writer, "{indent}card-ease-factor:: {}", logseq_srs_meta.ease_factor)?;
    writeln!(
        writer,
        "{indent}card-next-schedule:: {}",
        logseq_srs_meta.next_schedule.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    )?;
    writeln!(
        writer,
        "{indent}card-last-reviewed:: {}",
        logseq_srs_meta.last_reviewed.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    )?;
    writeln!(writer, "{indent}card-last-score:: {}", logseq_srs_meta.last_score)?;

    let fsrs_meta: FSRSMetaForStorage = (&srs_meta.fsrs_meta).into();
    writeln!(writer, "{indent}card-fsrs-metadata:: {}", serde_json::to_string(&fsrs_meta)?)?;

    Ok(())
}

pub fn format_card_storage(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: &CardBodyParts,
) -> Result<()> {
    // format_card_storage does not accept a CardBodyPart, as the card is always stored with all parts
    if let CardBodyParts::Prompt = card_body_parts {
        return Err(anyhow!("can not output just the prompt in storage format"));
    }
    let prompt_indent = " ".repeat(card.body.prompt_indent);
    let meta_indent = " ".repeat(card.body.prompt_indent + 2);
    format_card_storage_text(&mut writer, &card.body.prompt, &prompt_indent)?;
    format_card_storage_srs_meta(&mut writer, &card.metadata.srs_meta, &meta_indent)?;
    format_card_storage_text(&mut writer, &card.body.response, &prompt_indent)?;

    Ok(())
}
