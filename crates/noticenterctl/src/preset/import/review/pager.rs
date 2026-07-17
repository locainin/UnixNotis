//! Secure interactive paging for executable-content reviews

use anyhow::{Context, Result};
use std::io::{ErrorKind, Write};
use std::process::Stdio;

use crate::system_tools;

pub(in crate::preset) fn page_exec_content_review(review: &str) -> Result<bool> {
    let mut command = match system_tools::command("less") {
        Ok(command) => command,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("resolve trusted executable review pager"),
    };

    // A fixed pager and fixed arguments keep preset content away from a command shell
    command
        .arg("-R")
        .arg("--")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .env("LESSSECURE", "1")
        .env("LESSHISTFILE", "-")
        .env_remove("LESS")
        .env_remove("LESSOPEN")
        .env_remove("LESSCLOSE")
        .env_remove("LESSKEY")
        .env_remove("LESSKEY_SYSTEM");

    let mut child = command
        .spawn()
        .context("start executable content review pager")?;
    let mut stdin = child
        .stdin
        .take()
        .context("open executable content review pager input")?;

    // Closing stdin tells less that the complete bounded review has arrived
    stdin
        .write_all(review.as_bytes())
        .context("write executable content review to pager")?;
    drop(stdin);

    let status = child
        .wait()
        .context("wait for executable content review pager")?;
    if !status.success() {
        anyhow::bail!("executable content review pager exited with {status}");
    }
    Ok(true)
}
