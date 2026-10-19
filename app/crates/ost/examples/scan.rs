//! Imports every `.osp` / `.ost` in a folder and prints one line per file: counts, timing,
//! and how many takeoff points fall outside their page (a check on the coordinate mapping).
//! Usage: cargo run --release -p rea-ost --example scan -- <folder>

use std::time::Instant;

use rea_core::model::Geometry;
use rea_ost::{import_path, OstBid};

fn outside(bid: &OstBid) -> (usize, usize) {
    let (mut out, mut total) = (0, 0);
    for t in &bid.takeoffs {
        let Some(p) = bid.pages.iter().find(|p| p.id == t.page_id) else {
            continue;
        };
        let pts: Vec<_> = match &t.geometry {
            Geometry::Polyline(v) => v.clone(),
            Geometry::Polygon { outer, .. } => outer.clone(),
            Geometry::Point(pt) => vec![*pt],
        };
        // 2% slack for strokes drawn on the sheet border.
        let (mx, my) = (p.width * 0.02, p.height * 0.02);
        for pt in pts {
            total += 1;
            if pt.x < -mx || pt.y < -my || pt.x > p.width + mx || pt.y > p.height + my {
                out += 1;
            }
        }
    }
    (out, total)
}

fn main() {
    let dir = std::env::args().nth(1).expect("folder argument");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("readable folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("osp") || x.eq_ignore_ascii_case("ost"))
        })
        .collect();
    files.sort();
    let (mut ok, mut failed) = (0, 0);
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy();
        let started = Instant::now();
        match import_path(f) {
            Ok(imp) => {
                ok += 1;
                let b = &imp.bid;
                let (out, pts) = outside(b);
                let styles: std::collections::BTreeSet<_> = b
                    .page_extra
                    .values()
                    .filter_map(|e| e.get("ScaleStyle").cloned())
                    .collect();
                let unscaled = b.pages.iter().filter(|p| p.scale.is_none()).count();
                println!(
                    "OK   {name} | v{} | pages {} (unscaled {unscaled}) conds {} takeoffs {} areas {} alts {} plans {} | scale styles {:?} | pts outside page {out}/{pts} | {} ms",
                    imp.ost_version,
                    b.pages.len(),
                    b.conditions.len(),
                    b.takeoffs.len(),
                    b.bid_areas.len(),
                    imp.alternates.len(),
                    imp.plan_files.len(),
                    styles,
                    started.elapsed().as_millis()
                );
            }
            Err(e) => {
                failed += 1;
                println!("FAIL {name} | {e} | {} ms", started.elapsed().as_millis());
            }
        }
    }
    println!("done: {ok} ok, {failed} failed of {}", files.len());
}
