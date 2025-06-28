use std::fmt::Debug;
use std::fs::{self};
use std::ops::RangeInclusive;
use std::path::Path;

use anyhow::{Result, anyhow};
use chrono::{DateTime, FixedOffset};
use log::warn;

use markdown::mdast::{self, Node};
use markdown::{ParseOptions, to_mdast};

fn list_item_is_card(li: &mdast::ListItem) -> bool {
    // A ListItem "is a card" if its first child is a Paragraph whos child is a Text with
    // value that has substring "#card"
    // Example card:
    // ListItem {
    //   children: [
    //       Paragraph {
    //           children: [
    //               Text {
    //                   value: "What is the taxon common name for Angiosperm? #card\ncard-last-interval:: 97.66\ncard-repeats:: 5\ncard-ease-factor:: 3\ncard-next-schedule:: 2025-07-14T00:00:00.000Z\ncard-last-reviewed:: 2025-04-07T09:12:54.010Z\ncard-last-score:: 5",
    //                   position: Some(
    //                       4:3-10:22 (53-293),
    //                   ),
    //               },
    //           ],
    //           position: Some(
    //               4:3-10:22 (53-293),
    //           ),
    //       },
    //       ...
    //   ],
    //   position: Some(
    //       4:1-11:21 (51-314),
    //   ),
    //   spread: false,
    //   checked: None,
    // }

    if let Some(Node::Paragraph(p)) = li.children.first() {
        return p.children.iter().any(|child| {
            if let Node::Text(text) = child { text.value.contains("#card") } else { false }
        });
    }

    false
}

fn find_card_list_items(file_raw: &str) -> Result<Vec<mdast::ListItem>> {
    let tree = to_mdast(file_raw, &ParseOptions::default())
        .map_err(|x| anyhow!("could not parse markdown: {:?}", x))?;
    let Node::Root(r) = tree else {
        return Err(anyhow!("expected Root node, got: {:?}", tree));
    };
    let top_list = match r.children.as_slice() {
        [Node::Paragraph(_)] | [] => {
            // If it's just a paragraph, there are no cards.
            // If it's empty, there are no cards.
            warn!("file did not contain a list");
            return Ok(vec![]);
        }
        [Node::Paragraph(_), Node::List(l)] => l,
        [Node::List(l)] => l,
        top_nodes => {
            return Err(anyhow!("expected (Paragraph,)? List, got: {:?}", top_nodes));
        }
    };
    Ok(find_card_list_items_inner(top_list))
}

fn find_card_list_items_inner(list: &mdast::List) -> Vec<mdast::ListItem> {
    let mut cards = Vec::new();
    for node in &list.children {
        if let Node::ListItem(li) = node {
            if list_item_is_card(li) {
                cards.push(li.clone());
                // We don't want cards within cards, perhaps it is worth warning about this
                continue;
            }
            for child in &li.children {
                if let Node::List(l) = child {
                    let mut nested = find_card_list_items_inner(l);
                    cards.append(&mut nested);
                }
            }
        }
    }
    cards
}

fn range_from_position(position: &markdown::unist::Position) -> RangeInclusive<usize> {
    // start.line and end.line are 1-indexed
    // start "Represents the place of the first character of the parsed source region."
    // end "Represents the place of the first character after the parsed source region, whether it exists or not."
    RangeInclusive::new(position.start.line - 1, position.end.line - 1)
}

#[derive(PartialEq, Clone)]
pub struct Fingerprint(pub u64);

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

impl From<u64> for Fingerprint {
    fn from(value: u64) -> Self {
        Fingerprint(value)
    }
}

// Some considerations
// * I want to be able to hold all card metadata in memory, without holding all card data in memory
// * I want to be able to load one card at a time and immediately store it back modified
// * source_path is potentially used in lots of cards, avoid copying it
pub struct CardRef<'a> {
    pub source_path: &'a Path,
    // prompt_fingerprint is XXH3 64 and will remain valid within the version of the crate,
    // but not necessarily accross.
    // The intended use is to list a set of cards, then immediately act on them one by one.
    pub prompt_fingerprint: Fingerprint,
}

