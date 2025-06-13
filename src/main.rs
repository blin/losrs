use log::warn;
use markdown::{
    ParseOptions,
    mdast::{self, Node},
    to_mdast,
};

use std::{
    fs::{self},
    ops::RangeInclusive,
    path::PathBuf,
};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};

/// Work with Spaced Repetition Cards (SRS) embedded in Logseq pages
#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// prints cards in a file
    CardsInFile {
        /// The path to the file to read
        path: PathBuf,
    },
}

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
            if let Node::Text(text) = child {
                text.value.contains("#card")
            } else {
                false
            }
        });
    }

    false
}

fn find_card_list_items(list: &mdast::List) -> Vec<mdast::ListItem> {
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
                    let mut nested = find_card_list_items(l);
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

fn cards_in_file(path: &PathBuf) -> Result<()> {
    let file_raw = fs::read_to_string(path)?;
    let file_raw_lines: Vec<&str> = file_raw.lines().collect();

    let tree = to_mdast(&file_raw, &ParseOptions::default())
        .map_err(|x| anyhow!("could not parse markdown: {:?}", x))?;
    let Node::Root(r) = tree else {
        return Err(anyhow!("expected Root node, got: {:?}", tree));
    };
    let top_list = match r.children.as_slice() {
        [Node::Paragraph(_)] | [] => {
            // If it's just a paragraph, there are no cards.
            // If it's empty, there are no cards.
            warn!("file did not contain a list");
            return Ok(());
        }
        [Node::Paragraph(_), Node::List(l)] => l,
        [Node::List(l)] => l,
        top_nodes => {
            return Err(anyhow!("expected (Paragraph,)? List, got: {:?}", top_nodes));
        }
    };
    let card_list_items = find_card_list_items(top_list);
    for li in card_list_items.as_slice() {
        let p = li.position.as_ref().ok_or_else(|| {
            anyhow!(
                "expected card list item to have a position, card_list_item={:?}",
                li
            )
        })?;
        let li_range = range_from_position(p);
        let Some(li_lines) = file_raw_lines.get(li_range.clone()) else {
            return Err(anyhow!(
                "expected to have lines within {:?}, got nothing",
                li_range
            ));
        };
        for line in li_lines {
            println!("{}", line);
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbosity.into())
        .init();

    match cli.command {
        Commands::CardsInFile { path } => {
            if !path.exists() {
                return Err(anyhow!("{} does not exist", path.display()));
            }
            cards_in_file(&path).with_context(|| format!("when processing {}", path.display()))?;
        }
    }

    Ok(())
}
