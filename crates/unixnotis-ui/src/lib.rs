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
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

pub mod css;
