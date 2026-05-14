//! Interactive review for executable preset content
//!
//! Human-driven imports should get a chance to inspect risky content before trusting it

use anyhow::{anyhow, Context, Result};
use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use super::super::pathing::prompt_yes_no;
use super::checks::ImportedExecContent;

pub(super) fn confirm_import_exec_content(
    exec_content: &ImportedExecContent,
    allow_exec: bool,
) -> Result<()> {
    // Empty bundles stay on the normal import path without extra prompts
    if exec_content.commands.is_empty() && exec_content.files.is_empty() {
        return Ok(());
    }

    // Explicit trust should keep automation and existing scripted flows working
    if allow_exec {
        return Ok(());
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(anyhow!(
            "preset import found executable commands or bundled scripts; rerun interactively to inspect them or use --allow-exec only if the preset is trusted"
        ));
    }

    eprintln!("preset import warning: this preset contains executable commands or bundled scripts");
    eprintln!("preset import warning: be sure the source is trusted");
    eprintln!(
        "preset import warning: found {} command entr{} and {} bundled file{} with executable content",
        exec_content.commands.len(),
        if exec_content.commands.len() == 1 { "y" } else { "ies" },
        exec_content.files.len(),
        if exec_content.files.len() == 1 { "" } else { "s" }
    );

    // Review happens before the final trust prompt so the decision is made with context
    if prompt_yes_no("Inspect executable content now?")? {
        show_exec_content_in_pager(exec_content)?;
    }

    // A second prompt keeps pager exit from being treated as implicit approval
    if prompt_yes_no("Import this preset anyway?")? {
        return Ok(());
    }

    Err(anyhow!("preset command canceled"))
}

fn show_exec_content_in_pager(exec_content: &ImportedExecContent) -> Result<()> {
    let pager = pager_command_parts()?;
    let mut command = Command::new(&pager[0]);
    if pager.len() > 1 {
        command.args(&pager[1..]);
    }
    // The review is pushed through stdin so the pager can stay the single interactive UI
    let mut child = command
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("launch pager '{}'", pager.join(" ")))?;

    let review = render_exec_content_review(exec_content);
    let write_result = if let Some(mut stdin) = child.stdin.take() {
        // Closing stdin after the write lets the pager reach EOF and quit normally
        stdin.write_all(review.as_bytes())
    } else {
        Ok(())
    };

    finish_pager(child, &pager, write_result)
}

fn pager_command_parts() -> Result<Vec<String>> {
    // `$PAGER` wins so local pager setup keeps working during import review too
    let configured = env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let mut parts = shell_words::split(&configured).context("parse pager command")?;
    if parts.is_empty() {
        return Err(anyhow!("pager command is empty"));
    }
    if pager_looks_like_less(&parts) && !pager_enables_raw_control(&parts) {
        // `less -R` keeps ANSI colors visible instead of printing escape bytes
        parts.push("-R".to_string());
    }
    Ok(parts)
}

fn finish_pager(
    mut child: std::process::Child,
    pager: &[String],
    write_result: io::Result<()>,
) -> Result<()> {
    if let Err(err) = write_result {
        // A spawned pager still needs reaping even if stdin broke early
        let _ = child.wait();
        return Err(err).context("write executable content review to pager");
    }

    let status = child.wait().context("wait for pager")?;
    if status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "pager '{}' exited with status {}",
        pager.join(" "),
        status
    ))
}

fn render_exec_content_review(exec_content: &ImportedExecContent) -> String {
    // Styling is applied at render time so the gathered review model stays plain
    render_exec_content_review_with_style(exec_content, ReviewStyle::for_terminal())
}

fn render_exec_content_review_with_style(
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
) -> String {
    let mut lines = vec![
        style.title("UnixNotis preset executable content review"),
        String::new(),
        style.warning("This preset contains executable commands or bundled scripts"),
        style.note("Only continue if the source is trusted"),
    ];

    if !exec_content.commands.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Command entries"));
        for command in &exec_content.commands {
            // Slot names make it obvious which config field would become runnable
            lines.push(format!(
                "  {} = {}",
                style.slot(&command.slot),
                style.command(&command.command)
            ));
        }
    }

    if !exec_content.files.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Bundled executable files"));
        for file in &exec_content.files {
            lines.push(String::new());
            lines.push(style.file_header(format!(
                "== {} (mode {:o}) ==",
                file.relative_path.display(),
                file.mode
            )));
            match std::str::from_utf8(&file.contents) {
                // Text payloads are shown directly so the trust check can happen without unpacking
                Ok(text) => lines.push(style.file_body(text)),
                Err(_) => lines.push(style.note(format!(
                    "<non-UTF-8 file omitted; {} byte(s)>",
                    file.contents.len()
                ))),
            }
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn pager_looks_like_less(parts: &[String]) -> bool {
    let Some(program) = parts.first() else {
        return false;
    };
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "less")
}

