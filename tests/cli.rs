use assert_fs::prelude::FileWriteStr;
use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
use std::process::Command;

fn cli() -> Command {
    Command::new(get_cargo_bin("logseq-srs"))
}

macro_rules! test_card_output {
    ($name:ident, $cmd:expr, $args:expr, $content:expr ) => {
        #[test]
        fn $name() {
            let file = assert_fs::NamedTempFile::new("page.md").unwrap();
            file.write_str($content).unwrap();

            insta::with_settings!({
                filters => vec![
                    (r"/tmp/.tmp\w+/", "[TMP_DIR]/"),
                ],
            },
            {
                assert_cmd_snapshot!(cli().arg($cmd).arg(file.path()).args($args));
            });
        }
    };
}

test_card_output!(
    single_top_level_card,
    "show",
    Vec::<String>::new(),
    r#"\- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#
);

test_card_output!(
    single_top_level_card_clean,
    "show",
    vec!["--format=clean"],
    r#"\- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#
);

test_card_output!(
    card_with_data_after_metadata,
    "show",
    Vec::<String>::new(),
    r#"\- Not card
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
"#
);

test_card_output!(
    card_with_unicode_prompt,
    "show",
    Vec::<String>::new(),
    r#"\- Not card
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
"#
);

test_card_output!(
    single_top_level_card_metadata,
    "metadata",
    Vec::<String>::new(),
    r#"\- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#
);

test_card_output!(
    one_of_top_level_cards,
    "show",
    vec!["0x219dda4ed3b53642"],
    r#"\- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- What is the volume of a sphere (symbolic)? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-27T00:00:00.000Z
  card-last-reviewed:: 2025-03-28T07:46:41.223Z
  card-last-score:: 5
  - $$V = \frac{4}{3} \pi r^3$$
- Not card
"#
);
