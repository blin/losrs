use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::{self};
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::ops::RangeInclusive;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::LazyLock;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;

use markdown::ParseOptions;
use markdown::mdast::Node;
use markdown::mdast::{self};
use markdown::to_mdast;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;

use crate::output::CardBodyParts;
use crate::output::format_card_logseq;
use crate::settings::MetadataMode;
use crate::settings::StorageSettings;
use crate::types::Card;
use crate::types::CardBody;
use crate::types::CardId;
use crate::types::CardMetadata;
use crate::types::CardRef;
use crate::types::FSRSMeta;
use crate::types::LogseqSRSMeta;
use crate::types::SRSMeta;

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

struct CardLineRanges {
    prompt_range: RangeInclusive<usize>,
    response_range: RangeInclusive<usize>,
}

fn find_card_ranges(card: &mdast::ListItem) -> Result<CardLineRanges> {
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

    let l_position = response_list
        .position
        .as_ref()
        .ok_or_else(|| anyhow!("The p somehow didn't have a position"))?;
    let l_range = range_from_position(l_position);

    Ok(CardLineRanges { prompt_range: p_range, response_range: l_range })
}

fn destructure_card<'a>(
    card: &mdast::ListItem,
    file_raw_lines: &'a [&'a str],
) -> Result<(&'a [&'a str], &'a [&'a str])> {
    let ranges = find_card_ranges(card)?;
    let Some(prompt_lines) = file_raw_lines.get(ranges.prompt_range) else {
        return Err(anyhow!("Failed to get prompt lines"));
    };

    let Some(response_lines) = file_raw_lines.get(ranges.response_range) else {
        return Err(anyhow!("Failed to get response lines"));
    };

    Ok((prompt_lines, response_lines))
}

fn is_metadata_line(l: &str) -> bool {
    l.trim_start().starts_with("card-")
}

impl SRSMeta {
    fn from_prompt_lines(prompt_lines: &[&str]) -> Result<Self> {
        let mut logseq_srs_meta = LogseqSRSMeta::default();
        let mut fsrs_meta: Option<FSRSMeta> = None;

        for line in prompt_lines {
            let Some((k, v)) = line.trim().split_once(":: ") else {
                continue;
            };
            (|| -> Result<()> {
                match k {
                    "card-last-interval" => {
                        logseq_srs_meta.last_interval = v.parse()?;
                    }
                    "card-repeats" => {
                        logseq_srs_meta.repeats = v.parse()?;
                    }
                    "card-ease-factor" => {
                        logseq_srs_meta.ease_factor = v.parse()?;
                    }
                    "card-next-schedule" => {
                        logseq_srs_meta.next_schedule = DateTime::parse_from_rfc3339(v)?;
                    }
                    "card-last-reviewed" => {
                        logseq_srs_meta.last_reviewed = DateTime::parse_from_rfc3339(v)?;
                    }
                    "card-last-score" => {
                        logseq_srs_meta.last_score = v.parse()?;
                    }
                    "card-fsrs-metadata" => {
                        fsrs_meta = Some(serde_json::from_str(v)?);
                    }
                    _ => {}
                };
                Ok(())
            })()
            .with_context(|| anyhow!("when processing key '{}'", k))?;
        }
        match fsrs_meta {
            Some(fsrs_meta) => {
                let logseq_srs_meta: LogseqSRSMeta = (&fsrs_meta).into();
                Ok(SRSMeta { logseq_srs_meta, fsrs_meta })
            }
            None => {
                // This case includes "neither metadata is present",
                let fsrs_meta: FSRSMeta = (&logseq_srs_meta).into();
                Ok(SRSMeta { logseq_srs_meta, fsrs_meta })
            }
        }
    }
}

fn strip_prompt_metadata<'a>(
    prompt_lines: impl Iterator<Item = &'a str>,
) -> impl Iterator<Item = &'a str> {
    prompt_lines.filter(|l| !is_metadata_line(l))
}

fn strip_indent<'a>(
    lines: impl Iterator<Item = &'a str>,
    indent: &str,
) -> impl Iterator<Item = &'a str> {
    lines.map(move |line| line.strip_prefix(indent).unwrap_or(line))
}

