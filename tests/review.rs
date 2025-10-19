use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use anyhow::anyhow;
use serde::Serialize;

use rexpect::process::wait::WaitStatus;
use rexpect::session::PtySession;
use rexpect::session::spawn_command;

use crate::common::build_args;
use crate::common::construct_command;
use crate::common::insta_cmd_describe_program;
use crate::common::redacted_args;
use crate::common::redacted_text;

mod common;

#[derive(Serialize)]
pub struct ReviewInfo {
    program: String,
    args: Vec<String>,
    page_lines: Vec<String>,
    interaction_meta: Vec<(String, String)>,
}

impl ReviewInfo {
    fn new(cmd: &Command, page: &str, interaction_meta: Vec<(String, String)>) -> Self {
        ReviewInfo {
            program: insta_cmd_describe_program(cmd.get_program()),
            args: redacted_args(cmd),
            page_lines: page.split("\n").map(|s| s.to_owned()).collect(),
            interaction_meta,
        }
    }
}

struct TestCardReviewParams<'a> {
    args: Vec<&'a str>,
    page: &'a str,
    f: fn(&mut PtySession) -> Result<()>,
    interaction_meta: Vec<(String, String)>,
    expected_code: i32,
}

fn test_card_review_inner(params: TestCardReviewParams, snapshot_name: &str) -> Result<()> {
    let mut args: Vec<&str> = vec!["review", "$GRAPH_ROOT"];
    args.extend_from_slice(&(params.args));
    let (_graph_root, args) = build_args(&args, &[params.page])?;
    let cmd = construct_command(&args, vec![]);

    let cmd_info = &ReviewInfo::new(&cmd, params.page, params.interaction_meta);
    let mut p = spawn_command(cmd, Some(1000))?;

    (params.f)(&mut p)?;

    let status = p.process.status().ok_or(anyhow!("could not get process status"))?;
    let exit_code = match status {
        WaitStatus::Exited(_, exit_code) => exit_code,
        _ => return Err(anyhow!("expected process to exit, got {:?}", status)),
    };
    assert_eq!(
        exit_code, params.expected_code,
        "expected `losrs review` to exit with exit code {}, got {}",
        params.expected_code, exit_code
    );

    let file_raw = redacted_text(&read_solitary_page(_graph_root.path())?);
    insta::with_settings!({
        omit_expression => true,
        info => cmd_info,
    },
    {
        insta::assert_snapshot!(snapshot_name, file_raw);
    });

    Ok(())
}

macro_rules! test_card_review {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() -> Result<()> {
            let params: TestCardReviewParams = $params;
            test_card_review_inner(params, stringify!($name))
        }
    };
}

fn expect_review_interaction(p: &mut PtySession, remembered: bool) -> Result<()> {
    p.exp_string("Press any key to show the answer")?;
    p.send(" ")?;
    p.flush()?;

    p.exp_string("How much effort did recall require?")?;
    p.exp_string("1 - Little Effort; 2 - Some effort; 3 - Much Effort; 4 - Did not recall")?;
    p.send(if remembered { "2" } else { "4" })?;
    p.flush()?;

    p.read_line()?; // for the process to exit

    Ok(())
}

fn expect_no_review_interaction(p: &mut PtySession) -> Result<()> {
    p.exp_string("Reviewed all cards, huzzah!")?;

    Ok(())
}

fn expect_nope_out_interaction(p: &mut PtySession) -> Result<()> {
    p.exp_string("Ctrl+C or Esc to nope out")?;
    p.send_control('c')?;
    p.read_line()?; // for the process to exit

    Ok(())
}

