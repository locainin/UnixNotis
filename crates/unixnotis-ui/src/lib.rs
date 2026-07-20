//! GTK-oriented helpers shared by `UnixNotis` UI binaries
//!
//! # Example
//! ```
//! use unixnotis_ui::css::CssKind;
//!
//! let kind = CssKind::Panel;
//! assert!(matches!(kind, CssKind::Panel));
//! ```

pub mod css;
mod cut_corner;
pub mod icons;

pub use cut_corner::CutCorner;
