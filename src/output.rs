use std::fmt::Display;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use tempfile::NamedTempFile;

use crate::settings::OutputFormat;
use crate::settings::OutputSettings;
use crate::terminal::grab_term_size;
use crate::types::Card;
use crate::types::CardMetadata;
use crate::types::SRSMeta;

bitflags::bitflags! {
    pub struct CardBodyParts: u8 {
        const PROMPT   = 0b001;
        const SRS_META = 0b010;
        const RESPONSE = 0b100;
        const ALL = Self::PROMPT.bits() | Self::SRS_META.bits() | Self::RESPONSE.bits();
    }
}

fn show_card_inner(
    card: &Card,
    card_body_parts: CardBodyParts,
    output_settings: &OutputSettings,
) -> Result<()> {
    let mut result = Vec::new();
    match output_settings.format {
        OutputFormat::Clean => format_card_clean(card, &mut result, card_body_parts)?,
        OutputFormat::Typst => format_card_typst(card, &mut result, card_body_parts)?,
        OutputFormat::Logseq => format_card_logseq(card, &mut result, card_body_parts)?,
        OutputFormat::Sixel => {
            format_card_sixel(card, &mut result, card_body_parts, output_settings)?
        }
        OutputFormat::Kitty => show_card_kitty_or_iterm(card, card_body_parts, output_settings)?,
        OutputFormat::ITerm => show_card_kitty_or_iterm(card, card_body_parts, output_settings)?,
    };
    std::io::stdout().write_all(&result)?;
    Ok(())
}

pub fn show_card(card: &Card, output_settings: &OutputSettings) -> Result<()> {
    show_card_inner(card, CardBodyParts::ALL, output_settings)
}

pub fn show_card_prompt(card: &Card, output_settings: &OutputSettings) -> Result<()> {
    show_card_inner(card, CardBodyParts::PROMPT, output_settings)
}

pub fn show_metadata(cm: &CardMetadata) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&cm)?);
    Ok(())
}

fn card_to_markdown(card: &Card, card_body_parts: CardBodyParts) -> String {
    let mut parts = Vec::new();
    if card_body_parts.contains(CardBodyParts::PROMPT) {
        parts.push(card.body.prompt.as_str());
    }
    if card_body_parts.contains(CardBodyParts::RESPONSE) {
        parts.push(card.body.response.as_str());
    }
    parts.join("\n")
}

pub fn format_card_clean(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: CardBodyParts,
) -> Result<()> {
    writeln!(writer, "{}", card_to_markdown(card, card_body_parts))?;
    Ok(())
}

pub fn format_card_typst(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: CardBodyParts,
) -> Result<()> {
    let markdown = card_to_markdown(card, card_body_parts);
    let typst = markdown_to_typst(markdown)
        .with_context(|| "failed to convert markdown to typst using pandoc".to_owned())?;
    write!(writer, "{}", typst)?;
    Ok(())
}

fn card_to_png(
    card: &Card,
    card_body_parts: CardBodyParts,
    output_settings: &OutputSettings,
) -> Result<Vec<u8>> {
    let markdown = card_to_markdown(card, card_body_parts);
    let typst = markdown_to_typst(markdown)
        .with_context(|| "failed to convert markdown to typst using pandoc".to_owned())?;

    // [tag:logseq-dir-layout]
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
    let graph_root =
        card.metadata.card_ref.source_path.parent().and_then(Path::parent).ok_or(anyhow!(
            "page file does not have a grandparent. The page is {}",
            card.metadata.card_ref.source_path.display()
        ))?;

    let png_buf = typst_to_png(typst, graph_root, output_settings)
        .with_context(|| "failed to convert typst to png via typst cli".to_owned())?;

    Ok(png_buf)
}

pub fn format_card_sixel(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: CardBodyParts,
    output_settings: &OutputSettings,
) -> Result<()> {
    let png_buf = card_to_png(card, card_body_parts, output_settings)?;
    let sixel_buf = png_to_sixel(png_buf)
        .with_context(|| "failed to convert png to sixel via img2sixel cli".to_owned())?;
    writer.write_all(&sixel_buf)?;

    Ok(())
}

pub fn show_card_kitty_or_iterm(
    card: &Card,
    card_body_parts: CardBodyParts,
    output_settings: &OutputSettings,
) -> Result<()> {
    use image::ImageReader;
    use std::io::Cursor;

    let png_buf = card_to_png(card, card_body_parts, output_settings)?;

    let img = ImageReader::with_format(Cursor::new(png_buf), image::ImageFormat::Png).decode()?;

    let conf = viuer::Config {
        absolute_offset: false,
        use_kitty: true,
        use_iterm: true,
        // TODO: figure out how to make sixel via viuer look as good as sixel via img2sixel
        // use_sixel: false,
        ..Default::default()
    };

    viuer::print(&img, &conf).unwrap();

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

fn build_typst_frontmatter(
    output_settings: &OutputSettings,
    (columns, lines): (u16, u16),
) -> String {
    // Convert early to avoid sprinkling conversions later
    let base_font_size_pt = output_settings.base_font_size as f32;
    let columns = columns as f32;
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

    let height_pt = lines * base_font_size_pt * output_settings.line_height_scaling;
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

fn format_card_logseq_text(
    mut writer: impl std::io::Write,
    text: &str,
    indent: &str,
) -> Result<()> {
    for line in text.lines() {
        writeln!(writer, "{indent}{line}")?
    }
    Ok(())
}

fn format_card_logseq_srs_meta(
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

    writeln!(
        writer,
        "{indent}card-fsrs-metadata:: {}",
        serde_json::to_string(&srs_meta.fsrs_meta)?
    )?;

    Ok(())
}

pub fn format_card_logseq(
    card: &Card,
    mut writer: impl std::io::Write,
    card_body_parts: CardBodyParts,
) -> Result<()> {
    let prompt_indent = " ".repeat(card.body.prompt_indent);
    let meta_indent = " ".repeat(card.body.prompt_indent + 2);
    if card_body_parts.contains(CardBodyParts::PROMPT) {
        format_card_logseq_text(&mut writer, &card.body.prompt, &prompt_indent)?;
    }
    if card_body_parts.contains(CardBodyParts::SRS_META) {
        format_card_logseq_srs_meta(&mut writer, &card.metadata.srs_meta, &meta_indent)?;
    }
    if card_body_parts.contains(CardBodyParts::RESPONSE) {
        format_card_logseq_text(&mut writer, &card.body.response, &prompt_indent)?;
    }

    Ok(())
}
