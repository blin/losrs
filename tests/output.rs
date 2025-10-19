use std::collections::HashMap;
use std::process::Command;

use anyhow::Result;
use serde::Serialize;

use crate::common::build_args;
use crate::common::construct_command;
use crate::common::insta_cmd_describe_program;
use crate::common::redacted_args;
use crate::common::redacted_text;

mod common;

#[derive(Serialize)]
pub struct Info {
    program: String,
    args: Vec<String>,
    envs: HashMap<String, Option<String>>,
    page_lines: Vec<Vec<String>>,
}

impl Info {
    fn new(cmd: &Command, pages: Vec<&str>) -> Self {
        Info {
            program: insta_cmd_describe_program(cmd.get_program()),
            args: redacted_args(cmd),
            envs: cmd
                .get_envs()
                .map(|(k, v)| {
                    (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))
                })
                .collect(),
            page_lines: pages
                .iter()
                .map(|p| p.split("\n").map(|s| s.to_owned()).collect())
                .collect(),
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
        redacted_text(&String::from_utf8_lossy(&output.stdout)),
        redacted_text(&String::from_utf8_lossy(&output.stderr)),
    )
}

struct TestCardOutputParams<'a> {
    args: Vec<&'a str>,
    envs: Vec<(&'a str, &'a str)>,
    pages: Vec<&'a str>,
}

fn test_card_output_inner(params: TestCardOutputParams, snapshot_name: &str) -> Result<()> {
    let (_graph_root, args) = build_args(&params.args, &params.pages)?;

    let mut cmd = construct_command(args, params.envs);

    let output = cmd.output().unwrap();

    insta::with_settings!({
        omit_expression => true,
        info => &Info::new(&cmd, params.pages),
    },
    {
        insta::assert_snapshot!(snapshot_name, insta_cmd_format_output(output));
    });
    Ok(())
}

macro_rules! test_card_output {
    ($name:ident, $params:expr ) => {
        #[test]
        fn $name() -> Result<()> {
            test_card_output_inner($params, stringify!($name))
        }
    };
}

test_card_output!(
    root_help,
    TestCardOutputParams { args: vec!["--help"], envs: vec![], pages: vec![""] }
);

test_card_output!(
    show_help,
    TestCardOutputParams { args: vec!["show", "--help"], envs: vec![], pages: vec![""] }
);

test_card_output!(
    show,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT",],
        envs: vec![],
        pages: vec![
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    show_format_clean,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "clean")],
        pages: vec![
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    show_format_typst,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "typst")],
        pages: vec![
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    show_format_storage,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "storage")],
        pages: vec![
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    show_format_storage_card_without_metadata,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "storage")],
        pages: vec![
            r#"- Not card
- What is the antiderivative of $f(x) = x^r$ (symbolic)? #card
  - $$\int{x^r dx} = \frac{x^{(r+1)}}{r+1} + C$$
- Not card
"#
        ]
    }
);

test_card_output!(
    show_format_storage_card_with_fsrs_metadata,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "storage")],
        pages: vec![
            r#"- Not card
- What is a sphere? #card
  card-last-interval:: 9.0
  card-repeats:: 7
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-12-01T15:04:05.123456789Z
  card-last-reviewed:: 2025-11-22T15:04:05.123456789Z
  card-last-score:: 5
  card-fsrs-metadata:: {"due":"2025-12-01T15:04:05.123456789Z","stability":8.774341658142419,"difficulty":7.040172161986166,"elapsed_days":245,"scheduled_days":9,"reps":7,"lapses":1,"state":"Review","last_review":"2025-11-22T15:04:05.123456789Z"}
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#
        ]
    }
);

test_card_output!(
    show_format_storage_card_with_reordered_metadata,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "storage")],
        pages: vec![
            r#"- Not card
- What is the antiderivative of $f(x) = x^r$ (symbolic)? #card
  card-last-reviewed:: 2025-04-28T09:12:30.985Z
  card-last-interval:: 244.14
  card-ease-factor:: 3.1
  card-last-score:: 5
  card-repeats:: 6
  card-next-schedule:: 2025-12-28T00:00:00.000Z
  - $$\int{x^r dx} = \frac{x^{(r+1)}}{r+1} + C$$
- Not card
"#
        ]
    }
);

test_card_output!(
    show_card_with_data_after_metadata,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT",],
        envs: vec![],
        pages: vec![
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    show_card_with_unicode_prompt,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT",],
        envs: vec![],
        pages: vec![
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    metadata_help,
    TestCardOutputParams { args: vec!["metadata", "--help"], envs: vec![], pages: vec![""] }
);

test_card_output!(
    metadata,
    TestCardOutputParams {
        args: vec!["metadata", "$GRAPH_ROOT"],
        envs: vec![],
        pages: vec![
            r#"- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- What is the volume of a sphere (symbolic)? #card <!-- CSN:5 -->
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-27T00:00:00.000Z
  card-last-reviewed:: 2025-03-28T07:46:41.223Z
  card-last-score:: 5
  - $$V = \frac{4}{3} \pi r^3$$
- Not card
"#
        ]
    }
);

test_card_output!(
    show_with_fingerprint,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT", "0xb9de554a02212aca"],
        envs: vec![],
        pages: vec![
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    show_multiple_page_files,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT",],
        envs: vec![],
        pages: vec![
            r#"- Not card
- What is the volume of a sphere (symbolic)? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-27T00:00:00.000Z
  card-last-reviewed:: 2025-03-28T07:46:41.223Z
  card-last-score:: 5
  - $$V = \frac{4}{3} \pi r^3$$
- Not card
"#,
            r#"- Not card
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
        ]
    }
);

test_card_output!(
    show_card_is_deeply_nested,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT",],
        envs: vec![],
        pages: vec![
            r#"- Not card
- induction
  - Automatically generated induction principles for Inductive Types (in general)
    - What kind of function is a generated induction principle function (similarity, not implimentation)? #card
      card-last-score:: 5
      card-repeats:: 6
      card-next-schedule:: 2026-01-25T00:00:00.000Z
      card-last-interval:: 244.14
      card-ease-factor:: 3.5
      card-last-reviewed:: 2025-05-26T09:11:31.735Z
      - Fixpoint
- Not card
"#,
        ]
    }
);

test_card_output!(
    show_format_storage_card_is_deeply_nested,
    TestCardOutputParams {
        args: vec!["show", "$GRAPH_ROOT"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "storage")],
        pages: vec![
            r#"- Not card
- induction
  - Automatically generated induction principles for Inductive Types (in general)
    - What kind of function is a generated induction principle function (similarity, not implimentation)? #card
      card-last-score:: 5
      card-repeats:: 6
      card-next-schedule:: 2026-01-25T00:00:00.000Z
      card-last-interval:: 244.14
      card-ease-factor:: 3.5
      card-last-reviewed:: 2025-05-26T09:11:31.735Z
      - Fixpoint
- Not card
"#,
        ]
    }
);

test_card_output!(
    review_help,
    TestCardOutputParams { args: vec!["review", "--help"], envs: vec![], pages: vec![""] }
);

test_card_output!(
    config_help,
    TestCardOutputParams { args: vec!["config", "--help"], envs: vec![], pages: vec![""] }
);

test_card_output!(
    config_show,
    TestCardOutputParams { args: vec!["config", "show"], envs: vec![], pages: vec![""] }
);

test_card_output!(
    config_show_with_env_override,
    TestCardOutputParams {
        args: vec!["config", "show"],
        envs: vec![("LOSRS__OUTPUT__FORMAT", "sixel")],
        pages: vec![""]
    }
);
