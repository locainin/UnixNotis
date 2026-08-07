//! Raw Notify body validation before typed D-Bus decoding

mod cursor;
mod limits;
mod signature;
mod validator;
mod value;

pub(super) use limits::{
    PreflightError, MAX_NOTIFY_WIRE_BODY_BYTES, MAX_NOTIFY_WIRE_IMAGE_BYTES,
    MAX_NOTIFY_WIRE_IMAGE_DIMENSION,
};
pub(super) use validator::preflight_notify;

#[cfg(test)]
mod tests;
