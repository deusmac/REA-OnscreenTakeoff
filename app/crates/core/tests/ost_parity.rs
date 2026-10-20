//! Answer-key parity: recompute every OST BidTakeoffTotals row with rea_core and compare.
//!
//! Input is the CSV folder written by `tools/ost-answer-key.ps1` (never committed; it is
//! client data). Set `OST_ANSWER_KEY` to that folder; without it the test prints a note and
//! passes. Formats and the code table: docs/study/formats/ost-quantities.md.

use std::collections::HashMap;
use std::path::Path;

use rea_core::area::{area_results, slope_factor, tile_count_average, AreaShape};
use rea_core::count::{count_results, CountDims, CountShape};
use rea_core::linear::{linear_results, LinearInput, Opening, Slope};
use rea_core::scale::{Scale, ScaleStyle};

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

/// Position text "x;y;x;y;..." (real inches) as numbers.
fn nums(t: &Row) -> Vec<f64> {
    s(t, "Points")
        .split(';')
        .filter_map(|v| v.parse().ok())
        .collect()
}

fn pairs(v: &[f64]) -> Vec<(f64, f64)> {
    v.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

fn sign(t: &Row) -> f64 {
    if s(t, "IsNegativeQuantity") == "1" {
        -1.0
    } else {
        1.0
    }
}

struct Data {
    conds: HashMap<String, Row>,
    tks: Vec<Row>,
    kids: HashMap<String, Vec<usize>>,
}

impl Data {
    fn kids_of<'a>(&'a self, t: &Row) -> impl Iterator<Item = &'a Row> + 'a {
        self.kids
            .get(s(t, "UID"))
            .into_iter()
            .flatten()
            .map(|&i| &self.tks[i])
    }
    fn cond_of(&self, t: &Row) -> &Row {
        &self.conds[s(t, "BidConditionUID")]
    }
}

