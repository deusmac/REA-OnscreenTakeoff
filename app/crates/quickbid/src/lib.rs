//! Reader for QuickBid bid packages (`.qbp`): a Microsoft Cabinet holding one XML file.
//!
//! The reader streams the XML, builds typed rows for the bid tables and skips the
//! `Masters` section entirely, because it holds personal contact data (names, phones,
//! emails, addresses). See `docs/study/formats/quickbid-package.md`.

mod attrs;
mod model;
mod parse;
mod summary;

pub use model::*;
pub use parse::{read_package, read_package_xml};
pub use summary::{summarize, ConditionTotals, Summary};

/// Errors from reading a bid package.
#[derive(Debug, thiserror::Error)]
pub enum QbError {
    #[error("cabinet error: {0}")]
    Cab(#[from] std::io::Error),
    #[error("the cabinet holds no files")]
    EmptyCabinet,
    #[error("xml error: {0}")]
    Xml(String),
    #[error("not a BidPackage document")]
    NotBidPackage,
    #[error("package has no Bid element")]
    NoBid,
}
