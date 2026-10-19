//! Reads every `.qbp` in a folder and prints one line per file: counts, stored totals, timing.
//! Usage: cargo run --release -p rea-quickbid --example scan -- <folder>

use std::time::Instant;

use rea_quickbid::{read_package, summarize};

fn main() {
    let dir = std::env::args().nth(1).expect("folder argument");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("readable folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("qbp")))
        .collect();
    files.sort();
    let (mut ok, mut failed) = (0, 0);
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy();
        let started = Instant::now();
        let result = std::fs::read(f)
            .map_err(|e| e.to_string())
            .and_then(|b| read_package(&b).map_err(|e| e.to_string()));
        match result {
            Ok(pkg) => {
                ok += 1;
                let s = summarize(&pkg);
                let direct = s.mat_total_sum + s.lab_total_sum;
                println!(
                    "OK   {name} | bid {:?} total {:.2} | conds {} items {} sizes {} details {} | mat+lab {:.2} ({:.0}% of total) | {} ms",
                    s.bid_no,
                    s.bid_total.unwrap_or(0.0),
                    s.condition_count,
                    s.item_count,
                    s.item_size_count,
                    s.detail_count,
                    direct,
                    s.bid_total.filter(|t| *t != 0.0).map_or(0.0, |t| direct / t * 100.0),
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
