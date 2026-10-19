//! Summary totals and display rounding.
//!
//! A condition total is the sum of positive objects minus the sum of negative ones
//! (OST-06-110). Rows can be grouped by page, type and bid area in any combination, always
//! nested in the order page, then type, then area (OST-12-009). Display rounding never
//! touches stored values (OST-01-041).

use std::collections::BTreeMap;

/// One takeoff object's contribution to its condition.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub condition_id: u32,
    pub page_id: u32,
    pub type_label: String,
    /// `None` means unassigned (no bid area).
    pub area: Option<String>,
    pub negative: bool,
    /// Named quantity values, for example ("Qty 1", 12.0).
    pub values: Vec<(String, f64)>,
}

/// Which grouping levels are on. Nesting order is fixed and not configurable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Grouping {
    pub page: bool,
    pub type_label: bool,
    pub area: bool,
}

/// One group key. Derived order: page ascending, type ascending, area ascending.
// unknown: areas should follow the Bid Areas dialog layout order (OST-12-009), which core
// does not have yet; for now they sort by name, with unassigned first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Page(u32),
    Type(String),
    Area(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Empty when no grouping is on.
    pub group: Vec<Key>,
    pub condition_id: u32,
    /// Net totals per named quantity, in first-seen order.
    pub totals: Vec<(String, f64)>,
}

/// Net totals per condition, split by the chosen groups. Groups sort by key; rows inside a
/// group keep the order conditions first appear in `entries`.
pub fn summarize(entries: &[Entry], grouping: Grouping) -> Vec<Row> {
    let mut groups: BTreeMap<Vec<Key>, Vec<Row>> = BTreeMap::new();
    for e in entries {
        let mut key = Vec::new();
        if grouping.page {
            key.push(Key::Page(e.page_id));
        }
        if grouping.type_label {
            key.push(Key::Type(e.type_label.clone()));
        }
        if grouping.area {
            key.push(Key::Area(e.area.clone()));
        }
        let rows = groups.entry(key.clone()).or_default();
        let at = match rows.iter().position(|r| r.condition_id == e.condition_id) {
            Some(i) => i,
            None => {
                rows.push(Row {
                    group: key,
                    condition_id: e.condition_id,
                    totals: Vec::new(),
                });
                rows.len() - 1
            }
        };
        let sign = if e.negative { -1.0 } else { 1.0 };
        for (name, v) in &e.values {
            match rows[at].totals.iter_mut().find(|(n, _)| n == name) {
                Some((_, t)) => *t += sign * v,
                None => rows[at].totals.push((name.clone(), sign * v)),
            }
        }
    }
    groups.into_values().flatten().collect()
}

/// Per-column display setting (OST-01-042).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decimals {
    Fixed(u8),
    Floating,
}

impl Default for Decimals {
    /// Default display is 0 decimals (OST-01-041).
    fn default() -> Self {
        Decimals::Fixed(0)
    }
}

const MAX_DECIMALS: u8 = 6;