pub struct CardMetadata<'a> {
    pub card_ref: CardRef<'a>,
    pub prompt_prefix: String,
    pub spaced_repetition_metadata: SpacedRepetitionMetadata,
}

impl Debug for CardMetadata<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CardMetadata {{")?;
        writeln!(f, "  source_path        : {}", self.card_ref.source_path.display())?;
        writeln!(f, "  prompt_fingerprint : {}", self.card_ref.prompt_fingerprint)?;
        writeln!(f, "  prompt_prefix      : {}", self.prompt_prefix)?;
        writeln!(f, "  spaced_repetition  : SpacedRepetitionMetadata {{")?;
        writeln!(f, "    repeats       : {}", self.spaced_repetition_metadata.repeats)?;
        writeln!(f, "    next_schedule : {:?}", self.spaced_repetition_metadata.next_schedule)?;
        writeln!(f, "    last_reviewed : {:?}", self.spaced_repetition_metadata.last_reviewed)?;
        writeln!(f, "  }}")?;
        write!(f, "}}")
    }
}

fn fingerprint(s: &str) -> Fingerprint {
    xxhash_rust::xxh3::xxh3_64(s.as_bytes()).into()
}

fn destructure_card<'a>(
    card: &mdast::ListItem,
    file_raw_lines: &'a [&'a str],
) -> Result<(&'a [&'a str], &'a [&'a str])> {
    // TODO: allow multiple paragraphs followed by a list
    // take until list?
    let (prompt_paragraph, response_list) = match card.children.as_slice() {
        [Node::Paragraph(p), Node::List(l)] => (p, l),
        _ => {
            return Err(anyhow!(
                "Expected card children to be [Paragraph, List], got {:?}",
                card.children
            ));
        }
    };

    let p_position = prompt_paragraph
        .position
        .as_ref()
        .ok_or_else(|| anyhow!("The p somehow didn't have a position"))?;
    let p_range = range_from_position(p_position);
    let Some(p_lines) = file_raw_lines.get(p_range) else {
        return Err(anyhow!("Failed to get prompt lines"));
    };

    let l_position = response_list
        .position
        .as_ref()
        .ok_or_else(|| anyhow!("The p somehow didn't have a position"))?;
    let l_range = range_from_position(l_position);
    let Some(l_lines) = file_raw_lines.get(l_range) else {
        return Err(anyhow!("Failed to get response list lines"));
    };

    Ok((p_lines, l_lines))
}

pub fn strip_prompt_metadata(prompt: &str) -> String {
    prompt.split("\n").filter(|l| !is_metadata_line(l)).collect::<Vec<_>>().join("\n")
}

fn is_metadata_line(l: &str) -> bool {
    l.trim_start().starts_with("card-")
}

// Logseq standard format:
//   card-last-interval:: 39.06
//   card-repeats:: 4
//   card-ease-factor:: 1.0
//   card-next-schedule:: 2025-07-15T00:00:00.000Z
//   card-last-reviewed:: 2025-06-06T16:24:48.795Z
//   card-last-score:: 1
//
// We don't use most of these
#[derive(Debug)]
pub struct SpacedRepetitionMetadata {
    pub repeats: u8,
    pub next_schedule: DateTime<FixedOffset>,
    pub last_reviewed: DateTime<FixedOffset>,
}

impl Default for SpacedRepetitionMetadata {
    fn default() -> Self {
        Self {
            repeats: 0,
            next_schedule: DateTime::UNIX_EPOCH.fixed_offset(),
            last_reviewed: DateTime::UNIX_EPOCH.fixed_offset(),
        }
    }
}

impl SpacedRepetitionMetadata {
    fn from_prompt(prompt: &str) -> Result<Self> {
        let mut repeats: Option<u8> = None;
        let mut next_schedule: Option<DateTime<FixedOffset>> = None;
        let mut last_reviewed: Option<DateTime<FixedOffset>> = None;

        for line in prompt.split("\n") {
            let Some((k, v)) = line.trim().split_once(":: ") else {
                continue;
            };
            match k {
                "card-repeats" => {
                    repeats = Some(v.parse()?);
                }
                "card-next-schedule" => {
                    next_schedule = Some(DateTime::parse_from_rfc3339(v)?);
                }
                "card-last-reviewed" => {
                    last_reviewed = Some(DateTime::parse_from_rfc3339(v)?);
                }
                _ => {}
            };
        }
        if let (Some(repeats), Some(next_schedule), Some(last_reviewed)) =
            (repeats, next_schedule, last_reviewed)
        {
            Ok(SpacedRepetitionMetadata { repeats, next_schedule, last_reviewed })
        } else {
            Ok(SpacedRepetitionMetadata::default())
        }
    }
}

