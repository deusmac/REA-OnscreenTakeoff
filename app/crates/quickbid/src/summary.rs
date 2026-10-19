//! Numbers QuickBid stored, for comparing against our own pricing math later.

use crate::BidPackage;

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionTotals {
    pub uid: String,
    pub name: String,
    pub mat_total: f64,
    pub lab_total: f64,
    pub detail_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub bid_no: Option<i64>,
    pub bid_total: Option<f64>,
    pub condition_count: usize,
    pub item_count: usize,
    pub item_size_count: usize,
    pub detail_count: usize,
    pub conditions: Vec<ConditionTotals>,
    pub mat_total_sum: f64,
    pub lab_total_sum: f64,
}

/// Stored totals per condition (missing values count as zero).
pub fn summarize(pkg: &BidPackage) -> Summary {
    let conditions: Vec<ConditionTotals> = pkg
        .conditions
        .iter()
        .map(|c| ConditionTotals {
            uid: c.uid.clone(),
            name: c.name.clone(),
            mat_total: c.mat_total.unwrap_or(0.0),
            lab_total: c.lab_total.unwrap_or(0.0),
            detail_count: pkg.details_for(&c.uid).count(),
        })
        .collect();
    Summary {
        bid_no: pkg.bid.bid_no,
        bid_total: pkg.bid.bid_total,
        condition_count: conditions.len(),
        item_count: pkg.items.len(),
        item_size_count: pkg.item_sizes.len(),
        detail_count: pkg.details.len(),
        mat_total_sum: conditions.iter().map(|c| c.mat_total).sum(),
        lab_total_sum: conditions.iter().map(|c| c.lab_total).sum(),
        conditions,
    }
}
