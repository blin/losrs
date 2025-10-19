use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;

use insta_cmd::get_cargo_bin;
use regex::Regex;

// TempDir uses https://docs.rs/fastrand/latest/fastrand/struct.Rng.html#method.alphanumeric
// This regex only matches on unix paths,
// will need to do something else if anyone ever runs these tests on Windows.
static TMP_DIR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/.*?\.tmp[a-zA-Z0-9]{6}").unwrap());

pub fn redacted_args(cmd: &Command) -> Vec<String> {
    cmd.get_args()
        .map(|x| TMP_DIR_RE.replace_all(&x.to_string_lossy(), "[TMP_DIR]").to_string())
        .collect()
}

pub fn redacted_text(out: &str) -> String {
    TMP_DIR_RE.replace_all(out, "[TMP_DIR]").to_string()
}

pub fn build_args(args: &[&str], pages: &[&str]) -> Result<(tempfile::TempDir, Vec<String>)> {
    let graph_root = construct_graph_root(pages)?;

    let mut final_args: Vec<String> = Vec::new();

    let config_path = graph_root.path().join("losrs.toml");
    std::fs::File::create(&config_path)?;
    final_args.push(format!("--config={}", config_path.display()));

    let updated_args =
        args.iter().map(|arg| arg.replace("$GRAPH_ROOT", graph_root.path().to_str().unwrap()));
    final_args.extend(updated_args);

    Ok((graph_root, final_args))
}

fn construct_graph_root(pages: &[&str]) -> Result<tempfile::TempDir> {
    let graph_root = tempfile::TempDir::new()?;
    let pages_dir = graph_root.path().join("pages");
    std::fs::create_dir(pages_dir.as_path())?;

    pages.iter().enumerate().for_each(|(idx, page)| {
        fs::write(pages_dir.join(format!("{}.md", idx)), page)
            .expect("expect temp page writes to succeed")
    });

    Ok(graph_root)
}

pub fn construct_command<I, S>(args: I, envs: Vec<(&str, &str)>) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new(get_cargo_bin("losrs"));
    cmd.args(args).envs(envs);
    cmd
}

// Extracted from private function
// https://github.com/mitsuhiko/insta-cmd/blob/0.6.0/src/spawn.rs#L22-L30
pub fn insta_cmd_describe_program(cmd: &std::ffi::OsStr) -> String {
    let filename = Path::new(cmd).file_name().unwrap();
    let name = filename.to_string_lossy();
    let name = &name as &str;
    name.into()
}
