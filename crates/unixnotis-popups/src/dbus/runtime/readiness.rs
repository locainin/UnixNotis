//! GTK readiness wait and owner-generation readiness lease

use tokio::sync::watch;
use unixnotis_core::{timed_dbus_call, ControlProxy, INTERNAL_DBUS_CALL_TIMEOUT};

pub(super) struct PopupReadinessLease<'proxy, 'connection> {
    proxy: &'proxy ControlProxy<'connection>,
    published: bool,
}

impl<'proxy, 'connection> PopupReadinessLease<'proxy, 'connection> {
    pub(super) const fn new(proxy: &'proxy ControlProxy<'connection>) -> Self {
        Self {
            proxy,
            published: false,
        }
    }

    pub(super) async fn publish(&mut self) -> zbus::Result<()> {
        timed_dbus_call(self.proxy.mark_popups_ready()).await?;
        self.published = true;
        Ok(())
    }

    pub(super) async fn clear(&mut self) {
        if !self.published {
            return;
        }
        // No-autostart cleanup cannot revive a daemon that is already stopping
        let _ = timed_dbus_call(self.proxy.mark_popups_not_ready()).await;
        self.published = false;
    }
}

pub(super) async fn wait_for_gtk_runtime(gtk_ready_rx: &mut watch::Receiver<bool>) -> bool {
    if *gtk_ready_rx.borrow() {
        return true;
    }
    tokio::time::timeout(INTERNAL_DBUS_CALL_TIMEOUT, async {
        loop {
            if gtk_ready_rx.changed().await.is_err() {
                return *gtk_ready_rx.borrow();
            }
            if *gtk_ready_rx.borrow() {
                return true;
            }
        }
    })
    .await
    .unwrap_or(false)
}