static CARD_SERIAL_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#card( <!-- CSN:(?<csn>[0-9]+) -->)?").unwrap());

fn extract_serial_num(prompt: &str) -> Option<u64> {
    // If ever CSN is >u64 this will panic
    CARD_SERIAL_NUM_RE.captures(prompt)?.name("csn").map(|m| m.as_str().parse::<u64>().unwrap())
}

fn maybe_allocate_serial_num(
    card: &mut Card,
    serial_num_allocator: &mut dyn CardSerialNumAllocator,
) -> Result<()> {
    if card.metadata.card_ref.serial_num.is_some() {
        return Ok(());
    };
    let Some(serial_num) = serial_num_allocator.allocate() else {
        return Ok(());
    };
    let serial_num = serial_num.with_context(|| {
        anyhow!(
            "could not allocate serial number for card in {} with fingerprint {}",
            card.metadata.card_ref.source_path.display(),
            card.metadata.card_ref.prompt_fingerprint
        )
    })?;
    card.metadata.card_ref.serial_num = Some(serial_num);
    card.body.prompt = CARD_SERIAL_NUM_RE
        .replace(&card.body.prompt, format!("#card <!-- CSN:{} -->", serial_num))
        .to_string();
    Ok(())
}

struct Page {
    path: Rc<PathBuf>,
    file_raw: String,
    card_list_items: Vec<mdast::ListItem>,
}

impl Page {
    fn new(path: &Path) -> Result<Self> {
        let file_raw = fs::read_to_string(path)?;

        let card_list_items = find_card_list_items(&file_raw)
            .with_context(|| anyhow!("when searching for card list items"))?;
        Ok(Page { path: Rc::new(path.to_path_buf()), file_raw, card_list_items })
    }

    fn get_lines(&self) -> Vec<&str> {
        self.file_raw.lines().collect()
    }

    fn extract_card(&self, card_list_item: &mdast::ListItem) -> Result<Card> {
        let file_raw_lines = self.get_lines();
        let (prompt_lines, response_lines) = destructure_card(card_list_item, &file_raw_lines)?;

        let prompt_line_first = prompt_lines.first().unwrap_or(&"").to_owned().trim_end();
        let prompt_indent_size = prompt_line_first.chars().take_while(|c| *c == ' ').count();
        let prompt_indent = " ".repeat(prompt_indent_size);

        let prompt =
            strip_indent(strip_prompt_metadata(prompt_lines.iter().copied()), &prompt_indent)
                .collect::<Vec<_>>()
                .join("\n");

        let response = strip_indent(response_lines.iter().copied(), &prompt_indent)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Card {
            metadata: CardMetadata {
                card_ref: CardRef {
                    source_path: self.path.clone(),
                    prompt_fingerprint: prompt.as_str().into(),
                    serial_num: extract_serial_num(&prompt),
                },
                srs_meta: SRSMeta::from_prompt_lines(prompt_lines)
                    .with_context(|| "when extracting SRS meta")?,
            },
            body: CardBody { prompt, prompt_indent: prompt_indent_size, response },
        })
    }

    fn extract_cards(&self) -> Result<Vec<Card>> {
        self.card_list_items
            .iter()
            .map(|li| {
                self.extract_card(li).with_context(|| {
                    anyhow!(
                        "when extracting a card from list item on line {}",
                        li.position
                            .as_ref()
                            .map(|pos| pos.start.line)
                            .expect("so far list items always have a start...")
                    )
                })
            })
            .collect()
    }

    fn find_card(&self, card_ref: &CardRef) -> Result<(CardLineRanges, Card)> {
        for li in &self.card_list_items {
            let card = self.extract_card(li)?;
            if card.metadata.card_ref.prompt_fingerprint == card_ref.prompt_fingerprint {
                let card_ranges = find_card_ranges(li)?;
                return Ok((card_ranges, card));
            }
        }
        Err(anyhow!(
            "Card with fingerprint {} was not found in {}.",
            card_ref.prompt_fingerprint,
            card_ref.source_path.display(),
        ))
    }

    fn rewrite_card(
        &self,
        card: &Card,
        card_ranges: &CardLineRanges,
        card_body_parts: CardBodyParts,
    ) -> Result<()> {
        let file_raw_lines = self.get_lines();
        let mut f = File::create(self.path.as_ref())?;

        let pre_lines = &file_raw_lines[..*card_ranges.prompt_range.start()];
        if !pre_lines.is_empty() {
            f.write_all(pre_lines.join("\n").as_bytes())?;
            f.write_all("\n".as_bytes())?;
        }

        format_card_logseq(card, &f, card_body_parts)?;

        let post_lines = &file_raw_lines[*card_ranges.response_range.end() + 1..];
        if !post_lines.is_empty() {
            f.write_all(post_lines.join("\n").as_bytes())?;
            f.write_all("\n".as_bytes())?;
        }

        Ok(())
    }
}

