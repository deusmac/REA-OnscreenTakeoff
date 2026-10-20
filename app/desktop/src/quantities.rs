//! Condition totals for the Summary panel: every result column OST shows for a condition
//! (up to three), summed over the job by `rea_core::ost_totals` and shown in the
//! condition's display units.

use std::collections::HashMap;

use rea_core::model::{Condition, Id};
use rea_core::ost_totals::{bid_totals, condition_totals, display_unit, OstCondition, OstJob};

pub struct SummaryRow {
    pub id: Id,
    pub number: i64,
    pub name: String,
    pub color: [u8; 3],
    /// Value and unit per result slot; `None` for an unused slot or a result we do not model.
    pub qty: [Option<(f64, &'static str)>; 3],
}

pub fn summary(conditions: &[Condition], job: &OstJob) -> Vec<SummaryRow> {
    let totals = condition_totals(job, &bid_totals(job));
    let ost: HashMap<Id, &OstCondition> = job.conditions.iter().map(|c| (c.uid, c)).collect();
    let mut rows: Vec<SummaryRow> = conditions
        .iter()
        .map(|c| {
            let t = totals.get(&c.id);
            SummaryRow {
                id: c.id,
                number: c.number,
                name: c.name.clone(),
                color: c.color,
                qty: std::array::from_fn(|i| {
                    let o = ost.get(&c.id)?;
                    if o.uoms[i] == -1 {
                        return None;
                    }
                    // No takeoffs: OST shows 0.
                    let q = t.map_or(Some(0.0), |t| t[i])?;
                    let (unit, div) = display_unit(o.results[i], o.uoms[i]);
                    Some((q / div, unit))
                }),
            }
        })
        .collect();
    rows.sort_by_key(|r| r.number);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_core::model::ConditionStyle;
    use rea_core::ost_totals::{OstTakeoff, AREA, COUNT, LINEAR};

    fn cond(id: Id) -> Condition {
        Condition {
            id,
            project_id: 1,
            number: id,
            name: format!("C{id}"),
            style: ConditionStyle::Linear,
            ctype: String::new(),
            layer: String::new(),
            color: [0, 0, 0],
            pattern: String::new(),
            height: 0.0,
            thickness: 0.0,
            width: 0.0,
            depth: 0.0,
            slope: None,
            count_shape: None,
            results: [None, None, None],
        }
    }

    fn ost(uid: Id, kind: i64, results: [u32; 3], uoms: [i64; 3]) -> OstCondition {
        OstCondition {
            uid,
            kind,
            results,
            uoms,
            height: 96.0,
            ..Default::default()
        }
    }

    fn tk(uid: Id, condition: Id, position: &[f64]) -> OstTakeoff {
        OstTakeoff {
            uid,
            page: 1,
            condition,
            position: position.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn three_columns_in_display_units() {
        let job = OstJob {
            conditions: vec![
                ost(1, LINEAR, [1, 3, 2], [2, 5, 0]),
                ost(2, AREA, [11, 15, 0], [5, 2, -1]),
                ost(3, COUNT, [23, 24, 0], [0, 1, -1]),
                ost(4, COUNT, [23, 0, 0], [0, -1, -1]),
            ],
            takeoffs: vec![
                tk(10, 1, &[0.0, 0.0, 96.0, 0.0, 0.0]),
                tk(11, 1, &[96.0, 0.0, 192.0, 0.0, 0.0]),
                tk(20, 2, &[0.0, 0.0, 96.0, 0.0, 96.0, 96.0, 0.0, 96.0]),
                tk(30, 3, &[1.0, 1.0]),
                tk(31, 3, &[2.0, 2.0]),
            ],
            ..Default::default()
        };
        let conds: Vec<Condition> = (1..=4).map(cond).collect();
        let rows = summary(&conds, &job);
        let near = |q: Option<(f64, &str)>, v: f64, u: &str| {
            q.is_some_and(|(a, b)| (a - v).abs() < 1e-9 && b == u)
        };
        // 192 in = 16 LF; wall 16 LF * 8 ft = 128 SF; two segments.
        assert!(near(rows[0].qty[0], 16.0, "LF"));
        assert!(near(rows[0].qty[1], 128.0, "SF"));
        assert!(near(rows[0].qty[2], 2.0, "EA"));
        // 8 ft square: 64 SF, 32 LF.
        assert!(near(rows[1].qty[0], 64.0, "SF") && near(rows[1].qty[1], 32.0, "LF"));
        assert!(rows[1].qty[2].is_none());
        // Two counts of 8 ft total height.
        assert!(near(rows[2].qty[0], 2.0, "EA") && near(rows[2].qty[1], 16.0, "LF"));
        // A condition without takeoffs shows 0.
        assert!(near(rows[3].qty[0], 0.0, "EA"));
    }
}
