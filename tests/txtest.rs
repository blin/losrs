use anyhow::Context;
use anyhow::Result;
use insta_cmd::get_cargo_bin;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

pub use dissimilar::diff as __diff;
use txtar::Archive;

mod common;

pub fn format_diff(chunks: Vec<dissimilar::Chunk>) -> String {
    let mut buf = String::new();
    for chunk in chunks {
        let formatted = match chunk {
            dissimilar::Chunk::Equal(text) => text.into(),
            dissimilar::Chunk::Delete(text) => format!("\x1b[41m{}\x1b[0m", text),
            dissimilar::Chunk::Insert(text) => format!("\x1b[42m{}\x1b[0m", text),
        };
        buf.push_str(&formatted);
    }
    buf
}

// Copied from https://github.com/rust-lang/rust-analyzer/blob/1dbdac8f518e5d3400a0bbc0478a606ab70d8a44/crates/test_utils/src/lib.rs#L38
macro_rules! assert_eq_text {
    ($left:expr, $right:expr) => {
        assert_eq_text!($left, $right,)
    };
    ($left:expr, $right:expr, $($tt:tt)*) => {{
        let left = $left;
        let right = $right;
        if left != right {
            if left.trim() == right.trim() {
                std::eprintln!("Left:\n{:?}\n\nRight:\n{:?}\n\nWhitespace difference\n", left, right);
            } else {
                let diff = $crate::__diff(left, right);
                std::eprintln!("Left:\n{}\n\nRight:\n{}\n\nDiff:\n{}\n", left, right, $crate::format_diff(diff));
            }
            std::eprintln!($($tt)*);
            panic!("text differs");
        }
    }};
}

#[derive(Debug)]
struct RunLosrs {
    action_args: Vec<String>,
    action_envs: Vec<(String, String)>,
    expected_stdout: String,
}

impl RunLosrs {
    fn from_steps_dir(d: &Path, i: i32) -> Self {
        let action_args = step_data(d, i, "action_args").unwrap();
        let action_envs = step_data(d, i, "action_envs");
        let expected_stdout = step_data(d, i, "expected_stdout").unwrap();
        RunLosrs {
            action_args: action_args.split_whitespace().map(|s| s.to_string()).collect(),
            action_envs: action_envs
                .map(|envs_raw| {
                    envs_raw
                        .split_whitespace()
                        .map(|l| l.split_once("=").unwrap())
                        .map(|(a, b)| (a.to_string(), b.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            expected_stdout,
        }
    }

    fn perform_step_in(&self, graph_root: &Path) -> Result<()> {
        let mut cmd = Command::new(get_cargo_bin("losrs"));

        let updated_args = self
            .action_args
            .iter()
            .map(|arg| arg.replace("$GRAPH_ROOT", graph_root.to_str().unwrap()));

        cmd.args(updated_args);
        cmd.envs(
            self.action_envs
                .iter()
                .map(|(a, b)| (a.as_ref(), b.as_ref()))
                .collect::<Vec<(&str, &str)>>(),
        );
        let output = cmd.output().unwrap();
        let actual_stdout = common::redacted_text(&String::from_utf8_lossy(&output.stdout));
        assert_eq_text!(&self.expected_stdout, &actual_stdout);
        Ok(())
    }
}

#[derive(Debug)]
enum Action {
    RunLosrs(RunLosrs),
}

#[derive(Debug)]
struct Step {
    action: Action,
}

fn step_data(d: &Path, i: i32, name: &str) -> Option<String> {
    let p = d.join(format!("{:02}_{}", i, name));
    if !p.exists() {
        return None;
    }
    Some(fs::read_to_string(p).unwrap())
}

fn process_test_archive(archive: Archive) -> Result<(TempDir, Vec<Step>)> {
    let graph_root = TempDir::new()?;
    archive.materialize(graph_root.path())?;
    let steps_dir_path = graph_root.path().join("steps");
    if !steps_dir_path.is_dir() {
        panic!("aaa1")
    }

    let mut steps: Vec<Step> = Vec::new();
    for i in 1..10 {
        let Some(action_name) = step_data(&steps_dir_path, i, "action_name") else {
            break;
        };

        let action: Action = match action_name.trim_end() {
            "RunLosrs" => Action::RunLosrs(RunLosrs::from_steps_dir(&steps_dir_path, i)),
            _ => panic!("aaa2"),
        };
        steps.push(Step { action });
    }

    Ok((graph_root, steps))
}

fn perform_step_in(step: &Step, graph_root: &Path) -> Result<()> {
    match &step.action {
        Action::RunLosrs(run_losrs) => {
            run_losrs.perform_step_in(graph_root)?;
        }
    }
    Ok(())
}

fn test_file_inner(file_name: &str) -> Result<()> {
    let path = PathBuf::from(format!("{}/tests/cases/{}", env!("CARGO_MANIFEST_DIR"), file_name));
    let archive = txtar::from_str(
        &fs::read_to_string(&path)
            .with_context(|| format!("when trying to read {}", (&path).display()))?,
    );

    let (graph_root, steps) = process_test_archive(archive)?;
    for step in steps {
        perform_step_in(&step, graph_root.path())?;
    }

    Ok(())
}

macro_rules! test_file {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() -> Result<()> {
            test_file_inner($file)
        }
    };
}

test_file!(root_help, "root_help.txtar");

test_file!(show_help, "show_help.txtar");
test_file!(show_format_clean, "show_format_clean.txtar");
test_file!(show_format_typst, "show_format_typst.txtar");
test_file!(show_format_storage, "show_format_storage.txtar");
test_file!(
    show_format_storage_card_without_metadata,
    "show_format_storage_card_without_metadata.txtar"
);
test_file!(
    show_format_storage_card_with_fsrs_metadata,
    "show_format_storage_card_with_fsrs_metadata.txtar"
);
test_file!(
    show_format_storage_card_with_reordered_metadata,
    "show_format_storage_card_with_reordered_metadata.txtar"
);
test_file!(show_card_with_data_after_metadata, "show_card_with_data_after_metadata.txtar");
test_file!(show_card_with_unicode_prompt, "show_card_with_unicode_prompt.txtar");
test_file!(show_with_fingerprint, "show_with_fingerprint.txtar");
test_file!(show_multiple_page_files, "show_multiple_page_files.txtar");
test_file!(show_card_is_deeply_nested, "show_card_is_deeply_nested.txtar");
test_file!(
    show_format_storage_card_is_deeply_nested,
    "show_format_storage_card_is_deeply_nested.txtar"
);

test_file!(metadata_help, "metadata_help.txtar");
test_file!(metadata, "metadata.txtar");

test_file!(review_help, "review_help.txtar");

test_file!(config_help, "config_help.txtar");
// test_file!(config_show, "config_show.txtar");
