//! Command-line control surface for the UnixNotis D-Bus interface.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use unixnotis_core::{ControlProxy, NotificationView};
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
    OpenPanel,
    ClosePanel,
    Dnd {
        #[arg(value_enum)]
        state: DndState,
    },
    Clear,
    Dismiss {
        id: u32,
    },
    ListActive,
    ListHistory,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum DndState {
    On,
    Off,
    Toggle,
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
        Command::OpenPanel => proxy.open_panel().await?,
        Command::ClosePanel => proxy.close_panel().await?,
        Command::Clear => proxy.clear_all().await?,
        Command::Dismiss { id } => proxy.dismiss(id).await?,
        Command::ListActive => {
            let notifications = proxy.list_active().await?;
            print_notifications("active", &notifications);
        }
        Command::ListHistory => {
            let notifications = proxy.list_history().await?;
            print_notifications("history", &notifications);
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

fn print_notifications(label: &str, notifications: &[NotificationView]) {
    println!("{} notifications: {}", label, notifications.len());
    for notification in notifications {
        println!(
            "- #{id} [{app}] {summary}",
            id = notification.id,
            app = notification.app_name,
            summary = notification.summary
        );
    }
}