trait CardSerialNumAllocator {
    // None means we didn't attempt allocating a serial number,
    // because it does not make sense in the given context.
    fn allocate(&mut self) -> Option<Result<u64>>;
}

enum PageFiles {
    Single(PathBuf),
    SingleInGraphRoot(PathBuf, PathBuf),
    GraphRoot(PathBuf, Vec<PathBuf>),
}

fn find_page_files_inner(path: &Path) -> Result<PageFiles> {
    if !path.exists() {
        return Err(anyhow!("{} does not exist", path.display()));
    }
    // [ref:logseq-dir-layout]
    if !path.is_dir() {
        let Some(parent) = path.parent() else { return Ok(PageFiles::Single(path.to_path_buf())) };
        let parent_name = parent
            .file_name()
            .ok_or_else(|| anyhow!("parent of {} does not have a file name", path.display()))?;
        if parent_name == "pages" {
            // parent is definitely not root, so it definitely has a parent, unwrap is fine.
            return Ok(PageFiles::SingleInGraphRoot(
                parent.parent().unwrap().to_path_buf(),
                path.to_path_buf(),
            ));
        } else {
            return Ok(PageFiles::Single(path.to_path_buf()));
        }
    };

    let pages_dir = path.join("pages");
    if !pages_dir.exists() {
        return Err(anyhow!(
            "{} is a directory without a pages subdirectory, expected logseq graph root",
            path.display()
        ));
    }
    let page_files = std::fs::read_dir(pages_dir)?
        .filter_map(Result::ok)
        .map(|d| d.path())
        .filter(|p| p.is_file() && p.extension() == Some(OsStr::new("md")))
        .collect();
    Ok(PageFiles::GraphRoot(path.to_path_buf(), page_files))
}

fn find_graph_root(path: &Path) -> Result<Option<PathBuf>> {
    match find_page_files_inner(path)? {
        PageFiles::Single(_) => Ok(None),
        PageFiles::SingleInGraphRoot(graph_root, _) => Ok(Some(graph_root)),
        PageFiles::GraphRoot(graph_root, _) => Ok(Some(graph_root)),
    }
}

struct NoOpSerialNumAllocator {}

impl CardSerialNumAllocator for NoOpSerialNumAllocator {
    fn allocate(&mut self) -> Option<Result<u64>> {
        // Not attempted
        None
    }
}

struct GraphRootSerialNumAllocator {
    graph_root: PathBuf,
}

impl GraphRootSerialNumAllocator {
    fn allocate_inner(&mut self) -> Result<u64> {
        let card_serial_num_path = self.graph_root.join(".card-serial-num");

        if !card_serial_num_path.exists() {
            let mut card_serial_num_file = File::create(card_serial_num_path)?;
            card_serial_num_file.write_all(b"0\n")?;
            return Ok(0);
        }

        let mut card_serial_num_file =
            OpenOptions::new().read(true).write(true).open(&card_serial_num_path)?;

        let mut card_serial_num_raw = String::new();
        card_serial_num_file.read_to_string(&mut card_serial_num_raw)?;

        let mut card_serial: u64 = card_serial_num_raw.trim_end().parse()?;
        card_serial += 1;

        card_serial_num_file.seek(SeekFrom::Start(0))?;
        card_serial_num_file.set_len(0)?;
        card_serial_num_file.write_all(format!("{}\n", card_serial).as_bytes())?;

        Ok(card_serial)
    }
}