/// Every result OST can store for this condition, keyed by OST quantity code, in OST base
/// units (inches, square inches, cubic inches, each). `None` means not modeled.
fn results(d: &Data, c: &Row, ts: &[&Row]) -> HashMap<u32, Option<f64>> {
    let mut out: HashMap<u32, Option<f64>> = HashMap::new();
    let mut add = |code: u32, v: Option<f64>| {
        let e = out.entry(code).or_insert(Some(0.0));
        *e = match (*e, v) {
            (Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
    };
    // Positions are already real inches, so the drawing scale is 1 inch per unit.
    let unit = Scale::from_known_length((0.0, 0.0), (1.0, 0.0), 1.0, ScaleStyle::Custom).unwrap();
    let (rise, run) = (num(c, "Rise"), num(c, "Run"));
    match s(c, "Type") {
        "0" => {
            for t in ts.iter().filter(|t| s(t, "ParentUID").is_empty()) {
                let v = nums(t);
                let openings: Vec<Opening> = d
                    .kids_of(t)
                    .filter(|k| {
                        let kc = d.cond_of(k);
                        matches!(s(kc, "Type"), "2" | "3") && s(kc, "Backout") == "1"
                    })
                    .map(|k| {
                        let kc = d.cond_of(k);
                        Opening {
                            width: num(kc, "Width"),
                            height: num(kc, "Height"),
                        }
                    })
                    .collect();
                let pts = pairs(&v[..v.len().min(4)]);
                let sag = v.get(4).copied().unwrap_or(0.0);
                let r = linear_results(&LinearInput {
                    points: &pts,
                    sagittas: &[sag],
                    scale: &unit,
                    height: num(c, "Height"),
                    thickness: num(c, "Thickness"),
                    slope: (run != 0.0).then_some(Slope { rise, run }),
                    drops: if s(c, "DropRun") == "1" {
                        num(t, "Count")
                    } else {
                        0.0
                    },
                    openings: &openings,
                })
                .expect("valid slope");
                let k = sign(t);
                for (code, v) in [
                    (1, r.length),
                    (2, r.segment_count as f64),
                    (3, r.area_one_side),
                    (4, r.area_both_sides),
                    (5, r.area_top_or_bottom),
                    (6, r.area_top_and_bottom),
                    (7, r.area_one_end),
                    (8, r.area_both_ends),
                    (9, r.area_all_sides),
                    (20, r.volume),
                ] {
                    add(code, Some(k * v));
                }
            }
        }
        "1" => {
            let f = slope_factor(rise, run);
            for t in ts.iter().filter(|t| s(t, "ParentUID").is_empty()) {
                let outer = pairs(&nums(t));
                // A backout counts with its parent, whatever condition it sits in.
                let rings: Vec<Vec<(f64, f64)>> = d
                    .kids_of(t)
                    .filter(|k| s(d.cond_of(k), "Type") == "1")
                    .map(|k| pairs(&nums(k)))
                    .collect();
                let refs: Vec<&[(f64, f64)]> = rings.iter().map(Vec::as_slice).collect();
                let r = area_results(
                    &AreaShape {
                        outer: &outer,
                        backouts: &refs,
                    },
                    1.0,
                    num(c, "Thickness"),
                    f,
                );
                let tiles = tile_count_average(r.area, num(c, "GridSize1"), num(c, "GridSize2"));
                let k = sign(t);
                for (code, v) in [
                    (11, Some(r.area)),
                    (12, Some(r.area)), // minus attachments; attachments not modeled
                    (14, Some(r.perimeter_without_backouts)),
                    (15, Some(r.perimeter)),
                    (16, None), // grid length: origin convention unknown
                    (17, Some(tiles)),
                    (19, Some(1.0)),
                    (21, Some(r.volume)),
                    (33, Some(r.perimeter)), // plus attachments; attachments not modeled
                    (34, Some(r.area_ignore_backouts)),
                ] {
                    add(code, v.map(|v| k * v));
                }
            }
        }
        // Type 4: one point per takeoff and OST's total is the takeoff count (checked on
        // JC's databases, 2026-10-20), so it scores like a count.
        "2" | "3" | "4" => {
            let n: f64 = ts.iter().map(|t| sign(t)).sum();
            let shape = match s(c, "Shape") {
                "1" => CountShape::Circle,
                "3" => CountShape::Rectangle,
                "12" => CountShape::Shape12,
                _ => CountShape::Square,
            };
            let r = count_results(
                n,
                CountDims::new(shape, num(c, "Height"), num(c, "Width"), num(c, "Depth")),
            );
            for (code, v) in [
                (23, r.count),
                (24, r.total_height),
                (5, r.top_or_bottom),
                (13, r.perimeter),
                (25, r.width_side),
                (26, r.both_width_sides),
                (20, r.volume),
                (31, r.opening_perimeter_four_sides),
                (32, r.opening_perimeter_three_sides),
            ] {
                add(code, Some(v));
            }
        }
        _ => {}
    }
    out
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
    let conds: HashMap<String, Row> = load(dir, "conditions.csv")
        .into_iter()
        .map(|r| (s(&r, "UID").to_string(), r))
        .collect();
    let tks = load(dir, "takeoffs.csv");
    let totals = load(dir, "totals.csv");
    let typ: HashMap<(String, String), f64> = load(dir, "typareacounts.csv")
        .iter()
        .map(|r| {
            let k = (
                s(r, "BidAreaUID").to_string(),
                s(r, "BidTypAreaUID").to_string(),
            );
            (k, num(r, "Count"))
        })
        .collect();

    let key = |r: &Row, area: &str| {
        let f = |k: &str| s(r, k).to_string();
        let a = area.to_string();
        (
            f("BidUID"),
            f("BidPageUID"),
            f("BidZoneUID"),
            a,
            f("BidTypAreaUID"),
            f("BidConditionUID"),
        )
    };
    let uid_bid: HashMap<&str, &str> = tks.iter().map(|t| (s(t, "UID"), s(t, "BidUID"))).collect();
    let mut groups: HashMap<_, Vec<usize>> = HashMap::new();
    let mut kids: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, t) in tks.iter().enumerate() {
        groups
            .entry(key(t, s(t, "BidAreaUID")))
            .or_default()
            .push(i);
        let p = s(t, "ParentUID");
        // A parent UID from another bid is a stale copy link; OST ignores that child.
        if !p.is_empty() && uid_bid.get(p) == Some(&s(t, "BidUID")) {
            kids.entry(p.to_string()).or_default().push(i);
        }
    }
    let data = Data { conds, tks, kids };

    // Per condition type: (slots compared, slots matched, rows, rows all matched).
    let mut stats: HashMap<&str, [usize; 4]> = HashMap::new();
    let mut misses: HashMap<&str, Vec<(f64, String)>> = HashMap::new();
    let mut unmodeled: HashMap<(String, String), usize> = HashMap::new();
    let mut sum_differs = 0;
    for tot in &totals {
        let c = &data.conds[s(tot, "BidConditionUID")];
        let ty = type_name(s(c, "Type"));
        // A typical-area row is the area-free typical row times the area's repeat count.
        let (area, mult) = match (s(tot, "BidAreaUID"), s(tot, "BidTypAreaUID")) {
            (a, t) if !a.is_empty() && !t.is_empty() => (
                "",
                typ.get(&(a.to_string(), t.to_string()))
                    .copied()
                    .unwrap_or(0.0),
            ),
            (a, _) => (a, 1.0),
        };
        let ts: Vec<&Row> = groups
            .get(&key(tot, area))
            .into_iter()
            .flatten()
            .map(|&i| &data.tks[i])
            .collect();
        let vals = results(&data, c, &ts);
        let st = stats.entry(ty).or_default();
        let mut row_ok = true;
        for i in 1..=3 {
            if s(c, &format!("UOM{i}")) == "-1" {
                continue;
            }
            let code = s(c, &format!("Quantity{i}"));
            let q = num(tot, &format!("Quantity{i}"));
            if (q - num(tot, &format!("SumQuantity{i}"))).abs() > 1e-9 {
                sum_differs += 1;
            }
            st[0] += 1;
            let p = match vals.get(&code.parse().unwrap_or(u32::MAX)) {
                Some(Some(v)) => v * mult,
                _ => {
                    *unmodeled
                        .entry((ty.to_string(), code.to_string()))
                        .or_default() += 1;
                    row_ok = false;
                    continue;
                }
            };
            let err = (p - q).abs();
            if err <= 0.01 || err <= 0.01 * q.abs() {
                st[1] += 1;
            } else {
                row_ok = false;
                let rel = err / q.abs().max(1e-9);
                let note = format!(
                    "total {} cond {} code {code}: OST {q:.3} ours {p:.3}",
                    s(tot, "UID"),
                    s(c, "UID")
                );
                misses.entry(ty).or_default().push((rel, note));
            }
        }
        st[2] += 1;
        st[3] += usize::from(row_ok);
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

    // Regression guard: every modeled quantity must match (unmodeled codes are reported).
    let modeled = slots - unmodeled.values().sum::<usize>();
    assert!(
        ok as f64 >= 0.99 * modeled as f64,
        "parity fell below 99% of modeled quantities: {ok} of {modeled}"
    );
}
