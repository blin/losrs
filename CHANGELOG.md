# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Changed

- Default `storage.metadata_mode` is set to `in-graph-root`.

### Deprecated

- The `inline` `storage.metadata_mode` is deprecated and
  will eventually be removed.

## v0.4.0 - 2026-02-25

### Added

- Added "delay review by 1 day" button, useful for related cards.

## v0.3.0 - 2026-02-20

### Added

- Added "in-graph-root" card metadata storage,
  useful if you want to view the page files outside of Logseq.

### Changed

- If Card Serial Number is set,
  it will be displayed instead of prompt fingerprint during review.
- `losrs metadata` now produces JSON instead of custom format.

### Removed

- Prompt prefix is no longer included in the `losrs metadata` output.

## v0.2.0 - 2025-10-20

### Added

- On first review cards get assigned a "Card Serial Number (CSN)",
  stored as a `<!-- CSN:12345 -->` right after `#card`.
- Cards can be referred to by both prompt fingerprint and CSN (if assigned).

### Changed

- The `--at $TIMESTAMP` flag is split into `--at` and `--up-to` flags,
  enabling both gradual catching up on reviews and look ahead.
  I review cards in the morning, but don't mind picking up cards
  slightly ahead of schedule, so I run
  `losrs review . --up-to=$(date --iso-8601=d --utc)T13:00:00Z`.
  Probably a good idea to support a duration offset from "now" at some point.
- `losrs show` now outputs cards in page name order,
  instead of OS `readdir` order.

## v0.1.0 - 2025-08-27

What does one write in the changelog for the first release?

This release establishes the bones of losrs:

- Cards are stored in Logseq "page" files
  with Logseq metadata format (in-line with the card).
- "page" files are stored in Logseq graph root directory layout.
- Card matching is done on prompt fingerprint.
- FSRS metadata is bolted on to Logseq metadata format,
  it does not get hidden by Logseq.
- Cards are rendered via markdown -> typst -> sixel pipeline,
  or a subset of it if desired.
- Sixel rendering is configurable via a TOML config or ENV variables.
