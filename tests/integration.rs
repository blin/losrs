use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use rexpect::process::wait::WaitStatus;
use rexpect::session::PtySession;
use rexpect::session::spawn_command;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;
use test_utils::assert_eq_text;
use test_utils::cargo_bin;
use test_utils::redacted_text;

fn read_action_attribute(d: &Path, i: i32, attr: &str) -> Option<String> {
    let p = d.join(format!("{:02}_{}", i, attr));
    if !p.exists() {
        return None;
    }
    Some(fs::read_to_string(p).unwrap())
}

fn read_action_args(d: &Path, i: i32) -> Vec<String> {
    read_action_attribute(d, i, "action_args")
        .unwrap()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

fn read_action_envs(d: &Path, i: i32) -> Vec<(String, String)> {
    read_action_attribute(d, i, "action_envs")
        .map(|envs_raw| {
            envs_raw
                .split_whitespace()
                .map(|l| l.split_once("=").unwrap())
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn read_expected_exit_code(d: &Path, i: i32) -> i32 {
    read_action_attribute(d, i, "expected_exit_code")
        .map(|s| s.trim_end().parse::<i32>().unwrap())
        .unwrap_or(0)
}

fn read_review_actions(d: &Path, i: i32) -> Vec<ReviewAction> {
    read_action_attribute(d, i, "review_actions")
        .unwrap()
        .split("\n")
        .filter(|s| !s.is_empty())
        .map(|l| parse_review_action(l))
        .collect::<Result<Vec<ReviewAction>>>()
        .unwrap()
}

#[derive(Debug)]
struct RunLosrs {
    action_args: Vec<String>,
    action_envs: Vec<(String, String)>,
    expected_stdout: String,
}

fn build_args_in(graph_root: &Path, action_args: &Vec<String>) -> Vec<String> {
    let mut final_args: Vec<String> = Vec::new();

    let config_path = graph_root.join("losrs.toml");
    if config_path.exists() {
        final_args.push(format!("--config={}", config_path.display()));
    }

    let updated_args =
        action_args.iter().map(|arg| arg.replace("$GRAPH_ROOT", graph_root.to_str().unwrap()));
    final_args.extend(updated_args);
    final_args
}

impl RunLosrs {
    fn from_actions_dir(d: &Path, i: i32) -> Self {
        RunLosrs {
            action_args: read_action_args(d, i),
            action_envs: read_action_envs(d, i),
            expected_stdout: read_action_attribute(d, i, "expected_stdout").unwrap(),
        }
    }

    fn perform_action_in(&self, graph_root: &Path) -> Result<()> {
        let mut cmd = Command::new(cargo_bin!("losrs"));

        cmd.args(build_args_in(graph_root, &self.action_args));

        cmd.envs(self.action_envs.iter().map(|(k, v)| (k, v)));

        let output = cmd.output().unwrap();
        let actual_stdout = redacted_text(&String::from_utf8_lossy(&output.stdout));
        assert_eq_text!(&self.expected_stdout, &actual_stdout);
        Ok(())
    }
}

#[derive(Debug)]
enum ReviewAction {
    ExpString(String),
    Send(String),
    SendControl(char),
    Flush,
    ReadLine,
}

impl ReviewAction {
    fn perform(&self, p: &mut PtySession) -> Result<()> {
        match self {
            ReviewAction::ExpString(s) => {
                p.exp_string(s)?;
            }
            ReviewAction::Send(s) => {
                p.send(s)?;
            }
            ReviewAction::SendControl(c) => {
                p.send_control(*c)?;
            }
            ReviewAction::Flush => {
                p.flush()?;
            }
            ReviewAction::ReadLine => {
                p.read_line()?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct RunLosrsReview {
    action_args: Vec<String>,
    action_envs: Vec<(String, String)>,
    review_actions: Vec<ReviewAction>,
    expected_exit_code: i32,
}

fn parse_review_action(line: &str) -> Result<ReviewAction> {
    let Some((review_action_name, review_action_args)) = line.split_once(",") else {
        return Err(anyhow!("no comma found for review action line: {}", line));
    };
    let review_action_args = review_action_args.trim_end_matches(|x| x == '\n');

    match review_action_name {
        "exp_string" => Ok(ReviewAction::ExpString(review_action_args.to_owned())),
        "send" => Ok(ReviewAction::Send(review_action_args.to_owned())),
        "send_control" => {
            let c = review_action_args.chars().next().unwrap();
            Ok(ReviewAction::SendControl(c))
        }
        "flush" => Ok(ReviewAction::Flush),
        "read_line" => Ok(ReviewAction::ReadLine),
        _ => Err(anyhow!("unknown review action: {}", review_action_name)),
    }
}

impl RunLosrsReview {
    fn from_actions_dir(d: &Path, i: i32) -> Self {
        RunLosrsReview {
            action_args: read_action_args(d, i),
            action_envs: read_action_envs(d, i),
            review_actions: read_review_actions(d, i),
            expected_exit_code: read_expected_exit_code(d, i),
        }
    }

    fn perform_action_in(&self, graph_root: &Path) -> Result<()> {
        let mut cmd = Command::new(cargo_bin!("losrs"));

        cmd.args(build_args_in(graph_root, &self.action_args));

        cmd.envs(self.action_envs.iter().map(|(k, v)| (k, v)));

        let mut p = spawn_command(cmd, Some(1000))?;

        for review_action in &self.review_actions {
            review_action
                .perform(&mut p)
                .with_context(|| format!("while trying to execute action={:?}", review_action))?;
        }

        let status = p.process.status().ok_or(anyhow!("could not get process status"))?;
        let exit_code = match status {
            WaitStatus::Exited(_, exit_code) => exit_code,
            _ => return Err(anyhow!("expected process to exit, got {:?}", status)),
        };
        assert_eq!(
            exit_code, self.expected_exit_code,
            "expected `losrs review` to exit with exit code {}, got {}",
            self.expected_exit_code, exit_code
        );

        Ok(())
    }
}

#[derive(Debug)]
enum Action {
    RunLosrs(RunLosrs),
    RunLosrsReview(RunLosrsReview),
}

fn parse_actions(graph_root: &Path) -> Result<Vec<Action>> {
    let actions_dir = graph_root.join("actions");
    if !actions_dir.is_dir() {
        return Err(anyhow!("no actions directory"));
    }

    let mut actions: Vec<Action> = Vec::new();
    for i in 1..10 {
        let Some(action_name) =
            read_action_attribute(&actions_dir, i, "action_name").map(|x| x.trim_end().to_owned())
        else {
            break;
        };

        let action: Action = match action_name.as_str() {
            "RunLosrs" => Action::RunLosrs(RunLosrs::from_actions_dir(&actions_dir, i)),
            "RunLosrsReview" => {
                Action::RunLosrsReview(RunLosrsReview::from_actions_dir(&actions_dir, i))
            }
            _ => panic!("Unexpected action name: {}", action_name),
        };
        actions.push(action);
    }

    Ok(actions)
}

fn perform_action_in(action: &Action, graph_root: &Path) -> Result<()> {
    match action {
        Action::RunLosrs(run_losrs) => {
            run_losrs.perform_action_in(graph_root)?;
        }
        Action::RunLosrsReview(run_losrs_review) => {
            run_losrs_review.perform_action_in(graph_root)?;
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

    let graph_root = TempDir::new()?;
    archive.materialize(graph_root.path())?;

    let actions = parse_actions(graph_root.path())?;
    for action in actions {
        perform_action_in(&action, graph_root.path())?;
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
test_file!(show_card_is_deeply_nested, "show_card_is_deeply_nested.txtar");
test_file!(show_card_with_data_after_metadata, "show_card_with_data_after_metadata.txtar");
test_file!(show_card_with_unicode_prompt, "show_card_with_unicode_prompt.txtar");
test_file!(show_format_clean, "show_format_clean.txtar");
test_file!(show_format_storage, "show_format_storage.txtar");
test_file!(
    show_format_storage_card_is_deeply_nested,
    "show_format_storage_card_is_deeply_nested.txtar"
);
test_file!(
    show_format_storage_card_with_fsrs_metadata,
    "show_format_storage_card_with_fsrs_metadata.txtar"
);
test_file!(
    show_format_storage_card_with_reordered_metadata,
    "show_format_storage_card_with_reordered_metadata.txtar"
);
test_file!(
    show_format_storage_card_without_metadata,
    "show_format_storage_card_without_metadata.txtar"
);
test_file!(show_format_typst, "show_format_typst.txtar");
test_file!(show_multiple_page_files, "show_multiple_page_files.txtar");
test_file!(show_with_fingerprint, "show_with_fingerprint.txtar");

test_file!(metadata_help, "metadata_help.txtar");
test_file!(metadata, "metadata.txtar");

test_file!(config_help, "config_help.txtar");
test_file!(config_show, "config_show.txtar");
test_file!(config_show_with_env_override, "config_show_with_env_override.txtar");

test_file!(review_help, "review_help.txtar");

test_file!(review_card_artificial_not_due, "review_card_artificial_not_due.txtar");
test_file!(review_card_before_last_reviewed, "review_card_before_last_reviewed.txtar");
test_file!(review_card_not_due, "review_card_not_due.txtar");
test_file!(review_card_not_due_early, "review_card_not_due_early.txtar");
test_file!(review_card_second_remembered_no, "review_card_second_remembered_no.txtar");
test_file!(review_card_seed_0, "review_card_seed_0.txtar");
test_file!(review_card_seed_100, "review_card_seed_100.txtar");
test_file!(review_card_without_meta_remembered_no, "review_card_without_meta_remembered_no.txtar");
test_file!(
    review_card_without_meta_remembered_yes,
    "review_card_without_meta_remembered_yes.txtar"
);
test_file!(review_remembered_no, "review_remembered_no.txtar");
test_file!(review_remembered_yes, "review_remembered_yes.txtar");
test_file!(
    review_remembered_yes_csn_assigned_not_first,
    "review_remembered_yes_csn_assigned_not_first.txtar"
);
test_file!(
    review_remembered_yes_csn_not_assigned_not_first,
    "review_remembered_yes_csn_not_assigned_not_first.txtar"
);
test_file!(review_two_cards_seed_0, "review_two_cards_seed_0.txtar");
test_file!(review_two_cards_seed_100, "review_two_cards_seed_100.txtar");

// TODO: add a subcommand for serial number manipulation?
