use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::process::Command;

// TODO: switch to https://docs.rs/trycmd/latest/trycmd or similar

#[test]
fn single_top_level_card() -> Result<(), Box<dyn std::error::Error>> {
    let file = assert_fs::NamedTempFile::new("single-top-level-card.md")?;
    let content = r#"\
- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#;
    file.write_str(content)?;
    let expected_output = r#"- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
"#;
    let mut cmd = Command::cargo_bin("logseq-srs")?;

    cmd.arg("cards-in-file").arg(file.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected_output));

    Ok(())
}