/// Formats a value for display. Rounds half away from zero; the input is never changed.
// unknown: whether Floating trims trailing zeros (assumed yes); whether Fixed accepts more
// than 6 digits (clamped to 6 here).
pub fn format_display(value: f64, decimals: Decimals) -> String {
    let (n, trim) = match decimals {
        Decimals::Fixed(n) => (n.min(MAX_DECIMALS), false),
        Decimals::Floating => (MAX_DECIMALS, true),
    };
    let scale = 10f64.powi(i32::from(n));
    let mut s = format!("{:.*}", usize::from(n), (value * scale).round() / scale);
    if trim && s.contains('.') {
        s.truncate(s.trim_end_matches('0').trim_end_matches('.').len());
    }
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        s = s.trim_start_matches('-').to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(cond: u32, page: u32, ty: &str, area: Option<&str>, neg: bool, q: f64) -> Entry {
        Entry {
            condition_id: cond,
            page_id: page,
            type_label: ty.into(),
            area: area.map(String::from),
            negative: neg,
            values: vec![("Qty 1".into(), q)],
        }
    }

    fn total(r: &Row) -> f64 {
        r.totals[0].1
    }

    #[test]
    fn negative_subtracts() {
        // OST-06-110: 10 objects with 1 negative = 9 - 1 = 8.
        let mut v: Vec<Entry> = (0..9)
            .map(|_| e(1, 1, "Linear", None, false, 1.0))
            .collect();
        v.push(e(1, 1, "Linear", None, true, 1.0));
        let rows = summarize(&v, Grouping::default());
        assert_eq!(rows.len(), 1);
        assert_eq!(total(&rows[0]), 8.0);
    }

    #[test]
    fn multiple_named_columns() {
        // OST-06-110 applied per quantity column.
        let mut a = e(1, 1, "Area", None, false, 100.0);
        a.values.push(("Qty 2".into(), 40.0));
        let mut b = e(1, 1, "Area", None, true, 10.0);
        b.values.push(("Qty 2".into(), 4.0));
        let rows = summarize(&[a, b], Grouping::default());
        assert_eq!(
            rows[0].totals,
            vec![("Qty 1".into(), 90.0), ("Qty 2".into(), 36.0)]
        );
    }

    #[test]
    fn no_grouping_one_row_per_condition_in_first_seen_order() {
        // OST-12-009: ungrouped rows keep condition order.
        let v = [
            e(7, 2, "A", None, false, 1.0),
            e(3, 1, "B", None, false, 2.0),
            e(7, 1, "A", None, false, 4.0),
        ];
        let rows = summarize(&v, Grouping::default());
        let ids: Vec<u32> = rows.iter().map(|r| r.condition_id).collect();
        assert_eq!(ids, vec![7, 3]);
        assert_eq!(total(&rows[0]), 5.0);
    }

    #[test]
    fn group_by_area_unassigned_is_own_group() {
        // OST-12-009: unassigned is its own group.
        let v = [
            e(1, 1, "L", Some("North"), false, 5.0),
            e(1, 1, "L", None, false, 2.0),
            e(1, 1, "L", Some("North"), false, 1.0),
        ];
        let g = Grouping {
            area: true,
            ..Grouping::default()
        };
        let rows = summarize(&v, g);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].group, vec![Key::Area(None)]);
        assert_eq!(total(&rows[0]), 2.0);
        assert_eq!(total(&rows[1]), 6.0);
    }

    #[test]
    fn nesting_is_page_then_type_then_area() {
        // OST-12-009: fixed nesting order regardless of input order.
        let v = [
            e(1, 2, "A", Some("X"), false, 1.0),
            e(1, 1, "B", Some("Y"), false, 1.0),
            e(1, 1, "A", Some("Z"), false, 1.0),
            e(1, 1, "A", Some("Y"), false, 1.0),
        ];
        let g = Grouping {
            page: true,
            type_label: true,
            area: true,
        };
        let rows = summarize(&v, g);
        assert_eq!(
            rows[0].group,
            vec![
                Key::Page(1),
                Key::Type("A".into()),
                Key::Area(Some("Y".into()))
            ]
        );
        assert_eq!(rows[1].group[2], Key::Area(Some("Z".into())));
        assert_eq!(rows[2].group[1], Key::Type("B".into()));
        assert_eq!(rows[3].group[0], Key::Page(2));
    }

    #[test]
    fn grouping_page_and_area_only_skips_type() {
        // OST-12-009: any combination of groups.
        let v = [
            e(1, 1, "A", Some("X"), false, 1.0),
            e(1, 1, "B", Some("X"), false, 2.0),
        ];
        let g = Grouping {
            page: true,
            type_label: false,
            area: true,
        };
        let rows = summarize(&v, g);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].group.len(), 2);
        assert_eq!(total(&rows[0]), 3.0);
    }

    #[test]
    fn rows_in_group_keep_condition_order() {
        // OST-12-009: stable, deterministic order inside a group.
        let v = [
            e(9, 1, "A", None, false, 1.0),
            e(2, 1, "A", None, false, 1.0),
        ];
        let g = Grouping {
            page: true,
            ..Grouping::default()
        };
        let rows = summarize(&v, g);
        let ids: Vec<u32> = rows.iter().map(|r| r.condition_id).collect();
        assert_eq!(ids, vec![9, 2]);
    }

    #[test]
    fn default_rounding_is_zero_decimals() {
        // OST-01-041: 1.13 shows 1, 1.51 shows 2.
        assert_eq!(format_display(1.13, Decimals::default()), "1");
        assert_eq!(format_display(1.51, Decimals::default()), "2");
        assert_eq!(format_display(2.5, Decimals::default()), "3");
    }

    #[test]
    fn fixed_decimals_and_clamp() {
        // OST-01-042: per-column 0..n decimals.
        assert_eq!(format_display(1.23456, Decimals::Fixed(2)), "1.23");
        assert_eq!(format_display(1.5, Decimals::Fixed(3)), "1.500");
        assert_eq!(format_display(1.0, Decimals::Fixed(9)), "1.000000");
    }

    #[test]
    fn floating_up_to_six_digits() {
        // OST-01-042: Floating shows up to 6 digits right of the point.
        assert_eq!(format_display(1.25, Decimals::Floating), "1.25");
        assert_eq!(format_display(3.0, Decimals::Floating), "3");
        assert_eq!(
            format_display(1.123_456_789, Decimals::Floating),
            "1.123457"
        );
    }

    #[test]
    fn negative_zero_not_shown() {
        // OST-01-041: display rounding only; a tiny negative must not print "-0".
        assert_eq!(format_display(-0.4, Decimals::Fixed(0)), "0");
        assert_eq!(format_display(-2.5, Decimals::Fixed(0)), "-3");
    }

    #[test]
    fn rounding_leaves_math_full_precision() {
        // OST-01-041: totals keep full precision; only display rounds.
        let v = [
            e(1, 1, "A", None, false, 0.4),
            e(1, 1, "A", None, false, 0.4),
        ];
        let rows = summarize(&v, Grouping::default());
        assert_eq!(format_display(total(&rows[0]), Decimals::Fixed(0)), "1");
        assert!((total(&rows[0]) - 0.8).abs() < 1e-12);
    }
}
