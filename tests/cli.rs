use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::process::Command;

// TODO: switch to https://docs.rs/trycmd/latest/trycmd or similar

#[test]
fn single_top_level_card() -> Result<(), Box<dyn std::error::Error>> {
    let file = assert_fs::NamedTempFile::new("page.md")?;
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

#[test]
fn card_with_data_after_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let file = assert_fs::NamedTempFile::new("page.md")?;
    let content = r#"\
- Not card
- What is the relationship between angles $\\alpha$ and $\\gamma_{1}$ in the picture relative to the transversal?
  card-last-interval:: 30.0
  card-repeats:: 6
  card-ease-factor:: 2.5
  card-next-schedule:: 2026-01-01T00:00:00.000Z
  card-last-reviewed:: 2025-12-02T00:00:00.000Z
  card-last-score:: 5
  https://upload.wikimedia.org/wikipedia/commons/thumb/3/3d/Transverzala_8.svg/262px-Transverzala_8.svg.png
  #card
  - They are alternate angles.
- Not card
"#;
    file.write_str(content)?;
    let expected_output = r#"- What is the relationship between angles $\\alpha$ and $\\gamma_{1}$ in the picture relative to the transversal?
  card-last-interval:: 30.0
  card-repeats:: 6
  card-ease-factor:: 2.5
  card-next-schedule:: 2026-01-01T00:00:00.000Z
  card-last-reviewed:: 2025-12-02T00:00:00.000Z
  card-last-score:: 5
  https://upload.wikimedia.org/wikipedia/commons/thumb/3/3d/Transverzala_8.svg/262px-Transverzala_8.svg.png
  #card
  - They are alternate angles.
"#;
    let mut cmd = Command::cargo_bin("logseq-srs")?;

    cmd.arg("cards-in-file").arg(file.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected_output));

    Ok(())
}

#[test]
fn card_with_unicode_prompt() -> Result<(), Box<dyn std::error::Error>> {
    let file = assert_fs::NamedTempFile::new("page.md")?;
    let content = r#"\
- Not card
- Какова связь между углами $\\alpha$ и $\\gamma_{1}$ на изображении относительно секущей?
  card-last-interval:: 30.0
  card-repeats:: 6
  card-ease-factor:: 2.5
  card-next-schedule:: 2026-01-01T00:00:00.000Z
  card-last-reviewed:: 2025-12-02T00:00:00.000Z
  card-last-score:: 5
  https://upload.wikimedia.org/wikipedia/commons/thumb/3/3d/Transverzala_8.svg/262px-Transverzala_8.svg.png
  #card
  - Они накрест лежащие.
- Not card
"#;
    file.write_str(content)?;
    let expected_output = r#"- Какова связь между углами $\\alpha$ и $\\gamma_{1}$ на изображении относительно секущей?
  card-last-interval:: 30.0
  card-repeats:: 6
  card-ease-factor:: 2.5
  card-next-schedule:: 2026-01-01T00:00:00.000Z
  card-last-reviewed:: 2025-12-02T00:00:00.000Z
  card-last-score:: 5
  https://upload.wikimedia.org/wikipedia/commons/thumb/3/3d/Transverzala_8.svg/262px-Transverzala_8.svg.png
  #card
  - Они накрест лежащие.
"#;
    let mut cmd = Command::cargo_bin("logseq-srs")?;

    cmd.arg("cards-in-file").arg(file.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected_output));

    Ok(())
}

#[test]
fn single_top_level_card_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let file = assert_fs::NamedTempFile::new("page.md")?;
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
    // TODO: elide the filepath prefix, can't do an exact match otherwise
    //let expected_output = r#"CardMetadata { source_path: "/tmp/.tmpHHjPRD/page.md", prompt_fingerprint: 724424550506611259, prompt_prefix: "- What is a sphere? #card" }"#;
    let mut cmd = Command::cargo_bin("logseq-srs")?;

    cmd.arg("cards-in-file")
        .arg("--output=metadata")
        .arg(file.path());
    cmd.assert().success().stdout(predicate::str::contains(
        "prompt_fingerprint : 219dda4ed3b53642",
    ));

    Ok(())
}
