use std::path::Path;
use std::process::Command;

use serde::Serialize;

use assert_fs::prelude::FileWriteStr;
use insta_cmd::get_cargo_bin;

#[derive(Serialize)]
pub struct Info {
    program: String,
    args: Vec<String>,
    page_lines: Vec<String>,
}

impl Info {
    fn new(cmd: &Command, page: &str) -> Self {
        Info {
            program: insta_cmd_describe_program(cmd.get_program()),
            args: cmd.get_args().map(|x| x.to_string_lossy().into_owned()).collect(),
            page_lines: page.split("\n").map(|s| s.to_owned()).collect(),
        }
    }
}

// Extracted from macros
// https://github.com/mitsuhiko/insta-cmd/blob/0.6.0/src/macros.rs#L11-L17
fn insta_cmd_format_output(output: std::process::Output) -> String {
    format!(
        "success: {:?}\nexit_code: {}\n----- stdout -----\n{}\n----- stderr -----\n{}",
        output.status.success(),
        output.status.code().unwrap_or(!0),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

// Extracted from private function
// https://github.com/mitsuhiko/insta-cmd/blob/0.6.0/src/spawn.rs#L22-L30
fn insta_cmd_describe_program(cmd: &std::ffi::OsStr) -> String {
    let filename = Path::new(cmd).file_name().unwrap();
    let name = filename.to_string_lossy();
    let name = &name as &str;
    name.into()
}

macro_rules! test_card_output {
    ($name:ident, $subcommand:expr, $args:expr, $page:expr ) => {
        #[test]
        fn $name() {
            // TODO: replace with tempfile::NamedTempFile to have one fewer dependency
            let file = assert_fs::NamedTempFile::new("page.md").unwrap();
            file.write_str($page).unwrap();

            let mut cmd = Command::new(get_cargo_bin("logseq-srs"));
            cmd.arg($subcommand).arg(file.path()).args($args);
            let output = cmd.output().unwrap();

            insta::with_settings!({
                omit_expression => true,
                info => &Info::new(&cmd, $page),
                filters => vec![
                    (r"/tmp/.tmp\w+/", "[TMP_DIR]/"),
                ],
            },
            {
                insta::assert_snapshot!(insta_cmd_format_output(output));
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
    single_top_level_card_typst,
    "show",
    vec!["--format=typst"],
    r#"\- Not card
- What is the antiderivative of $f(x) = x^r$ (symbolic)? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-12-28T00:00:00.000Z
  card-last-reviewed:: 2025-04-28T09:12:30.985Z
  card-last-score:: 5
  - $$\int{x^r dx} = \frac{x^{(r+1)}}{r+1} + C$$
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
