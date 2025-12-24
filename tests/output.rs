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
