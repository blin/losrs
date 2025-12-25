use anyhow::Context;
use anyhow::Result;
use insta_cmd::get_cargo_bin;
use rexpect::session::PtySession;
use rexpect::session::spawn_command;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;
use test_utils::assert_eq_text;
use txtar::Archive;

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

        let mut final_args: Vec<String> = Vec::new();

        let config_path = graph_root.join("losrs.toml");
        if config_path.exists() {
            final_args.push(format!("--config={}", config_path.display()));
        }

        let updated_args = self
            .action_args
            .iter()
            .map(|arg| arg.replace("$GRAPH_ROOT", graph_root.to_str().unwrap()));
        final_args.extend(updated_args);

        cmd.args(final_args);
        cmd.envs(
            self.action_envs
                .iter()
                .map(|(a, b)| (a.as_ref(), b.as_ref()))
                .collect::<Vec<(&str, &str)>>(),
        );
        let output = cmd.output().unwrap();
        let actual_stdout = test_utils::redacted_text(&String::from_utf8_lossy(&output.stdout));
        assert_eq_text!(&self.expected_stdout, &actual_stdout);
        Ok(())
    }
}

#[derive(Debug)]
struct ReviewAction {
    name: String,
    args: String,
}

impl ReviewAction {
    fn dodo(&self, p: &mut PtySession) -> Result<()> {
        match self.name.as_str() {
            "exp_string" => {
                p.exp_string(&self.args)?;
            }
            "send" => {
                p.send(&self.args)?;
            }
            "flush" => {
                p.flush()?;
            }
            "read_line" => {
                p.read_line()?;
            }
            _ => panic!("aaa7"),
        }
        Ok(())
    }
}

#[derive(Debug)]
struct RunLosrsReview {
    action_args: Vec<String>,
    action_envs: Vec<(String, String)>,
    review_actions: Vec<ReviewAction>,
}

fn parse_review_action(line: &str) -> Result<ReviewAction> {
    let Some((review_action_name, review_action_args)) = line.split_once(",") else {
        panic!("no comma found for review action line: {}", line)
    };
    Ok(ReviewAction {
        name: review_action_name.trim_end().to_owned(),
        args: review_action_args.trim_end_matches(|x| x == '\n').to_owned(),
    })
}

impl RunLosrsReview {
    fn from_steps_dir(d: &Path, i: i32) -> Self {
        let action_args = step_data(d, i, "action_args").unwrap();
        let action_envs = step_data(d, i, "action_envs");
        let review_actions = step_data(d, i, "review_actions").unwrap();
        let review_actions: Vec<ReviewAction> = review_actions
            .split("\n")
            .filter(|s| !s.is_empty())
            .map(|l| parse_review_action(l))
            .collect::<Result<Vec<ReviewAction>>>()
            .unwrap();
        RunLosrsReview {
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
            review_actions,
        }
    }

    fn perform_step_in(&self, graph_root: &Path) -> Result<()> {
        let mut cmd = Command::new(get_cargo_bin("losrs"));

        let mut final_args: Vec<String> = Vec::new();

        let config_path = graph_root.join("losrs.toml");
        if config_path.exists() {
            final_args.push(format!("--config={}", config_path.display()));
        }

        let updated_args = self
            .action_args
            .iter()
            .map(|arg| arg.replace("$GRAPH_ROOT", graph_root.to_str().unwrap()));
        final_args.extend(updated_args);

        cmd.args(final_args);
        cmd.envs(
            self.action_envs
                .iter()
                .map(|(a, b)| (a.as_ref(), b.as_ref()))
                .collect::<Vec<(&str, &str)>>(),
        );
        let mut p = spawn_command(cmd, Some(1000))?;

        for review_action in &self.review_actions {
            review_action.dodo(&mut p).with_context(|| {
                format!(
                    "while trying to execute action={},args={}",
                    review_action.name, review_action.args
                )
            })?;
        }

        Ok(())
    }
}

#[derive(Debug)]
enum Action {
    RunLosrs(RunLosrs),
    RunLosrsReview(RunLosrsReview),
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
        let action_name = action_name.trim_end();

        let action: Action = match action_name {
            "RunLosrs" => Action::RunLosrs(RunLosrs::from_steps_dir(&steps_dir_path, i)),
            "RunLosrsReview" => {
                Action::RunLosrsReview(RunLosrsReview::from_steps_dir(&steps_dir_path, i))
            }
            _ => panic!("Unexpected action name: {}", action_name),
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
        Action::RunLosrsReview(run_losrs_review) => {
            run_losrs_review.perform_step_in(graph_root)?;
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


test_file!(config_help, "config_help.txtar");
test_file!(config_show, "config_show.txtar");
test_file!(config_show_with_env_override, "config_show_with_env_override.txtar");

test_file!(review_help, "review_help.txtar");
test_file!(review_remembered_yes, "review_remembered_yes.txtar");
