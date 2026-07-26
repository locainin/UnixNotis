//! Shared catalog of standalone notification daemons

/// A process that may own the freedesktop notifications bus name
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KnownNotificationDaemon {
    pub name: &'static str,
    // Some daemons use D-Bus activation or desktop startup instead of a user unit
    pub systemd_unit: Option<&'static str>,
}

/// Standalone daemons safe to identify and stop by their exact owner process
pub const KNOWN_NOTIFICATION_DAEMONS: &[KnownNotificationDaemon] = &[
    KnownNotificationDaemon {
        name: "unixnotis-daemon",
        systemd_unit: Some("unixnotis-daemon.service"),
    },
    KnownNotificationDaemon {
        name: "fnott",
        systemd_unit: Some("fnott.service"),
    },
    KnownNotificationDaemon {
        name: "mako",
        systemd_unit: Some("mako.service"),
    },
    KnownNotificationDaemon {
        name: "dunst",
        systemd_unit: Some("dunst.service"),
    },
    KnownNotificationDaemon {
        name: "swaync",
        systemd_unit: Some("swaync.service"),
    },
    KnownNotificationDaemon {
        name: "xfce4-notifyd",
        systemd_unit: Some("xfce4-notifyd.service"),
    },
    KnownNotificationDaemon {
        name: "wired",
        systemd_unit: Some("wired.service"),
    },
    KnownNotificationDaemon {
        name: "notify-osd",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "quickshell",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "hyprnotify",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "lxqt-notificationd",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "mate-notification-daemon",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "notification-daemon",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "deadd-notification-center",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "tiramisu",
        systemd_unit: None,
    },
    KnownNotificationDaemon {
        name: "runst",
        systemd_unit: None,
    },
];