fn read_solitary_page(graph_root: &Path) -> Result<String> {
    let page_paths = fs::read_dir(graph_root.join("pages"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>();
    if page_paths.len() != 1 {
        return Err(anyhow!(
            "expected {graph_root:?} to have exactly 1 page, got: {}",
            page_paths.len()
        ));
    }
    let page_path = &page_paths[0];
    Ok(fs::read_to_string(page_path)?)
}

test_card_review!(
    review_remembered_yes,
    TestCardReviewParams {
        args: vec!["--at=2025-11-22T15:04:05.123456789Z"],
        page: r#"- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_review_interaction(p, true) },
        interaction_meta: vec![
            ("expected type of interaction".to_string(), "review".to_string()),
            ("remembered".to_string(), "true".to_string())
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_remembered_no,
    TestCardReviewParams {
        args: vec!["--at=2025-11-22T15:04:05.123456789Z"],
        page: r#"- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_review_interaction(p, false) },
        interaction_meta: vec![
            ("expected type of interaction".to_string(), "review".to_string()),
            ("remembered".to_string(), "false".to_string())
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_card_without_meta_remembered_yes,
    TestCardReviewParams {
        args: vec!["--at=2025-11-22T15:04:05.123456789Z"],
        page: r#"- Not card
- What is a sphere? #card
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_review_interaction(p, true) },
        interaction_meta: vec![
            ("expected type of interaction".to_string(), "review".to_string()),
            ("remembered".to_string(), "true".to_string())
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_card_without_meta_remembered_no,
    TestCardReviewParams {
        args: vec!["--at=2025-11-22T15:04:05.123456789Z"],
        page: r#"- Not card
- What is a sphere? #card
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_review_interaction(p, false) },
        interaction_meta: vec![
            ("expected type of interaction".to_string(), "review".to_string()),
            ("remembered".to_string(), "false".to_string())
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_card_second_remembered_no,
    TestCardReviewParams {
        args: vec!["--at=2025-11-23T15:04:05.123456789Z"],
        page: r#"- Not card
- What is a sphere? #card
  card-fsrs-metadata:: {"due":"2025-11-23T15:04:05.123456789Z","stability":0.4072,"difficulty":7.2102,"elapsed_days":0,"scheduled_days":1,"reps":1,"lapses":0,"state":"Review","last_review":"2025-11-22T15:04:05.123456789Z"}
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_review_interaction(p, false) },
        interaction_meta: vec![
            ("expected type of interaction".to_string(), "review".to_string()),
            ("remembered".to_string(), "false".to_string())
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_card_not_due_early,
    TestCardReviewParams {
        args: vec!["--at=2025-03-22T09:54:57.202Z", "--up-to=2025-11-21T00:00:00.000Z"],
        page: r#"- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_review_interaction(p, true) },
        interaction_meta: vec![
            ("expected type of interaction".to_string(), "review".to_string()),
            ("remembered".to_string(), "true".to_string())
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_card_artificial_not_due,
    TestCardReviewParams {
        args: vec!["--at=2025-11-21T00:00:00.000Z", "--up-to=2025-11-20T00:00:00.000Z"],
        page: r#"- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_no_review_interaction(p) },
        interaction_meta: vec![(
            "expected type of interaction".to_string(),
            "no review".to_string()
        ),],
        expected_code: 0
    }
);

test_card_review!(
    review_card_before_last_reviewed,
    TestCardReviewParams {
        args: vec!["--at=2025-03-21T09:54:57.202Z", "--up-to=2025-11-21T00:00:00.000Z"],
        page: r#"- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> {
            p.exp_string("before it was last reviewed")?;
            Ok(())
        },
        interaction_meta: vec![(
            "expected type of interaction".to_string(),
            "no review".to_string()
        ),],
        expected_code: 1
    }
);

test_card_review!(
    review_card_not_due,
    TestCardReviewParams {
        args: vec!["--at=2024-01-01T15:04:05.123456789Z"],
        page: r#"- Not card
- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> { expect_no_review_interaction(p) },
        interaction_meta: vec![(
            "expected type of interaction".to_string(),
            "no review".to_string()
        ),],
        expected_code: 0
    }
);

test_card_review!(
    review_card_seed_0,
    TestCardReviewParams {
        args: vec!["--at=2025-09-01T15:04:05.123456789Z", "--seed=0"],
        page: r#"- Not card
- Alphabet forward cards
  - What is Gregg Simplified for "N" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward short stroke
  - What is Gregg Simplified for "M" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward long stroke
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> {
            expect_review_interaction(p, true)?;
            expect_nope_out_interaction(p)?;
            Ok(())
        },
        interaction_meta: vec![
            (
                "expected type of interaction".to_string(),
                "review first card, then nope out".to_string()
            ),
            (
                "first card with given seed".to_string(),
                r#"What is Gregg Simplified for "M" (description)?"#.to_string()
            ),
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_card_seed_100,
    TestCardReviewParams {
        args: vec!["--at=2025-09-01T15:04:05.123456789Z", "--seed=100"],
        page: r#"- Not card
- Alphabet forward cards
  - What is Gregg Simplified for "N" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward short stroke
  - What is Gregg Simplified for "M" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward long stroke
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> {
            expect_review_interaction(p, true)?;
            expect_nope_out_interaction(p)?;
            Ok(())
        },
        interaction_meta: vec![
            (
                "expected type of interaction".to_string(),
                "review first card, then nope out".to_string()
            ),
            (
                "first card with given seed".to_string(),
                r#"What is Gregg Simplified for "N" (description)?"#.to_string()
            ),
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_two_cards_seed_0,
    TestCardReviewParams {
        args: vec!["--at=2025-09-01T15:04:05.123456789Z", "--seed=0"],
        page: r#"- Not card
- Alphabet forward cards
  - What is Gregg Simplified for "N" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward short stroke
  - What is Gregg Simplified for "M" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward long stroke
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> {
            expect_review_interaction(p, true)?;
            expect_review_interaction(p, true)?;
            Ok(())
        },
        interaction_meta: vec![
            (
                "expected type of interaction".to_string(),
                "review first card, then second card".to_string()
            ),
            (
                "first card with given seed".to_string(),
                r#"What is Gregg Simplified for "M" (description)?"#.to_string()
            ),
        ],
        expected_code: 0
    }
);

test_card_review!(
    review_two_cards_seed_100,
    TestCardReviewParams {
        args: vec!["--at=2025-09-01T15:04:05.123456789Z", "--seed=100"],
        page: r#"- Not card
- Alphabet forward cards
  - What is Gregg Simplified for "N" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward short stroke
  - What is Gregg Simplified for "M" (description)? #card
    card-last-interval:: 15.0
    card-repeats:: 4
    card-ease-factor:: 1.0
    card-next-schedule:: 2025-08-12T09:03:05.489Z
    card-last-reviewed:: 2025-07-04T09:03:05.489Z
    card-last-score:: 1
    - forward long stroke
- Not card
"#,
        f: |p: &mut PtySession| -> Result<()> {
            expect_review_interaction(p, true)?;
            expect_review_interaction(p, true)?;
            Ok(())
        },
        interaction_meta: vec![
            (
                "expected type of interaction".to_string(),
                "review first card, then second card".to_string()
            ),
            (
                "first card with given seed".to_string(),
                r#"What is Gregg Simplified for "N" (description)?"#.to_string()
            ),
        ],
        expected_code: 0
    }
);

#[test]
fn newline_writeback_on_review() -> Result<()> {
    let args = vec!["review", "$GRAPH_ROOT", "--at=2025-11-22T15:04:05.123456789Z"];
    let page = r#"- What is a sphere? #card
  card-last-interval:: 244.14
  card-repeats:: 6
  card-ease-factor:: 3.1
  card-next-schedule:: 2025-11-21T00:00:00.000Z
  card-last-reviewed:: 2025-03-22T09:54:57.202Z
  card-last-score:: 5
  - Set of points in a 3 dimensional space that are equidistant from a center point.
- Not card"#;
    let (_graph_root, args) = build_args(&args, &[page])?;
    let cmd = construct_command(&args, vec![]);

    let mut p = spawn_command(cmd, Some(1000))?;

    expect_review_interaction(&mut p, true)?;

    let status = p.process.status().ok_or(anyhow!("could not get process status"))?;
    match status {
        WaitStatus::Exited(_, _) => {}
        _ => return Err(anyhow!("expected process to exit, got {:?}", status)),
    }

    let page_raw = read_solitary_page(_graph_root.path())?;
    let leading_newline_count = page_raw.chars().take_while(|&c| c == '\n').count();
    let trailing_newline_count = page_raw.chars().rev().take_while(|&c| c == '\n').count();

    assert_eq!(leading_newline_count, 0, "expect 0 leading newlines when file is just one card");
    assert_eq!(trailing_newline_count, 1, "expect 1 trailing newline when file is just one card");

    Ok(())
}

// TODO: test review outside graph root
// TODO: test review when serial number already assigned
// TODO: test review when serial number has been initialized
// TODO: display serial number via metadata command