fn pager_enables_raw_control(parts: &[String]) -> bool {
    parts.iter().skip(1).any(|part| {
        part == "-R"
            || part == "-r"
            || (part.starts_with('-')
                && !part.starts_with("--")
                && part.chars().skip(1).any(|flag| matches!(flag, 'R' | 'r')))
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReviewStyle {
    color: bool,
}

impl ReviewStyle {
    fn for_terminal() -> Self {
        // Pager review is only useful with color on real terminals
        let color = io::stdout().is_terminal()
            && env::var_os("NO_COLOR").is_none()
            && env::var("CLICOLOR")
                .map(|value| value != "0")
                .unwrap_or(true)
            && env::var("TERM")
                .map(|value| value != "dumb")
                .unwrap_or(true);
        Self { color }
    }

    fn paint(self, text: impl Into<String>, prefix: &str) -> String {
        let text = text.into();
        if !self.color {
            // Plain output keeps the review readable when color is disabled upstream
            return text;
        }
        format!("\u{1b}[{prefix}m{text}\u{1b}[0m")
    }

    fn title(self, text: impl Into<String>) -> String {
        self.paint(text, "1;36")
    }

    fn warning(self, text: impl Into<String>) -> String {
        self.paint(text, "1;33")
    }

    fn note(self, text: impl Into<String>) -> String {
        self.paint(text, "2")
    }

    fn section(self, text: impl Into<String>) -> String {
        self.paint(text, "1;35")
    }

    fn slot(self, text: impl Into<String>) -> String {
        self.paint(text, "1;34")
    }

    fn command(self, text: impl Into<String>) -> String {
        self.paint(text, "32")
    }

    fn file_header(self, text: impl Into<String>) -> String {
        self.paint(text, "1;33")
    }

    fn file_body(self, text: impl Into<String>) -> String {
        self.paint(text, "37")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        finish_pager, pager_command_parts, pager_enables_raw_control,
        render_exec_content_review_with_style, ReviewStyle,
    };
    use crate::preset::import::checks::{
        ImportedExecCommand, ImportedExecContent, ImportedExecFile,
    };
    use std::env;
    use std::io;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::Mutex;

    // Pager tests mutate one process-global env var, so they need one tiny lock
    static PAGER_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn exec_review_renders_commands_and_files() {
        let review = render_exec_content_review_with_style(
            &ImportedExecContent {
                commands: vec![ImportedExecCommand {
                    slot: "widgets.stats[0].cmd".to_string(),
                    command: "scripts/check.sh".to_string(),
                }],
                files: vec![ImportedExecFile {
                    relative_path: PathBuf::from("scripts/check.sh"),
                    contents: b"#!/bin/sh\necho ok\n".to_vec(),
                    mode: 0o755,
                }],
            },
            ReviewStyle { color: false },
        );

        assert!(review.contains("widgets.stats[0].cmd = scripts/check.sh"));
        assert!(review.contains("== scripts/check.sh (mode 755) =="));
        assert!(review.contains("#!/bin/sh"));
    }

    #[test]
    fn exec_review_style_can_add_color() {
        let title = ReviewStyle { color: true }.title("review");
        assert!(title.contains("\u{1b}[1;36m"));
        assert!(title.ends_with("\u{1b}[0m"));
    }

    #[test]
    fn pager_command_adds_raw_control_for_less() {
        let _guard = PAGER_ENV_LOCK.lock().expect("lock pager env");
        let original = env::var_os("PAGER");
        unsafe {
            env::set_var("PAGER", "less -F");
        }

        let pager = pager_command_parts().expect("build pager");

        match original {
            Some(value) => unsafe {
                env::set_var("PAGER", value);
            },
            None => unsafe {
                env::remove_var("PAGER");
            },
        }

        assert_eq!(pager, vec!["less", "-F", "-R"]);
    }

    #[test]
    fn pager_command_keeps_existing_raw_control_flag() {
        assert!(pager_enables_raw_control(&[
            "less".to_string(),
            "-FR".to_string()
        ]));
        assert!(pager_enables_raw_control(&[
            "less".to_string(),
            "-R".to_string()
        ]));
        assert!(!pager_enables_raw_control(&[
            "less".to_string(),
            "-F".to_string()
        ]));
    }

    #[test]
    fn pager_command_respects_quoted_arguments() {
        let _guard = PAGER_ENV_LOCK.lock().expect("lock pager env");
        let original = env::var_os("PAGER");
        unsafe {
            env::set_var("PAGER", "less --prompt='unixnotis review'");
        }

        let pager = pager_command_parts().expect("build pager");

        match original {
            Some(value) => unsafe {
                env::set_var("PAGER", value);
            },
            None => unsafe {
                env::remove_var("PAGER");
            },
        }

        assert_eq!(
            pager,
            vec![
                "less".to_string(),
                "--prompt=unixnotis review".to_string(),
                "-R".to_string()
            ]
        );
    }

    #[test]
    fn finish_pager_reaps_child_after_stdin_failure() {
        let child = Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("spawn pager");

        let error = finish_pager(
            child,
            &["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe")),
        )
        .expect_err("stdin failure should surface");

        assert!(error
            .to_string()
            .contains("write executable content review to pager"));
    }
}
