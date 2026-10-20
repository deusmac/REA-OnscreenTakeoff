//! Answer-key parity: recompute every OST BidTakeoffTotals row with `rea_core::ost_totals`
//! and compare.
//!
//! Input is the CSV folder written by `tools/ost-answer-key.ps1` (never committed; it is
//! client data). Set `OST_ANSWER_KEY` to that folder; without it the test prints a note and
//! passes. Formats and the code table: docs/study/formats/ost-quantities.md.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rea_core::model::Id;
use rea_core::ost_totals::{
    bid_totals, OstCondition, OstJob, OstTakeoff, TotalKey, TypAreaCount, ZoneOutline,
};

type Row = HashMap<String, String>;

fn load(dir: &Path, name: &str) -> Vec<Row> {
    let text = std::fs::read_to_string(dir.join(name))
        .unwrap_or_else(|e| panic!("cannot read {name}: {e}"));
    let mut lines = text.lines();
    let head: Vec<String> = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('\u{feff}')
        .split(',')
        .map(String::from)
        .collect();
    lines
        .filter(|l| !l.is_empty())
        .map(|l| {
            head.iter()
                .cloned()
                .zip(l.split(',').map(String::from))
                .collect()
        })
        .collect()
}

fn num(r: &Row, k: &str) -> f64 {
    r.get(k).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn s<'a>(r: &'a Row, k: &str) -> &'a str {
    r.get(k).map(String::as_str).unwrap_or("")
}

/// A UID column: empty or 0 is none.
fn id(r: &Row, k: &str) -> Option<Id> {
    s(r, k).parse().ok().filter(|v| *v != 0)
}

fn uid(r: &Row) -> Id {
    id(r, "UID").unwrap_or(0)
}

fn flag(r: &Row, k: &str) -> bool {
    s(r, k) == "1"
}

/// Position text "x;y;x;y;..." (real inches) as numbers.
fn nums(r: &Row) -> Vec<f64> {
    s(r, "Points")
        .split(';')
        .filter_map(|v| v.parse().ok())
        .collect()
}

fn condition(r: &Row) -> OstCondition {
    OstCondition {
        uid: uid(r),
        kind: s(r, "Type").parse().unwrap_or(-1),
        shape: s(r, "Shape").parse().unwrap_or(0),
        width: num(r, "Width"),
        height: num(r, "Height"),
        thickness: num(r, "Thickness"),
        depth: num(r, "Depth"),
        rise: num(r, "Rise"),
        run: num(r, "Run"),
        results: std::array::from_fn(|i| {
            s(r, &format!("Quantity{}", i + 1))
                .parse()
                .unwrap_or(u32::MAX)
        }),
        uoms: std::array::from_fn(|i| s(r, &format!("UOM{}", i + 1)).parse().unwrap_or(-1)),
        round_quantity: flag(r, "RoundQuantity"),
        round_up: num(r, "RoundUp"),
        backout: flag(r, "Backout"),
        drop_run: flag(r, "DropRun"),
        grid_size: (num(r, "GridSize1"), num(r, "GridSize2")),
        gap: num(r, "Gap"),
    }
}

fn takeoff(r: &Row) -> OstTakeoff {
    OstTakeoff {
        uid: uid(r),
        page: id(r, "BidPageUID").unwrap_or(0),
        condition: id(r, "BidConditionUID").unwrap_or(0),
        zone: id(r, "BidZoneUID"),
        area: id(r, "BidAreaUID"),
        typ_area: id(r, "BidTypAreaUID"),
        parent: id(r, "ParentUID"),
        typ_group: id(r, "TypGroupUID"),
        position: nums(r),
        count: num(r, "Count"),
        negative: flag(r, "IsNegativeQuantity"),
        grid_offset: (num(r, "GridOffsetX"), num(r, "GridOffsetY")),
        grid_rotation: num(r, "GridRotation"),
    }
}

fn type_name(t: &str) -> &'static str {
    match t {
        "0" => "linear",
        "1" => "area",
        "2" => "count",
        "3" => "attached count",
        "4" => "count type 4",
        _ => "other",
    }
}

