//! GTK-oriented helpers shared by UnixNotis UI binaries.
//!
//! # Example
//! ```
//! use unixnotis_ui::css::CssKind;
//!
//! let kind = CssKind::Panel;
//! assert!(matches!(kind, CssKind::Panel));
//! ```

#![allow(
    clippy::nursery,
    clippy::pedantic,
    reason = "pedantic and nursery cleanup is tracked incrementally across existing code"
)]

pub mod css;