fn extract_card<'a>(
    card_list_item: &mdast::ListItem,
    path: &'a Path,
    file_raw_lines: &[&str],
) -> Result<Card<'a>> {
    let (prompt_lines, response_lines) = destructure_card(card_list_item, file_raw_lines)?;

    let prompt_line_first = prompt_lines.first().unwrap_or(&"").to_owned().trim_end();
    let prompt_indent = prompt_line_first.chars().take_while(|c| c.is_whitespace()).count();
    // prompt_indent+2 to strip `- `
    let prompt_prefix = prompt_line_first.chars().skip(prompt_indent + 2).take(64).collect();

    let prompt = prompt_lines.join("\n");
    let clean_prompt = strip_prompt_metadata(&prompt);
    let response = response_lines.join("\n");

    Ok(Card {
        metadata: CardMetadata {
            card_ref: CardRef { source_path: path, prompt_fingerprint: fingerprint(&clean_prompt) },
            prompt_prefix,
            spaced_repetition_metadata: SpacedRepetitionMetadata::from_prompt(&prompt)?,
        },
        body: CardBody { prompt, response },
    })
}

pub fn extract_card_metadatas(path: &Path) -> Result<Vec<CardMetadata>> {
    let file_raw = fs::read_to_string(path)?;
    let file_raw_lines: Vec<&str> = file_raw.lines().collect();

    let card_list_items = find_card_list_items(&file_raw)?;

    let cards = card_list_items
        .iter()
        .map(|li| extract_card(li, path, &file_raw_lines))
        .collect::<Result<Vec<_>, _>>()?;
    let card_metadatas = cards.into_iter().map(|c| c.metadata).collect();

    Ok(card_metadatas)
}

pub struct CardBody {
    // Both prompt and response are stored as read from file
    pub prompt: String,
    pub response: String,
}

pub struct Card<'a> {
    pub metadata: CardMetadata<'a>,
    pub body: CardBody,
}

pub fn extract_card_by_ref<'a>(card_ref: &CardRef<'a>) -> Result<Card<'a>> {
    let path = card_ref.source_path;
    let file_raw = fs::read_to_string(path)?;
    let file_raw_lines: Vec<&str> = file_raw.lines().collect();

    let card_list_items = find_card_list_items(&file_raw)?;

    for li in card_list_items.as_slice() {
        let c = extract_card(li, path, &file_raw_lines)?;
        if c.metadata.card_ref.prompt_fingerprint == card_ref.prompt_fingerprint {
            return Ok(c);
        }
    }
    Err(anyhow!(
        "Card with fingerprint {} was not found in {}.",
        card_ref.prompt_fingerprint,
        card_ref.source_path.display(),
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::parse::{CardMetadata, CardRef, SpacedRepetitionMetadata};

    #[test]
    fn test_card_metadata_debug() {
        let path: PathBuf = "/tmp/page.md".into();
        let prompt_prefix = "What is love? #card".to_owned();
        let card_metadata = CardMetadata {
            card_ref: CardRef { source_path: &path, prompt_fingerprint: 1.into() },
            prompt_prefix: prompt_prefix,
            spaced_repetition_metadata: SpacedRepetitionMetadata::default(),
        };
        let expected = r#"CardMetadata {
  source_path        : /tmp/page.md
  prompt_fingerprint : 0x0000000000000001
  prompt_prefix      : What is love? #card
  spaced_repetition  : SpacedRepetitionMetadata {
    repeats       : 0
    next_schedule : 1970-01-01T00:00:00+00:00
    last_reviewed : 1970-01-01T00:00:00+00:00
  }
}"#;
        assert_eq!(format!("{:?}", card_metadata), expected);
    }
}
