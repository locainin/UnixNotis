//! Post-start D-Bus readiness enforcement and bounded failure diagnostics

use std::future::Future;
use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};
use unixnotis_core::{ControlProxy, NotificationsProxy, CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME};
use zbus::{fdo::DBusProxy, names::BusName, Connection};

use super::super::super::{log_line, run_command, ActionContext};

const INSTALL_READINESS_TIMEOUT: Duration = Duration::from_secs(20);
const DBUS_METHOD_TIMEOUT: Duration = Duration::from_secs(2);
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub fn enforce_service_readiness(ctx: &mut ActionContext) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create installer readiness runtime")?;
    let address = stable_user_bus_address();
    let result = runtime.block_on(async {
        let builder = zbus::connection::Builder::address(address.as_str())
            .context("prepare stable user-bus connection")?;
        let connection = tokio::time::timeout(DBUS_METHOD_TIMEOUT, builder.build())
            .await
            .context("stable user-bus connection timed out")?
            .context("connect to stable user bus")?;
        let readiness =
            wait_until_ready_with_probe(INSTALL_READINESS_TIMEOUT, || probe_readiness(&connection))
                .await;
        if let Err(error) = readiness {
            let owners = readiness_owner_diagnostics(&connection).await;
            return Err(error.context(owners));
        }
        Ok(())
    });

    if let Err(error) = result {
        log_line(ctx, format!("UnixNotis readiness failed: {error:#}"));
        if ctx.paths.service.is_systemd() {
            log_systemd_failure_diagnostics(ctx);
        }
        return Err(error.context("UnixNotis did not become ready after service start"));
    }
    log_line(ctx, "UnixNotis D-Bus readiness verified");
    Ok(())
}

async fn wait_until_ready_with_probe<F, Fut>(timeout: Duration, mut probe: F) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let last_failure = match probe().await {
            Ok(()) => return Ok(()),
            Err(error) => format!("{error:#}"),
        };
        let now = tokio::time::Instant::now();
        if now >= deadline {
            bail!("UnixNotis readiness timed out; last observation: {last_failure}");
        }
        tokio::time::sleep(READINESS_POLL_INTERVAL.min(deadline - now)).await;
    }
}

async fn probe_readiness(connection: &Connection) -> Result<()> {
    let dbus = DBusProxy::new(connection)
        .await
        .context("create readiness D-Bus proxy")?;
    let notification_owner = get_owner(&dbus, NOTIFICATIONS_BUS_NAME).await?;
    let control_owner = get_owner(&dbus, CONTROL_BUS_NAME).await?;
    ensure!(
        notification_owner == control_owner,
        "D-Bus owners differ: notifications={notification_owner}, control={control_owner}"
    );

    let control = ControlProxy::new(connection)
        .await
        .context("create control readiness proxy")?;
    tokio::time::timeout(DBUS_METHOD_TIMEOUT, control.get_state())
        .await
        .context("Control.GetState timed out")?
        .context("Control.GetState failed")?;

    let notifications = NotificationsProxy::new(connection)
        .await
        .context("create notification readiness proxy")?;
    tokio::time::timeout(DBUS_METHOD_TIMEOUT, notifications.get_server_information())
        .await
        .context("GetServerInformation timed out")?
        .context("GetServerInformation failed")?;
    Ok(())
}

async fn get_owner(dbus: &DBusProxy<'_>, name: &'static str) -> Result<String> {
    let name = BusName::try_from(name).context("invalid readiness bus name")?;
    let owner = tokio::time::timeout(DBUS_METHOD_TIMEOUT, dbus.get_name_owner(name.clone()))
        .await
        .context("D-Bus owner lookup timed out")?
        .with_context(|| format!("{name} has no owner"))?;
    Ok(owner.to_string())
}

async fn readiness_owner_diagnostics(connection: &Connection) -> String {
    let Ok(dbus) = DBusProxy::new(connection).await else {
        return "owner diagnostics unavailable: failed to create D-Bus proxy".to_string();
    };
    let notifications = diagnostic_owner(&dbus, NOTIFICATIONS_BUS_NAME).await;
    let control = diagnostic_owner(&dbus, CONTROL_BUS_NAME).await;
    format!("D-Bus owners: {NOTIFICATIONS_BUS_NAME}={notifications}; {CONTROL_BUS_NAME}={control}")
}

async fn diagnostic_owner(dbus: &DBusProxy<'_>, name: &'static str) -> String {
    let Ok(name) = BusName::try_from(name) else {
        return "<invalid name>".to_string();
    };
    match tokio::time::timeout(DBUS_METHOD_TIMEOUT, dbus.get_name_owner(name)).await {
        Ok(Ok(owner)) => owner.to_string(),
        Ok(Err(error)) => format!("<{error}>"),
        Err(_) => "<timed out>".to_string(),
    }
}

fn stable_user_bus_address() -> String {
    format!(
        "unix:path=/run/user/{}/bus",
        rustix::process::getuid().as_raw()
    )
}

fn log_systemd_failure_diagnostics(ctx: &mut ActionContext) {
    let show_args = [
        "--user",
        "show",
        "unixnotis-daemon.service",
        "-p",
        "LoadState",
        "-p",
        "ActiveState",
        "-p",
        "SubState",
        "-p",
        "Result",
        "-p",
        "ExecMainStatus",
        "-p",
        "FragmentPath",
        "-p",
        "ExecStart",
    ];
    match crate::system_tools::command("systemctl") {
        Ok(mut command) => {
            command.args(show_args);
            let _ = run_command(ctx, "systemctl readiness diagnostics", command, None);
        }
        Err(error) => log_line(
            ctx,
            format!("Warning: systemctl readiness diagnostics unavailable ({error})"),
        ),
    }

    match crate::system_tools::command("journalctl") {
        Ok(mut command) => {
            // The line count and shared log reader cap both dimensions of diagnostic output
            command.args([
                "--user",
                "-u",
                "unixnotis-daemon.service",
                "-n",
                "100",
                "--no-pager",
                "--output=short-monotonic",
            ]);
            let _ = run_command(ctx, "journalctl readiness diagnostics", command, None);
        }
        Err(error) => log_line(
            ctx,
            format!("Warning: journal readiness diagnostics unavailable ({error})"),
        ),
    }
}

#[cfg(test)]
#[path = "tests/readiness.rs"]
mod tests;