impl CardSerialNumAllocator for GraphRootSerialNumAllocator {
    fn allocate(&mut self) -> Option<Result<u64>> {
        Some(
            self.allocate_inner()
                .with_context(|| anyhow!("failed to read/write card serial number")),
        )
    }
}

fn choose_serial_num_allocator(path: &Path) -> Result<Box<dyn CardSerialNumAllocator>> {
    let Some(graph_root) = find_graph_root(path)? else {
        return Ok(Box::new(NoOpSerialNumAllocator {}));
    };
    Ok(Box::new(GraphRootSerialNumAllocator { graph_root }))
}

#[derive(Debug, Serialize, Deserialize)]
struct InGraphRootCardMetadata {
    serial_num: u64,
    fsrs_meta: FSRSMeta,
}

enum MetadataSource {
    PageFiles,
    GraphRoot(PathBuf),
}

pub struct StorageManager {
    serial_num_allocator: Box<dyn CardSerialNumAllocator>,
    metadata_source: MetadataSource,
}

impl StorageManager {
    pub fn new(path: &Path, settings: &StorageSettings) -> Result<Self> {
        let metadata_source: MetadataSource = match settings.metadata_mode {
            MetadataMode::Inline => MetadataSource::PageFiles,
            MetadataMode::InGraphRoot => {
                let Some(graph_root) = find_graph_root(path)? else {
                    return Err(anyhow!("there is no graph root for {}", path.display()));
                };
                MetadataSource::GraphRoot(graph_root)
            }
        };
        Ok(Self { serial_num_allocator: choose_serial_num_allocator(path)?, metadata_source })
    }

