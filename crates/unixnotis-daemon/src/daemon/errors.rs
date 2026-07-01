//! Shared D-Bus error conversion helpers

pub(crate) fn to_fdo_error(err: zbus::Error) -> zbus::fdo::Error {
    // zbus interface methods need fdo errors, but inner helpers return zbus errors
    zbus::fdo::Error::Failed(err.to_string())
}
