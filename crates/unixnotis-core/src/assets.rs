//! Embedded non-CSS default files shipped with UnixNotis

pub struct DefaultScript {
    pub relative_path: &'static str,
    pub contents: &'static str,
}

pub const DEFAULT_BLUE_LIGHT_STATE_SCRIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/scripts/unixnotis-blue-light-state"
));
pub const DEFAULT_BLUE_LIGHT_LIB_SCRIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/scripts/unixnotis-blue-light-lib"
));
pub const DEFAULT_BLUE_LIGHT_ON_SCRIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/scripts/unixnotis-blue-light-on"
));
pub const DEFAULT_BLUE_LIGHT_OFF_SCRIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/scripts/unixnotis-blue-light-off"
));

pub const DEFAULT_SCRIPTS: &[DefaultScript] = &[
    DefaultScript {
        relative_path: "scripts/unixnotis-blue-light-lib",
        contents: DEFAULT_BLUE_LIGHT_LIB_SCRIPT,
    },
    DefaultScript {
        relative_path: "scripts/unixnotis-blue-light-state",
        contents: DEFAULT_BLUE_LIGHT_STATE_SCRIPT,
    },
    DefaultScript {
        relative_path: "scripts/unixnotis-blue-light-on",
        contents: DEFAULT_BLUE_LIGHT_ON_SCRIPT,
    },
    DefaultScript {
        relative_path: "scripts/unixnotis-blue-light-off",
        contents: DEFAULT_BLUE_LIGHT_OFF_SCRIPT,
    },
];