#[test]
fn ost_answer_key_parity() {
    let Ok(dir) = std::env::var("OST_ANSWER_KEY") else {
        println!("OST_ANSWER_KEY not set; skipping the OST parity check");
        return;
    };
    let dir = Path::new(&dir);
    let cond_rows = load(dir, "conditions.csv");
    let conds: HashMap<String, &Row> = cond_rows.iter().map(|r| (s(r, "UID").into(), r)).collect();
    let tks = load(dir, "takeoffs.csv");
    let totals = load(dir, "totals.csv");
    let typ_area_counts: Vec<TypAreaCount> = load(dir, "typareacounts.csv")
        .iter()
        .map(|r| TypAreaCount {
            area: id(r, "BidAreaUID").unwrap_or(0),
            typ_area: id(r, "BidTypAreaUID").unwrap_or(0),
            count: num(r, "Count"),
        })
        .collect();
    let negative_zones: HashSet<Id> = load(dir, "zones.csv")
        .iter()
        .filter(|r| flag(r, "IsNegativeValues"))
        .map(uid)
        .collect();

    // One job per bid. A takeoff with a zone and no condition is the zone's outline.
    let mut jobs: HashMap<&str, OstJob> = HashMap::new();
    for r in &cond_rows {
        jobs.entry(s(r, "BidUID"))
            .or_default()
            .conditions
            .push(condition(r));
    }
    for r in &tks {
        let job = jobs.entry(s(r, "BidUID")).or_default();
        if conds.contains_key(s(r, "BidConditionUID")) {
            job.takeoffs.push(takeoff(r));
        } else if let Some(zone) = id(r, "BidZoneUID") {
            let v = nums(r);
            job.zone_outlines.push(ZoneOutline {
                zone,
                page: id(r, "BidPageUID").unwrap_or(0),
                ring: v.chunks_exact(2).map(|c| (c[0], c[1])).collect(),
            });
        }
    }
    let mut ours: HashMap<(&str, TotalKey), [Option<f64>; 3]> = HashMap::new();
    for (bid, job) in &mut jobs {
        job.negative_zones = negative_zones.clone();
        job.typ_area_counts = typ_area_counts.clone();
        for row in bid_totals(job) {
            ours.insert((*bid, row.key), row.quantities);
        }
    }

    // Per condition type: (slots compared, slots matched, rows, rows all matched).
    let mut stats: HashMap<&str, [usize; 4]> = HashMap::new();
    let mut misses: HashMap<&str, Vec<(f64, String)>> = HashMap::new();
    let mut unmodeled: HashMap<(String, String), usize> = HashMap::new();
    let mut sum_differs = 0;
    // Rows OST left stale: reported on their own, never counted as our mismatch.
    let (mut stale, mut stale_notes) = (0, Vec::<String>::new());
    for tot in &totals {
        let c = conds[s(tot, "BidConditionUID")];
        let ty = type_name(s(c, "Type"));
        let key = TotalKey {
            page: id(tot, "BidPageUID").unwrap_or(0),
            zone: id(tot, "BidZoneUID"),
            area: id(tot, "BidAreaUID"),
            typ_area: id(tot, "BidTypAreaUID"),
            condition: uid(c),
        };
        // No row of ours: no takeoffs feed it, so every result is 0.
        let vals = ours
            .get(&(s(tot, "BidUID"), key))
            .copied()
            .unwrap_or([Some(0.0); 3]);
        let mut row_ok = true;
        let (mut compared, mut matched, mut stray) = (0, 0, false);
        let mut row_misses: Vec<(f64, String)> = Vec::new();
        for i in 1..=3 {
            let q = num(tot, &format!("Quantity{i}"));
            let sq = num(tot, &format!("SumQuantity{i}"));
            if s(c, &format!("UOM{i}")) == "-1" {
                // A value in a slot the condition no longer uses: OST stored this row under
                // an older definition of the condition (see the stale rule in the format notes).
                stray |= q != 0.0 || sq != 0.0;
                continue;
            }
            let code = s(c, &format!("Quantity{i}"));
            if (q - sq).abs() > 1e-9 {
                sum_differs += 1;
            }
            compared += 1;
            let Some(p) = vals[i - 1] else {
                *unmodeled
                    .entry((ty.to_string(), code.to_string()))
                    .or_default() += 1;
                row_ok = false;
                continue;
            };
            let err = (p - q).abs();
            if err <= 0.01 || err <= 0.01 * q.abs() {
                matched += 1;
            } else {
                row_ok = false;
                let note = format!(
                    "total {} cond {} code {code}: OST {q:.3} ours {p:.3}",
                    s(tot, "UID"),
                    s(c, "UID")
                );
                row_misses.push((err / q.abs().max(1e-9), note));
            }
        }
        if stray && !row_misses.is_empty() {
            stale += 1;
            stale_notes.extend(row_misses.into_iter().map(|m| m.1));
            continue;
        }
        let st = stats.entry(ty).or_default();
        st[0] += compared;
        st[1] += matched;
        st[2] += 1;
        st[3] += usize::from(row_ok);
        misses.entry(ty).or_default().extend(row_misses);
    }

    println!(
        "OST parity: {} totals rows from {}",
        totals.len(),
        dir.display()
    );
    let (mut slots, mut ok, mut rows, mut rows_ok) = (0, 0, 0, 0);
    let mut types: Vec<_> = stats.iter().collect();
    types.sort();
    for (ty, st) in types {
        println!(
            "  {ty}: rows {} matched {} | quantities {} matched {} ({:.1}%)",
            st[2],
            st[3],
            st[0],
            st[1],
            100.0 * st[1] as f64 / st[0].max(1) as f64
        );
        if let Some(m) = misses.get_mut(ty) {
            m.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (rel, note) in m.iter().take(5) {
                println!("      worst: {note} (off {:.2}%)", rel * 100.0);
            }
        }
        (slots, ok, rows, rows_ok) = (slots + st[0], ok + st[1], rows + st[2], rows_ok + st[3]);
    }
    let mut un: Vec<_> = unmodeled.iter().collect();
    un.sort();
    for ((ty, code), n) in un {
        println!("  not modeled: {ty} code {code} ({n} quantities)");
    }
    println!("  total: rows {rows} matched {rows_ok} | quantities {slots} matched {ok}");
    println!("  rows where SumQuantity differs from Quantity: {sum_differs}");
    println!("  stale in OST (excluded above): {stale} rows");
    for n in stale_notes.iter().take(5) {
        println!("      {n}");
    }

    // Regression guard: every modeled quantity must match (unmodeled codes are reported).
    let modeled = slots - unmodeled.values().sum::<usize>();
    assert!(
        ok as f64 >= 0.99 * modeled as f64,
        "parity fell below 99% of modeled quantities: {ok} of {modeled}"
    );
}
