use std::process::Command;

use anyhow::Result;
use anyhow::anyhow;
use assert_fs::prelude::FileWriteStr;
use insta_cmd::get_cargo_bin;
use rexpect::process::wait::WaitStatus;
use rexpect::session::spawn_command;

#[test]
fn review_flow() -> Result<()> {
    let page_contents = r#"- Not card
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
    let file = assert_fs::NamedTempFile::new("page.md").unwrap();
    file.write_str(page_contents).unwrap();

    let mut cmd = Command::new(get_cargo_bin("logseq-srs"));
    cmd.arg("review").arg(file.path());

    println!("{:?}", cmd);
    let mut p = spawn_command(cmd, Some(1000))?;

    p.exp_string("Press any key to show the answer")?;
    p.send(" ")?;
    p.flush()?;

    p.exp_string("Remembered?")?;
    p.send("y")?;
    p.flush()?;

    p.exp_string("Press any key to continue")?;
    p.send(" ")?;
    p.flush()?;

    p.read_line()?; // for the process to exit

    let status = p.process.status().ok_or(anyhow!("could not get process status"))?;
    match status {
        WaitStatus::Exited(_, _) => return Ok(()),
        _ => return Err(anyhow!("expected process to exit, got {:?}", status)),
    }
}
