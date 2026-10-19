//! Core takeoff model and math. No I/O and no UI, so every formula is unit-testable.
//!
//! Formulas cite ledger IDs from `docs/study/` (for example OST-01-038).

pub mod model;
pub mod units;

/// Crate version, shown in the app status bar.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