    pub fn find_page_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        match find_page_files_inner(path)? {
            PageFiles::Single(page_path) => Ok(vec![page_path]),
            PageFiles::SingleInGraphRoot(_, page_path) => Ok(vec![page_path]),
            PageFiles::GraphRoot(_, page_paths) => Ok(page_paths),
        }
    }

    fn load_card_metas_from_page(&self, page_file: &Path) -> Result<Vec<CardMetadata>> {
        let page = Page::new(page_file)?;
        let card_metadatas = page.extract_cards()?.into_iter().map(|c| c.metadata).collect();
        Ok(card_metadatas)
    }

    pub fn load_card_body_by_ref(&self, card_ref: &CardRef) -> Result<CardBody> {
        let page = Page::new(&card_ref.source_path)?;
        let (_card_ranges, card) = page.find_card(card_ref)?;
        Ok(card.body)
    }

    fn get_card_metadata_path(graph_root: &Path) -> PathBuf {
        graph_root.join(".card-metadata.jsonl")
    }

    fn load_fsrs_metas(graph_root: &Path) -> Result<BTreeMap<u64, FSRSMeta>> {
        let card_metadata_path = Self::get_card_metadata_path(graph_root);

        if !card_metadata_path.exists() {
            // Will create on first write
            return Ok(BTreeMap::new());
        }

        let mut fsrs_metas_by_csn: BTreeMap<u64, FSRSMeta> = BTreeMap::new();
        for line in fs::read_to_string(&card_metadata_path)?.lines() {
            let cm: InGraphRootCardMetadata = serde_json::from_str(line)?;
            fsrs_metas_by_csn.insert(cm.serial_num, cm.fsrs_meta);
        }
        Ok(fsrs_metas_by_csn)
    }

    fn store_fsrs_metas(graph_root: &Path, fsrs_metas: BTreeMap<u64, FSRSMeta>) -> Result<()> {
        assert!(!fsrs_metas.is_empty());
        let card_metadata_path = Self::get_card_metadata_path(graph_root);

        let mut card_metadata_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&card_metadata_path)
            .with_context(|| {
                anyhow!("when opening {} for writing", card_metadata_path.display())
            })?;

        // BTreeMap guarantees that metadata is written in serial_num order
        for (csn, fsrs_meta) in fsrs_metas.into_iter() {
            let v = InGraphRootCardMetadata { serial_num: csn, fsrs_meta };
            let cm = serde_json::to_string(&v)?;
            card_metadata_file.write_all(cm.as_bytes())?;
            card_metadata_file.write_all(b"\n")?;
        }
        card_metadata_file.sync_all()?;

        Ok(())
    }

    fn merge_page_and_graph_root_card_metas(
        page_card_metas: Vec<CardMetadata>,
        mut graph_root_card_metas_by_csn: BTreeMap<u64, FSRSMeta>,
    ) -> Vec<CardMetadata> {
        let mut card_metas: Vec<CardMetadata> = Vec::new();
        for page_card_meta in page_card_metas {
            let Some(csn) = page_card_meta.card_ref.serial_num else {
                card_metas.push(page_card_meta);
                continue;
            };
            let Some(fsrs_meta) = graph_root_card_metas_by_csn.remove(&csn) else {
                card_metas.push(page_card_meta);
                continue;
            };
            card_metas.push(CardMetadata {
                card_ref: page_card_meta.card_ref,
                srs_meta: SRSMeta { logseq_srs_meta: (&fsrs_meta).into(), fsrs_meta },
            });
        }
        card_metas
    }

    pub fn load_card_metas(&self, page_file: &Path) -> Result<Vec<CardMetadata>> {
        let page_card_metas = self.load_card_metas_from_page(page_file)?;
        match &self.metadata_source {
            MetadataSource::PageFiles => Ok(page_card_metas),
            MetadataSource::GraphRoot(graph_root) => {
                Ok(Self::merge_page_and_graph_root_card_metas(
                    page_card_metas,
                    Self::load_fsrs_metas(graph_root)?,
                ))
            }
        }
    }

    pub fn rewrite_card_meta(&mut self, card_ref: &CardRef, srs_meta: &SRSMeta) -> Result<()> {
        let page = Page::new(&card_ref.source_path)?;
        let (card_ranges, mut card) = page.find_card(card_ref)?;
        card.metadata.srs_meta = srs_meta.clone();
        maybe_allocate_serial_num(&mut card, self.serial_num_allocator.as_mut())?;

        match &self.metadata_source {
            MetadataSource::PageFiles => {
                page.rewrite_card(&card, &card_ranges, CardBodyParts::ALL)
            }
            MetadataSource::GraphRoot(graph_root) => {
                page.rewrite_card(
                    &card,
                    &card_ranges,
                    CardBodyParts::PROMPT | CardBodyParts::RESPONSE,
                )?;
                let csn = card.metadata.card_ref.serial_num.unwrap();

                let mut card_fsrs_metas_by_csn = Self::load_fsrs_metas(graph_root)?;
                card_fsrs_metas_by_csn.insert(csn, srs_meta.fsrs_meta.clone());
                Self::store_fsrs_metas(graph_root, card_fsrs_metas_by_csn)
            }
        }
    }

    pub fn select_card_metadata(
        &self,
        path: &Path,
        card_id: Option<CardId>,
    ) -> Result<Vec<CardMetadata>> {
        let page_files: Vec<PathBuf> = self.find_page_files(path)?;
        let mut all_card_metadatas: Vec<CardMetadata> = Vec::new();
        for page_file in page_files.into_iter() {
            // avoid copying page_file just so we can print it later
            let context = format!("when extracting card metadatas from {}", &page_file.display());
            let mut card_metadatas = self.load_card_metas(&page_file).with_context(|| context)?;

            if let Some(card_id) = card_id.clone() {
                let p: Box<dyn Fn(&CardMetadata) -> bool> = match &card_id {
                    CardId::Fingerprint(fingerprint) => {
                        Box::new(|cm: &CardMetadata| cm.card_ref.prompt_fingerprint == *fingerprint)
                    }
                    CardId::SerialNum(serial_num) => {
                        Box::new(|cm: &CardMetadata| cm.card_ref.serial_num == Some(*serial_num))
                    }
                };
                card_metadatas.retain(p);
            }
            all_card_metadatas.extend(card_metadatas);
        }
        Ok(all_card_metadatas)
    }
}
