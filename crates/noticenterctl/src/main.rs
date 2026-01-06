//! Command-line control surface for the UnixNotis D-Bus interface.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::process::Command as ProcCommand;
use unixnotis_core::{ControlProxy, NotificationView, PanelDebugLevel};
use unixnotis_core::util;
use zbus::Connection;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    TogglePanel,
    OpenPanel {
        #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "info")]
        debug: Option<DebugLevelArg>,
    },
    ClosePanel,
    Dnd {
        #[arg(value_enum)]
        state: DndState,
    },
    Clear,
    Dismiss {
        id: u32,
    },
    ListActive {
        #[arg(long)]
        full: bool,
    },
    ListHistory {
        #[arg(long)]
        full: bool,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum DndState {
    On,
    Off,
    Toggle,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum DebugLevelArg {
    Critical,
    Warn,
    Info,
    Verbose,
}

impl From<DebugLevelArg> for PanelDebugLevel {
    fn from(value: DebugLevelArg) -> Self {
        match value {
            DebugLevelArg::Critical => PanelDebugLevel::Critical,
            DebugLevelArg::Warn => PanelDebugLevel::Warn,
            DebugLevelArg::Info => PanelDebugLevel::Info,
            DebugLevelArg::Verbose => PanelDebugLevel::Verbose,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    match args.command {
        Command::TogglePanel => proxy.toggle_panel().await?,
        Command::OpenPanel { debug } => {
            if let Some(level) = debug {
                proxy.open_panel_debug(level.into()).await?;
                follow_debug_logs().context("follow unixnotis debug logs")?;
            } else {
                proxy.open_panel().await?;
            }
        }
        Command::ClosePanel => proxy.close_panel().await?,
        Command::Clear => proxy.clear_all().await?,
        Command::Dismiss { id } => proxy.dismiss(id).await?,
        Command::ListActive { full } => {
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = proxy.list_active().await?;
            print_notifications("active", &notifications, allow_full);
        }
        Command::ListHistory { full } => {
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = proxy.list_history().await?;
            print_notifications("history", &notifications, allow_full);
        }
        Command::Dnd { state } => match state {
            DndState::On => proxy.set_dnd(true).await?,
            DndState::Off => proxy.set_dnd(false).await?,
            DndState::Toggle => {
                let current = proxy.get_state().await?;
                proxy.set_dnd(!current.dnd_enabled).await?;
            }
        },
    }

    Ok(())
}

fn print_notifications(label: &str, notifications: &[NotificationView], full: bool) {
    let limit = if full {
        util::diagnostic_log_limit()
    } else {
        util::default_log_limit()
    };
    println!("{} notifications: {}", label, notifications.len());
    for notification in notifications {
        let summary = util::sanitize_log_value(&notification.summary, limit);
        println!(
            "- #{id} [{app}] {summary}",
            id = notification.id,
            app = notification.app_name,
            summary = summary
        );
    }
}

fn follow_debug_logs() -> Result<()> {
    let status = ProcCommand::new("journalctl")
        .args([
            "--user",
            "-f",
            "-u",
            "unixnotis-daemon.service",
            "-o",
            "cat",
        ])
        .status()
        .context("start journalctl follow")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("journalctl exited with status {}", status))
    }
}
